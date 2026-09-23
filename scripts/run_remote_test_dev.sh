#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REMOTE_TEST_HOST="127.0.0.2"

append_no_proxy() {
  local current_value="$1"
  local host="$2"
  case ",${current_value}," in
    *",${host},"*) printf '%s' "${current_value}" ;;
    ",,") printf '%s' "${host}" ;;
    *) printf '%s,%s' "${current_value}" "${host}" ;;
  esac
}

# --- 阶段 1：让 Docker 远端测试地址绕过系统 HTTP 代理 ---
export NO_PROXY="$(append_no_proxy "${NO_PROXY:-}" "${REMOTE_TEST_HOST}")"
export no_proxy="$(append_no_proxy "${no_proxy:-}" "${REMOTE_TEST_HOST}")"

# --- 阶段 2：使用正常 Harbor 开发入口启动 GUI ---
cd "${ROOT_DIR}"
exec npm run tauri:dev
