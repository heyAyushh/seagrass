#!/usr/bin/env bash
set -euo pipefail

CANONICAL_REPO="/Users/ay/Documents/codes/solana/seagrass"
repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

case "$repo_root" in
  "$CANONICAL_REPO"|"$CANONICAL_REPO"/*)
    ;;
  *)
    printf 'wrong repository: %s\nexpected: %s\n' "$repo_root" "$CANONICAL_REPO" >&2
    exit 1
    ;;
esac

case "$PWD" in
  "$CANONICAL_REPO"|"$CANONICAL_REPO"/*)
    ;;
  *)
    printf 'wrong working directory: %s\nexpected under: %s\n' "$PWD" "$CANONICAL_REPO" >&2
    exit 1
    ;;
esac

printf 'canonical repository OK: %s\n' "$repo_root"
