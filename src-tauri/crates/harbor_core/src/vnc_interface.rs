use std::path::Path;
use std::process::Command;

use crate::taskcard::TaskCommand;

pub const VNC_PORT: u16 = 23682;
const DISPLAY_NUMBER: u16 = 82;

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

Xtigervnc "${DISPLAY}" \
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
  openbox --sm-disable >>"${infrastructure_log}" 2>&1 &
fi
child_pids+=("$!")

# GNOME Session 会在初始化时接管窗口管理器；等待其稳定后再放行 Task。
if [[ "${desktop_session}" != openbox ]]; then
  sleep 3
fi

websockify \
  --web "${web_root}" \
  "0.0.0.0:${panel_port}" \
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
  && grep -aq 'harbor-vnc-server' "/proc/${supervisor_pid}/cmdline" 2>/dev/null \
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
  setsid bash -c "${server_script}" harbor-vnc-server \
    "${runtime_dir}" "${display_number}" "${panel_port}" \
    </dev/null >>"${infrastructure_log}" 2>&1 &

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

const REQUIRED_COMMANDS: [&str; 4] = ["Xtigervnc", "websockify", "flock", "setsid"];
const NOVNC_ASSETS: &str = "/usr/share/novnc";
const INSTALL_HINT: &str =
    "sudo apt install tigervnc-standalone-server novnc websockify openbox util-linux";
const GNOME_INSTALL_HINT: &str =
    "sudo apt install ubuntu-session gnome-session gnome-shell gnome-session-flashback dbus-x11";

pub fn vnc_port() -> u16 {
    VNC_PORT
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
        command_available("gnome-session") && command_available("dbus-run-session")
            && (Path::new("/usr/share/gnome-session/sessions/ubuntu.session").is_file()
                || Path::new("/usr/share/gnome-session/sessions/gnome.session").is_file()
                || Path::new(
                    "/usr/share/gnome-session/sessions/gnome-flashback-metacity.session",
                )
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
    let mut command = Command::new("bash");
    command
        .arg("-c")
        .arg(ENSURE_DISPLAY_SCRIPT)
        .arg("harbor-vnc-task")
        .arg(VNC_PORT.to_string())
        .arg(DISPLAY_NUMBER.to_string())
        .arg(SHARED_DISPLAY_SCRIPT)
        .args(application);
    Ok(command)
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
        let remote = build_command(&definition, false).unwrap();
        let remote_args = remote
            .get_args()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();
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
        assert!(ENSURE_DISPLAY_SCRIPT.contains("exec \"$@\""));
    }
}
