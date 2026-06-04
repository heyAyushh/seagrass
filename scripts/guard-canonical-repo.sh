#!/usr/bin/env bash
set -euo pipefail

REQUIRED_MARKERS=("AGENTS.md" "VERSION" "src")

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)"

for marker in "${REQUIRED_MARKERS[@]}"; do
  if [[ ! -e "$repo_root/$marker" ]]; then
    printf 'wrong repository: %s\nmissing marker: %s\n' "$repo_root" "$marker" >&2
    exit 1
  fi
done

case "$PWD" in
  "$repo_root"|"$repo_root"/*)
    ;;
  *)
    printf 'wrong working directory: %s\nexpected under: %s\n' "$PWD" "$repo_root" >&2
    exit 1
    ;;
esac

printf 'canonical repository OK: %s\n' "$repo_root"
