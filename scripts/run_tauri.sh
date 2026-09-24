#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HARBOR_VERSION="$("$ROOT_DIR/scripts/git_version.sh")"
export HARBOR_VERSION

# --- 阶段 1：先生成与当前源码匹配的托管运行时 ---
case "${1:-}" in
  dev|build)
    "$ROOT_DIR/scripts/build_core_release.sh"
    ;;
esac

# --- 阶段 2：在运行时哈希固定后编译并启动 GUI ---
printf 'Harbor version %s\n' "$HARBOR_VERSION"
exec "$ROOT_DIR/node_modules/.bin/tauri" \
  "$@" \
  --config "{\"version\":\"$HARBOR_VERSION\"}"
