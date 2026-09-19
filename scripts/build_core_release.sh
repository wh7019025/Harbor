#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE_BIN="$ROOT_DIR/src-tauri/target/release/harbor_core"
HASH_FILE="$CORE_BIN.sha256"
CORE_CACHE_DIR="$ROOT_DIR/src-tauri/target/release/harbor_core-builds"

# --- 阶段 1：只构建 release harbor_core ---
cargo build \
  --release \
  --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" \
  -p harbor_core \
  --bin harbor_core

# --- 阶段 2：生成与二进制绑定的 SHA-256 清单 ---
sha256sum "$CORE_BIN" | awk '{print $1}' >"$HASH_FILE.new"
mv -f "$HASH_FILE.new" "$HASH_FILE"
CORE_HASH="$(cat "$HASH_FILE")"

# --- 阶段 3：保留按哈希命名的不可变构建产物 ---
mkdir -p "$CORE_CACHE_DIR"
CORE_CACHE_BIN="$CORE_CACHE_DIR/$CORE_HASH"
if [[ ! -f "$CORE_CACHE_BIN" ]]; then
  cp "$CORE_BIN" "$CORE_CACHE_BIN.new"
  chmod 755 "$CORE_CACHE_BIN.new"
  mv -f "$CORE_CACHE_BIN.new" "$CORE_CACHE_BIN"
fi
printf 'harbor_core sha256 %s\n' "$CORE_HASH"
