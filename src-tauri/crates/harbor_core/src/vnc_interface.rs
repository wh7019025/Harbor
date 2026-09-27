use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::service::CoreServiceStatus;
use crate::taskcard::TaskCommand;

pub use harbor_protocol::web_api::{PHYSICAL_VNC_PORT, VIRTUAL_VNC_PORT as VNC_PORT};
const DISPLAY_NUMBER: u16 = 82;
const X11_SOCKET_DIR: &str = "/tmp/.X11-unix";

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
    if physical_display_name().is_none() {
        return Some("真实桌面不可用：未检测到活动的本机 X11 DISPLAY".into());
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

pub fn shared_error() -> Option<String> {
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
    write_novnc_index(runtime_dir.as_path(), "Harbor Display", true)?;
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
    let display_name = physical_display_name().ok_or_else(|| {
        "physical display unavailable; active local X11 DISPLAY not found".to_string()
    })?;
    let runtime_dir = physical_runtime_dir();
    let (ensure_script, server_script) = write_runtime_scripts(
        runtime_dir.as_path(),
        "harbor-vnc-physical-ensure.sh",
        ENSURE_PHYSICAL_DISPLAY_SCRIPT,
        "harbor-vnc-physical-server.sh",
        PHYSICAL_DISPLAY_SCRIPT,
    )?;
    write_novnc_index(runtime_dir.as_path(), "Harbor Physical Display", false)?;
    let output = Command::new("bash")
        .arg(ensure_script)
        .arg(PHYSICAL_VNC_PORT.to_string())
        .arg(display_name)
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

fn physical_display_name() -> Option<String> {
    // --- 阶段 1：优先采用用户或当前会话明确指定的真实显示器 ---
    for variable in ["HARBOR_PHYSICAL_DISPLAY", "DISPLAY"] {
        let Some(display_name) = std::env::var_os(variable) else {
            continue;
        };
        let Some(display_number) = local_display_number(display_name.to_string_lossy().as_ref())
        else {
            continue;
        };
        if display_number != DISPLAY_NUMBER && x11_socket_path(display_number).exists() {
            return Some(format!(":{display_number}"));
        }
    }

    // --- 阶段 2：Core 常由 SSH 启动，没有 DISPLAY；从活动 X11 socket 中发现 ---
    let mut display_numbers = fs::read_dir(X11_SOCKET_DIR)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .strip_prefix('X')?
                .parse()
                .ok()
        })
        .filter(|display_number| *display_number != DISPLAY_NUMBER)
        .collect::<Vec<u16>>();
    display_numbers.sort_unstable();
    display_numbers
        .into_iter()
        .next()
        .map(|display_number| format!(":{display_number}"))
}

fn local_display_number(display_name: &str) -> Option<u16> {
    display_name
        .trim()
        .strip_prefix(':')?
        .split('.')
        .next()?
        .parse()
        .ok()
}

fn x11_socket_path(display_number: u16) -> PathBuf {
    Path::new(X11_SOCKET_DIR).join(format!("X{display_number}"))
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

fn write_novnc_index(runtime_dir: &Path, title: &str, resize_session: bool) -> Result<(), String> {
    let content = NOVNC_INDEX_HTML.replace("__TITLE__", title).replace(
        "__RESIZE_SESSION__",
        if resize_session { "true" } else { "false" },
    );
    let path = runtime_dir.join("harbor-index.html");
    fs::write(&path, content).map_err(|error| format!("write {} failed: {error}", path.display()))
}

const NOVNC_INDEX_HTML: &str = r##"<!doctype html>
<meta charset="utf-8">
<title>__TITLE__</title>
<style>
  html, body, #screen { width: 100%; height: 100%; margin: 0; overflow: hidden; background: #1e1e1e; }
  #status { position: fixed; z-index: 2; top: 10px; left: 12px; color: #bbb; font: 13px sans-serif; }
  #clipboard-tools { position: fixed; z-index: 3; top: 8px; right: 10px; display: flex; gap: 6px; }
  .clipboard-button { width: 34px; height: 34px; display: grid; place-items: center; padding: 0; color: #d4d4d4; background: rgba(45, 45, 48, .92); border: 1px solid #4b4b4f; border-radius: 7px; cursor: pointer; }
  .clipboard-button:hover { color: #fff; background: rgba(62, 62, 66, .96); }
  .clipboard-button:disabled { color: #777; cursor: default; opacity: .65; }
  .clipboard-button svg { width: 17px; height: 17px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  #clipboard-message { position: fixed; z-index: 3; top: 48px; right: 10px; max-width: 320px; padding: 6px 9px; color: #ddd; background: rgba(30, 30, 30, .94); border: 1px solid #454545; border-radius: 6px; font: 12px sans-serif; }
</style>
<div id="status">Connecting…</div>
<div id="clipboard-tools">
  <button id="clipboard-send" class="clipboard-button" type="button" title="发送本机剪贴板到远端" disabled>
    <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="8" y="3" width="8" height="4" rx="1"></rect><path d="M16 5h2a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h2"></path><path d="m9 14 3 3 3-3M12 9v8"></path></svg>
  </button>
  <button id="clipboard-copy" class="clipboard-button" type="button" title="复制远端剪贴板到本机" disabled>
    <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="8" y="3" width="8" height="4" rx="1"></rect><path d="M16 5h2a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h2"></path><path d="m9 12 3-3 3 3M12 9v8"></path></svg>
  </button>
</div>
<div id="clipboard-message" hidden></div>
<div id="screen"></div>
<script type="module">
  import RFB from './core/rfb.js?harbor-hidpi=2';

  const status = document.getElementById('status');
  const sendButton = document.getElementById('clipboard-send');
  const copyButton = document.getElementById('clipboard-copy');
  const message = document.getElementById('clipboard-message');
  let remoteClipboard = null;
  let messageTimer = null;

  function showMessage(text) {
    message.textContent = text;
    message.hidden = false;
    window.clearTimeout(messageTimer);
    messageTimer = window.setTimeout(() => { message.hidden = true; }, 2400);
  }

  async function readLocalClipboard() {
    try {
      return await navigator.clipboard.readText();
    } catch (_) {
      return window.prompt('浏览器未允许读取剪贴板，请在此粘贴要发送的文本：', '') ?? '';
    }
  }

  async function writeLocalClipboard(text) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch (_) {
      const input = document.createElement('textarea');
      input.value = text;
      input.style.position = 'fixed';
      input.style.opacity = '0';
      document.body.appendChild(input);
      input.select();
      const copied = document.execCommand('copy');
      input.remove();
      return copied;
    }
  }

  const socketScheme = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
  const socketUrl = socketScheme + window.location.host + '/websockify';
  const rfb = new RFB(document.getElementById('screen'), socketUrl, { shared: true });
  rfb.scaleViewport = true;
  rfb.resizeSession = __RESIZE_SESSION__;
  rfb.qualityLevel = 9;
  rfb.compressionLevel = 2;
  rfb.addEventListener('connect', () => {
    status.hidden = true;
    sendButton.disabled = false;
  });
  rfb.addEventListener('disconnect', (event) => {
    status.hidden = false;
    status.textContent = event.detail.clean ? 'Disconnected' : 'Connection closed';
    sendButton.disabled = true;
    copyButton.disabled = true;
  });
  rfb.addEventListener('clipboard', (event) => {
    remoteClipboard = event.detail.text;
    copyButton.disabled = false;
    showMessage('已收到远端剪贴板');
  });

  sendButton.addEventListener('click', async () => {
    const text = await readLocalClipboard();
    if (!text) {
      showMessage('本机剪贴板为空');
      return;
    }
    rfb.clipboardPasteFrom(text);
    showMessage('已发送到远端剪贴板');
  });

  copyButton.addEventListener('click', async () => {
    if (remoteClipboard === null) {
      showMessage('尚未收到远端剪贴板');
      return;
    }
    showMessage(await writeLocalClipboard(remoteClipboard) ? '已复制远端剪贴板' : '复制失败');
  });
</script>
"##;

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
cp "${runtime_dir}/harbor-index.html" "${web_root}/index.html"

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
  -AcceptCutText=1 \
  -SendCutText=1 \
  -AcceptSetDesktopSize=1 \
  -desktop "Harbor" \
  -nolisten tcp >>"${infrastructure_log}" 2>&1 &
child_pids+=("$!")

for _ in {1..50}; do
  [[ -S "${vnc_socket}" ]] && break
  sleep 0.1
done
if [[ ! -S "${vnc_socket}" ]]; then
  echo "shared virtual display failed to start TigerVNC on ${DISPLAY}" >&2
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
  echo "shared virtual display failed to publish noVNC on port ${panel_port}" >&2
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
  echo "remote_display_virtual failed to start the shared Harbor VNC desktop" >&2
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
cp "${runtime_dir}/harbor-index.html" "${web_root}/index.html"

# --- 阶段 3：抓取已发现的真实 DISPLAY 并发布 noVNC ---
export DISPLAY="${display_name}"
export XAUTHORITY="${xauthority}"
clipboard_args=()
if X0tigervnc -help 2>&1 | grep -qi 'AcceptCutText'; then
  clipboard_args+=("-AcceptCutText=1" "-SendCutText=1")
fi
setsid X0tigervnc \
  -display "${display_name}" \
  -rfbport 0 \
  -rfbunixpath "${vnc_socket}" \
  -rfbunixmode 384 \
  -SecurityTypes None \
  "${clipboard_args[@]}" \
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
    process_args="$(cat "${process_cmdline}" 2>/dev/null | tr '\0' ' ' || true)"
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
        "remote_display_virtual unavailable; missing remote dependencies: {}; install with: {install_hint}",
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
    fn remote_virtual_display_wraps_argv_for_shared_remote_access() {
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
        assert!(NOVNC_INDEX_HTML.contains("new RFB"));
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
        assert_eq!(local_display_number(":1"), Some(1));
        assert_eq!(local_display_number(":1.0"), Some(1));
        assert_eq!(local_display_number("localhost:1"), None);
        assert_ne!(DISPLAY_NUMBER, 1);
        assert!(PHYSICAL_DISPLAY_SCRIPT.contains("setsid X0tigervnc"));
        assert!(PHYSICAL_DISPLAY_SCRIPT.contains("127.0.0.1:${panel_port}"));
        assert!(ENSURE_PHYSICAL_DISPLAY_SCRIPT.contains("stale_pids"));
        assert!(ENSURE_PHYSICAL_DISPLAY_SCRIPT.contains("cat \"${process_cmdline}\""));
        assert!(PHYSICAL_DISPLAY_SCRIPT.contains("DISPLAY=\"${display_name}\""));
        assert!(ENSURE_PHYSICAL_DISPLAY_SCRIPT.contains("9>&- </dev/null"));
        assert!(NOVNC_INDEX_HTML.contains("rfb.clipboardPasteFrom(text)"));
        assert!(NOVNC_INDEX_HTML.contains("addEventListener('clipboard'"));
        assert!(SHARED_DISPLAY_SCRIPT.contains("-AcceptCutText=1"));
        assert!(PHYSICAL_DISPLAY_SCRIPT.contains("-SendCutText=1"));
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
