#!/usr/bin/env bash
# Golden-path smoke: build or locate seagrass, run diagnostics on a known-broken fixture.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FIXTURE="${ROOT}/fixtures/smoke-broken.rs"
SMOKE_PATTERN='payer must be provided|space must be provided|missing `payer|missing `space|missing-payer|init` constraint is missing `payer'

run_diagnostics() {
  if [[ -x "${ROOT}/target/debug/seagrass" ]]; then
    "${ROOT}/target/debug/seagrass" diagnostics "$FIXTURE" --json
    return
  fi
  if [[ -x "${ROOT}/target/release/seagrass" ]]; then
    "${ROOT}/target/release/seagrass" diagnostics "$FIXTURE" --json
    return
  fi
  if command -v seagrass >/dev/null 2>&1; then
    seagrass diagnostics "$FIXTURE" --json
    return
  fi
  cargo run -p seagrass-cli --quiet -- diagnostics "$FIXTURE" --json
}

echo "==> Seagrass install smoke"
echo "    fixture: fixtures/smoke-broken.rs"

if [[ ! -f "$FIXTURE" ]]; then
  echo "error: smoke fixture missing: $FIXTURE" >&2
  exit 2
fi

set +e
JSON="$(run_diagnostics)"
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
