pub mod app_log;
pub mod service;
pub mod settings;
pub mod taskcard;
pub mod terminal;
pub mod version;
mod vnc_interface;
pub mod web_api;

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use settings::{
    core_pid_path, load_settings, runtime_data_dir, save_settings, workspace_data_dir, Settings,
    Workspace, WorkspaceMode,
};
use taskcard::TaskCardService;
use web_api::{bind_addr, router, WebApiState};

#[derive(Debug)]
struct CoreInstanceLock {
    _file: File,
}

#[cfg(unix)]
impl Drop for CoreInstanceLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        unsafe {
            libc::flock(self._file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

fn acquire_core_instance_at(path: &Path) -> Result<CoreInstanceLock, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {} failed: {error}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("open {} failed: {error}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            let mut owner = String::new();
            let _ = file.read_to_string(&mut owner);
            let owner = owner.trim();
            return Err(if owner.is_empty() {
                "another harbor_core is already running on this machine".into()
            } else {
                format!("another harbor_core is already running on this machine (pid {owner})")
            });
        }
    }

    file.set_len(0)
        .map_err(|error| format!("truncate {} failed: {error}", path.display()))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("seek {} failed: {error}", path.display()))?;
    write!(file, "{}", std::process::id())
        .map_err(|error| format!("write {} failed: {error}", path.display()))?;
    file.sync_data()
        .map_err(|error| format!("sync {} failed: {error}", path.display()))?;
    Ok(CoreInstanceLock { _file: file })
}

fn acquire_core_instance() -> Result<CoreInstanceLock, String> {
    acquire_core_instance_at(core_pid_path().as_path())
}

pub struct CoreArgs {
    pub localhost_only: bool,
    pub remote_runtime: bool,
    pub workspace_id: Option<String>,
}

impl CoreArgs {
    pub fn from_env() -> Result<Self, String> {
        let mut localhost_only = true;
        let mut remote_runtime = false;
        let mut workspace_id = None;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--version" | "-V" => {
                    println!("{}", version::APP_VERSION);
                    std::process::exit(0);
                }
                "--localhost-only" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--localhost-only requires true or false".to_string())?;
                    localhost_only = parse_bool(&value)?;
                }
                "--remote-runtime" => {
                    remote_runtime = true;
                }
                "--workspace" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--workspace requires an id".to_string())?;
                    workspace_id = Some(value);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        Ok(Self {
            localhost_only,
            remote_runtime,
            workspace_id,
        })
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(format!("invalid boolean: {value}")),
    }
}

pub fn ensure_workspace(
    settings: &mut Settings,
    workspace_id: Option<&str>,
    persist: bool,
) -> Result<String, String> {
    if let Some(id) = workspace_id.map(str::trim).filter(|id| !id.is_empty()) {
        if !settings
            .workspaces
            .iter()
            .any(|workspace| workspace.id == id)
        {
            settings.workspaces.push(Workspace {
                id: id.to_string(),
                name: id.to_string(),
                mode: WorkspaceMode::Local,
                ssh: None,
                localhost_only: None,
                search_paths: Vec::new(),
            });
        }
        settings.current_workspace = id.to_string();
    }
    settings.normalize();
    if persist {
        save_settings(settings)?;
    }
    Ok(settings.current_workspace.clone())
}

pub fn make_taskcard(settings: &Settings, remote_runtime: bool) -> Result<TaskCardService, String> {
    let taskcard = TaskCardService::new(runtime_data_dir(), settings.current_search_paths()?)?;
    taskcard.set_log_dir(
        workspace_data_dir(settings.current()?.id.as_str(), remote_runtime).join("log"),
    )?;
    Ok(taskcard)
}

pub async fn run_async(args: CoreArgs) -> Result<(), String> {
    // --- 阶段 1：获取机器级单实例锁 ---
    let _instance_lock = acquire_core_instance()?;

    // --- 阶段 2：加载 workspace 与任务服务 ---
    let mut settings = if args.remote_runtime {
        Settings::default()
    } else {
        load_settings()
    };
    let workspace_id = ensure_workspace(
        &mut settings,
        args.workspace_id.as_deref(),
        !args.remote_runtime,
    )?;
    let taskcard = make_taskcard(&settings, args.remote_runtime).map_err(|error| {
        crate::app_log::core(&error);
        error
    })?;
    taskcard.set_remote_runtime(args.remote_runtime);

    // --- 阶段 3：远端 Core 后台维持虚拟与真实 VNC 桌面 ---
    if args.remote_runtime {
        std::thread::spawn(|| {
            if let Err(error) = crate::vnc_interface::ensure_shared_display() {
                crate::app_log::core(&error);
            } else {
                crate::app_log::core("shared Harbor VNC desktop ready");
            }
        });
        std::thread::spawn(|| {
            if let Err(error) = crate::vnc_interface::ensure_physical_display() {
                crate::app_log::core(&error);
            } else {
                crate::app_log::core("physical Harbor VNC display ready");
            }
        });
    }

    // --- 阶段 4：绑定 API 端口并提供服务 ---
    let addr = bind_addr(args.localhost_only);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|error| format!("listen {addr} failed: {error}"))?;
    crate::app_log::core(&format!(
        "harbor_core {} workspace={workspace_id} listen={} localhost_only={}",
        version::APP_VERSION,
        web_api::listen_url(args.localhost_only),
        args.localhost_only
    ));
    let state = WebApiState::new(taskcard, settings, args.localhost_only, args.remote_runtime);
    let shutdown_taskcard = state.taskcard.clone();
    let shutdown_terminal = state.terminal.clone();
    let serve_result = axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|error| format!("harbor_core server failed: {error}"));

    // --- 阶段 5：关闭 Core 基础设施，但保留独立运行的 Task ---
    if args.remote_runtime {
        crate::vnc_interface::shutdown_displays();
    }
    shutdown_terminal.lock().stop();
    let managed_count = shutdown_taskcard.lock().managed_processes().len();
    crate::app_log::core(&format!(
        "harbor_core stopping; leaving {managed_count} managed process groups running"
    ));
    serve_result
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use super::acquire_core_instance_at;

    #[test]
    #[cfg(unix)]
    fn core_instance_lock_rejects_a_second_process_lock() {
        let path = std::env::temp_dir().join(format!(
            "harbor-core-lock-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = acquire_core_instance_at(&path).unwrap();
        let owner = std::fs::read_to_string(&path).unwrap();
        assert_eq!(owner.trim().parse::<u32>().unwrap(), std::process::id());
        let error = acquire_core_instance_at(&path).unwrap_err();
        assert!(error.contains("already running"));
        drop(first);
        let second = acquire_core_instance_at(&path).unwrap();
        drop(second);
        let _ = std::fs::remove_file(path);
    }
}
