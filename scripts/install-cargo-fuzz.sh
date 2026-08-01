#!/usr/bin/env bash
# Deterministic cargo-fuzz installer for Seagrass fuzz CI and local replay.
#
# cargo-fuzz's locked dependency tree (notably cargo-platform) currently
# requires rustc >= 1.91. The repo MSRV stays on rust-toolchain.toml (1.89.0),
# so installs must use an explicit newer toolchain such as nightly.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PINNED_CARGO_FUZZ_VERSION="0.13.2"
INSTALL_TOOLCHAIN="nightly"
MIN_INSTALL_RUSTC_MINOR=91

CHECK_TOOLCHAIN_ONLY=0
PRINT_COMMAND_ONLY=0
FORCE=0

usage() {
  cat <<'EOF'
Usage: scripts/install-cargo-fuzz.sh [options]

Install the pinned cargo-fuzz release with a toolchain new enough for its
locked dependency MSRV, independent of the repository 1.89.0 pin.

Options:
  --check-toolchain  Verify the install toolchain rustc meets MSRV, then exit.
  --print-command    Print the exact cargo install argv and exit.
  --force            Pass --force to cargo install.
  -h, --help         Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check-toolchain)
      CHECK_TOOLCHAIN_ONLY=1
      shift
      ;;
    --print-command)
      PRINT_COMMAND_ONLY=1
      shift
      ;;
    --force)
      FORCE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

INSTALL_ARGS=(
  "+${INSTALL_TOOLCHAIN}"
  install
  cargo-fuzz
  --locked
  --version
  "${PINNED_CARGO_FUZZ_VERSION}"
)
if [[ "${FORCE}" -eq 1 ]]; then
  INSTALL_ARGS+=(--force)
fi

if [[ "${PRINT_COMMAND_ONLY}" -eq 1 ]]; then
  printf 'cargo'
  printf ' %q' "${INSTALL_ARGS[@]}"
  printf '\n'
  exit 0
fi

RUSTC_VERSION="$(rustc "+${INSTALL_TOOLCHAIN}" --version)"
if [[ ! "${RUSTC_VERSION}" =~ rustc[[:space:]]+1\.([0-9]+)\. ]]; then
  echo "error: could not parse rustc version from: ${RUSTC_VERSION}" >&2
  exit 1
fi
RUSTC_MINOR="${BASH_REMATCH[1]}"
if (( RUSTC_MINOR < MIN_INSTALL_RUSTC_MINOR )); then
  echo "error: ${INSTALL_TOOLCHAIN} rustc is 1.${RUSTC_MINOR}, but cargo-fuzz ${PINNED_CARGO_FUZZ_VERSION} requires rustc 1.${MIN_INSTALL_RUSTC_MINOR}+ (cargo-platform MSRV)" >&2
  exit 1
fi
echo "ok: ${INSTALL_TOOLCHAIN} rustc 1.${RUSTC_MINOR} meets cargo-fuzz ${PINNED_CARGO_FUZZ_VERSION} install MSRV"

if [[ "${CHECK_TOOLCHAIN_ONLY}" -eq 1 ]]; then
  exit 0
fi

echo "==> cargo ${INSTALL_ARGS[*]}"
cargo "${INSTALL_ARGS[@]}"

FUZZ_VERSION="$(cargo fuzz --version)"
if [[ "${FUZZ_VERSION}" != *"${PINNED_CARGO_FUZZ_VERSION}"* ]]; then
  echo "error: cargo fuzz --version did not report ${PINNED_CARGO_FUZZ_VERSION}: ${FUZZ_VERSION}" >&2
  exit 1
fi
echo "Verified cargo-fuzz ${PINNED_CARGO_FUZZ_VERSION}"
