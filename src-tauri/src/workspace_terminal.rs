use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

use harbor_core::settings::{config_dir, Settings, WorkspaceMode};

use crate::{core_client, core_process, ssh_tunnel};

const START_TIMEOUT: Duration = Duration::from_secs(8);

pub struct WorkspaceTerminal {
    workspace_id: String,
    local_port: u16,
    child: Child,
}

pub fn open(
    settings: &Settings,
    current: &mut Option<WorkspaceTerminal>,
) -> Result<String, String> {
    let workspace = settings.current()?;
    // --- 阶段 1：复用当前 Workspace 的健康终端 ---
    if let Some(terminal) = current.as_mut() {
        let running = terminal
            .child
            .try_wait()
            .map_err(|error| format!("check workspace terminal failed: {error}"))?
            .is_none();
        if running && terminal.workspace_id == workspace.id {
            return Ok(terminal.url());
        }
    }
    stop(current);

    // --- 阶段 2：部署 ttyd，并由远端 Core 确保机器级服务可用 ---
    if workspace.mode != WorkspaceMode::Remote {
        return Err("workspace terminal is only available for remote workspaces".into());
    }
    let ssh = workspace
        .ssh
        .as_ref()
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
    core_process::ensure_remote_ttyd_runtime(ssh)?;
    let workdir = workspace
        .search_paths
        .first()
        .cloned()
        .unwrap_or_else(|| "$HOME".into());
    let title = format!("Harbor · {}", workspace.name);
    let terminal = core_client::ensure_terminal(settings, &workdir, &title)?;
    if !terminal.ready {
        return Err("harbor_core did not make managed ttyd ready".into());
    }
    let local_port = ssh_tunnel::available_local_port("workspace terminal")?;

    // --- 阶段 3：GUI 只建立到 Core 托管 ttyd 的 SSH Tunnel ---
    let log_dir = config_dir().join("log");
    fs::create_dir_all(&log_dir)
        .map_err(|error| format!("create {} failed: {error}", log_dir.display()))?;
    let log_path = log_dir.join("workspace-terminal.log");
    let child = ssh_tunnel::spawn_forward_only(ssh, local_port, terminal.port, log_path.as_path())?;
    let mut terminal = WorkspaceTerminal {
        workspace_id: workspace.id.clone(),
        local_port,
        child,
    };

    // --- 阶段 4：等待 ttyd 页面真正可用 ---
    if let Err(error) = wait_ready(&mut terminal, &log_path) {
        let _ = terminal.child.kill();
        let _ = terminal.child.wait();
        return Err(error);
    }
    let url = terminal.url();
    *current = Some(terminal);
    Ok(url)
}

pub fn stop(current: &mut Option<WorkspaceTerminal>) {
    if let Some(mut terminal) = current.take() {
        let _ = terminal.child.kill();
        let _ = terminal.child.wait();
    }
}

impl WorkspaceTerminal {
    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.local_port)
    }
}

fn wait_ready(terminal: &mut WorkspaceTerminal, log_path: &std::path::Path) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < START_TIMEOUT {
        if terminal
            .child
            .try_wait()
            .map_err(|error| format!("check workspace terminal failed: {error}"))?
            .is_some()
        {
            return Err(terminal_error(
                log_path,
                "workspace terminal exited before becoming ready",
            ));
        }
        if terminal_http_ready(terminal.local_port) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(terminal_error(
        log_path,
        "workspace terminal did not become ready within 8 seconds",
    ))
}

fn terminal_http_ready(port: u16) -> bool {
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

fn terminal_error(log_path: &std::path::Path, fallback: &str) -> String {
    let log = fs::read_to_string(log_path).unwrap_or_default();
    let detail = log.lines().rev().take(8).collect::<Vec<_>>();
    if detail.is_empty() {
        fallback.to_string()
    } else {
        format!(
            "{fallback}: {}",
            detail.into_iter().rev().collect::<Vec<_>>().join(" | ")
        )
    }
}
