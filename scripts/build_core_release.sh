#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE_BIN="$ROOT_DIR/src-tauri/target/release/harbor_core"
HASH_FILE="$CORE_BIN.sha256"
CORE_CACHE_DIR="$ROOT_DIR/src-tauri/target/release/harbor_core-builds"
TTYD_BIN="$ROOT_DIR/src-tauri/target/release/harbor_ttyd"
TTYD_HASH_FILE="$TTYD_BIN.sha256"
TTYD_CACHE_DIR="$ROOT_DIR/src-tauri/target/release/harbor_ttyd-builds"
TTYD_VERSION="1.7.7"
TTYD_RELEASE_SHA256="8a217c968aba172e0dbf3f34447218dc015bc4d5e59bf51db2f2cd12b7be4f55"
TTYD_DOWNLOAD_DIR="$ROOT_DIR/src-tauri/target/release/harbor_ttyd-downloads"
HARBOR_VERSION="${HARBOR_VERSION:-$("$ROOT_DIR/scripts/git_version.sh")}"
export HARBOR_VERSION

# --- 阶段 1：解析 Git 版本并构建 release harbor_core ---
printf 'Harbor version %s\n' "$HARBOR_VERSION"
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

# --- 阶段 4：固化 Harbor 托管的 ttyd 二进制 ---
if [[ -n "${HARBOR_TTYD_BIN:-}" ]]; then
  TTYD_SOURCE="$HARBOR_TTYD_BIN"
else
  mkdir -p "$TTYD_DOWNLOAD_DIR"
  TTYD_SOURCE="$TTYD_DOWNLOAD_DIR/ttyd.x86_64-$TTYD_VERSION"
  if [[ ! -f "$TTYD_SOURCE" ]]; then
    curl -fL --retry 3 \
      "https://github.com/tsl0922/ttyd/releases/download/$TTYD_VERSION/ttyd.x86_64" \
      -o "$TTYD_SOURCE.new"
    mv -f "$TTYD_SOURCE.new" "$TTYD_SOURCE"
  fi
  printf '%s  %s\n' "$TTYD_RELEASE_SHA256" "$TTYD_SOURCE" | sha256sum --check --status || {
    printf 'downloaded ttyd %s failed SHA-256 verification\n' "$TTYD_VERSION" >&2
    exit 1
  }
fi
[[ -f "$TTYD_SOURCE" ]] || { printf 'ttyd not found at %s\n' "$TTYD_SOURCE" >&2; exit 1; }
file "$TTYD_SOURCE" | grep -q 'statically linked' || {
  printf 'managed ttyd must be a statically linked Linux binary: %s\n' "$TTYD_SOURCE" >&2
  exit 1
}
cp "$TTYD_SOURCE" "$TTYD_BIN.new"
chmod 755 "$TTYD_BIN.new"
mv -f "$TTYD_BIN.new" "$TTYD_BIN"
sha256sum "$TTYD_BIN" | awk '{print $1}' >"$TTYD_HASH_FILE.new"
mv -f "$TTYD_HASH_FILE.new" "$TTYD_HASH_FILE"
TTYD_HASH="$(cat "$TTYD_HASH_FILE")"
mkdir -p "$TTYD_CACHE_DIR"
TTYD_CACHE_BIN="$TTYD_CACHE_DIR/$TTYD_HASH"
if [[ ! -f "$TTYD_CACHE_BIN" ]]; then
  cp "$TTYD_BIN" "$TTYD_CACHE_BIN.new"
  chmod 755 "$TTYD_CACHE_BIN.new"
  mv -f "$TTYD_CACHE_BIN.new" "$TTYD_CACHE_BIN"
fi
printf 'ttyd sha256 %s\n' "$TTYD_HASH"
