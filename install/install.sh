#!/bin/sh
set -eu

REPOSITORY="microsoft/termprobe"
BINARY_NAME="termprobe"

fail() {
  echo "Error: $*" >&2
  exit 1
}

command -v uname >/dev/null 2>&1 || fail "uname is required."
command -v tar >/dev/null 2>&1 || fail "tar is required."

case "$(uname -s)" in
  Darwin) OS="apple-darwin" ;;
  Linux) OS="unknown-linux-musl" ;;
  MINGW*|MSYS*|CYGWIN*)
    fail "Use install.ps1 to install termprobe on Windows."
    ;;
  *) fail "Unsupported operating system: $(uname -s)" ;;
esac

case "$(uname -m)" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) fail "Unsupported architecture: $(uname -m)" ;;
esac

TARGET="${ARCH}-${OS}"
ASSET="${BINARY_NAME}-${TARGET}.tar.gz"
VERSION="${TERMPROBE__VERSION:-latest}"
TOKEN="${GITHUB_TOKEN:-${GH_TOKEN:-}}"
TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t termprobe)"
ARCHIVE_PATH="${TMP_DIR}/${ASSET}"
EXTRACT_DIR="${TMP_DIR}/extract"

cleanup() {
  rm -rf -- "$TMP_DIR"
}
trap cleanup EXIT HUP INT TERM

download() {
  url="$1"
  destination="$2"

  if command -v curl >/dev/null 2>&1; then
    if [ -n "$TOKEN" ]; then
      curl --proto '=https' --tlsv1.2 -fsSL \
        -H "Authorization: Bearer ${TOKEN}" \
        "$url" -o "$destination"
    else
      curl --proto '=https' --tlsv1.2 -fsSL "$url" -o "$destination"
    fi
  elif command -v wget >/dev/null 2>&1; then
    if [ -n "$TOKEN" ]; then
      wget -q --header="Authorization: Bearer ${TOKEN}" \
        -O "$destination" "$url"
    else
      wget -q -O "$destination" "$url"
    fi
  else
    fail "curl or wget is required."
  fi
}

if [ -z "$VERSION" ] || [ "$VERSION" = "latest" ]; then
  RELEASE_URL="https://github.com/${REPOSITORY}/releases/latest/download"
elif [ "$VERSION" = "beta" ]; then
  command -v awk >/dev/null 2>&1 || fail "awk is required to resolve beta releases."
  RELEASES_PATH="${TMP_DIR}/releases.json"
  download \
    "https://api.github.com/repos/${REPOSITORY}/releases?per_page=100" \
    "$RELEASES_PATH" ||
    fail "Could not query GitHub releases."

  VERSION="$(
    awk '
      /^    "tag_name": "/ {
        tag = $0
        sub(/^    "tag_name": "/, "", tag)
