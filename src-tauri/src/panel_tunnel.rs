use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

use harbor_common::settings::{config_dir, Workspace, WorkspaceMode};

use crate::ssh_tunnel;

const START_TIMEOUT: Duration = Duration::from_secs(8);

pub struct PanelTunnel {
    workspace_id: String,
    remote_port: u16,
    local_port: u16,
    child: Child,
}

pub fn resolve(
    workspace: &Workspace,
    label: &str,
    remote_url: &str,
    tunnels: &mut HashMap<String, PanelTunnel>,
) -> Result<String, String> {
    if workspace.mode != WorkspaceMode::Remote {
        return Ok(remote_url.to_string());
    }
    let remote_port = url_port(remote_url)?;

    // --- 阶段 1：复用同一窗口仍然存活的 SSH 隧道 ---
    if let Some(tunnel) = tunnels.get_mut(label) {
        let running = tunnel
            .child
            .try_wait()
            .map_err(|error| format!("check panel tunnel failed: {error}"))?
            .is_none();
        if running && tunnel.workspace_id == workspace.id && tunnel.remote_port == remote_port {
            return Ok(local_url(remote_url, tunnel.local_port)?);
        }
    }
    stop(tunnels, label);

    // --- 阶段 2：将远端面板转发到代理绕过范围内的本机地址 ---
    let ssh = workspace
        .ssh
        .as_ref()
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
    let local_port = ssh_tunnel::available_local_port("panel tunnel")?;
    let log_dir = config_dir().join("log");
    fs::create_dir_all(&log_dir)
        .map_err(|error| format!("create {} failed: {error}", log_dir.display()))?;
    let log_path = log_dir.join("panel-tunnel.log");
    let child = ssh_tunnel::spawn_forward(
        ssh,
        local_port,
        remote_port,
        "while :; do sleep 3600; done",
        log_path.as_path(),
    )?;
    let mut tunnel = PanelTunnel {
        workspace_id: workspace.id.clone(),
        remote_port,
        local_port,
        child,
    };

    // --- 阶段 3：等待面板页面通过本地隧道可访问 ---
    if let Err(error) = wait_ready(&mut tunnel) {
        let _ = tunnel.child.kill();
        let _ = tunnel.child.wait();
        return Err(error);
    }
    let url = local_url(remote_url, local_port)?;
    tunnels.insert(label.to_string(), tunnel);
    Ok(url)
}

pub fn stop(tunnels: &mut HashMap<String, PanelTunnel>, label: &str) {
    if let Some(mut tunnel) = tunnels.remove(label) {
        let _ = tunnel.child.kill();
        let _ = tunnel.child.wait();
    }
}

pub fn stop_all(tunnels: &mut HashMap<String, PanelTunnel>) {
    for (_, mut tunnel) in tunnels.drain() {
        let _ = tunnel.child.kill();
        let _ = tunnel.child.wait();
    }
}

fn url_port(url: &str) -> Result<u16, String> {
    let (_, authority, _) = url_parts(url)?;
    authority
        .rsplit_once(':')
        .ok_or_else(|| format!("remote panel url requires an explicit port: {url}"))?
        .1
        .parse::<u16>()
        .map_err(|_| format!("invalid remote panel port in {url}"))
}

fn local_url(remote_url: &str, local_port: u16) -> Result<String, String> {
    let (scheme, _, suffix) = url_parts(remote_url)?;
    Ok(format!("{scheme}127.0.0.1:{local_port}{suffix}"))
}

fn url_parts(url: &str) -> Result<(&str, &str, &str), String> {
    let scheme_end = url
        .find("://")
        .map(|index| index + 3)
        .ok_or_else(|| format!("panel url must be http or https: {url}"))?;
    let path_start = url[scheme_end..]
        .find('/')
        .map(|index| scheme_end + index)
        .unwrap_or(url.len());
    Ok((
        &url[..scheme_end],
        &url[scheme_end..path_start],
        &url[path_start..],
    ))
}

fn wait_ready(tunnel: &mut PanelTunnel) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < START_TIMEOUT {
        if tunnel
            .child
            .try_wait()
            .map_err(|error| format!("check panel tunnel failed: {error}"))?
            .is_some()
        {
            return Err("panel SSH tunnel exited before becoming ready".into());
        }
        if http_ready(tunnel.local_port) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err("panel SSH tunnel did not become ready within 8 seconds".into())
}

fn http_ready(port: u16) -> bool {
    let address = ([127, 0, 0, 1], port).into();
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(200)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
    if stream
        .write_all(b"GET / HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut response = [0u8; 16];
    stream
        .read(&mut response)
        .is_ok_and(|count| count > 5 && response.starts_with(b"HTTP/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_remote_panel_url_to_local_tunnel() {
        assert_eq!(
            local_url("http://10.43.30.30:23682/vnc_auto.html", 40123).unwrap(),
            "http://127.0.0.1:40123/vnc_auto.html"
        );
        assert_eq!(url_port("http://10.43.30.30:23682/").unwrap(), 23682);
    }
}
