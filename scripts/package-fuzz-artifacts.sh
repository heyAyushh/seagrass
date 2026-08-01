#!/usr/bin/env bash
# Package fuzz corpus + crash artifacts for CI upload.
# Always creates the artifacts directory so packaging stays deterministic even
# when an earlier fuzz step exits before cargo-fuzz writes crashes.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$#" -ne 1 ]]; then
  echo "usage: $0 <output-tar.gz>" >&2
  exit 2
fi

OUT="$1"
mkdir -p fuzz/corpus fuzz/artifacts

tar -czf "$OUT" \
  -C . \
  fuzz/corpus \
  fuzz/artifacts

echo "ok: wrote ${OUT}"
