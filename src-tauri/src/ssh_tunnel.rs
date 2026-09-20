use std::fs::OpenOptions;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use harbor_core::settings::{ssh_exec_command_with_args, WorkspaceSsh};

pub fn available_local_port(context: &str) -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("reserve {context} port failed: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("read {context} port failed: {error}"))
}

pub fn spawn_forward(
    ssh: &WorkspaceSsh,
    local_port: u16,
    remote_port: u16,
    remote_command: &str,
    log_path: &Path,
) -> Result<Child, String> {
    // --- 阶段 1：构造仅监听本机的 SSH 端口转发 ---
    let forwarding = format!("127.0.0.1:{local_port}:127.0.0.1:{remote_port}");
    let extra_args = vec![
        "-o".into(),
        "ExitOnForwardFailure=yes".into(),
        "-L".into(),
        forwarding,
    ];
    let ssh_command = ssh_exec_command_with_args(ssh, remote_command, &extra_args)?;

    // --- 阶段 2：将隧道诊断信息写入 Harbor 日志 ---
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|error| format!("open {} failed: {error}", log_path.display()))?;
    let stderr = stdout
        .try_clone()
        .map_err(|error| format!("clone {} failed: {error}", log_path.display()))?;
    let mut command = Command::new(&ssh_command.program);
    command
        .args(&ssh_command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    if ssh_command.sshpass {
        if ssh.password.is_empty() {
            return Err("sshpass requires a password".into());
        }
        command.env("SSHPASS", ssh.password.as_str());
        command.env_remove("SSH_ASKPASS");
    }
    command
        .spawn()
        .map_err(|error| format!("start SSH tunnel failed: {error}"))
}
