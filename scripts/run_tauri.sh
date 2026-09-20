#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HARBOR_VERSION="$("$ROOT_DIR/scripts/git_version.sh")"
export HARBOR_VERSION

printf 'Harbor version %s\n' "$HARBOR_VERSION"
exec "$ROOT_DIR/node_modules/.bin/tauri" \
  "$@" \
  --config "{\"version\":\"$HARBOR_VERSION\"}"
