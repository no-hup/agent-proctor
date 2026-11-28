#!/usr/bin/env sh
set -e

# agent-proctor installer
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/no-hup/agent-proctor/main/install.sh | sh
# Options via environment variables:
#   PROCTOR_VERSION: e.g. "v0.2.3" (defaults to latest)
#   PROCTOR_INSTALL_DIR: directory to install binary into (e.g. /usr/local/bin or ~/.local/bin)

REPO="no-hup/agent-proctor"

# 1. Detect OS
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
case "$OS" in
  linux*)  TARGET_OS="linux" ;;
  darwin*) TARGET_OS="darwin" ;;
  *)
    echo "Error: Unsupported operating system: $OS. agent-proctor currently supports Linux and macOS." >&2
    exit 1
    ;;
esac

# 2. Detect Architecture
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64)   TARGET_ARCH="x86_64" ;;
  aarch64|arm64)  TARGET_ARCH="aarch64" ;;
  *)
    echo "Error: Unsupported architecture: $ARCH. agent-proctor currently supports x86_64 and aarch64." >&2
    exit 1
    ;;
esac

# 3. Determine Version
VERSION="${PROCTOR_VERSION:-latest}"
if [ "$VERSION" = "latest" ]; then
  TAR_URL="https://github.com/${REPO}/releases/latest/download/proctor-${TARGET_OS}-${TARGET_ARCH}.tar.gz"
else
  # Ensure version has 'v' prefix if omitted
  case "$VERSION" in
    v*) TAG="$VERSION" ;;
    *)  TAG="v$VERSION" ;;
  esac
  TAR_URL="https://github.com/${REPO}/releases/download/${TAG}/proctor-${TARGET_OS}-${TARGET_ARCH}.tar.gz"
fi

# 4. Determine Destination Directory
if [ -n "$PROCTOR_INSTALL_DIR" ]; then
  INSTALL_DIR="$PROCTOR_INSTALL_DIR"
elif [ -w "/usr/local/bin" ]; then
  INSTALL_DIR="/usr/local/bin"
else
  INSTALL_DIR="${HOME}/.local/bin"
fi

mkdir -p "$INSTALL_DIR"

# 5. Download and Extract
TMPDIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'proctor-install')"
trap 'rm -rf "$TMPDIR"' EXIT INT TERM

echo "==> Downloading agent-proctor (${TARGET_OS}-${TARGET_ARCH}) from ${TAR_URL}..."

if command -v curl >/dev/null 2>&1; then
  curl -sSfL "$TAR_URL" -o "${TMPDIR}/proctor.tar.gz"
  curl -sSfL "${TAR_URL}.sha256" -o "${TMPDIR}/proctor.tar.gz.sha256" 2>/dev/null || true
elif command -v wget >/dev/null 2>&1; then
  wget -qO "${TMPDIR}/proctor.tar.gz" "$TAR_URL"
  wget -qO "${TMPDIR}/proctor.tar.gz.sha256" "${TAR_URL}.sha256" 2>/dev/null || true
else
  echo "Error: neither curl nor wget found. Please install curl or wget." >&2
  exit 1
fi

# Verify SHA-256 checksum if checksum file is present
if [ -s "${TMPDIR}/proctor.tar.gz.sha256" ]; then
  EXPECTED_HASH="$(awk '{print $1}' "${TMPDIR}/proctor.tar.gz.sha256")"
  if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_HASH="$(sha256sum "${TMPDIR}/proctor.tar.gz" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    ACTUAL_HASH="$(shasum -a 256 "${TMPDIR}/proctor.tar.gz" | awk '{print $1}')"
  else
    ACTUAL_HASH=""
  fi
  if [ -n "$ACTUAL_HASH" ]; then
    if [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
      echo "Error: SHA-256 checksum mismatch! Expected: $EXPECTED_HASH, Got: $ACTUAL_HASH" >&2
      exit 1
    fi
    echo "==> Verified SHA-256 checksum ($ACTUAL_HASH)"
  fi
fi

echo "==> Extracting binary..."
tar -xzf "${TMPDIR}/proctor.tar.gz" -C "$TMPDIR"

if [ ! -f "${TMPDIR}/proctor" ]; then
  echo "Error: archive did not contain 'proctor' executable binary." >&2
  exit 1
fi

chmod +x "${TMPDIR}/proctor"
mv "${TMPDIR}/proctor" "${INSTALL_DIR}/proctor"

echo "==> Installed proctor to ${INSTALL_DIR}/proctor"

# 6. Check PATH
case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo ""
    echo "Notice: ${INSTALL_DIR} is not currently in your PATH."
    echo "Add it to your shell profile by running:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac

echo ""
echo "agent-proctor installed successfully!"
echo "Run 'proctor --help' or 'proctor scan --base main' to get started."
