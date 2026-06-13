#!/bin/sh

set -eu

REPOSITORY="heyAyushh/seagrass"
GITHUB_API_BASE="https://api.github.com/repos"
GITHUB_RELEASE_BASE="https://github.com/${REPOSITORY}/releases/download"
SERVER_BINARY_NAME="seagrass"
DEFAULT_SYSTEM_INSTALL_DIR="/usr/local/bin"
DEFAULT_USER_INSTALL_DIR="${HOME}/.local/bin"
VERSION_TAG=""
INSTALL_DIR=""
TMP_DIR=""

usage() {
  echo "Usage: sh scripts/install.sh [--version v0.1.2] [--install-dir /usr/local/bin]" >&2
}

cleanup() {
  if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
    rm -rf "$TMP_DIR"
  fi
}

trap cleanup EXIT INT HUP TERM

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      if [ "$#" -lt 2 ]; then
        usage
        exit 1
      fi
      VERSION_TAG="$2"
      shift 2
      ;;
    --install-dir)
      if [ "$#" -lt 2 ]; then
        usage
        exit 1
      fi
      INSTALL_DIR="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

download_stdout() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1"
    return
  fi
  if command -v wget >/dev/null 2>&1; then
    wget -qO- "$1"
    return
  fi
  echo "curl or wget is required" >&2
  exit 1
}

download_file() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1" -o "$2"
    return
  fi
  if command -v wget >/dev/null 2>&1; then
    wget -q "$1" -O "$2"
    return
  fi
  echo "curl or wget is required" >&2
  exit 1
}

is_musl_linux() {
  if ! command -v ldd >/dev/null 2>&1; then
    return 1
  fi
  ldd --version 2>&1 | grep -qi musl
}

linux_x86_64_target() {
  if is_musl_linux; then
    echo "x86_64-unknown-linux-musl"
    return
  fi
  echo "x86_64-unknown-linux-gnu"
}

detect_target() {
  os_name=$(uname -s)
  arch_name=$(uname -m)

  case "${os_name}:${arch_name}" in
    Linux:x86_64)
      linux_x86_64_target
      ;;
    Darwin:arm64|Darwin:aarch64)
      echo "aarch64-apple-darwin"
      ;;
    Darwin:x86_64)
      echo "x86_64-apple-darwin"
      ;;
    *)
      echo "Seagrass: prebuilt binary not available for ${os_name}/${arch_name}. Install from source: cargo install seagrass-cli --locked" >&2
      exit 1
      ;;
  esac
}

latest_release_tag() {
  latest_json=$(download_stdout "${GITHUB_API_BASE}/${REPOSITORY}/releases/latest")
  tag=$(printf '%s\n' "$latest_json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | sed -n '1p')
  if [ -z "$tag" ]; then
    echo "could not determine latest Seagrass release tag" >&2
    exit 1
  fi
  echo "$tag"
}

if [ -z "$VERSION_TAG" ]; then
  VERSION_TAG=$(latest_release_tag)
fi

case "$VERSION_TAG" in
  v*)
    VERSION="${VERSION_TAG#v}"
    TAG="$VERSION_TAG"
    ;;
  *)
    VERSION="$VERSION_TAG"
    TAG="v${VERSION_TAG}"
    ;;
esac

TARGET=$(detect_target)
ARCHIVE_NAME="${SERVER_BINARY_NAME}-${VERSION}-${TARGET}.tar.gz"
DOWNLOAD_URL="${GITHUB_RELEASE_BASE}/${TAG}/${ARCHIVE_NAME}"
CHECKSUM_URL="${DOWNLOAD_URL}.sha256"

if [ -z "$INSTALL_DIR" ]; then
  if [ -d "$DEFAULT_SYSTEM_INSTALL_DIR" ] && [ -w "$DEFAULT_SYSTEM_INSTALL_DIR" ]; then
    INSTALL_DIR="$DEFAULT_SYSTEM_INSTALL_DIR"
  else
    INSTALL_DIR="$DEFAULT_USER_INSTALL_DIR"
  fi
fi

TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/seagrass-install.XXXXXX")
ARCHIVE_PATH="${TMP_DIR}/${ARCHIVE_NAME}"
CHECKSUM_PATH="${ARCHIVE_PATH}.sha256"

download_file "$DOWNLOAD_URL" "$ARCHIVE_PATH"
download_file "$CHECKSUM_URL" "$CHECKSUM_PATH"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$TMP_DIR" && sha256sum -c "${ARCHIVE_NAME}.sha256")
elif command -v shasum >/dev/null 2>&1; then
  (cd "$TMP_DIR" && shasum -a 256 -c "${ARCHIVE_NAME}.sha256")
else
  echo "sha256sum or shasum is required" >&2
  exit 1
fi

tar -xzf "$ARCHIVE_PATH" -C "$TMP_DIR"
BINARY_PATH=$(find "$TMP_DIR" -type f -name "$SERVER_BINARY_NAME" -print | sed -n '1p')
if [ -z "$BINARY_PATH" ]; then
  echo "release archive did not contain ${SERVER_BINARY_NAME}" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
INSTALL_PATH="${INSTALL_DIR}/${SERVER_BINARY_NAME}"
cp "$BINARY_PATH" "$INSTALL_PATH"
chmod 755 "$INSTALL_PATH"

echo "Seagrass ${VERSION} installed to ${INSTALL_PATH}. Run: seagrass --version"
