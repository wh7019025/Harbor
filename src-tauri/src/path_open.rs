use serde::Serialize;
use std::process::{Command, Stdio};

use harbor_core::settings::{expand_path, Workspace, WorkspaceMode, WorkspaceSsh};

#[derive(Clone, Debug, Serialize)]
pub struct PathOpeners {
    pub file_manager: bool,
    pub vscode: bool,
    pub cursor: bool,
}

pub fn detect_path_openers() -> PathOpeners {
    PathOpeners {
        file_manager: cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows"
        )),
        vscode: has_command("code"),
        cursor: has_command("cursor"),
    }
}

pub fn open_path_with(path: &str, target: &str, workspace: &Workspace) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("path cannot be empty".into());
    }
    if workspace.mode == WorkspaceMode::Remote {
        return open_remote_path(path, target, workspace);
    }
    let absolute = expand_path(path);
    let path = absolute.to_string_lossy();
    open_path_with_absolute(path.as_ref(), target)
}

fn open_remote_path(path: &str, target: &str, workspace: &Workspace) -> Result<(), String> {
    let ssh = workspace
        .ssh
        .as_ref()
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
    match target {
        "file_manager" => {
            let uri = remote_sftp_uri(path, ssh);
            open_file_manager(uri.as_str())
        }
        "vscode" => open_remote_editor("code", path, ssh),
        "cursor" => open_remote_editor("cursor", path, ssh),
        _ => Err(format!("unknown open target: {target}")),
    }
}

fn open_remote_editor(bin: &str, path: &str, ssh: &WorkspaceSsh) -> Result<(), String> {
    let remote = format!("ssh-remote+{}", remote_ssh_authority(ssh));
    run_open(bin, &["--remote", remote.as_str(), path])
}

fn remote_sftp_uri(path: &str, ssh: &WorkspaceSsh) -> String {
    format!(
        "sftp://{}{}",
        remote_ssh_authority(ssh),
        percent_encode_path(path)
    )
}

fn remote_ssh_authority(ssh: &WorkspaceSsh) -> String {
    let host = ssh.host.trim();
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let user = percent_encode_component(ssh.user.trim());
    let port = (ssh.port != 22)
        .then(|| format!(":{}", ssh.port))
        .unwrap_or_default();
    if user.is_empty() {
        format!("{host}{port}")
    } else {
        format!("{user}@{host}{port}")
    }
}

fn percent_encode_component(value: &str) -> String {
    percent_encode(value, false)
}

fn percent_encode_path(value: &str) -> String {
    let path = if value.starts_with('/') {
        value.to_string()
    } else {
        format!("/{value}")
    };
    percent_encode(path.as_str(), true)
}

fn percent_encode(value: &str, preserve_slash: bool) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~')
            || (preserve_slash && byte == b'/')
        {
            encoded.push(byte as char);
        } else {
            encoded.push_str(format!("%{byte:02X}").as_str());
        }
    }
    encoded
}

fn open_path_with_absolute(path: &str, target: &str) -> Result<(), String> {
    match target {
        "vscode" => run_open("code", &[path]),
        "cursor" => run_open("cursor", &[path]),
        "file_manager" => open_file_manager(path),
        _ => Err(format!("unknown open target: {target}")),
    }
}

fn has_command(name: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {name}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_open(bin: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(bin)
        .args(args)
        .status()
        .map_err(|error| format!("failed to launch {bin}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{bin} exited with {status}"))
    }
}

fn open_file_manager(path: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        if path.starts_with("sftp://") && has_command("nautilus") {
            return run_open("nautilus", &[path]);
        }
        return run_open("xdg-open", &[path]);
    }
    #[cfg(target_os = "macos")]
    {
        return run_open("open", &[path]);
    }
    #[cfg(target_os = "windows")]
    {
        return run_open("explorer", &[path]);
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = path;
        Err("unsupported platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_core::settings::WorkspaceSshAuth;

    fn remote_ssh() -> WorkspaceSsh {
        WorkspaceSsh {
            host: "10.43.30.29".into(),
            user: "robot user".into(),
            port: 2222,
            auth: WorkspaceSshAuth::Key,
            identity_file: String::new(),
            password: String::new(),
        }
    }

    #[test]
    fn remote_sftp_uri_encodes_user_and_path() {
        assert_eq!(
            remote_sftp_uri("/home/robot user/项目", &remote_ssh()),
            "sftp://robot%20user@10.43.30.29:2222/home/robot%20user/%E9%A1%B9%E7%9B%AE"
        );
    }
}
