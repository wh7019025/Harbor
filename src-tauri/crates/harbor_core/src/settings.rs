use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::version::APP_VERSION;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceMode {
    #[default]
    Local,
    Remote,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceSshAuth {
    #[default]
    Key,
    Sshpass,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSsh {
    pub host: String,
    #[serde(default)]
    pub user: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    #[serde(default)]
    pub auth: WorkspaceSshAuth,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub identity_file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,
}

fn default_ssh_port() -> u16 {
    22
}

impl Default for WorkspaceSsh {
    fn default() -> Self {
        Self {
            host: String::new(),
            user: String::new(),
            port: default_ssh_port(),
            auth: WorkspaceSshAuth::Key,
            identity_file: String::new(),
            password: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub mode: WorkspaceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<WorkspaceSsh>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub localhost_only: Option<bool>,
    #[serde(default)]
    pub search_paths: Vec<String>,
}

impl Workspace {
    pub fn localhost_only(&self) -> bool {
        self.localhost_only
            .unwrap_or(self.mode != WorkspaceMode::Remote)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_workspace_id")]
    pub current_workspace: String,
    #[serde(default = "default_workspaces")]
    pub workspaces: Vec<Workspace>,
    #[serde(alias = "metrics_fast_ms")]
    pub performance_metrics_interval_ms: u64,
    #[serde(alias = "metrics_slow_ms")]
    pub resource_metrics_interval_ms: u64,
}

fn default_workspace_id() -> String {
    "default".to_string()
}

fn default_workspaces() -> Vec<Workspace> {
    vec![default_workspace()]
}

pub fn default_workspace() -> Workspace {
    Workspace {
        id: default_workspace_id(),
        name: "default".to_string(),
        mode: WorkspaceMode::Local,
        ssh: None,
        localhost_only: Some(true),
        search_paths: Vec::new(),
    }
}

pub fn normalize_workspace_ssh(
    mode: &WorkspaceMode,
    ssh: Option<WorkspaceSsh>,
) -> Result<Option<WorkspaceSsh>, String> {
    if *mode != WorkspaceMode::Remote {
        return Ok(None);
    }
    let ssh = ssh.unwrap_or_default();
    let host = ssh.host.trim().to_string();
    if host.is_empty() {
        return Err("remote workspace requires an SSH host".into());
    }
    if ssh.port == 0 {
        return Err("ssh port must be between 1 and 65535".into());
    }
    let auth = ssh.auth;
    Ok(Some(WorkspaceSsh {
        host,
        user: ssh.user.trim().to_string(),
        port: ssh.port,
        auth,
        identity_file: if auth == WorkspaceSshAuth::Key {
            ssh.identity_file.trim().to_string()
        } else {
            String::new()
        },
        password: if auth == WorkspaceSshAuth::Sshpass {
            ssh.password
        } else {
            String::new()
        },
    }))
}

#[derive(Clone, Debug, PartialEq)]
pub struct SshVerifyCommand {
    pub program: String,
    pub args: Vec<String>,
    pub sshpass: bool,
}

fn ssh_target(ssh: &WorkspaceSsh) -> String {
    if ssh.user.is_empty() {
        ssh.host.clone()
    } else {
        format!("{}@{}", ssh.user, ssh.host)
    }
}

pub fn ssh_exec_command(ssh: &WorkspaceSsh, remote: &str) -> Result<SshVerifyCommand, String> {
    ssh_exec_command_with_args(ssh, remote, &[])
}

pub fn ssh_exec_command_with_args(
    ssh: &WorkspaceSsh,
    remote: &str,
    extra_args: &[String],
) -> Result<SshVerifyCommand, String> {
    let target = ssh_target(ssh);
    let mut ssh_args = vec![
        "-o".to_string(),
        "ConnectTimeout=8".to_string(),
        "-o".to_string(),
        "ConnectionAttempts=1".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-p".to_string(),
        ssh.port.to_string(),
    ];
    ssh_args.extend(extra_args.iter().cloned());
    match ssh.auth {
        WorkspaceSshAuth::Key => {
            ssh_args.splice(
                0..0,
                [
                    "-o".to_string(),
                    "BatchMode=yes".to_string(),
                    "-o".to_string(),
                    "PasswordAuthentication=no".to_string(),
                ],
            );
            if !ssh.identity_file.is_empty() {
                let identity = expand_path(ssh.identity_file.as_str());
                if !identity.is_file() {
                    return Err(format!(
                        "ssh identity file not found: {}",
                        identity.display()
                    ));
                }
                ssh_args.push("-i".to_string());
                ssh_args.push(identity.display().to_string());
                ssh_args.push("-o".to_string());
                ssh_args.push("IdentitiesOnly=yes".to_string());
            }
            ssh_args.push(target);
            ssh_args.push(remote.to_string());
            Ok(SshVerifyCommand {
                program: "ssh".into(),
                args: ssh_args,
                sshpass: false,
            })
        }
        WorkspaceSshAuth::Sshpass => {
            ssh_args.splice(
                0..0,
                [
                    "-o".to_string(),
                    "PreferredAuthentications=password".to_string(),
                    "-o".to_string(),
                    "PubkeyAuthentication=no".to_string(),
                    "-o".to_string(),
                    "KbdInteractiveAuthentication=no".to_string(),
                    "-o".to_string(),
                    "NumberOfPasswordPrompts=1".to_string(),
                ],
            );
            ssh_args.push(target);
            ssh_args.push(remote.to_string());
            let mut args = vec!["-e".to_string(), "ssh".to_string()];
            args.extend(ssh_args);
            Ok(SshVerifyCommand {
                program: "sshpass".into(),
                args,
                sshpass: true,
            })
        }
    }
}

pub fn ssh_verify_command(ssh: &WorkspaceSsh) -> Result<SshVerifyCommand, String> {
    ssh_exec_command(ssh, "true")
}

fn run_ssh_command(ssh: &WorkspaceSsh, command: &SshVerifyCommand) -> Result<String, String> {
    if command.sshpass && ssh.password.is_empty() {
        return Err("sshpass requires a password".into());
    }
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if command.sshpass {
        process.env("SSHPASS", ssh.password.as_str());
        process.env_remove("SSH_ASKPASS");
    }
    let output = process.output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!("{} is not installed", command.program)
        } else {
            format!("run {} failed: {error}", command.program)
        }
    })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        return Err(format!(
            "ssh failed with status {}",
            output.status.code().unwrap_or(-1)
        ));
    }
    Err(format!("ssh failed: {stderr}"))
}

pub fn ssh_run(ssh: &WorkspaceSsh, remote: &str) -> Result<String, String> {
    let ssh = normalize_workspace_ssh(&WorkspaceMode::Remote, Some(ssh.clone()))?
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
    let command = ssh_exec_command(&ssh, remote)?;
    run_ssh_command(&ssh, &command)
}

pub fn verify_workspace_ssh(ssh: WorkspaceSsh) -> Result<(), String> {
    ssh_run(&ssh, "true").map(|_| ())
}

pub fn scp_file(ssh: &WorkspaceSsh, local: &Path, remote_path: &str) -> Result<(), String> {
    let ssh = normalize_workspace_ssh(&WorkspaceMode::Remote, Some(ssh.clone()))?
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
    let target = format!("{}:{}", ssh_target(&ssh), remote_path);
    let mut args = vec![
        "-o".to_string(),
        "ConnectTimeout=8".to_string(),
        "-o".to_string(),
        "ConnectionAttempts=1".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-P".to_string(),
        ssh.port.to_string(),
    ];
    match ssh.auth {
        WorkspaceSshAuth::Key => {
            args.splice(
                0..0,
                [
                    "-o".to_string(),
                    "BatchMode=yes".to_string(),
                    "-o".to_string(),
                    "PasswordAuthentication=no".to_string(),
                ],
            );
            if !ssh.identity_file.is_empty() {
                let identity = expand_path(ssh.identity_file.as_str());
                if !identity.is_file() {
                    return Err(format!(
                        "ssh identity file not found: {}",
                        identity.display()
                    ));
                }
                args.push("-i".to_string());
                args.push(identity.display().to_string());
            }
            args.push(local.display().to_string());
            args.push(target);
            let command = SshVerifyCommand {
                program: "scp".into(),
                args,
                sshpass: false,
            };
            run_ssh_command(&ssh, &command).map(|_| ())
        }
        WorkspaceSshAuth::Sshpass => {
            args.splice(
                0..0,
                [
                    "-o".to_string(),
                    "PreferredAuthentications=password".to_string(),
                    "-o".to_string(),
                    "PubkeyAuthentication=no".to_string(),
                    "-o".to_string(),
                    "KbdInteractiveAuthentication=no".to_string(),
                    "-o".to_string(),
                    "NumberOfPasswordPrompts=1".to_string(),
                ],
            );
            args.push(local.display().to_string());
            args.push(target);
            let mut wrapped = vec!["-e".to_string(), "scp".to_string()];
            wrapped.extend(args);
            let command = SshVerifyCommand {
                program: "sshpass".into(),
                args: wrapped,
                sshpass: true,
            };
            run_ssh_command(&ssh, &command).map(|_| ())
        }
    }
}

pub fn ssh_send_file(
    ssh: &WorkspaceSsh,
    local: &Path,
    remote_shell_dest: &str,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let ssh = normalize_workspace_ssh(&WorkspaceMode::Remote, Some(ssh.clone()))?
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
    let command = ssh_exec_command(&ssh, &format!("cat > {remote_shell_dest}"))?;
    if command.sshpass && ssh.password.is_empty() {
        return Err("sshpass requires a password".into());
    }
    let total = fs::metadata(local)
        .map_err(|error| format!("stat {} failed: {error}", local.display()))?
        .len();
    let mut file = fs::File::open(local)
        .map_err(|error| format!("open {} failed: {error}", local.display()))?;
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if command.sshpass {
        process.env("SSHPASS", ssh.password.as_str());
        process.env_remove("SSH_ASKPASS");
    }
    let mut child = process.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!("{} is not installed", command.program)
        } else {
            format!("run {} failed: {error}", command.program)
        }
    })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "ssh stdin unavailable".to_string())?;
    let mut sent = 0u64;
    let mut buf = [0u8; 65536];
    on_progress(0, total);
    loop {
        let read = file
            .read(&mut buf)
            .map_err(|error| format!("read {} failed: {error}", local.display()))?;
        if read == 0 {
            break;
        }
        stdin
            .write_all(&buf[..read])
            .map_err(|error| format!("send harbor_core failed: {error}"))?;
        stdin
            .flush()
            .map_err(|error| format!("send harbor_core failed: {error}"))?;
        sent += read as u64;
        on_progress(sent, total);
    }
    drop(stdin);
    let output = child
        .wait_with_output()
        .map_err(|error| format!("ssh copy failed: {error}"))?;
    if output.status.success() {
        on_progress(total, total);
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        return Err(format!(
            "ssh copy failed with status {}",
            output.status.code().unwrap_or(-1)
        ));
    }
    Err(format!("ssh copy failed: {stderr}"))
}

pub fn resolve_local_search_path(path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("path cannot be empty".into());
    }
    let expanded = expand_path(trimmed);
    if !expanded.is_dir() {
        return Err(format!("path is not a directory: {}", expanded.display()));
    }
    Ok(stored_path(&expanded.to_string_lossy()))
}

pub fn add_current_search_path(settings: &mut Settings, path: &str) -> Result<String, String> {
    let stored = resolve_local_search_path(path)?;
    let workspace = settings.current_mut()?;
    if workspace
        .search_paths
        .iter()
        .any(|item| expand_path(item) == expand_path(stored.as_str()))
    {
        return Ok(stored);
    }
    workspace.search_paths.push(stored.clone());
    Ok(stored)
}

pub fn remove_current_search_path(settings: &mut Settings, path: &str) -> Result<(), String> {
    let expanded = expand_path(path);
    settings
        .current_mut()?
        .search_paths
        .retain(|item| expand_path(item) != expanded);
    Ok(())
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            current_workspace: default_workspace_id(),
            workspaces: default_workspaces(),
            performance_metrics_interval_ms: 1000,
            resource_metrics_interval_ms: 10000,
        }
    }
}

impl Settings {
    pub fn normalize(&mut self) {
        if self.workspaces.is_empty() {
            self.workspaces = default_workspaces();
        }
        if !self
            .workspaces
            .iter()
            .any(|workspace| workspace.id == self.current_workspace)
        {
            self.current_workspace = self.workspaces[0].id.clone();
        }
        for workspace in &mut self.workspaces {
            if workspace.mode != WorkspaceMode::Remote {
                workspace.mode = WorkspaceMode::Local;
                workspace.ssh = None;
            }
        }
    }

    pub fn current(&self) -> Result<&Workspace, String> {
        self.workspaces
            .iter()
            .find(|workspace| workspace.id == self.current_workspace)
            .ok_or_else(|| format!("workspace not found: {}", self.current_workspace))
    }

    pub fn current_mut(&mut self) -> Result<&mut Workspace, String> {
        let id = self.current_workspace.clone();
        self.workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
            .ok_or_else(|| format!("workspace not found: {id}"))
    }

    pub fn current_search_paths(&self) -> Result<Vec<PathBuf>, String> {
        Ok(self
            .current()?
            .search_paths
            .iter()
            .map(|path| expand_path(path))
            .collect())
    }
}

pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn config_dir() -> PathBuf {
    home_dir().join(".harbor")
}

pub fn workspace_data_dir(id: &str) -> PathBuf {
    config_dir().join("workspace").join(id)
}

pub fn runtime_data_dir() -> PathBuf {
    config_dir().join("runtime")
}

pub fn core_bin_dir() -> PathBuf {
    config_dir().join("core").join(APP_VERSION)
}

pub fn core_bin_path() -> PathBuf {
    core_bin_dir().join("harbor_core")
}

pub fn core_pid_path() -> PathBuf {
    config_dir().join("run").join("harbor_core.pid")
}

pub fn harbor_log_path() -> PathBuf {
    config_dir().join("log").join("harbor.log")
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    let mut settings = if let Ok(raw) = fs::read_to_string(&path) {
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        Settings::default()
    };
    settings.normalize();
    settings
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create config dir failed: {e}"))?;
    let path = settings_path();
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| format!("write settings failed: {e}"))
}

pub fn workspace_id_from_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("workspace name cannot be empty".into());
    }
    let mut id = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_' | ' ') {
            if !id.ends_with('-') {
                id.push('-');
            }
        }
    }
    let id = id.trim_matches('-').to_string();
    if id.is_empty() {
        return Err("workspace name must contain letters or digits".into());
    }
    Ok(id)
}

pub fn unique_workspace_id(settings: &Settings, name: &str) -> Result<String, String> {
    let base = workspace_id_from_name(name)?;
    if !settings
        .workspaces
        .iter()
        .any(|workspace| workspace.id == base)
    {
        return Ok(base);
    }
    for index in 2..1000 {
        let candidate = format!("{base}-{index}");
        if !settings
            .workspaces
            .iter()
            .any(|workspace| workspace.id == candidate)
        {
            return Ok(candidate);
        }
    }
    Err("could not allocate workspace id".into())
}

pub fn expand_path(value: &str) -> PathBuf {
    expand_path_with_base(value, &home_dir())
}

pub fn expand_path_with_base(value: &str, base: &Path) -> PathBuf {
    let value = value.trim();
    if value.is_empty() {
        return base.to_path_buf();
    }
    if value == "~" {
        return home_dir();
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn collapse_path(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        expand_path(path.to_string_lossy().as_ref())
    };
    let home = home_dir();
    if absolute == home {
        return "~/".into();
    }
    if let Ok(rel) = absolute.strip_prefix(&home) {
        let rel = rel.to_string_lossy().replace('\\', "/");
        if rel.is_empty() || rel == "." {
            return "~/".into();
        }
        return format!("~/{rel}/");
    }
    let mut display = absolute.to_string_lossy().replace('\\', "/");
    if !display.ends_with('/') {
        display.push('/');
    }
    display
}

const PATH_SUGGESTION_LIMIT: usize = 50;

fn display_dir_path(path: &Path) -> String {
    let mut display = expand_path(path.to_string_lossy().as_ref())
        .to_string_lossy()
        .replace('\\', "/");
    if display != "/" && !display.ends_with('/') {
        display.push('/');
    }
    display
}

fn home_prefix() -> String {
    display_dir_path(&home_dir())
}

fn normalize_home_prefix(home: &str) -> String {
    let home = home.trim().replace('\\', "/");
    if home.is_empty() || home == "/" {
        return "/".into();
    }
    format!("{}/", home.trim_end_matches('/'))
}

fn normalize_path_query_with_home(prefix: &str, home: &str) -> String {
    let raw = prefix.trim();
    let home = normalize_home_prefix(home);
    if raw.is_empty() || raw == "~" || raw == "~/" {
        return home;
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return format!("{home}{rest}");
    }
    if raw.starts_with('/') {
        return raw.to_string();
    }
    if let Some(rest) = raw.strip_prefix('~') {
        return format!("{home}{}", rest.trim_start_matches('/'));
    }
    format!("{home}{raw}")
}

pub fn path_suggestion_query(prefix: &str) -> String {
    normalize_path_query_with_home(prefix, home_prefix().as_str())
}

fn stored_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    if path == "/" {
        path
    } else {
        path.trim_end_matches('/').to_string()
    }
}

fn split_path_query(query: &str) -> (String, String) {
    if query.ends_with('/') {
        (query.to_string(), String::new())
    } else if let Some((parent, name)) = query.rsplit_once('/') {
        (format!("{parent}/"), name.to_string())
    } else {
        ("/".into(), query.to_string())
    }
}

fn suggest_dirs(parent: &Path, name_prefix: &str, limit: usize) -> Vec<String> {
    let show_hidden = name_prefix.starts_with('.');
    let prefix_lower = name_prefix.to_ascii_lowercase();
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "." || name == ".." {
            continue;
        }
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        if !prefix_lower.is_empty() && !name.to_ascii_lowercase().starts_with(prefix_lower.as_str())
        {
            continue;
        }
        names.push(display_dir_path(&entry.path()));
    }
    names.sort();
    names.truncate(limit);
    names
}

pub fn list_path_suggestions(prefix: &str) -> Vec<String> {
    let query = path_suggestion_query(prefix);
    let (parent_query, name_prefix) = split_path_query(query.as_str());
    let parent = expand_path(parent_query.as_str());
    if !parent.is_dir() {
        return Vec::new();
    }
    suggest_dirs(&parent, name_prefix.as_str(), PATH_SUGGESTION_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_path_with_base_supports_tilde_and_relative() {
        let home = home_dir();
        assert_eq!(expand_path_with_base("~", &home), home);
        assert_eq!(
            expand_path_with_base("~/projects/foo", &home),
            home.join("projects/foo")
        );
        assert_eq!(
            expand_path_with_base("relative/dir", &home),
            home.join("relative/dir")
        );
        assert_eq!(
            expand_path_with_base("/abs/path", &home),
            PathBuf::from("/abs/path")
        );
    }

    #[test]
    fn collapse_path_uses_tilde_for_home() {
        let home = home_dir();
        assert_eq!(collapse_path(home.as_path()), "~/");
        assert_eq!(collapse_path(&home.join(".harbor")), "~/.harbor/");
        assert_eq!(
            collapse_path(&home.join("projects/demo")),
            "~/projects/demo/"
        );
    }

    #[test]
    fn local_workspace_localhost_only_defaults_true() {
        let settings: Settings = serde_json::from_str(
            r#"{
                "performance_metrics_interval_ms": 1000,
                "resource_metrics_interval_ms": 10000
            }"#,
        )
        .unwrap();
        assert!(settings.current().unwrap().localhost_only());
        assert_eq!(settings.current_workspace, "default");
        assert_eq!(settings.workspaces.len(), 1);
    }

    #[test]
    fn workspace_id_from_name_slugifies() {
        assert_eq!(workspace_id_from_name("My Lab").unwrap(), "my-lab");
        assert!(workspace_id_from_name("   ").is_err());
    }

    #[test]
    fn unique_workspace_id_avoids_collisions() {
        let mut settings = Settings::default();
        settings.workspaces.push(Workspace {
            id: "lab".into(),
            name: "lab".into(),
            mode: WorkspaceMode::Local,
            ssh: None,
            localhost_only: Some(true),
            search_paths: Vec::new(),
        });
        assert_eq!(unique_workspace_id(&settings, "lab").unwrap(), "lab-2");
    }

    #[test]
    fn workspace_defaults_to_local_without_ssh() {
        let workspace: Workspace = serde_json::from_str(
            r#"{
                "id": "lab",
                "name": "lab"
            }"#,
        )
        .unwrap();
        assert_eq!(workspace.mode, WorkspaceMode::Local);
        assert_eq!(workspace.ssh, None);
        assert!(workspace.localhost_only());
    }

    #[test]
    fn remote_workspace_localhost_only_defaults_false() {
        let workspace: Workspace = serde_json::from_str(
            r#"{
                "id": "lab",
                "name": "lab",
                "mode": "remote",
                "ssh": { "host": "box.example" }
            }"#,
        )
        .unwrap();
        assert!(!workspace.localhost_only());
    }

    #[test]
    fn remote_workspace_requires_ssh_host() {
        assert!(normalize_workspace_ssh(&WorkspaceMode::Remote, None).is_err());
        let ssh = normalize_workspace_ssh(
            &WorkspaceMode::Remote,
            Some(WorkspaceSsh {
                host: "  box.example  ".into(),
                user: " se ".into(),
                port: 2222,
                auth: WorkspaceSshAuth::Key,
                identity_file: " ~/.ssh/id_ed25519 ".into(),
                password: String::new(),
            }),
        )
        .unwrap()
        .unwrap();
        assert_eq!(ssh.host, "box.example");
        assert_eq!(ssh.user, "se");
        assert_eq!(ssh.port, 2222);
        assert_eq!(ssh.auth, WorkspaceSshAuth::Key);
        assert_eq!(ssh.identity_file, "~/.ssh/id_ed25519");
        assert_eq!(
            normalize_workspace_ssh(&WorkspaceMode::Local, Some(ssh)),
            Ok(None)
        );
    }

    #[test]
    fn ssh_verify_command_uses_key_or_sshpass() {
        let key = WorkspaceSsh {
            host: "box.example".into(),
            user: "se".into(),
            port: 2222,
            auth: WorkspaceSshAuth::Key,
            identity_file: String::new(),
            password: String::new(),
        };
        let command = ssh_verify_command(&key).unwrap();
        assert_eq!(command.program, "ssh");
        assert!(!command.sshpass);
        assert!(command.args.contains(&"se@box.example".to_string()));
        assert!(command.args.contains(&"2222".to_string()));

        let sshpass = WorkspaceSsh {
            auth: WorkspaceSshAuth::Sshpass,
            ..key
        };
        let command = ssh_verify_command(&sshpass).unwrap();
        assert_eq!(command.program, "sshpass");
        assert!(command.sshpass);
        assert_eq!(command.args[0], "-e");
        assert_eq!(command.args[1], "ssh");
        assert!(!command
            .args
            .windows(2)
            .any(|pair| pair[0] == "-p" && pair[1] != "2222"));
    }

    #[test]
    fn ssh_exec_command_places_tunnel_before_target() {
        let ssh = WorkspaceSsh {
            host: "box.example".into(),
            user: "se".into(),
            port: 22,
            auth: WorkspaceSshAuth::Key,
            identity_file: String::new(),
            password: String::new(),
        };
        let command = ssh_exec_command_with_args(
            &ssh,
            "ttyd bash",
            &["-L".into(), "127.0.0.1:1234:127.0.0.1:29386".into()],
        )
        .unwrap();
        let target = command
            .args
            .iter()
            .position(|arg| arg == "se@box.example")
            .unwrap();
        let tunnel = command.args.iter().position(|arg| arg == "-L").unwrap();
        assert!(tunnel < target);
        assert_eq!(command.args.last().unwrap(), "ttyd bash");
    }

    #[test]
    fn verify_sshpass_requires_password() {
        let error = verify_workspace_ssh(WorkspaceSsh {
            host: "box.example".into(),
            user: "se".into(),
            port: 22,
            auth: WorkspaceSshAuth::Sshpass,
            identity_file: String::new(),
            password: String::new(),
        })
        .unwrap_err();
        assert!(error.contains("password"));
        let ssh = normalize_workspace_ssh(
            &WorkspaceMode::Remote,
            Some(WorkspaceSsh {
                host: "box.example".into(),
                user: "se".into(),
                port: 22,
                auth: WorkspaceSshAuth::Sshpass,
                identity_file: String::new(),
                password: " secret ".into(),
            }),
        )
        .unwrap()
        .unwrap();
        assert_eq!(ssh.password, " secret ");
    }

    #[test]
    fn runtime_data_dir_is_shared_under_harbor() {
        assert_eq!(runtime_data_dir(), home_dir().join(".harbor/runtime"));
    }

    #[test]
    fn workspace_data_dir_keeps_logs_separate() {
        assert_eq!(
            workspace_data_dir("lab"),
            home_dir().join(".harbor/workspace/lab")
        );
    }

    #[test]
    fn old_settings_json_does_not_migrate_search_paths() {
        let settings: Settings = serde_json::from_str(
            r#"{
                "taskcard_root": "~/.harbor/harbor_taskcfg",
                "search_paths": ["/old/project"],
                "metrics_fast_ms": 500,
                "metrics_slow_ms": 2000,
                "web_api_localhost_only": false
            }"#,
        )
        .unwrap();
        assert_eq!(settings.current_workspace, "default");
        assert_eq!(settings.workspaces, default_workspaces());
        assert!(settings.current().unwrap().search_paths.is_empty());
        assert_eq!(settings.performance_metrics_interval_ms, 500);
        assert_eq!(settings.resource_metrics_interval_ms, 2000);
        assert!(settings.current().unwrap().localhost_only());
    }

    #[test]
    fn path_suggestions_list_matching_directories() {
        let root = std::env::temp_dir().join(format!(
            "harbor-path-suggest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("projects")).unwrap();
        fs::create_dir_all(root.join("pictures")).unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join("readme.txt"), "x").unwrap();
        let found = suggest_dirs(&root, "pro", 16);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("projects/"));
        let all = suggest_dirs(&root, "", 16);
        assert!(all.iter().any(|path| path.ends_with("pictures/")));
        assert!(!all.iter().any(|path| path.contains(".hidden")));
        let hidden = suggest_dirs(&root, ".", 16);
        assert!(hidden.iter().any(|path| path.contains(".hidden")));
        for index in 0..60 {
            fs::create_dir_all(root.join(format!("candidate-{index:02}"))).unwrap();
        }
        let capped = suggest_dirs(&root, "candidate-", PATH_SUGGESTION_LIMIT);
        assert_eq!(capped.len(), 50);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn normalize_path_query_defaults_to_home() {
        let home = format!(
            "{}/",
            home_dir()
                .to_string_lossy()
                .replace('\\', "/")
                .trim_end_matches('/')
        );
        assert_eq!(path_suggestion_query(""), home);
        assert_eq!(path_suggestion_query("~"), home);
        assert_eq!(path_suggestion_query("projects"), format!("{home}projects"));
        assert_eq!(
            path_suggestion_query("~/projects"),
            format!("{home}projects")
        );
        assert_eq!(
            split_path_query(&format!("{home}projects/foo")),
            (format!("{home}projects/"), "foo".into())
        );
        assert_eq!(split_path_query(&home), (home, "".into()));
    }

    #[test]
    fn normalize_path_query_with_home_uses_remote_home() {
        assert_eq!(
            normalize_path_query_with_home("", "/home/box"),
            "/home/box/"
        );
        assert_eq!(normalize_path_query_with_home("~", "/root"), "/root/");
        assert_eq!(
            normalize_path_query_with_home("~/src", "/home/box"),
            "/home/box/src"
        );
        assert_eq!(
            normalize_path_query_with_home("/opt/data", "/home/box"),
            "/opt/data"
        );
    }

    #[test]
    fn core_bin_path_is_versioned() {
        assert_eq!(
            core_bin_path(),
            home_dir()
                .join(".harbor/core")
                .join(APP_VERSION)
                .join("harbor_core")
        );
    }
}
