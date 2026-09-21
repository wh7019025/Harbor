use std::fs;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harbor_core::settings::{
    core_bin_dir, core_bin_path, core_pid_path, ssh_run, ssh_send_file, Settings, Workspace,
    WorkspaceMode, WorkspaceSsh,
};
use harbor_core::version::APP_VERSION;
use harbor_core::web_api::{CORE_API_REVISION, WEB_API_PORT};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::core_client::{
    claim_access_url, fetch_access_url, fetch_health, fetch_health_url, local_core_url,
    release_access, switch_workspace, CoreHealth,
};

const EXPECTED_CORE_SHA256: &str = env!("HARBOR_CORE_SHA256");
const BUILD_CORE_PATH: &str = env!("HARBOR_CORE_BUILD_PATH");
const EXPECTED_TTYD_SHA256: &str = env!("HARBOR_TTYD_SHA256");
const BUILD_TTYD_PATH: &str = env!("HARBOR_TTYD_BUILD_PATH");

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
    inflight: bool,
    retry_after: Option<Instant>,
    last_error: Option<String>,
    progress: DeployProgress,
}

fn remote_deploy_gate() -> &'static Mutex<RemoteDeployGate> {
    static GATE: OnceLock<Mutex<RemoteDeployGate>> = OnceLock::new();
    GATE.get_or_init(|| {
        Mutex::new(RemoteDeployGate {
            inflight: false,
            retry_after: None,
            last_error: None,
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

fn bump_deploy_percent(percent: u8) {
    let mut gate = remote_deploy_gate()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    gate.progress.active = true;
    gate.progress.percent = percent.min(100);
}

pub fn deploy_progress() -> DeployProgress {
    remote_deploy_gate()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .progress
        .clone()
}

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

fn packaged_ttyd_bin() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| format!("current exe failed: {error}"))?;
    let candidates = [
        exe.parent()
            .ok_or_else(|| "cannot resolve ttyd next to Harbor".to_string())?
            .join("ttyd"),
        PathBuf::from("/usr/lib/harbor/ttyd"),
        PathBuf::from(BUILD_TTYD_PATH),
    ];
    for candidate in candidates {
        if candidate.is_file() && sha256_file(&candidate)? == EXPECTED_TTYD_SHA256 {
            return Ok(candidate);
        }
    }
    Err(format!(
        "managed ttyd not found; expected sha256 {EXPECTED_TTYD_SHA256}; restart or reinstall Harbor"
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

fn shared_objects(bin: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("ldd")
        .arg(bin)
        .output()
        .map_err(|error| format!("ldd failed: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = format!("{stdout}\n{stderr}");
    if text.contains("not a dynamic executable") {
        return Ok(Vec::new());
    }
    if !output.status.success() {
        let stderr = stderr.trim().to_string();
        return Err(if stderr.is_empty() {
            format!("ldd {} failed", bin.display())
        } else {
            format!("ldd {} failed: {stderr}", bin.display())
        });
    }
    let mut files = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.contains("linux-vdso") || line.contains("linux-gate") {
            continue;
        }
        if line.contains("not found") {
            return Err(format!(
                "missing shared library for {}: {line}",
                bin.display()
            ));
        }
        let path = if let Some((_, right)) = line.split_once(" => ") {
            right.split_whitespace().next().unwrap_or("")
        } else {
            line.split_whitespace().next().unwrap_or("")
        };
        if path.starts_with('/') {
            files.push(PathBuf::from(path));
        }
    }
    Ok(files)
}

fn runtime_fingerprint(binary_hash: &str, runtime: &[PathBuf]) -> Result<String, String> {
    let mut entries = runtime
        .iter()
        .map(|path| {
            let name = path
                .file_name()
                .ok_or_else(|| format!("invalid runtime path {}", path.display()))?
                .to_string_lossy()
                .into_owned();
            Ok((name, sha256_file(path)?))
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort();
    let mut hasher = Sha256::new();
    hasher.update(binary_hash.as_bytes());
    for (name, hash) in entries {
        hasher.update(b"\n");
        hasher.update(name.as_bytes());
        hasher.update(b"=");
        hasher.update(hash.as_bytes());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn loader_basename(files: &[PathBuf]) -> Option<String> {
    files.iter().find_map(|path| {
        let name = path.file_name()?.to_string_lossy();
        if name.starts_with("ld-linux") || name.starts_with("ld-musl") {
            Some(name.into_owned())
        } else {
            None
        }
    })
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

fn create_remote_runtime_bundle(
    bin: &Path,
    binary_name: &str,
    runtime: &[PathBuf],
) -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_root =
        std::env::temp_dir().join(format!("harbor-runtime-{}-{nonce}", std::process::id()));
    let staging = temp_root.join("files");
    let archive = temp_root.join("runtime.tar.gz");

    // --- 阶段 1：收集 core 与兼容运行库 ---
    fs::create_dir_all(&staging)
        .map_err(|error| format!("create {} failed: {error}", staging.display()))?;
    let mut files = vec![(bin.to_path_buf(), binary_name.to_string())];
    for lib in runtime {
        let name = lib
            .file_name()
            .ok_or_else(|| format!("invalid runtime path {}", lib.display()))?
            .to_string_lossy()
            .into_owned();
        files.push((lib.clone(), name));
    }
    for (source, name) in files {
        fs::copy(&source, staging.join(name))
            .map_err(|error| format!("copy {} failed: {error}", source.display()))?;
    }

    // --- 阶段 2：压缩为单个远端部署包 ---
    let output = Command::new("tar")
        .args(["-czf"])
        .arg(&archive)
        .arg("-C")
        .arg(&staging)
        .arg(".")
        .output()
        .map_err(|error| format!("create runtime archive failed: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("create runtime archive failed: {stderr}"));
    }
    fs::remove_dir_all(&staging)
        .map_err(|error| format!("clean {} failed: {error}", staging.display()))?;
    Ok(archive)
}

fn send_remote_runtime(ssh: &WorkspaceSsh, bin: &Path, runtime: &[PathBuf]) -> Result<(), String> {
    let dir = remote_core_dir();
    let archive = create_remote_runtime_bundle(bin, "harbor_core", runtime)?;
    let total = fs::metadata(&archive)
        .map_err(|error| format!("stat {} failed: {error}", archive.display()))?
        .len();
    let remote_archive = format!("\"$HOME/{dir}/runtime.tar.gz.new\"");

    // --- 阶段 1：发送压缩部署包 ---
    set_deploy_progress(2, 0, total);
    let send_result = ssh_send_file(ssh, &archive, remote_archive.as_str(), |sent, _| {
        let percent = if total == 0 {
            2
        } else {
            (2 + sent.saturating_mul(88) / total).min(90) as u8
        };
        set_deploy_progress(percent, sent, total);
    });
    let _ = fs::remove_dir_all(archive.parent().unwrap_or_else(|| Path::new("/tmp")));
    send_result?;

    // --- 阶段 2：在远端展开运行环境 ---
    ssh_run(
        ssh,
        &format!("tar -xzf {remote_archive} -C \"$HOME/{dir}\" && rm -f {remote_archive}"),
    )?;
    set_deploy_progress(90, total, total);
    Ok(())
}

pub fn remote_ttyd_command(ssh: &WorkspaceSsh) -> Result<String, String> {
    let local_bin = packaged_ttyd_bin()?;
    let runtime = shared_objects(local_bin.as_path())?;
    let runtime_hash = runtime_fingerprint(EXPECTED_TTYD_SHA256, &runtime)?;
    let loader = loader_basename(&runtime);
    let dir = format!(".harbor/tools/ttyd/{EXPECTED_TTYD_SHA256}");
    let remote_state = ssh_run(
        ssh,
        &format!(
            "if [ -x \"$HOME/{dir}/ttyd\" ]; then sha256sum \"$HOME/{dir}/ttyd\" | cut -d ' ' -f 1; cat \"$HOME/{dir}/.runtime-sha256\" 2>/dev/null || true; fi"
        ),
    )?;
    let mut remote_state = remote_state.lines();
    let binary_matches = remote_state.next() == Some(EXPECTED_TTYD_SHA256);
    let runtime_matches = remote_state.next() == Some(runtime_hash.as_str());
    if !binary_matches || !runtime_matches {
        deploy_remote_ttyd(
            ssh,
            local_bin.as_path(),
            &runtime,
            dir.as_str(),
            runtime_hash.as_str(),
        )?;
    }
    Ok(remote_runtime_exec(
        dir.as_str(),
        "ttyd",
        loader.as_deref(),
        "",
    ))
}

fn deploy_remote_ttyd(
    ssh: &WorkspaceSsh,
    local_bin: &Path,
    runtime: &[PathBuf],
    dir: &str,
    runtime_hash: &str,
) -> Result<(), String> {
    // --- 阶段 1：打包并上传 ttyd 与兼容运行库 ---
    let archive = create_remote_runtime_bundle(local_bin, "ttyd", runtime)?;
    let remote_archive = format!("\"$HOME/{dir}/runtime.tar.gz.new\"");
    ssh_run(ssh, &format!("mkdir -p \"$HOME/{dir}\""))?;
    let send_result = ssh_send_file(ssh, &archive, remote_archive.as_str(), |_, _| {});
    let _ = fs::remove_dir_all(archive.parent().unwrap_or_else(|| Path::new("/tmp")));
    send_result?;

    // --- 阶段 2：展开、授权并校验托管二进制 ---
    ssh_run(
        ssh,
        &format!(
            "tar -xzf {remote_archive} -C \"$HOME/{dir}\" && rm -f {remote_archive} && chmod +x \"$HOME/{dir}/ttyd\" && printf '%s\\n' '{runtime_hash}' > \"$HOME/{dir}/.runtime-sha256\""
        ),
    )?;
    let remote_hash = ssh_run(
        ssh,
        &format!("sha256sum \"$HOME/{dir}/ttyd\" | cut -d ' ' -f 1"),
    )?;
    if remote_hash.trim() != EXPECTED_TTYD_SHA256 {
        return Err(format!(
            "remote managed ttyd hash mismatch: expected {EXPECTED_TTYD_SHA256}, got {}",
            remote_hash.trim()
        ));
    }
    Ok(())
}

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

fn terminate_pid(pid: i32) {
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    for _ in 0..20 {
        let alive = unsafe { libc::kill(pid, 0) == 0 };
        if !alive {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
}

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
                "harbor_core {} does not support access leases; automatic replacement is disabled, use force deploy after closing other Harbor windows",
                health.version
            )),
        },
    }
}

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
            claim_compatible_access(local_core_url().as_str())
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

fn remote_core_ready(settings: &Settings, workspace: &Workspace) -> Result<bool, String> {
    let Ok(base) = remote_base(workspace) else {
        return Ok(false);
    };
    match fetch_health_url(&base) {
        Ok(health)
            if health.ok
                && health.version == APP_VERSION
                && health.api_revision == CORE_API_REVISION
                && health.workspace_id == workspace.id =>
        {
            claim_compatible_access(base.as_str())?;
            Ok(true)
        }
        Ok(health)
            if health.ok
                && health.version == APP_VERSION
                && health.api_revision == CORE_API_REVISION =>
        {
            claim_compatible_access(base.as_str())?;
            switch_workspace(settings, workspace.id.as_str())?;
            Ok(true)
        }
        Ok(health) if health.ok => {
            require_replaceable_core(base.as_str(), &health)?;
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn kick_remote_deploy(workspace: Workspace) {
    let mut gate = remote_deploy_gate()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if gate.inflight {
        return;
    }
    if gate
        .retry_after
        .map(|deadline| Instant::now() < deadline)
        .unwrap_or(false)
    {
        return;
    }
    gate.inflight = true;
    gate.progress = DeployProgress {
        active: true,
        percent: 0,
        transferred: 0,
        total: 0,
    };
    drop(gate);
    if let Some(ssh) = workspace.ssh.as_ref() {
        harbor_core::app_log::gui(&format!(
            "deploying harbor_core {APP_VERSION} to {}:{}",
            ssh.host.trim(),
            WEB_API_PORT
        ));
    }
    thread::spawn(move || {
        let result = deploy_remote_core(&workspace);
        let mut gate = remote_deploy_gate()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        gate.inflight = false;
        match result {
            Ok(()) => {
                harbor_core::app_log::gui("remote harbor_core ready");
                gate.last_error = None;
                gate.retry_after = None;
                gate.progress.active = false;
                gate.progress.percent = 100;
            }
            Err(error) => {
                harbor_core::app_log::gui(&format!("deploy harbor_core failed: {error}"));
                gate.last_error = Some(error);
                gate.retry_after = Some(Instant::now() + Duration::from_secs(15));
                gate.progress.active = false;
            }
        }
    });
}

fn ensure_remote_core(settings: &Settings, workspace: &Workspace) -> Result<(), String> {
    if workspace.ssh.is_none() {
        return Err("remote workspace requires SSH settings".into());
    }
    if remote_core_ready(settings, workspace)? {
        return Ok(());
    }
    kick_remote_deploy(workspace.clone());
    let gate = remote_deploy_gate()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if gate.inflight {
        Err("copying harbor_core to remote…".into())
    } else {
        Err(gate
            .last_error
            .clone()
            .unwrap_or_else(|| "remote harbor_core unreachable".into()))
    }
}

pub fn deploy_remote_core(workspace: &Workspace) -> Result<(), String> {
    let ssh = workspace
        .ssh
        .as_ref()
        .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
    let local_bin = install_local_core_bin()?;
    let runtime = shared_objects(local_bin.as_path())?;
    let loader = loader_basename(&runtime);
    let dir = remote_core_dir();
    let running_pid = remote_base(workspace)
        .ok()
        .and_then(|base| fetch_health_url(base.as_str()).ok())
        .map(|health| health.pid)
        .filter(|pid| *pid > 0);
    ssh_run(
        ssh,
        &format!("mkdir -p \"$HOME/{dir}\" \"$HOME/.harbor/run\" \"$HOME/.harbor/log\""),
    )?;
    harbor_core::app_log::gui(&format!(
        "copying harbor_core {APP_VERSION} and runtime libs to {}",
        ssh.host.trim()
    ));
    send_remote_runtime(ssh, local_bin.as_path(), &runtime)?;
    let remote_hash = ssh_run(
        ssh,
        &format!("sha256sum \"$HOME/{dir}/harbor_core\" | cut -d ' ' -f 1"),
    )?;
    let remote_hash = remote_hash.trim();
    if remote_hash != EXPECTED_CORE_SHA256 {
        return Err(format!(
            "remote managed harbor_core hash mismatch: expected {EXPECTED_CORE_SHA256}, got {remote_hash}"
        ));
    }
    let stop = if let Some(pid) = running_pid {
        format!(
            r#"if kill -0 {pid} 2>/dev/null; then
  kill {pid} 2>/dev/null || true
  sleep 0.4
fi"#
        )
    } else {
        r#"if [ -f "$HOME/.harbor/run/harbor_core.pid" ]; then
  old=$(cat "$HOME/.harbor/run/harbor_core.pid")
  if [ -n "$old" ] && kill -0 "$old" 2>/dev/null; then
    kill "$old" 2>/dev/null || true
    sleep 0.4
  fi
fi"#
        .to_string()
    };
    ssh_run(ssh, stop.as_str())?;
    let dest = format!("\"$HOME/{dir}/harbor_core\"");
    let mut chmod = format!("chmod +x {dest}");
    if let Some(loader) = loader.as_deref() {
        chmod.push_str(&format!(r#" "$HOME/{dir}/{loader}""#));
    }
    let exec_version = remote_core_exec(loader.as_deref(), "--version");
    ssh_run(ssh, &format!("{chmod} && {exec_version}"))?;
    bump_deploy_percent(92);
    let localhost_only = if workspace.localhost_only() {
        "true"
    } else {
        "false"
    };
    let exec = remote_core_exec(
        loader.as_deref(),
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
    bump_deploy_percent(96);
    if let Err(error) = wait_health(
        remote_base(workspace)?.as_str(),
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
    claim_compatible_access(remote_base(workspace)?.as_str())?;
    bump_deploy_percent(100);
    Ok(())
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn ensure_core(settings: &Settings) -> Result<(), String> {
    let current = settings.current()?.clone();
    if current.mode == WorkspaceMode::Remote {
        ensure_remote_core(settings, &current)
    } else {
        ensure_local_core(settings, &current)
    }
}

pub fn restart_core(settings: &Settings) -> Result<(), String> {
    let workspace = settings.current()?.clone();
    if workspace.mode == WorkspaceMode::Remote {
        deploy_remote_core(&workspace)
    } else {
        spawn_local_core(&workspace)
    }
}

pub fn heartbeat_core(settings: &Settings) -> Result<(), String> {
    claim_compatible_access(crate::core_client::core_base_url(settings)?.as_str())
}

pub fn release_core(settings: &Settings) {
    let _ = release_access(settings);
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
    fn embedded_hash_matches_managed_ttyd() {
        assert_eq!(
            sha256_file(Path::new(BUILD_TTYD_PATH)).unwrap(),
            EXPECTED_TTYD_SHA256
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
