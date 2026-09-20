use std::path::Path;
use std::process::Command;

use crate::taskcard::{TaskCommand, VncInterface};

const FORCE_DISPLAY_SCRIPT: &str = r##"
set -Eeuo pipefail

# --- 阶段 1：读取 Harbor 注入的显示配置 ---
panel_port="$1"
bind_host="$2"
display_number="$3"
shift 3

required_commands=(Xtigervnc openbox websockify)
missing_dependencies=()
for command_name in "${required_commands[@]}"; do
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    missing_dependencies+=("${command_name}")
  fi
done
if [[ ! -d /usr/share/novnc ]]; then
  missing_dependencies+=("noVNC assets (/usr/share/novnc)")
fi
if ((${#missing_dependencies[@]})); then
  printf 'vnc_interface unavailable; missing remote dependencies: %s\n' "$(IFS=', '; echo "${missing_dependencies[*]}")" >&2
  echo "install with: sudo apt install tigervnc-standalone-server novnc websockify openbox" >&2
  exit 1
fi

runtime_base="${XDG_RUNTIME_DIR:-/tmp}"
runtime_dir="$(mktemp -d "${runtime_base}/harbor-display-${UID}-${panel_port}-XXXXXX")"
web_root="${runtime_dir}/www"
vnc_socket="${runtime_dir}/vnc.sock"
infrastructure_log="${runtime_dir}/infrastructure.log"
mkdir -p "${web_root}"

child_pids=()
cleanup() {
  trap - EXIT INT TERM
  if ((${#child_pids[@]})); then
    kill "${child_pids[@]}" 2>/dev/null || true
    wait "${child_pids[@]}" 2>/dev/null || true
  fi
  rm -rf "${runtime_dir}"
}
trap cleanup EXIT INT TERM

# --- 阶段 2：准备适配 HiDPI 的 noVNC 页面 ---
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
  const socketUrl = 'ws://' + window.location.host + '/websockify';
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

# --- 阶段 3：建立隔离的虚拟 X11 桌面 ---
export DISPLAY=":${display_number}"
export GDK_BACKEND=x11
export QT_QPA_PLATFORM=xcb
export SDL_VIDEODRIVER=x11
unset WAYLAND_DISPLAY

Xtigervnc "${DISPLAY}" \
  -geometry 1920x1080 \
  -depth 24 \
  -rfbport 0 \
  -rfbunixpath "${vnc_socket}" \
  -rfbunixmode 384 \
  -SecurityTypes None \
  -AcceptSetDesktopSize=1 \
  -desktop "Harbor Task" \
  -nolisten tcp >>"${infrastructure_log}" 2>&1 &
child_pids+=("$!")

for _ in {1..50}; do
  [[ -S "${vnc_socket}" ]] && break
  sleep 0.1
done
if [[ ! -S "${vnc_socket}" ]]; then
  echo "vnc_interface failed to start TigerVNC on ${DISPLAY}" >&2
  cat "${infrastructure_log}" >&2
  exit 1
fi

openbox --sm-disable >>"${infrastructure_log}" 2>&1 &
child_pids+=("$!")

websockify \
  --web "${web_root}" \
  "${bind_host}:${panel_port}" \
  --unix-target "${vnc_socket}" >>"${infrastructure_log}" 2>&1 &
child_pids+=("$!")

# --- 阶段 4：在虚拟显示屏中运行原始 Task ---
"$@" &
application_pid="$!"
child_pids+=("${application_pid}")
set +e
wait "${application_pid}"
status="$?"
set -e
exit "${status}"
"##;

const REQUIRED_COMMANDS: [&str; 3] = ["Xtigervnc", "openbox", "websockify"];
const NOVNC_ASSETS: &str = "/usr/share/novnc";
const INSTALL_HINT: &str = "sudo apt install tigervnc-standalone-server novnc websockify openbox";

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
    if missing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "vnc_interface unavailable; missing remote dependencies: {}; install with: {INSTALL_HINT}",
        missing.join(", ")
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

pub fn build_command(
    definition: &TaskCommand,
    panel: &VncInterface,
    sudo: bool,
) -> Result<Command, String> {
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

    // --- 阶段 2：发布远端桌面面板 ---
    let display_number = 100 + u32::from(panel.interface_port) % 10_000;
    let mut command = Command::new("bash");
    command
        .arg("-c")
        .arg(FORCE_DISPLAY_SCRIPT)
        .arg("harbor-force-display")
        .arg(panel.interface_port.to_string())
        .arg("0.0.0.0")
        .arg(display_number.to_string())
        .args(application);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vnc_interface_wraps_argv_for_remote_access() {
        let definition = TaskCommand {
            argv: vec!["demo".into(), "--flag".into()],
            shell: String::new(),
            script: String::new(),
        };
        let panel = VncInterface {
            panel_name: "display".into(),
            interface_port: 23682,
        };
        let remote = build_command(&definition, &panel, false).unwrap();
        let remote_args = remote
            .get_args()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();
        assert!(remote_args.iter().any(|arg| arg == "0.0.0.0"));
        assert_eq!(remote_args.last().map(|arg| arg.as_ref()), Some("--flag"));
        assert!(FORCE_DISPLAY_SCRIPT.contains("const socketUrl = 'ws://'"));
        assert!(FORCE_DISPLAY_SCRIPT.contains("new RFB"));
    }
}
