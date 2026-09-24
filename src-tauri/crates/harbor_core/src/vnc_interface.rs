use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::service::CoreServiceStatus;
use crate::taskcard::TaskCommand;

pub const VNC_PORT: u16 = 23682;
pub const PHYSICAL_VNC_PORT: u16 = 23683;
const DISPLAY_NUMBER: u16 = 82;
const PHYSICAL_DISPLAY: &str = ":0";

fn runtime_dir() -> PathBuf {
    let runtime_base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    runtime_base.join(format!("harbor-vnc-{}", unsafe { libc::geteuid() }))
}

fn physical_runtime_dir() -> PathBuf {
    let runtime_base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    runtime_base.join(format!("harbor-vnc-physical-{}", unsafe {
        libc::geteuid()
    }))
}

pub fn is_ready() -> bool {
    runtime_dir().join("ready").is_file()
}

pub fn is_physical_ready() -> bool {
    physical_runtime_dir().join("ready").is_file()
}

pub fn physical_error() -> Option<String> {
    if is_physical_ready() {
        return None;
    }
    if !command_available("X0tigervnc") {
        return Some(
            "真实桌面不可用：缺少 X0tigervnc；请安装：sudo apt install tigervnc-scraping-server"
                .into(),
        );
    }
    if !Path::new("/tmp/.X11-unix/X0").exists() {
        return Some("真实桌面不可用：未检测到活动的 X11 DISPLAY=:0".into());
    }
    let log_path = physical_runtime_dir().join("infrastructure.log");
    std::fs::read_to_string(log_path)
        .ok()
        .and_then(|content| {
            content
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(str::to_string)
        })
        .map(|line| format!("真实桌面启动失败：{line}"))
}

fn shared_error() -> Option<String> {
    if is_ready() {
        return None;
    }
    if let Err(error) = validate_dependencies() {
        return Some(error);
    }
    let log_path = runtime_dir().join("infrastructure.log");
    fs::read_to_string(log_path)
        .ok()
        .and_then(|content| {
            content
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(str::to_string)
        })
        .map(|line| format!("虚拟桌面启动失败：{line}"))
}

pub fn service_statuses() -> Vec<CoreServiceStatus> {
    vec![
        display_service_status(
            "virtual-vnc",
            "虚拟桌面 VNC",
            VNC_PORT,
            runtime_dir().as_path(),
            "harbor-vnc-server",
            shared_error(),
        ),
        display_service_status(
            "physical-vnc",
            "真实桌面 VNC",
            PHYSICAL_VNC_PORT,
            physical_runtime_dir().as_path(),
            "harbor-vnc-physical-server",
            physical_error(),
        ),
    ]
}

fn display_service_status(
    id: &str,
    name: &str,
    port: u16,
    runtime_dir: &Path,
    process_marker: &str,
    error: Option<String>,
) -> CoreServiceStatus {
    let pid = supervisor_pid(runtime_dir, process_marker);
    let ready = pid.is_some() && runtime_dir.join("ready").is_file();
    CoreServiceStatus {
        id: id.into(),
        name: name.into(),
        kind: "vnc".into(),
        state: if ready {
            "running"
        } else if error.is_some() {
            "unavailable"
        } else {
            "stopped"
        }
        .into(),
        pid,
        port,
        bind: format!("127.0.0.1:{port}"),
        detail: if ready { None } else { error },
        managed_by_core: true,
        stoppable: false,
    }
}

fn supervisor_pid(runtime_dir: &Path, process_marker: &str) -> Option<u32> {
    let pid = fs::read_to_string(runtime_dir.join("supervisor.pid"))
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|pid| *pid > 0)?;
    fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .filter(|command| String::from_utf8_lossy(command).contains(process_marker))
        .map(|_| pid)
}

pub fn ensure_shared_display() -> Result<(), String> {
    validate_dependencies()?;
    let runtime_dir = runtime_dir();
    let (ensure_script, server_script) = write_runtime_scripts(
        runtime_dir.as_path(),
        "harbor-vnc-ensure.sh",
        ENSURE_DISPLAY_SCRIPT,
        "harbor-vnc-server.sh",
        SHARED_DISPLAY_SCRIPT,
    )?;
    let output = Command::new("bash")
        .arg(ensure_script)
        .arg(VNC_PORT.to_string())
        .arg(DISPLAY_NUMBER.to_string())
        .arg(server_script)
        .arg("true")
        .output()
        .map_err(|error| format!("start shared Harbor VNC desktop failed: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
        format!(
            "start shared Harbor VNC desktop failed with status {}",
            output.status
        )
    } else {
        stderr
    })
}

pub fn ensure_physical_display() -> Result<(), String> {
    validate_physical_dependencies()?;
    let runtime_dir = physical_runtime_dir();
    let (ensure_script, server_script) = write_runtime_scripts(
        runtime_dir.as_path(),
        "harbor-vnc-physical-ensure.sh",
        ENSURE_PHYSICAL_DISPLAY_SCRIPT,
        "harbor-vnc-physical-server.sh",
        PHYSICAL_DISPLAY_SCRIPT,
    )?;
    let output = Command::new("bash")
        .arg(ensure_script)
        .arg(PHYSICAL_VNC_PORT.to_string())
        .arg(PHYSICAL_DISPLAY)
        .arg(server_script)
        .output()
        .map_err(|error| format!("start physical Harbor VNC display failed: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
        format!(
            "start physical Harbor VNC display failed with status {}",
            output.status
        )
    } else {
        stderr
    })
}

pub fn shutdown_displays() {
    shutdown_display(runtime_dir().as_path(), "harbor-vnc-server");
    shutdown_display(
        physical_runtime_dir().as_path(),
        "harbor-vnc-physical-server",
    );
}

fn shutdown_display(runtime_dir: &Path, process_marker: &str) {
    let supervisor_file = runtime_dir.join("supervisor.pid");
    let supervisor_pid = fs::read_to_string(&supervisor_file)
        .ok()
        .and_then(|raw| raw.trim().parse::<i32>().ok())
        .filter(|pid| *pid > 0);
    if let Some(pid) = supervisor_pid {
        let owned = fs::read(format!("/proc/{pid}/cmdline"))
            .ok()
            .is_some_and(|cmdline| String::from_utf8_lossy(&cmdline).contains(process_marker));
        if owned {
            unsafe {
                libc::kill(-pid, libc::SIGTERM);
                libc::kill(pid, libc::SIGTERM);
            }
            for _ in 0..20 {
                if unsafe { libc::kill(pid, 0) } != 0 {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            if unsafe { libc::kill(pid, 0) } == 0 {
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                    libc::kill(pid, libc::SIGKILL);
                }
            }
        }
    }
    for name in ["supervisor.pid", "ready", "vnc.sock", "startup.lock"] {
        let _ = fs::remove_file(runtime_dir.join(name));
    }
}

fn write_runtime_scripts(
    runtime_dir: &Path,
    ensure_name: &str,
    ensure_content: &str,
    server_name: &str,
    server_content: &str,
) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(runtime_dir)
        .map_err(|error| format!("create {} failed: {error}", runtime_dir.display()))?;
    fs::set_permissions(runtime_dir, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("protect {} failed: {error}", runtime_dir.display()))?;
    let ensure_path = runtime_dir.join(ensure_name);
    let server_path = runtime_dir.join(server_name);
    for (path, content) in [
        (ensure_path.as_path(), ensure_content),
        (server_path.as_path(), server_content),
    ] {
        fs::write(path, content)
            .map_err(|error| format!("write {} failed: {error}", path.display()))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("protect {} failed: {error}", path.display()))?;
    }
    Ok((ensure_path, server_path))
}

const SHARED_DISPLAY_SCRIPT: &str = r##"
set -Eeuo pipefail

# --- 阶段 1：读取机器级 VNC 配置 ---
runtime_dir="$1"
display_number="$2"
panel_port="$3"
web_root="${runtime_dir}/www"
vnc_socket="${runtime_dir}/vnc.sock"
infrastructure_log="${runtime_dir}/infrastructure.log"
supervisor_file="${runtime_dir}/supervisor.pid"
ready_file="${runtime_dir}/ready"
mkdir -p "${web_root}"
rm -f "${ready_file}"
printf '%s\n' "$$" >"${supervisor_file}"

child_pids=()
cleanup() {
  trap - EXIT INT TERM
  if ((${#child_pids[@]})); then
    for child_pid in "${child_pids[@]}"; do
      kill -TERM -- "-${child_pid}" 2>/dev/null || kill -TERM "${child_pid}" 2>/dev/null || true
    done
    wait "${child_pids[@]}" 2>/dev/null || true
  fi
  rm -f "${vnc_socket}" "${supervisor_file}" "${ready_file}"
}
trap cleanup EXIT INT TERM

# --- 阶段 2：准备适配 HiDPI 的 noVNC 页面 ---
rm -rf "${web_root}"
mkdir -p "${web_root}"
cp -a /usr/share/novnc/. "${web_root}/"
sed -i 's/this\._display\.scale = 1\.0;/this._display.scale = this._resizeSession ? 1 \/ Math.min(window.devicePixelRatio || 1, 2) : 1.0;/' "${web_root}/core/rfb.js"
sed -i 's/return { w: r\.width, h: r\.height };/const pixelRatio = this._resizeSession ? Math.min(window.devicePixelRatio || 1, 2) : 1;\n        return { w: r.width * pixelRatio, h: r.height * pixelRatio };/' "${web_root}/core/rfb.js"
printf '{"version":"system package"}\n' >"${web_root}/package.json"
cat >"${web_root}/index.html" <<'EOF'
<!doctype html>
<meta charset="utf-8">
<title>Harbor Display</title>
<style>
  html, body, #screen { width: 100%; height: 100%; margin: 0; overflow: hidden; background: #1e1e1e; }
  #status { position: fixed; z-index: 1; top: 10px; left: 12px; color: #bbb; font: 13px sans-serif; }
</style>
<div id="status">Connecting…</div>
<div id="screen"></div>
<script type="module">
  import RFB from './core/rfb.js?harbor-hidpi=2';

  const status = document.getElementById('status');
  const socketScheme = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
  const socketUrl = socketScheme + window.location.host + '/websockify';
  const rfb = new RFB(document.getElementById('screen'), socketUrl, { shared: true });
  rfb.scaleViewport = true;
  rfb.resizeSession = true;
  rfb.qualityLevel = 9;
  rfb.compressionLevel = 2;
  rfb.addEventListener('connect', () => { status.hidden = true; });
  rfb.addEventListener('disconnect', (event) => {
    status.hidden = false;
    status.textContent = event.detail.clean ? 'Disconnected' : 'Connection closed';
  });
</script>
EOF

# --- 阶段 3：启动机器级共享虚拟桌面 ---
export DISPLAY=":${display_number}"
export GDK_BACKEND=x11
export QT_QPA_PLATFORM=xcb
export SDL_VIDEODRIVER=x11
export XDG_SESSION_TYPE=x11
unset WAYLAND_DISPLAY

setsid Xtigervnc "${DISPLAY}" \
  -geometry 1920x1080 \
  -depth 24 \
  -rfbport 0 \
  -rfbunixpath "${vnc_socket}" \
  -rfbunixmode 384 \
  -SecurityTypes None \
  -AcceptSetDesktopSize=1 \
  -desktop "Harbor" \
  -nolisten tcp >>"${infrastructure_log}" 2>&1 &
child_pids+=("$!")

for _ in {1..50}; do
  [[ -S "${vnc_socket}" ]] && break
  sleep 0.1
done
if [[ ! -S "${vnc_socket}" ]]; then
  echo "shared vnc_interface failed to start TigerVNC on ${DISPLAY}" >&2
  exit 1
fi

desktop_session="${HARBOR_VNC_DESKTOP_SESSION:-auto}"
if [[ "${desktop_session}" == auto ]]; then
  if command -v gnome-session >/dev/null 2>&1 \
    && command -v dbus-run-session >/dev/null 2>&1 \
    && command -v gnome-shell >/dev/null 2>&1 \
    && [[ -f /usr/share/gnome-session/sessions/ubuntu.session ]]; then
    desktop_session=ubuntu
  elif command -v gnome-session >/dev/null 2>&1 \
    && command -v dbus-run-session >/dev/null 2>&1 \
    && command -v gnome-shell >/dev/null 2>&1 \
    && [[ -f /usr/share/gnome-session/sessions/gnome.session ]]; then
    desktop_session=gnome
  elif command -v gnome-session >/dev/null 2>&1 \
    && command -v dbus-run-session >/dev/null 2>&1 \
    && [[ -f /usr/share/gnome-session/sessions/gnome-flashback-metacity.session ]]; then
    desktop_session=gnome-flashback-metacity
  else
    desktop_session=openbox
  fi
fi
if [[ "${desktop_session}" != openbox ]]; then
  export XDG_RUNTIME_DIR="${runtime_dir}/xdg-runtime"
  if [[ "${desktop_session}" == ubuntu ]]; then
    export XDG_CURRENT_DESKTOP=ubuntu:GNOME
    export XDG_SESSION_DESKTOP=ubuntu
    export GNOME_SHELL_SESSION_MODE=ubuntu
  else
    export XDG_CURRENT_DESKTOP=GNOME
    export XDG_SESSION_DESKTOP=gnome
  fi
  export LIBGL_ALWAYS_SOFTWARE=1
  mkdir -p "${XDG_RUNTIME_DIR}"
  chmod 700 "${XDG_RUNTIME_DIR}"
  if [[ "${desktop_session}" == ubuntu ]]; then
    setsid dbus-run-session -- bash -c '
      export GNOME_SHELL_SESSION_MODE=ubuntu
      gsettings set org.gnome.shell disable-user-extensions false
      gsettings set org.gnome.shell enabled-extensions "['\''ubuntu-dock@ubuntu.com'\'']"
      exec gnome-session --session=ubuntu
    ' harbor-ubuntu-desktop >>"${infrastructure_log}" 2>&1 &
  elif [[ "${desktop_session}" == gnome ]]; then
    setsid dbus-run-session -- gnome-shell --x11 >>"${infrastructure_log}" 2>&1 &
  elif [[ "${desktop_session}" == gnome-flashback-metacity ]]; then
    setsid dbus-run-session -- bash -c '
      desktop_pids=()
      cleanup_desktop() {
        trap - EXIT INT TERM
        kill "${desktop_pids[@]}" 2>/dev/null || true
        wait "${desktop_pids[@]}" 2>/dev/null || true
      }
      trap cleanup_desktop EXIT INT TERM
      gnome-flashback & desktop_pids+=("$!")
      metacity --replace & desktop_pids+=("$!")
      gnome-panel & desktop_pids+=("$!")
      nautilus --no-default-window & desktop_pids+=("$!")
      wait -n "${desktop_pids[@]}"
    ' harbor-gnome-desktop >>"${infrastructure_log}" 2>&1 &
  else
    setsid dbus-run-session -- gnome-session --session="${desktop_session}" >>"${infrastructure_log}" 2>&1 &
  fi
else
  setsid openbox --sm-disable >>"${infrastructure_log}" 2>&1 &
fi
child_pids+=("$!")

# GNOME Session 会在初始化时接管窗口管理器；等待其稳定后再放行 Task。
if [[ "${desktop_session}" != openbox ]]; then
  sleep 3
fi

setsid websockify \
  --web "${web_root}" \
  "127.0.0.1:${panel_port}" \
  --unix-target "${vnc_socket}" >>"${infrastructure_log}" 2>&1 &
child_pids+=("$!")

for _ in {1..50}; do
  if kill -0 "${child_pids[2]}" 2>/dev/null \
    && (exec 8<>"/dev/tcp/127.0.0.1/${panel_port}") 2>/dev/null; then
    touch "${ready_file}"
    break
  fi
  sleep 0.1
done
if [[ ! -f "${ready_file}" ]]; then
  echo "shared vnc_interface failed to publish noVNC on port ${panel_port}" >&2
  exit 1
fi

# --- 阶段 4：持续托管共享桌面基础设施 ---
wait -n "${child_pids[@]}"
"##;

const ENSURE_DISPLAY_SCRIPT: &str = r##"
set -Eeuo pipefail

# --- 阶段 1：定位当前用户唯一的 Harbor VNC 运行时 ---
panel_port="$1"
display_number="$2"
server_script="$3"
shift 3
runtime_base="${XDG_RUNTIME_DIR:-/tmp}"
runtime_dir="${runtime_base}/harbor-vnc-${UID}"
lock_file="${runtime_dir}/startup.lock"
supervisor_file="${runtime_dir}/supervisor.pid"
vnc_socket="${runtime_dir}/vnc.sock"
ready_file="${runtime_dir}/ready"
infrastructure_log="${runtime_dir}/infrastructure.log"
mkdir -p "${runtime_dir}"
chmod 700 "${runtime_dir}"
exec 9>"${lock_file}"
flock 9

# --- 阶段 2：复用健康的共享桌面，清理不完整的旧实例 ---
supervisor_pid=""
if [[ -f "${supervisor_file}" ]]; then
  supervisor_pid="$(cat "${supervisor_file}" 2>/dev/null || true)"
fi
if [[ "${supervisor_pid}" =~ ^[0-9]+$ ]] \
  && kill -0 "${supervisor_pid}" 2>/dev/null \
  && grep -aq 'harbor-vnc-server.sh' "/proc/${supervisor_pid}/cmdline" 2>/dev/null \
  && [[ -S "${vnc_socket}" && -f "${ready_file}" ]] \
  && (exec 8<>"/dev/tcp/127.0.0.1/${panel_port}") 2>/dev/null; then
  shared_display_ready=true
else
  shared_display_ready=false
  if [[ "${supervisor_pid}" =~ ^[0-9]+$ ]] \
    && kill -0 "${supervisor_pid}" 2>/dev/null \
    && grep -aq 'harbor-vnc-server' "/proc/${supervisor_pid}/cmdline" 2>/dev/null; then
    kill -TERM -- "-${supervisor_pid}" 2>/dev/null || kill -TERM "${supervisor_pid}" 2>/dev/null || true
  fi
  rm -f "${supervisor_file}" "${vnc_socket}" "${ready_file}"
fi

# --- 阶段 3：按需启动机器级共享桌面 ---
if [[ "${shared_display_ready}" != true ]]; then
  : >"${infrastructure_log}"
  setsid bash "${server_script}" \
    "${runtime_dir}" "${display_number}" "${panel_port}" \
    9>&- </dev/null >>"${infrastructure_log}" 2>&1 &

  for _ in {1..100}; do
    if [[ -f "${supervisor_file}" && -S "${vnc_socket}" && -f "${ready_file}" ]]; then
      supervisor_pid="$(cat "${supervisor_file}" 2>/dev/null || true)"
      if [[ "${supervisor_pid}" =~ ^[0-9]+$ ]] \
        && kill -0 "${supervisor_pid}" 2>/dev/null \
        && (exec 8<>"/dev/tcp/127.0.0.1/${panel_port}") 2>/dev/null; then
        shared_display_ready=true
        break
      fi
    fi
    sleep 0.1
  done
fi

if [[ "${shared_display_ready}" != true ]]; then
  echo "vnc_interface failed to start the shared Harbor VNC desktop" >&2
  cat "${infrastructure_log}" >&2 || true
  exit 1
fi

# --- 阶段 4：在共享 DISPLAY 中运行当前 Task ---
flock -u 9
export DISPLAY=":${display_number}"
export GDK_BACKEND=x11
export QT_QPA_PLATFORM=xcb
export SDL_VIDEODRIVER=x11
unset WAYLAND_DISPLAY XAUTHORITY
exec "$@"
"##;

const PHYSICAL_DISPLAY_SCRIPT: &str = r##"
set -Eeuo pipefail

# --- 阶段 1：准备真实桌面抓取环境 ---
runtime_dir="$1"
panel_port="$2"
display_name="$3"
web_root="${runtime_dir}/www"
vnc_socket="${runtime_dir}/vnc.sock"
infrastructure_log="${runtime_dir}/infrastructure.log"
supervisor_file="${runtime_dir}/supervisor.pid"
ready_file="${runtime_dir}/ready"
mkdir -p "${web_root}"
rm -f "${ready_file}" "${vnc_socket}"
printf '%s\n' "$$" >"${supervisor_file}"

child_pids=()
cleanup() {
  trap - EXIT INT TERM
  if ((${#child_pids[@]})); then
    for child_pid in "${child_pids[@]}"; do
      kill -TERM -- "-${child_pid}" 2>/dev/null || kill -TERM "${child_pid}" 2>/dev/null || true
    done
    wait "${child_pids[@]}" 2>/dev/null || true
  fi
  rm -f "${vnc_socket}" "${supervisor_file}" "${ready_file}"
}
trap cleanup EXIT INT TERM

if [[ ! -S "/tmp/.X11-unix/X${display_name#:}" ]]; then
  echo "active X11 display ${display_name} not found" >&2
  exit 1
fi

xauthority="${HARBOR_PHYSICAL_XAUTHORITY:-${XAUTHORITY:-}}"
if [[ -z "${xauthority}" || ! -r "${xauthority}" ]]; then
  for candidate in \
    "${XDG_RUNTIME_DIR:-/run/user/${UID}}/gdm/Xauthority" \
    "${HOME}/.Xauthority"; do
    if [[ -r "${candidate}" ]]; then
      xauthority="${candidate}"
      break
    fi
  done
fi
if [[ -z "${xauthority}" || ! -r "${xauthority}" ]]; then
  xauthority="$(ps -u "${USER}" -o args= | sed -n 's/.*[[:space:]]-auth[[:space:]]\([^[:space:]]*\).*/\1/p' | head -n 1)"
fi
if [[ -z "${xauthority}" || ! -r "${xauthority}" ]]; then
  echo "Xauthority for ${display_name} not found; set HARBOR_PHYSICAL_XAUTHORITY" >&2
  exit 1
fi

# --- 阶段 2：准备 noVNC 页面 ---
rm -rf "${web_root}"
mkdir -p "${web_root}"
cp -a /usr/share/novnc/. "${web_root}/"
cat >"${web_root}/index.html" <<'EOF'
<!doctype html>
<meta charset="utf-8">
<title>Harbor Physical Display</title>
<style>
  html, body, #screen { width: 100%; height: 100%; margin: 0; overflow: hidden; background: #1e1e1e; }
  #status { position: fixed; z-index: 1; top: 10px; left: 12px; color: #bbb; font: 13px sans-serif; }
</style>
<div id="status">Connecting…</div>
<div id="screen"></div>
<script type="module">
  import RFB from './core/rfb.js';

  const status = document.getElementById('status');
  const socketScheme = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
  const socketUrl = socketScheme + window.location.host + '/websockify';
  const rfb = new RFB(document.getElementById('screen'), socketUrl, { shared: true });
  rfb.scaleViewport = true;
  rfb.resizeSession = false;
  rfb.qualityLevel = 9;
  rfb.compressionLevel = 2;
  rfb.addEventListener('connect', () => { status.hidden = true; });
  rfb.addEventListener('disconnect', (event) => {
    status.hidden = false;
    status.textContent = event.detail.clean ? 'Disconnected' : 'Connection closed';
  });
</script>
EOF

# --- 阶段 3：抓取 DISPLAY=:0 并发布 noVNC ---
export DISPLAY="${display_name}"
export XAUTHORITY="${xauthority}"
setsid X0tigervnc \
  -display "${display_name}" \
  -rfbport 0 \
  -rfbunixpath "${vnc_socket}" \
  -rfbunixmode 384 \
  -SecurityTypes None \
  -AlwaysShared=1 \
  -AcceptPointerEvents=1 \
  -AcceptKeyEvents=1 >>"${infrastructure_log}" 2>&1 &
child_pids+=("$!")

for _ in {1..50}; do
  [[ -S "${vnc_socket}" ]] && break
  sleep 0.1
done
if [[ ! -S "${vnc_socket}" ]]; then
  echo "x0vncserver failed to capture ${display_name}" >&2
  exit 1
fi

setsid websockify \
  --web "${web_root}" \
  "127.0.0.1:${panel_port}" \
  --unix-target "${vnc_socket}" >>"${infrastructure_log}" 2>&1 &
child_pids+=("$!")

for _ in {1..50}; do
  if kill -0 "${child_pids[1]}" 2>/dev/null \
    && (exec 8<>"/dev/tcp/127.0.0.1/${panel_port}") 2>/dev/null; then
    touch "${ready_file}"
    break
  fi
  sleep 0.1
done
if [[ ! -f "${ready_file}" ]]; then
  echo "physical noVNC failed to listen on port ${panel_port}" >&2
  exit 1
fi

# --- 阶段 4：持续托管真实桌面抓取通路 ---
wait -n "${child_pids[@]}"
"##;

const ENSURE_PHYSICAL_DISPLAY_SCRIPT: &str = r##"
set -Eeuo pipefail

# --- 阶段 1：定位机器级真实桌面运行时 ---
panel_port="$1"
display_name="$2"
server_script="$3"
runtime_base="${XDG_RUNTIME_DIR:-/tmp}"
runtime_dir="${runtime_base}/harbor-vnc-physical-${UID}"
lock_file="${runtime_dir}/startup.lock"
supervisor_file="${runtime_dir}/supervisor.pid"
vnc_socket="${runtime_dir}/vnc.sock"
ready_file="${runtime_dir}/ready"
infrastructure_log="${runtime_dir}/infrastructure.log"
mkdir -p "${runtime_dir}"
chmod 700 "${runtime_dir}"
exec 9>"${lock_file}"
flock 9

# --- 阶段 2：复用健康实例或清理残留状态 ---
supervisor_pid=""
if [[ -f "${supervisor_file}" ]]; then
  supervisor_pid="$(cat "${supervisor_file}" 2>/dev/null || true)"
fi
if [[ "${supervisor_pid}" =~ ^[0-9]+$ ]] \
  && kill -0 "${supervisor_pid}" 2>/dev/null \
  && grep -aq 'harbor-vnc-physical-server.sh' "/proc/${supervisor_pid}/cmdline" 2>/dev/null \
  && [[ -S "${vnc_socket}" && -f "${ready_file}" ]] \
  && (exec 8<>"/dev/tcp/127.0.0.1/${panel_port}") 2>/dev/null; then
  physical_display_ready=true
else
  physical_display_ready=false
  if [[ "${supervisor_pid}" =~ ^[0-9]+$ ]] \
    && kill -0 "${supervisor_pid}" 2>/dev/null \
    && grep -aq 'harbor-vnc-physical-server' "/proc/${supervisor_pid}/cmdline" 2>/dev/null; then
    kill -TERM -- "-${supervisor_pid}" 2>/dev/null || kill -TERM "${supervisor_pid}" 2>/dev/null || true
  fi

  # 旧版本可能只终止了 x0vncserver 包装进程，留下独立的 X0tigervnc 或 websockify。
  # 只清理命令行中包含本运行目录的 Harbor 基础设施，避免影响用户自己的 VNC 服务。
  stale_pids=()
  for process_cmdline in /proc/[0-9]*/cmdline; do
    process_pid="${process_cmdline#/proc/}"
    process_pid="${process_pid%/cmdline}"
    [[ "${process_pid}" == "$$" || "${process_pid}" == "${PPID}" ]] && continue
    process_args="$(tr '\0' ' ' <"${process_cmdline}" 2>/dev/null || true)"
    if [[ "${process_args}" == *"${runtime_dir}"* ]] \
      && [[ "${process_args}" == *X0tigervnc* \
        || "${process_args}" == *x0vncserver* \
        || "${process_args}" == *websockify* \
        || "${process_args}" == *harbor-vnc-physical-server* ]]; then
      stale_pids+=("${process_pid}")
    fi
  done
  if ((${#stale_pids[@]})); then
    for stale_pid in "${stale_pids[@]}"; do
      kill -TERM -- "-${stale_pid}" 2>/dev/null || kill -TERM "${stale_pid}" 2>/dev/null || true
    done
    sleep 0.2
    for stale_pid in "${stale_pids[@]}"; do
      kill -0 "${stale_pid}" 2>/dev/null || continue
      kill -KILL -- "-${stale_pid}" 2>/dev/null || kill -KILL "${stale_pid}" 2>/dev/null || true
    done
  fi
  rm -f "${supervisor_file}" "${vnc_socket}" "${ready_file}"
fi

# --- 阶段 3：启动并等待真实桌面通路 ---
if [[ "${physical_display_ready}" != true ]]; then
  : >"${infrastructure_log}"
  setsid bash "${server_script}" \
    "${runtime_dir}" "${panel_port}" "${display_name}" \
    9>&- </dev/null >>"${infrastructure_log}" 2>&1 &

  for _ in {1..100}; do
    if [[ -f "${supervisor_file}" && -S "${vnc_socket}" && -f "${ready_file}" ]]; then
      supervisor_pid="$(cat "${supervisor_file}" 2>/dev/null || true)"
      if [[ "${supervisor_pid}" =~ ^[0-9]+$ ]] \
        && kill -0 "${supervisor_pid}" 2>/dev/null \
        && (exec 8<>"/dev/tcp/127.0.0.1/${panel_port}") 2>/dev/null; then
        physical_display_ready=true
        break
      fi
    fi
    sleep 0.1
  done
fi

if [[ "${physical_display_ready}" != true ]]; then
  echo "physical display failed to start" >&2
  cat "${infrastructure_log}" >&2 || true
  exit 1
fi
"##;

const REQUIRED_COMMANDS: [&str; 4] = ["Xtigervnc", "websockify", "flock", "setsid"];
const NOVNC_ASSETS: &str = "/usr/share/novnc";
const INSTALL_HINT: &str =
    "sudo apt install tigervnc-standalone-server novnc websockify openbox util-linux";
const GNOME_INSTALL_HINT: &str =
    "sudo apt install ubuntu-session gnome-session gnome-shell gnome-session-flashback dbus-x11";

pub fn vnc_port() -> u16 {
    VNC_PORT
}

pub fn physical_vnc_port() -> u16 {
    PHYSICAL_VNC_PORT
}

fn validate_physical_dependencies() -> Result<(), String> {
    let mut missing = ["X0tigervnc", "websockify", "flock", "setsid"]
        .iter()
        .filter(|command_name| !command_available(command_name))
        .map(|command_name| (*command_name).to_string())
        .collect::<Vec<_>>();
    if !Path::new(NOVNC_ASSETS).is_dir() {
        missing.push(format!("noVNC assets ({NOVNC_ASSETS})"));
    }
    if missing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "physical display unavailable; missing remote dependencies: {}; install with: sudo apt install tigervnc-scraping-server novnc websockify util-linux",
        missing.join(", "),
    ))
}

pub fn validate_dependencies() -> Result<(), String> {
    // --- 阶段 1：在启动任务前汇总远端 VNC 依赖 ---
    let mut missing = REQUIRED_COMMANDS
        .iter()
        .filter(|command_name| !command_available(command_name))
        .map(|command_name| (*command_name).to_string())
        .collect::<Vec<_>>();
    if !Path::new(NOVNC_ASSETS).is_dir() {
        missing.push(format!("noVNC assets ({NOVNC_ASSETS})"));
    }
    let desktop_session = std::env::var("HARBOR_VNC_DESKTOP_SESSION")
        .unwrap_or_default()
        .trim()
        .to_string();
    let use_gnome = if desktop_session.is_empty() || desktop_session == "auto" {
        command_available("gnome-session")
            && command_available("dbus-run-session")
            && (Path::new("/usr/share/gnome-session/sessions/ubuntu.session").is_file()
                || Path::new("/usr/share/gnome-session/sessions/gnome.session").is_file()
                || Path::new("/usr/share/gnome-session/sessions/gnome-flashback-metacity.session")
                    .is_file())
    } else {
        desktop_session != "openbox"
    };
    let use_flashback = use_gnome
        && (desktop_session.is_empty()
            || desktop_session == "auto"
            || desktop_session == "gnome-flashback-metacity");
    if !use_gnome {
        if !command_available("openbox") {
            missing.push("openbox".into());
        }
    } else if use_flashback {
        for command_name in [
            "dbus-run-session",
            "gnome-flashback",
            "metacity",
            "gnome-panel",
            "nautilus",
        ] {
            if !command_available(command_name) {
                missing.push(command_name.into());
            }
        }
    } else {
        for command_name in ["gnome-session", "gnome-shell", "dbus-run-session"] {
            if !command_available(command_name) {
                missing.push(command_name.into());
            }
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    let install_hint = if use_gnome {
        GNOME_INSTALL_HINT
    } else {
        INSTALL_HINT
    };
    Err(format!(
        "vnc_interface unavailable; missing remote dependencies: {}; install with: {install_hint}",
        missing.join(", "),
    ))
}

fn command_available(command_name: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg("command -v \"$1\" >/dev/null 2>&1")
        .arg("harbor-vnc-dependency-check")
        .arg(command_name)
        .status()
        .is_ok_and(|status| status.success())
}

pub fn build_command(definition: &TaskCommand, sudo: bool) -> Result<Command, String> {
    // --- 阶段 1：构造用户原始命令 ---
    let mut application = Vec::new();
    if sudo {
        application.extend([
            "sudo".into(),
            "-S".into(),
            "-p".into(),
            "".into(),
            "--".into(),
        ]);
    }
    if !definition.argv.is_empty() {
        application.extend(definition.argv.clone());
    } else if !definition.shell.trim().is_empty() && !definition.script.trim().is_empty() {
        application.extend([
            definition.shell.trim().to_string(),
            "-lc".into(),
            definition.script.clone(),
        ]);
    } else {
        return Err("command requires argv or shell + script".into());
    }

    // --- 阶段 2：复用机器级共享远端桌面 ---
    let runtime_dir = runtime_dir();
    let (ensure_script, server_script) = write_runtime_scripts(
        runtime_dir.as_path(),
        "harbor-vnc-ensure.sh",
        ENSURE_DISPLAY_SCRIPT,
        "harbor-vnc-server.sh",
        SHARED_DISPLAY_SCRIPT,
    )?;
    Ok(build_vnc_command(
        application,
        ensure_script.as_path(),
        server_script.as_path(),
    ))
}

fn build_vnc_command(
    application: Vec<String>,
    ensure_script: &Path,
    server_script: &Path,
) -> Command {
    let mut command = Command::new("bash");
    command
        .arg(ensure_script)
        .arg(VNC_PORT.to_string())
        .arg(DISPLAY_NUMBER.to_string())
        .arg(server_script)
        .args(application);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vnc_interface_wraps_argv_for_shared_remote_access() {
        let definition = TaskCommand {
            argv: vec!["demo".into(), "--flag".into()],
            shell: String::new(),
            script: String::new(),
        };
        let remote = build_vnc_command(
            definition.argv,
            Path::new("/tmp/harbor-vnc-ensure.sh"),
            Path::new("/tmp/harbor-vnc-server.sh"),
        );
        let remote_args = remote
            .get_args()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();
        assert!(remote_args[0].ends_with("harbor-vnc-ensure.sh"));
        assert!(remote_args[3].ends_with("harbor-vnc-server.sh"));
        assert!(remote_args
            .iter()
            .all(|arg| !arg.contains("set -Eeuo pipefail")));
        assert!(remote_args.iter().any(|arg| arg == "23682"));
        assert_eq!(remote_args.last().map(|arg| arg.as_ref()), Some("--flag"));
        assert!(ENSURE_DISPLAY_SCRIPT.contains("harbor-vnc-server"));
        assert!(SHARED_DISPLAY_SCRIPT.contains("new RFB"));
        assert!(SHARED_DISPLAY_SCRIPT.contains("HARBOR_VNC_DESKTOP_SESSION"));
        assert!(SHARED_DISPLAY_SCRIPT.contains("ubuntu.session"));
        assert!(SHARED_DISPLAY_SCRIPT.contains("XDG_CURRENT_DESKTOP=ubuntu:GNOME"));
        assert!(SHARED_DISPLAY_SCRIPT.contains("GNOME_SHELL_SESSION_MODE=ubuntu"));
        assert!(SHARED_DISPLAY_SCRIPT.contains("ubuntu-dock@ubuntu.com"));
        assert!(SHARED_DISPLAY_SCRIPT.contains("gnome-session"));
        assert!(SHARED_DISPLAY_SCRIPT.contains("gnome-shell --x11"));
        assert!(SHARED_DISPLAY_SCRIPT.contains("openbox --sm-disable"));
        assert!(ENSURE_DISPLAY_SCRIPT.contains("flock 9"));
        assert!(ENSURE_DISPLAY_SCRIPT.contains("9>&- </dev/null"));
        assert!(ENSURE_DISPLAY_SCRIPT.contains("exec \"$@\""));
    }

    #[test]
    fn physical_display_uses_distinct_port_and_valid_shell_scripts() {
        assert_eq!(PHYSICAL_VNC_PORT, 23683);
        assert!(PHYSICAL_DISPLAY_SCRIPT.contains("setsid X0tigervnc"));
        assert!(PHYSICAL_DISPLAY_SCRIPT.contains("127.0.0.1:${panel_port}"));
        assert!(ENSURE_PHYSICAL_DISPLAY_SCRIPT.contains("stale_pids"));
        assert!(PHYSICAL_DISPLAY_SCRIPT.contains("DISPLAY=\"${display_name}\""));
        assert!(ENSURE_PHYSICAL_DISPLAY_SCRIPT.contains("9>&- </dev/null"));
        for script in [PHYSICAL_DISPLAY_SCRIPT, ENSURE_PHYSICAL_DISPLAY_SCRIPT] {
            let status = Command::new("bash")
                .args(["-n", "-c", script])
                .status()
                .unwrap();
            assert!(status.success());
        }
    }

    #[test]
    fn shutdown_display_removes_stale_runtime_state() {
        let runtime_dir =
            std::env::temp_dir().join(format!("harbor-vnc-shutdown-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&runtime_dir).unwrap();
        for name in ["supervisor.pid", "ready", "vnc.sock", "startup.lock"] {
            fs::write(runtime_dir.join(name), "999999999").unwrap();
        }

        shutdown_display(runtime_dir.as_path(), "harbor-vnc-test-server");

        for name in ["supervisor.pid", "ready", "vnc.sock", "startup.lock"] {
            assert!(!runtime_dir.join(name).exists());
        }
        let _ = fs::remove_dir_all(runtime_dir);
    }
}
