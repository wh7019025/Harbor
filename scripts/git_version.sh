#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE_VERSION="0.2.0-preview"
REQUIRE_TAG=false

if [[ "${1:-}" == "--require-tag" ]]; then
  REQUIRE_TAG=true
elif [[ -n "${1:-}" ]]; then
  printf 'unknown git_version option: %s\n' "$1" >&2
  exit 2
fi

validate_version() {
  local version="$1"
  if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
    printf 'invalid Harbor version: %s\n' "$version" >&2
    exit 1
  fi
}

if ! $REQUIRE_TAG && [[ -n "${HARBOR_VERSION:-}" ]]; then
  validate_version "$HARBOR_VERSION"
  printf '%s\n' "$HARBOR_VERSION"
  exit 0
fi

if ! git -C "$ROOT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  if $REQUIRE_TAG; then
    printf 'release version requires a Git checkout and an exact version tag\n' >&2
    exit 1
  fi
  printf '%s\n' "$BASE_VERSION"
  exit 0
fi

exact_tag="$(git -C "$ROOT_DIR" tag --points-at HEAD --list 'v[0-9]*' | sort -V | tail -n 1)"
dirty_suffix=""
if [[ -n "$(git -C "$ROOT_DIR" status --porcelain)" ]]; then
  dirty_suffix=".dirty"
fi

if [[ -n "$exact_tag" ]]; then
  version="${exact_tag#v}"
  if [[ -n "$dirty_suffix" ]]; then
    if $REQUIRE_TAG; then
      printf 'release tag %s has tracked working tree changes\n' "$exact_tag" >&2
      exit 1
    fi
    version="${version}+dirty"
  fi
else
  if $REQUIRE_TAG; then
    printf 'release builds require an exact v* Git tag on HEAD\n' >&2
    exit 1
  fi
  base_tag="$(git -C "$ROOT_DIR" describe --tags --match 'v[0-9]*' --abbrev=0 2>/dev/null || true)"
  if [[ -z "$base_tag" ]]; then
    version="${BASE_VERSION}+0.g$(git -C "$ROOT_DIR" rev-parse --short=8 HEAD)${dirty_suffix}"
  else
    commit_count="$(git -C "$ROOT_DIR" rev-list --count "${base_tag}..HEAD")"
    short_sha="$(git -C "$ROOT_DIR" rev-parse --short=8 HEAD)"
    version="${base_tag#v}+${commit_count}.g${short_sha}${dirty_suffix}"
  fi
fi

validate_version "$version"
printf '%s\n' "$version"
