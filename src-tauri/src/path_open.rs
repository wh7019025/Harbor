use serde::Serialize;
use std::process::{Command, Stdio};

use crate::settings::expand_path;

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

pub fn open_path_with(path: &str, target: &str) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("path cannot be empty".into());
    }
    let absolute = expand_path(path);
    let path = absolute.to_string_lossy();
    open_path_with_absolute(path.as_ref(), target)
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
