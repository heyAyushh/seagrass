#!/usr/bin/env bash
# Golden-path smoke: build or locate seagrass, run diagnostics on a known-broken fixture.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FIXTURE="${ROOT}/fixtures/smoke-broken.rs"
SMOKE_PATTERN='payer must be provided|space must be provided|missing `payer|missing `space|missing-payer|init` constraint is missing `payer'
VERSION="$(tr -d '[:space:]' < "${ROOT}/VERSION")"

run_seagrass() {
  if [[ -x "${ROOT}/target/debug/seagrass" ]]; then
    "${ROOT}/target/debug/seagrass" "$@"
    return
  fi
  if [[ -x "${ROOT}/target/release/seagrass" ]]; then
    "${ROOT}/target/release/seagrass" "$@"
    return
  fi
  if command -v seagrass >/dev/null 2>&1; then
    seagrass "$@"
    return
  fi
  cargo run -p seagrass-cli --quiet -- "$@"
}

echo "==> Seagrass install smoke"
echo "    fixture: fixtures/smoke-broken.rs"

if [[ ! -f "$FIXTURE" ]]; then
  echo "error: smoke fixture missing: $FIXTURE" >&2
  exit 2
fi

VERSION_OUTPUT="$(run_seagrass --version)"
if ! printf '%s' "$VERSION_OUTPUT" | grep -Eq "(^|[[:space:]])v?${VERSION}([[:space:]]|$)"; then
  echo "error: seagrass --version did not contain ${VERSION}: ${VERSION_OUTPUT}" >&2
  exit 1
fi

set +e
JSON="$(run_seagrass diagnostics "$FIXTURE" --json)"
STATUS=$?
set -e
if [[ "$STATUS" -ge 2 ]]; then
  echo "error: seagrass diagnostics failed with exit $STATUS" >&2
  exit "$STATUS"
fi
if ! printf '%s' "$JSON" | grep -Eq "$SMOKE_PATTERN"; then
  echo "error: expected Anchor init companion diagnostics in JSON output" >&2
  printf '%s\n' "$JSON" >&2
  exit 1
fi

COUNT="$(printf '%s' "$JSON" | grep -c '"message"' || true)"
echo "ok: seagrass emitted ${COUNT} diagnostic(s) including init companion guidance"
exit 0
