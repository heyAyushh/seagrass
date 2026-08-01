#!/usr/bin/env bash
# Deterministic fuzz build/run wrapper that always uses nightly so the
# repository rust-toolchain.toml 1.89.0 pin cannot shadow libFuzzer builds.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$#" -lt 1 ]]; then
  cat <<'EOF' >&2
usage:
  scripts/run-fuzz.sh build [target...]
  scripts/run-fuzz.sh run <target> [-- <libFuzzer args...>]
EOF
  exit 2
fi

ACTION="$1"
shift

case "${ACTION}" in
  build)
    if [[ "$#" -eq 0 ]]; then
      exec cargo +nightly fuzz build
    fi
    exec cargo +nightly fuzz build "$@"
    ;;
  run)
    if [[ "$#" -lt 1 ]]; then
      echo "error: run requires a fuzz target name" >&2
      exit 2
    fi
    exec cargo +nightly fuzz run "$@"
    ;;
  *)
    echo "error: unknown action '${ACTION}' (expected build or run)" >&2
    exit 2
    ;;
esac
