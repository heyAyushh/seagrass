#!/usr/bin/env bash
# Deterministic fuzz build/run wrapper that always uses nightly so the
# repository rust-toolchain.toml 1.89.0 pin cannot shadow libFuzzer builds.
#
# AddressSanitizer is disabled (-s none) because current nightly + cargo-fuzz
# 0.13.2 fails to link with undefined __sancov_gen_* symbols under ASAN
# (rust-fuzz/cargo-fuzz#404). Coverage instrumentation still runs via libFuzzer.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Keep the sanitizer selection in one place so CI and local runs stay aligned.
FUZZ_SANITIZER="${FUZZ_SANITIZER:-none}"

if [[ "$#" -lt 1 ]]; then
  cat <<'EOF' >&2
usage:
  scripts/run-fuzz.sh build [target...]
  scripts/run-fuzz.sh run <target> [-- <libFuzzer args...>]

Environment:
  FUZZ_SANITIZER  cargo-fuzz -s value (default: none)
EOF
  exit 2
fi

ACTION="$1"
shift

case "${ACTION}" in
  build)
    if [[ "$#" -eq 0 ]]; then
      exec cargo +nightly fuzz build -s "${FUZZ_SANITIZER}"
    fi
    exec cargo +nightly fuzz build -s "${FUZZ_SANITIZER}" "$@"
    ;;
  run)
    if [[ "$#" -lt 1 ]]; then
      echo "error: run requires a fuzz target name" >&2
      exit 2
    fi
    exec cargo +nightly fuzz run -s "${FUZZ_SANITIZER}" "$@"
    ;;
  *)
    echo "error: unknown action '${ACTION}' (expected build or run)" >&2
    exit 2
    ;;
esac
