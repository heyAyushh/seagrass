#!/usr/bin/env bash
# Deterministic fuzz build/run wrapper that always uses nightly so the
# repository rust-toolchain.toml 1.89.0 pin cannot shadow libFuzzer builds.
#
# AddressSanitizer is disabled (-s none) because current nightly + cargo-fuzz
# 0.13.2 fails to link with undefined __sancov_gen_* symbols under ASAN
# (rust-fuzz/cargo-fuzz#404). Coverage instrumentation still runs via libFuzzer.
#
# Resource defaults avoid aborting hour-long CI shards on libFuzzer's 2 GiB RSS
# cap / 1200s per-input timeout. OOM and timeout artifacts are still written;
# real crashes (panics) still fail the run.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Keep the sanitizer selection in one place so CI and local runs stay aligned.
FUZZ_SANITIZER="${FUZZ_SANITIZER:-none}"
# GitHub ubuntu-latest has ~7 GiB RAM; default libFuzzer RSS cap is 2048 MiB.
FUZZ_RSS_LIMIT_MB="${FUZZ_RSS_LIMIT_MB:-4096}"
# Bound hung inputs so one path cannot burn a full shard hour.
FUZZ_INPUT_TIMEOUT_SEC="${FUZZ_INPUT_TIMEOUT_SEC:-60}"
# Continue after resource-limit findings; crash/leak findings still abort.
FUZZ_IGNORE_OOMS="${FUZZ_IGNORE_OOMS:-1}"
FUZZ_IGNORE_TIMEOUTS="${FUZZ_IGNORE_TIMEOUTS:-1}"

if [[ "$#" -lt 1 ]]; then
  cat <<'EOF' >&2
usage:
  scripts/run-fuzz.sh build [target...]
  scripts/run-fuzz.sh run <target> [-- <libFuzzer args...>]

Environment:
  FUZZ_SANITIZER            cargo-fuzz -s value (default: none)
  FUZZ_RSS_LIMIT_MB         libFuzzer -rss_limit_mb (default: 4096)
  FUZZ_INPUT_TIMEOUT_SEC    libFuzzer -timeout seconds (default: 60)
  FUZZ_IGNORE_OOMS          libFuzzer -ignore_ooms (default: 1)
  FUZZ_IGNORE_TIMEOUTS      libFuzzer -ignore_timeouts (default: 1)
EOF
  exit 2
fi

ACTION="$1"
shift

default_fuzzer_flags() {
  printf '%s\n' \
    "-rss_limit_mb=${FUZZ_RSS_LIMIT_MB}" \
    "-timeout=${FUZZ_INPUT_TIMEOUT_SEC}" \
    "-ignore_ooms=${FUZZ_IGNORE_OOMS}" \
    "-ignore_timeouts=${FUZZ_IGNORE_TIMEOUTS}"
}

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
    TARGET="$1"
    shift
    EXTRA_ARGS=()
    if [[ "${1:-}" == "--" ]]; then
      shift
      EXTRA_ARGS=("$@")
    elif [[ "$#" -gt 0 ]]; then
      EXTRA_ARGS=("$@")
    fi
    mapfile -t DEFAULT_FLAGS < <(default_fuzzer_flags)
    # Defaults first; later CLI flags override when libFuzzer sees duplicates.
    exec cargo +nightly fuzz run -s "${FUZZ_SANITIZER}" "${TARGET}" -- \
      "${DEFAULT_FLAGS[@]}" \
      "${EXTRA_ARGS[@]}"
    ;;
  *)
    echo "error: unknown action '${ACTION}' (expected build or run)" >&2
    exit 2
    ;;
esac
