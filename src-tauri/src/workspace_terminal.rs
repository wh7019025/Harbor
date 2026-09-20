use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

use harbor_core::settings::{config_dir, ssh_run, Workspace, WorkspaceMode};

use crate::{core_process, ssh_tunnel};

const REMOTE_TTYD_PORT_START: u16 = 29386;
const REMOTE_TTYD_PORT_END: u16 = 29486;
const START_TIMEOUT: Duration = Duration::from_secs(8);

pub struct WorkspaceTerminal {
    workspace_id: String,
    local_port: u16,
    child: Child,
}

pub fn open(
    workspace: &Workspace,
    current: &mut Option<WorkspaceTerminal>,
) -> Result<String, String> {
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

    // --- 阶段 2：检查远端依赖并准备端口 ---
    if workspace.mode != WorkspaceMode::Remote {
        return Err("workspace terminal is only available for remote workspaces".into());
    }
    let ssh = workspace
        .ssh
        .as_ref()
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
    let ttyd = core_process::remote_ttyd_command(ssh)?;
    let remote_port = available_remote_port(ssh)?;
    let local_port = ssh_tunnel::available_local_port("workspace terminal")?;

    // --- 阶段 3：通过同一 SSH 会话启动 ttyd 与本地转发 ---
    let workdir = workspace
        .search_paths
        .first()
        .map(|path| shell_single_quote(path))
        .unwrap_or_else(|| "\"$HOME\"".into());
    let title = shell_single_quote(format!("Harbor · {}", workspace.name).as_str());
    let remote = format!(
        r#"workdir={workdir}
shell="${{SHELL:-/bin/bash}}"
exec {ttyd} -W -O -m 1 -o -i 127.0.0.1 -p {remote_port} -w "$workdir" -t rendererType=canvas -t titleFixed={title} "$shell" -l"#
    );
    let log_dir = config_dir().join("log");
    fs::create_dir_all(&log_dir)
        .map_err(|error| format!("create {} failed: {error}", log_dir.display()))?;
    let log_path = log_dir.join("workspace-terminal.log");
    let child = ssh_tunnel::spawn_forward(
        ssh,
        local_port,
        remote_port,
        remote.as_str(),
        log_path.as_path(),
    )?;
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

fn available_remote_port(ssh: &harbor_core::settings::WorkspaceSsh) -> Result<u16, String> {
    let command = format!(
        r#"command -v ss >/dev/null 2>&1 || {{ echo "remote terminal requires ss from iproute2" >&2; exit 1; }}
port={REMOTE_TTYD_PORT_START}
while [ "$port" -le {REMOTE_TTYD_PORT_END} ]; do
  if ! ss -H -ltn "sport = :$port" | grep -q .; then
    printf '%s\n' "$port"
    exit 0
  fi
  port=$((port + 1))
done
echo "no free remote terminal port in {REMOTE_TTYD_PORT_START}-{REMOTE_TTYD_PORT_END}" >&2
exit 1"#
    );
    let output = ssh_run(ssh, command.as_str())?;
    let port = output
        .trim()
        .parse::<u16>()
        .map_err(|_| format!("invalid remote terminal port: {}", output.trim()))?;
    if !(REMOTE_TTYD_PORT_START..=REMOTE_TTYD_PORT_END).contains(&port) {
        return Err(format!("remote terminal port out of range: {port}"));
    }
    Ok(port)
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

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_handles_apostrophes() {
        assert_eq!(shell_single_quote("robot's ws"), "'robot'\\''s ws'");
    }
}
