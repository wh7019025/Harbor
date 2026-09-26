use std::fs;
use std::io::Read;
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use harbor_common as harbor_support;
use harbor_common::settings::{
    core_bin_dir, core_bin_path, core_pid_path, ssh_run, ssh_send_file, Settings, Workspace,
    WorkspaceMode, WorkspaceSsh,
};
use harbor_common::version::APP_VERSION;
use harbor_protocol::web_api::{CORE_API_REVISION, WEB_API_PORT};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::core_client::{
    claim_access_url, fetch_access_url, fetch_health, fetch_health_url, local_core_url,
    release_access, switch_workspace, CoreHealth,
};

const EXPECTED_CORE_SHA256: &str = env!("HARBOR_CORE_SHA256");
const EXPECTED_CORE_RUNTIME_SHA256: &str = env!("HARBOR_CORE_RUNTIME_SHA256");
const CORE_RUNTIME_LOADER: &str = env!("HARBOR_CORE_RUNTIME_LOADER");
const BUILD_CORE_RUNTIME_PATH: &str = env!("HARBOR_CORE_RUNTIME_BUILD_PATH");
#[cfg(target_os = "linux")]
const BUILD_CORE_PATH: &str = env!("HARBOR_CORE_BUILD_PATH");
const EXPECTED_TTYD_SHA256: &str = env!("HARBOR_TTYD_SHA256");
const EXPECTED_TTYD_RUNTIME_SHA256: &str = env!("HARBOR_TTYD_RUNTIME_SHA256");
const TTYD_RUNTIME_LOADER: &str = env!("HARBOR_TTYD_RUNTIME_LOADER");
const BUILD_TTYD_RUNTIME_PATH: &str = env!("HARBOR_TTYD_RUNTIME_BUILD_PATH");
const CORE_RUNTIME_NAME: &str = "harbor_core-runtime-linux-x86_64.tar.gz";
const TTYD_RUNTIME_NAME: &str = "harbor_ttyd-runtime-linux-x86_64.tar.gz";

static ARTIFACT_RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_artifact_resource_dir(path: PathBuf) {
    let _ = ARTIFACT_RESOURCE_DIR.set(path);
}

#[derive(Clone, Debug, Serialize)]
pub struct DeployProgress {
    pub active: bool,
    pub percent: u8,
    pub transferred: u64,
    pub total: u64,
}

impl Default for DeployProgress {
    fn default() -> Self {
        Self {
            active: false,
            percent: 0,
            transferred: 0,
            total: 0,
        }
    }
}

struct RemoteDeployGate {
    progress: DeployProgress,
}

fn remote_deploy_gate() -> &'static Mutex<RemoteDeployGate> {
    static GATE: OnceLock<Mutex<RemoteDeployGate>> = OnceLock::new();
    GATE.get_or_init(|| {
        Mutex::new(RemoteDeployGate {
            progress: DeployProgress::default(),
        })
    })
}

fn set_deploy_progress(percent: u8, transferred: u64, total: u64) {
    let mut gate = remote_deploy_gate()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    gate.progress = DeployProgress {
        active: true,
        percent: percent.min(100),
        transferred,
        total,
    };
}

fn finish_deploy_progress() {
    remote_deploy_gate()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .progress
        .active = false;
}

pub fn deploy_progress() -> DeployProgress {
    remote_deploy_gate()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .progress
        .clone()
}

#[cfg(target_os = "linux")]
pub fn packaged_core_bin() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| format!("current exe failed: {error}"))?;
    let sibling = exe
        .parent()
        .ok_or_else(|| "cannot resolve harbor_core next to Harbor".to_string())?
        .join("harbor_core");
    let candidates = [sibling, PathBuf::from(BUILD_CORE_PATH)];
    let mut rejected = Vec::new();
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        let hash = sha256_file(&candidate)?;
        if hash == EXPECTED_CORE_SHA256 {
            return Ok(candidate);
        }
        rejected.push(format!("{}={hash}", candidate.display()));
    }
    Err(format!(
        "GUI build is stale or its immutable release harbor_core is missing; restart/rebuild Harbor. expected sha256 {EXPECTED_CORE_SHA256}; candidates: {}",
        rejected.join(", ")
    ))
}

fn packaged_runtime_bundle(
    file_name: &str,
    build_path: &str,
    expected_hash: &str,
) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| format!("current exe failed: {error}"))?;
    let mut candidates = vec![
        exe.parent()
            .ok_or_else(|| "cannot resolve Harbor runtime directory".to_string())?
            .join(file_name),
        PathBuf::from("/usr/lib/harbor").join(file_name),
    ];
    if let Some(resource_dir) = ARTIFACT_RESOURCE_DIR.get() {
        candidates.push(resource_dir.join(file_name));
    }
    candidates.push(PathBuf::from(build_path));
    let mut rejected = Vec::new();
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        let hash = sha256_file(&candidate)?;
        if hash == expected_hash {
            return Ok(candidate);
        }
        rejected.push(format!("{}={hash}", candidate.display()));
    }
    Err(format!(
        "matching Linux runtime {file_name} not found; expected sha256 {expected_hash}; candidates: {}",
        rejected.join(", ")
    ))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("open {} for hashing failed: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("hash {} failed: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn managed_local_core_matches() -> bool {
    let path = core_bin_path();
    path.is_file()
        && sha256_file(&path)
            .map(|hash| hash == EXPECTED_CORE_SHA256)
            .unwrap_or(false)
}

fn runtime_loader(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn ensure_remote_runtime_architecture(ssh: &WorkspaceSsh) -> Result<(), String> {
    let architecture = ssh_run(ssh, "uname -m")?;
    match architecture.trim() {
        "x86_64" | "amd64" => Ok(()),
        other => Err(format!(
            "remote architecture {other} is unsupported by this Harbor build; expected x86_64"
        )),
    }
}

fn remote_core_dir() -> String {
    format!(".harbor/core/{APP_VERSION}")
}

fn remote_core_exec(loader: Option<&str>, args: &str) -> String {
    let dir = remote_core_dir();
    remote_runtime_exec(dir.as_str(), "harbor_core", loader, args)
}

fn remote_runtime_exec(dir: &str, binary: &str, loader: Option<&str>, args: &str) -> String {
    let args = args.trim();
    match loader {
        Some(loader) => format!(
            r#""$HOME/{dir}/{loader}" --library-path "$HOME/{dir}" "$HOME/{dir}/{binary}"{tail}"#,
            tail = if args.is_empty() {
                String::new()
            } else {
                format!(" {args}")
            }
        ),
        None => format!(
            r#""$HOME/{dir}/{binary}"{tail}"#,
            tail = if args.is_empty() {
                String::new()
            } else {
                format!(" {args}")
            }
        ),
    }
}

fn send_remote_runtime(
    ssh: &WorkspaceSsh,
    archive: &Path,
    dir: &str,
    runtime_hash: &str,
    binary_name: &str,
    binary_hash: &str,
) -> Result<(), String> {
    let total = fs::metadata(&archive)
        .map_err(|error| format!("stat {} failed: {error}", archive.display()))?
        .len();
    let remote_archive = format!("\"$HOME/{dir}/runtime.tar.gz.new\"");

    // --- 阶段 1：发送压缩部署包 ---
    set_deploy_progress(2, 0, total);
    ssh_run(ssh, &format!("mkdir -p \"$HOME/{dir}\""))?;
    ssh_send_file(ssh, archive, remote_archive.as_str(), |sent, _| {
        let percent = if total == 0 {
            2
        } else {
            (2 + sent.saturating_mul(88) / total).min(90) as u8
        };
        set_deploy_progress(percent, sent, total);
    })?;

    // --- 阶段 2：远端解压并校验二进制 ---
    ssh_run(
        ssh,
        &format!(
            "test \"$(sha256sum {remote_archive} | cut -d ' ' -f 1)\" = {} && tar -xzf {remote_archive} -C \"$HOME/{dir}\" && rm -f {remote_archive} && chmod +x \"$HOME/{dir}/{binary_name}\" && test \"$(sha256sum \"$HOME/{dir}/{binary_name}\" | cut -d ' ' -f 1)\" = {} && printf '%s\\n' {} > \"$HOME/{dir}/.runtime-sha256\"",
            shell_single_quote(runtime_hash),
            shell_single_quote(binary_hash),
            shell_single_quote(runtime_hash),
        ),
    )?;
    set_deploy_progress(90, total, total);
    Ok(())
}

pub fn ensure_remote_ttyd_runtime(ssh: &WorkspaceSsh) -> Result<(), String> {
    harbor_support::app_log::gui(&format!(
        "workspace terminal 1/3: checking managed ttyd on {}",
        ssh.host.trim()
    ));
    ensure_remote_runtime_architecture(ssh)?;
    let archive = packaged_runtime_bundle(
        TTYD_RUNTIME_NAME,
        BUILD_TTYD_RUNTIME_PATH,
        EXPECTED_TTYD_RUNTIME_SHA256,
    )?;
    let loader = runtime_loader(TTYD_RUNTIME_LOADER);
    let dir = format!(".harbor/tools/ttyd/{EXPECTED_TTYD_SHA256}");
    let remote_state = ssh_run(
        ssh,
        &format!(
            "if [ -x \"$HOME/{dir}/ttyd\" ]; then sha256sum \"$HOME/{dir}/ttyd\" | cut -d ' ' -f 1; cat \"$HOME/{dir}/.runtime-sha256\" 2>/dev/null || true; fi"
        ),
    )?;
    let mut remote_state = remote_state.lines();
    let binary_matches = remote_state.next() == Some(EXPECTED_TTYD_SHA256);
    let runtime_matches = remote_state.next() == Some(EXPECTED_TTYD_RUNTIME_SHA256);
    if !binary_matches || !runtime_matches {
        harbor_support::app_log::gui("workspace terminal 2/3: deploying managed ttyd");
        send_remote_runtime(
            ssh,
            archive.as_path(),
            dir.as_str(),
            EXPECTED_TTYD_RUNTIME_SHA256,
            "ttyd",
            EXPECTED_TTYD_SHA256,
        )?;
    } else {
        harbor_support::app_log::gui("workspace terminal 2/3: reusing managed ttyd");
    }
    let command = remote_runtime_exec(dir.as_str(), "ttyd", loader, r#""$@""#);
    let launcher = format!("#!/bin/sh\nexec {command}\n");
    ssh_run(
        ssh,
        &format!(
            "printf '%s' {} > \"$HOME/{dir}/run\" && chmod +x \"$HOME/{dir}/run\" && ln -sfn \"$HOME/{dir}\" \"$HOME/.harbor/tools/ttyd/current\"",
            shell_single_quote(launcher.as_str())
        ),
    )?;
    harbor_support::app_log::gui("workspace terminal 3/3: managed ttyd runtime ready");
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn install_local_core_bin() -> Result<PathBuf, String> {
    let source = packaged_core_bin()?;
    let dest_dir = core_bin_dir();
    fs::create_dir_all(&dest_dir)
        .map_err(|error| format!("create {} failed: {error}", dest_dir.display()))?;
    let dest = core_bin_path();
    let installed_matches = dest.is_file()
        && sha256_file(&dest)
            .map(|hash| hash == EXPECTED_CORE_SHA256)
            .unwrap_or(false);
    if !installed_matches {
        let tmp = dest.with_file_name("harbor_core.new");
        fs::copy(&source, &tmp).map_err(|error| format!("copy harbor_core failed: {error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&tmp)
                .map_err(|error| format!("stat harbor_core failed: {error}"))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&tmp, perms)
                .map_err(|error| format!("chmod harbor_core failed: {error}"))?;
        }
        fs::rename(&tmp, &dest).map_err(|error| format!("replace harbor_core failed: {error}"))?;
    }
    let installed_hash = sha256_file(&dest)?;
    if installed_hash != EXPECTED_CORE_SHA256 {
        return Err(format!(
            "managed harbor_core hash mismatch after install: expected {EXPECTED_CORE_SHA256}, got {installed_hash}"
        ));
    }
    Ok(dest)
}

fn wait_health(base: &str, expected_workspace: Option<&str>) -> Result<CoreHealth, String> {
    let mut last = "harbor_core did not become ready".to_string();
    for _ in 0..40 {
        match fetch_health_url(base) {
            Ok(health) if health.ok => {
                if health.version != APP_VERSION {
                    last = format!(
                        "harbor_core version {} does not match GUI {APP_VERSION}",
                        health.version
                    );
                } else if health.api_revision != CORE_API_REVISION {
                    last = format!(
                        "harbor_core API revision {} does not match GUI {CORE_API_REVISION}",
                        health.api_revision
                    );
                } else if let Some(expected) = expected_workspace {
                    if health.workspace_id != expected {
                        last = format!(
                            "harbor_core workspace {} does not match {expected}",
                            health.workspace_id
                        );
                    } else {
                        return Ok(health);
                    }
                } else {
                    return Ok(health);
                }
            }
            Ok(_) => last = "harbor_core health returned ok=false".into(),
            Err(error) => last = error,
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(last)
}

#[cfg(target_os = "linux")]
fn terminate_pid(pid: i32) {
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    for _ in 0..200 {
        if !pid_is_running(pid) {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
}

#[cfg(target_os = "linux")]
fn pid_is_running(pid: i32) -> bool {
    if unsafe { libc::kill(pid, 0) != 0 } {
        return false;
    }
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| stat.rfind(')').map(|end| stat[end + 1..].to_string()))
        .and_then(|fields| fields.split_whitespace().next().map(str::to_string))
        .is_none_or(|state| state != "Z")
}

fn remote_terminate_script(pid: &str) -> String {
    format!(
        r#"if kill -0 {pid} 2>/dev/null; then
  kill {pid} 2>/dev/null || true
  for attempt in $(seq 1 100); do
    kill -0 {pid} 2>/dev/null || break
    sleep 0.1
  done
  if kill -0 {pid} 2>/dev/null; then
    kill -9 {pid} 2>/dev/null || true
  fi
fi"#
    )
}

#[cfg(target_os = "linux")]
fn stop_local_core() {
    let health_pid = fetch_health_url(local_core_url().as_str())
        .ok()
        .and_then(|health| i32::try_from(health.pid).ok())
        .filter(|pid| *pid > 0);
    let file_pid = fs::read_to_string(core_pid_path())
        .ok()
        .and_then(|raw| raw.trim().parse::<i32>().ok());
    if let Some(pid) = health_pid.or(file_pid) {
        terminate_pid(pid);
    }
}

#[cfg(target_os = "linux")]
fn spawn_local_core(workspace: &Workspace) -> Result<(), String> {
    stop_local_core();
    thread::sleep(Duration::from_millis(200));
    let bin = install_local_core_bin()?;
    let localhost_only = workspace.localhost_only();
    let mut command = Command::new(&bin);
    command
        .arg("--localhost-only")
        .arg(if localhost_only { "true" } else { "false" })
        .arg("--workspace")
        .arg(workspace.id.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command
        .spawn()
        .map_err(|error| format!("start harbor_core failed: {error}"))?;
    wait_health(local_core_url().as_str(), Some(workspace.id.as_str()))?;
    claim_compatible_access(local_core_url().as_str())?;
    Ok(())
}

fn claim_compatible_access(base: &str) -> Result<(), String> {
    match claim_access_url(base) {
        Ok(_) => Ok(()),
        Err(error) => match fetch_access_url(base) {
            Ok(access) if access.owner_version.as_deref() == Some(APP_VERSION) => Ok(()),
            _ => Err(error),
        },
    }
}

fn require_replaceable_core(base: &str, health: &CoreHealth) -> Result<(), String> {
    match claim_access_url(base) {
        Ok(_) => Ok(()),
        Err(claim_error) => match fetch_access_url(base) {
            Ok(access) if access.occupied => Err(format!(
            "harbor_core {} is in use by Harbor {}; automatic replacement is disabled",
            health.version,
            access.owner_version.as_deref().unwrap_or("unknown")
        )),
            Ok(_) => Err(claim_error),
            Err(_) => Err(format!(
                "harbor_core {} does not support access leases; close other Harbor windows and connect again",
                health.version
            )),
        },
    }
}

#[cfg(target_os = "linux")]
fn ensure_local_core(settings: &Settings, workspace: &Workspace) -> Result<(), String> {
    let managed_core_matches = managed_local_core_matches();
    match fetch_health_url(&local_core_url()) {
        Ok(health)
            if health.ok
                && health.version == APP_VERSION
                && health.api_revision == CORE_API_REVISION
                && managed_core_matches
                && health.workspace_id == workspace.id =>
        {
            claim_compatible_access(local_core_url().as_str())?;
            switch_workspace(settings, workspace.id.as_str())
        }
        Ok(health)
            if health.ok
                && health.version == APP_VERSION
                && health.api_revision == CORE_API_REVISION
                && managed_core_matches =>
        {
            claim_compatible_access(local_core_url().as_str())?;
            switch_workspace(settings, workspace.id.as_str())
        }
        Ok(health) if health.ok => {
            require_replaceable_core(local_core_url().as_str(), &health)?;
            spawn_local_core(workspace)
        }
        _ => spawn_local_core(workspace),
    }
}

fn remote_base(workspace: &Workspace) -> Result<String, String> {
    let ssh = workspace
        .ssh
        .as_ref()
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
    Ok(format!("http://{}:{WEB_API_PORT}", ssh.host.trim()))
}

pub fn connect_remote_core(settings: &Settings) -> Result<(), String> {
    let workspace = settings.current()?.clone();
    if workspace.mode != WorkspaceMode::Remote {
        return Err("current workspace is not remote".into());
    }
    let ssh = workspace
        .ssh
        .as_ref()
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;

    // --- 阶段 1：验证 SSH，不触碰远端 Core ---
    harbor_support::app_log::gui(&format!(
        "remote connect 1/4: testing SSH to {}",
        ssh.host.trim()
    ));
    harbor_support::settings::verify_workspace_ssh(ssh.clone())?;

    // --- 阶段 2：优先复用已经运行且版本匹配的 Core ---
    let base = remote_base(&workspace)?;
    harbor_support::app_log::gui(&format!(
        "remote connect 2/4: checking harbor_core at {base}"
    ));
    match fetch_health_url(base.as_str()) {
        Ok(health)
            if health.ok
                && health.version == APP_VERSION
                && health.api_revision == CORE_API_REVISION =>
        {
            claim_compatible_access(base.as_str())?;
            switch_workspace(settings, workspace.id.as_str())?;
            harbor_support::app_log::gui("remote connect 4/4: reused running harbor_core");
            return Ok(());
        }
        Ok(health) if health.ok => {
            harbor_support::app_log::gui(&format!(
                "remote connect 3/4: running harbor_core {} is incompatible",
                health.version
            ));
            require_replaceable_core(base.as_str(), &health)?;
        }
        Ok(_) | Err(_) => {
            harbor_support::app_log::gui(
                "remote connect 3/4: harbor_core is not reachable; checking managed release",
            );
        }
    }

    // --- 阶段 3：仅在远端缺少匹配 release 时复制，再唤醒 Core ---
    let result = deploy_remote_core(settings, false);
    finish_deploy_progress();
    result
}

pub fn deploy_remote_core(settings: &Settings, force: bool) -> Result<(), String> {
    let workspace = settings.current()?.clone();
    let ssh = workspace
        .ssh
        .as_ref()
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
    ensure_remote_runtime_architecture(ssh)?;
    let runtime_bundle = packaged_runtime_bundle(
        CORE_RUNTIME_NAME,
        BUILD_CORE_RUNTIME_PATH,
        EXPECTED_CORE_RUNTIME_SHA256,
    )?;
    let loader = runtime_loader(CORE_RUNTIME_LOADER);
    let dir = remote_core_dir();
    let running_pid = remote_base(&workspace)
        .ok()
        .and_then(|base| fetch_health_url(base.as_str()).ok())
        .map(|health| health.pid)
        .filter(|pid| *pid > 0);
    ssh_run(
        ssh,
        &format!("mkdir -p \"$HOME/{dir}\" \"$HOME/.harbor/run\" \"$HOME/.harbor/log\""),
    )?;

    // --- 阶段 1：拒绝终止无法通过 API 确认归属的存活 Core ---
    if running_pid.is_none() && !force {
        let unmanaged_pid = ssh_run(
            ssh,
            r#"if [ -f "$HOME/.harbor/run/harbor_core.pid" ]; then
  pid=$(cat "$HOME/.harbor/run/harbor_core.pid")
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then printf '%s\n' "$pid"; fi
fi"#,
        )?;
        if !unmanaged_pid.trim().is_empty() {
            return Err(format!(
                "remote harbor_core pid {} is alive but its API is unreachable; refusing to replace it automatically",
                unmanaged_pid.trim()
            ));
        }
    }

    // --- 阶段 2：确认远端是否已有完全匹配的 release ---
    let remote_state = ssh_run(
        ssh,
        &format!(
            "if [ -x \"$HOME/{dir}/harbor_core\" ]; then sha256sum \"$HOME/{dir}/harbor_core\" | cut -d ' ' -f 1; cat \"$HOME/{dir}/.runtime-sha256\" 2>/dev/null || true; fi"
        ),
    )?;
    let mut remote_state = remote_state.lines();
    let binary_matches = remote_state.next() == Some(EXPECTED_CORE_SHA256);
    let runtime_matches = remote_state.next() == Some(EXPECTED_CORE_RUNTIME_SHA256);
    if binary_matches && runtime_matches {
        harbor_support::app_log::gui(&format!(
            "remote connect 3/4: matching harbor_core {APP_VERSION} already installed"
        ));
    } else {
        harbor_support::app_log::gui(&format!(
            "remote connect 3/4: copying harbor_core {APP_VERSION} to {}",
            ssh.host.trim()
        ));
        send_remote_runtime(
            ssh,
            runtime_bundle.as_path(),
            dir.as_str(),
            EXPECTED_CORE_RUNTIME_SHA256,
            "harbor_core",
            EXPECTED_CORE_SHA256,
        )?;
    }

    // --- 阶段 3：停止已确认可替换的旧 Core ---
    let stop = if let Some(pid) = running_pid {
        remote_terminate_script(pid.to_string().as_str())
    } else {
        format!(
            r#"if [ -f "$HOME/.harbor/run/harbor_core.pid" ]; then
  old=$(cat "$HOME/.harbor/run/harbor_core.pid")
  if [ -n "$old" ]; then
    if kill -0 "$old" 2>/dev/null && ! grep -aq 'harbor_core' "/proc/$old/cmdline" 2>/dev/null; then
      echo "refusing to terminate pid $old: process is not harbor_core" >&2
      exit 1
    fi
{}
  fi
fi"#,
            remote_terminate_script("\"$old\"")
        )
    };
    ssh_run(ssh, stop.as_str())?;
    // --- 阶段 4：校验并唤醒匹配版本 Core ---
    let dest = format!("\"$HOME/{dir}/harbor_core\"");
    let mut chmod = format!("chmod +x {dest}");
    if let Some(loader) = loader {
        chmod.push_str(&format!(r#" "$HOME/{dir}/{loader}""#));
    }
    let exec_version = remote_core_exec(loader, "--version");
    ssh_run(ssh, &format!("{chmod} && {exec_version}"))?;
    let localhost_only = if workspace.localhost_only() {
        "true"
    } else {
        "false"
    };
    let exec = remote_core_exec(
        loader,
        &format!(
            "--localhost-only {localhost_only} --remote-runtime --workspace {id}",
            id = shell_single_quote(&workspace.id),
        ),
    );
    let start = format!(
        r#"setsid {exec} </dev/null >>"$HOME/.harbor/log/harbor.log" 2>&1 &
echo $! > "$HOME/.harbor/run/harbor_core.pid"
sleep 0.3
if ! kill -0 "$(cat "$HOME/.harbor/run/harbor_core.pid")" 2>/dev/null; then
  echo "harbor_core exited immediately" >&2
  exit 1
fi
"#,
    );
    ssh_run(ssh, start.as_str())?;
    if let Err(error) = wait_health(
        remote_base(&workspace)?.as_str(),
        Some(workspace.id.as_str()),
    ) {
        let remote_log = ssh_run(
            ssh,
            "tail -n 20 \"$HOME/.harbor/log/harbor.log\" 2>/dev/null",
        )
        .unwrap_or_default();
        let remote_log = remote_log.trim();
        if remote_log.is_empty() {
            return Err(error);
        }
        return Err(format!("{error}; remote log:\n{remote_log}"));
    }
    claim_compatible_access(remote_base(&workspace)?.as_str())?;
    switch_workspace(settings, workspace.id.as_str())?;
    harbor_support::app_log::gui("remote connect 4/4: harbor_core started and connected");
    Ok(())
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn ensure_core(settings: &Settings) -> Result<(), String> {
    let current = settings.current()?.clone();
    if current.mode == WorkspaceMode::Remote {
        Err("remote workspace requires an explicit connection".into())
    } else {
        ensure_local_core_for_platform(settings, &current)
    }
}

#[cfg(target_os = "linux")]
fn ensure_local_core_for_platform(
    settings: &Settings,
    workspace: &Workspace,
) -> Result<(), String> {
    ensure_local_core(settings, workspace)
}

#[cfg(not(target_os = "linux"))]
fn ensure_local_core_for_platform(
    _settings: &Settings,
    _workspace: &Workspace,
) -> Result<(), String> {
    Err("local workspaces require Linux; use a remote workspace on this platform".into())
}

pub fn restart_core(settings: &Settings) -> Result<(), String> {
    let workspace = settings.current()?.clone();
    if workspace.mode == WorkspaceMode::Remote {
        let result = deploy_remote_core(settings, true);
        finish_deploy_progress();
        result
    } else {
        restart_local_core_for_platform(&workspace)
    }
}

#[cfg(target_os = "linux")]
fn restart_local_core_for_platform(workspace: &Workspace) -> Result<(), String> {
    spawn_local_core(workspace)
}

#[cfg(not(target_os = "linux"))]
fn restart_local_core_for_platform(_workspace: &Workspace) -> Result<(), String> {
    Err("local workspaces require Linux; use a remote workspace on this platform".into())
}

pub fn heartbeat_core(settings: &Settings) -> Result<(), String> {
    claim_compatible_access(crate::core_client::core_base_url(settings)?.as_str())
}

pub fn release_core(settings: &Settings) {
    let _ = release_access(settings);
}

pub fn shutdown_core(settings: &Settings) -> Result<(), String> {
    // --- 阶段 1：通过健康接口确认当前 Workspace 的 Core ---
    let workspace = settings.current()?.clone();
    let health = fetch_health(settings)?;
    let pid = i32::try_from(health.pid)
        .ok()
        .filter(|pid| *pid > 0)
        .ok_or_else(|| "harbor_core did not report a valid pid".to_string())?;

    // --- 阶段 2：停止 Core 管理的全部 Task ---
    let stop_errors = crate::core_client::stop_all(settings)?;
    if !stop_errors.is_empty() {
        return Err(format!(
            "部分 Task 停止失败，已取消退出：\n{}",
            stop_errors.join("\n")
        ));
    }
    release_core(settings);

    // --- 阶段 3：在 Core 所在机器终止已确认的进程 ---
    if workspace.mode == WorkspaceMode::Remote {
        let ssh = workspace
            .ssh
            .as_ref()
            .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
        let script = format!(
            r#"{}
if [ -f "$HOME/.harbor/run/harbor_core.pid" ] && [ "$(cat "$HOME/.harbor/run/harbor_core.pid")" = "{pid}" ]; then
  rm -f "$HOME/.harbor/run/harbor_core.pid"
fi"#,
            remote_terminate_script(pid.to_string().as_str())
        );
        ssh_run(ssh, script.as_str())?;
    } else {
        shutdown_local_core_for_platform(pid)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn shutdown_local_core_for_platform(pid: i32) -> Result<(), String> {
    terminate_pid(pid);
    if pid_is_running(pid) {
        return Err(format!("harbor_core pid {pid} did not exit"));
    }
    let pid_path = core_pid_path();
    if fs::read_to_string(&pid_path).is_ok_and(|raw| raw.trim() == pid.to_string()) {
        let _ = fs::remove_file(pid_path);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn shutdown_local_core_for_platform(_pid: i32) -> Result<(), String> {
    Err("local workspaces require Linux; use a remote workspace on this platform".into())
}

pub fn probe_core(settings: &Settings) -> Result<(), String> {
    let health = fetch_health(settings)?;
    if health.version != APP_VERSION {
        return Err(format!(
            "harbor_core version {} does not match GUI {APP_VERSION}",
            health.version
        ));
    }
    if health.api_revision != CORE_API_REVISION {
        return Err(format!(
            "harbor_core API revision {} does not match GUI {CORE_API_REVISION}",
            health.api_revision
        ));
    }
    if settings.current()?.mode == WorkspaceMode::Local && !managed_local_core_matches() {
        return Err(format!(
            "managed harbor_core hash does not match GUI release core {EXPECTED_CORE_SHA256}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_file_matches_known_digest() {
        let path = std::env::temp_dir().join(format!("harbor-core-sha256-{}", std::process::id()));
        fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn embedded_hash_matches_release_core() {
        assert_eq!(
            sha256_file(Path::new(BUILD_CORE_PATH)).unwrap(),
            EXPECTED_CORE_SHA256
        );
    }

    #[test]
    fn embedded_hash_matches_core_runtime_bundle() {
        assert_eq!(
            sha256_file(Path::new(BUILD_CORE_RUNTIME_PATH)).unwrap(),
            EXPECTED_CORE_RUNTIME_SHA256
        );
    }

    #[test]
    fn embedded_hash_matches_ttyd_runtime_bundle() {
        assert_eq!(
            sha256_file(Path::new(BUILD_TTYD_RUNTIME_PATH)).unwrap(),
            EXPECTED_TTYD_RUNTIME_SHA256
        );
    }

    #[test]
    fn remote_runtime_command_uses_bundled_loader() {
        assert_eq!(
            remote_runtime_exec(
                ".harbor/tools/ttyd/hash",
                "ttyd",
                Some("ld-linux-x86-64.so.2"),
                "-p 29386"
            ),
            r#""$HOME/.harbor/tools/ttyd/hash/ld-linux-x86-64.so.2" --library-path "$HOME/.harbor/tools/ttyd/hash" "$HOME/.harbor/tools/ttyd/hash/ttyd" -p 29386"#
        );
    }
}
