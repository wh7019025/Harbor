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
RUNTIME_ARCH="x86_64"
CORE_RUNTIME="$ROOT_DIR/src-tauri/target/release/harbor_core-runtime-linux-$RUNTIME_ARCH.tar.gz"
CORE_RUNTIME_HASH_FILE="$CORE_RUNTIME.sha256"
CORE_RUNTIME_ENV="$CORE_RUNTIME.env"
CORE_RUNTIME_CACHE_DIR="$ROOT_DIR/src-tauri/target/release/harbor_core-runtime-builds"
TTYD_RUNTIME="$ROOT_DIR/src-tauri/target/release/harbor_ttyd-runtime-linux-$RUNTIME_ARCH.tar.gz"
TTYD_RUNTIME_HASH_FILE="$TTYD_RUNTIME.sha256"
TTYD_RUNTIME_ENV="$TTYD_RUNTIME.env"
TTYD_RUNTIME_CACHE_DIR="$ROOT_DIR/src-tauri/target/release/harbor_ttyd-runtime-builds"
HARBOR_VERSION="${HARBOR_VERSION:-$("$ROOT_DIR/scripts/git_version.sh")}"
export HARBOR_VERSION

build_runtime_bundle() {
  local binary="$1"
  local binary_name="$2"
  local binary_hash="$3"
  local archive="$4"
  local archive_hash_file="$5"
  local env_file="$6"
  local cache_dir="$7"
  local staging
  local loader=""
  staging="$(mktemp -d)"

  # --- 阶段 1：收集 Linux 二进制及其动态运行库 ---
  cp "$binary" "$staging/$binary_name"
  chmod 755 "$staging/$binary_name"
  while IFS= read -r library; do
    [[ -n "$library" && -f "$library" ]] || continue
    cp -L "$library" "$staging/$(basename "$library")"
    case "$(basename "$library")" in
      ld-linux*|ld-musl*) loader="$(basename "$library")" ;;
    esac
  done < <(
    ldd "$binary" 2>/dev/null | awk '
      /=> \/[^ ]+/ { print $3 }
      /^[[:space:]]*\// { print $1 }
    '
  )

  # --- 阶段 2：生成可复现的不可变运行包 ---
  tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner \
    -C "$staging" -cf - . | gzip -n >"$archive.new"
  mv -f "$archive.new" "$archive"
  sha256sum "$archive" | awk '{print $1}' >"$archive_hash_file.new"
  mv -f "$archive_hash_file.new" "$archive_hash_file"
  local archive_hash
  archive_hash="$(cat "$archive_hash_file")"
  {
    printf 'architecture=%s\n' "$RUNTIME_ARCH"
    printf 'binary_sha256=%s\n' "$binary_hash"
    printf 'archive_sha256=%s\n' "$archive_hash"
    printf 'loader=%s\n' "$loader"
  } >"$env_file.new"
  mv -f "$env_file.new" "$env_file"

  # --- 阶段 3：保留按归档哈希命名的不可变副本 ---
  mkdir -p "$cache_dir"
  if [[ ! -f "$cache_dir/$archive_hash" ]]; then
    cp "$archive" "$cache_dir/$archive_hash.new"
    mv -f "$cache_dir/$archive_hash.new" "$cache_dir/$archive_hash"
  fi
  rm -rf "$staging"
  printf '%s runtime sha256 %s\n' "$binary_name" "$archive_hash"
}

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

# --- 阶段 5：预生成 GUI 可直接传输的 Linux 运行包 ---
build_runtime_bundle \
  "$CORE_BIN" harbor_core "$CORE_HASH" \
  "$CORE_RUNTIME" "$CORE_RUNTIME_HASH_FILE" "$CORE_RUNTIME_ENV" "$CORE_RUNTIME_CACHE_DIR"
build_runtime_bundle \
  "$TTYD_BIN" ttyd "$TTYD_HASH" \
  "$TTYD_RUNTIME" "$TTYD_RUNTIME_HASH_FILE" "$TTYD_RUNTIME_ENV" "$TTYD_RUNTIME_CACHE_DIR"
