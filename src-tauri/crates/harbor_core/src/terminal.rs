use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::service::CoreServiceStatus;
use crate::settings::config_dir;

pub const TTYD_PORT: u16 = 29386;
const START_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Debug, PartialEq)]
struct TerminalConfig {
    workdir: String,
    title: String,
}

#[derive(Debug)]
struct ManagedTerminal {
    child: Child,
    config: TerminalConfig,
}

#[derive(Debug, Default)]
pub struct TerminalService {
    terminal: Option<ManagedTerminal>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TerminalStatus {
    pub ready: bool,
    pub port: u16,
}

impl TerminalService {
    pub fn ensure(&mut self, workdir: String, title: String) -> Result<TerminalStatus, String> {
        let mut workdir = required(workdir, "terminal workdir")?;
        if workdir == "$HOME" {
            workdir = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        }
        let config = TerminalConfig {
            workdir,
            title: required(title, "terminal title")?,
        };

        // --- 阶段 1：复用配置一致且健康的机器级 ttyd ---
        if let Some(terminal) = self.terminal.as_mut() {
            let running = terminal
                .child
                .try_wait()
                .map_err(|error| format!("check managed ttyd failed: {error}"))?
                .is_none();
            if running && terminal.config == config && terminal_http_ready() {
                return Ok(ready_status());
            }
        }
        self.stop();

        // --- 阶段 2：确认固定端口可用并启动 ttyd ---
        if terminal_port_open() {
            return Err(format!(
                "Harbor ttyd default port {TTYD_PORT} is already in use"
            ));
        }
        let log_path = config_dir().join("log").join("workspace-terminal.log");
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create {} failed: {error}", parent.display()))?;
        }
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|error| format!("open {} failed: {error}", log_path.display()))?;
        let stderr = stdout
            .try_clone()
            .map_err(|error| format!("clone {} failed: {error}", log_path.display()))?;
        let executable = ttyd_executable();
        if !executable.is_file() {
            return Err(format!(
                "managed ttyd runtime not found at {}",
                executable.display()
            ));
        }
        let port = TTYD_PORT.to_string();
        let title = format!("titleFixed={}", config.title);
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
        let child = Command::new("setsid")
            .arg(executable)
            .args([
                "-W",
                "-O",
                "-m",
                "1",
                "-i",
                "127.0.0.1",
                "-p",
                port.as_str(),
                "-w",
                config.workdir.as_str(),
                "-t",
                "rendererType=canvas",
                "-t",
                title.as_str(),
                shell.as_str(),
                "-l",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| format!("start managed ttyd failed: {error}"))?;
        self.terminal = Some(ManagedTerminal { child, config });

        // --- 阶段 3：等待 HTTP 服务可用，失败时回收进程组 ---
        let started = Instant::now();
        while started.elapsed() < START_TIMEOUT {
            let terminal = self.terminal.as_mut().expect("terminal was just created");
            if terminal
                .child
                .try_wait()
                .map_err(|error| format!("check managed ttyd failed: {error}"))?
                .is_some()
            {
                self.terminal = None;
                return Err(terminal_error(
                    &log_path,
                    "managed ttyd exited before becoming ready",
                ));
            }
            if terminal_http_ready() {
                crate::app_log::core(&format!("managed ttyd ready on 127.0.0.1:{TTYD_PORT}"));
                return Ok(ready_status());
            }
            thread::sleep(Duration::from_millis(100));
        }
        self.stop();
        Err(terminal_error(
            &log_path,
            "managed ttyd did not become ready within 8 seconds",
        ))
    }

    pub fn stop(&mut self) {
        let Some(mut terminal) = self.terminal.take() else {
            return;
        };
        if terminal.child.try_wait().ok().flatten().is_some() {
            return;
        }
        let pid = terminal.child.id() as i32;
        #[cfg(unix)]
        unsafe {
            libc::kill(-pid, libc::SIGTERM);
        }
        for _ in 0..10 {
            if terminal.child.try_wait().ok().flatten().is_some() {
                crate::app_log::core("managed ttyd stopped");
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        #[cfg(unix)]
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        let _ = terminal.child.wait();
        crate::app_log::core("managed ttyd stopped");
    }

    pub fn status(&mut self) -> CoreServiceStatus {
        let pid = self.terminal.as_mut().and_then(|terminal| {
            terminal
                .child
                .try_wait()
                .ok()
                .filter(Option::is_none)
                .map(|_| terminal.child.id())
        });
        if pid.is_none() {
            self.terminal = None;
        }
        let ready = pid.is_some() && terminal_http_ready();
        let port_conflict = pid.is_none() && terminal_port_open();
        CoreServiceStatus {
            id: "terminal".into(),
            name: "远端终端 ttyd".into(),
            kind: "terminal".into(),
            state: if ready {
                "running"
            } else if pid.is_some() {
                "starting"
            } else if port_conflict {
                "unavailable"
            } else {
                "stopped"
            }
            .into(),
            pid,
            port: TTYD_PORT,
            bind: format!("127.0.0.1:{TTYD_PORT}"),
            detail: if port_conflict {
                Some(format!("端口 {TTYD_PORT} 已被非 Harbor ttyd 进程占用"))
            } else {
                None
            },
            managed_by_core: true,
            stoppable: false,
        }
    }
}

fn ttyd_executable() -> std::path::PathBuf {
    config_dir()
        .join("tools")
        .join("ttyd")
        .join("current")
        .join("run")
}

fn ready_status() -> TerminalStatus {
    TerminalStatus {
        ready: true,
        port: TTYD_PORT,
    }
}

fn required(value: String, field: &str) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(format!("{field} is required"))
    } else {
        Ok(value)
    }
}

fn terminal_port_open() -> bool {
    let address = ([127, 0, 0, 1], TTYD_PORT).into();
    TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok()
}

fn terminal_http_ready() -> bool {
    let address = ([127, 0, 0, 1], TTYD_PORT).into();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_ttyd_uses_stable_runtime_path() {
        assert!(ttyd_executable().ends_with(".harbor/tools/ttyd/current/run"));
    }
}
