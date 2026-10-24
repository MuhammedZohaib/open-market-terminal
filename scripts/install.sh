#!/usr/bin/env bash
set -euo pipefail

# Open Market Terminal (OMT) — One-line installer for Linux and macOS.
# Usage:
#   curl -proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/MuhammedZohaib/open-market-terminal/main/scripts/install.sh | bash
# Options via env variables:
#   VERSION="v0.1.0"      # Specify exact release version
#   INSTALL_DIR="/path"   # Specify install directory (default: /usr/local/bin or ~/.local/bin)

REPO="MuhammedZohaib/open-market-terminal"

# Detect OS
OS="$(uname -s)"
case "$OS" in
  Linux)
    OS_TYPE="linux"
    ;;
  Darwin)
    OS_TYPE="apple-darwin"
    ;;
  *)
    echo "Error: Unsupported operating system: $OS" >&2
    exit 1
    ;;
esac

# Detect Architecture
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64)
    ARCH_TYPE="x86_64"
    ;;
  aarch64|arm64)
    ARCH_TYPE="aarch64"
    ;;
  *)
    echo "Error: Unsupported CPU architecture: $ARCH" >&2
    exit 1
    ;;
esac

# Detect Linux libc (glibc vs musl)
LIBC="gnu"
if [ "$OS_TYPE" = "linux" ]; then
  if ldd --version 2>&1 | grep -iq musl || [ -f /lib/ld-musl-x86_64.so.1 ] || [ -f /lib/ld-musl-aarch64.so.1 ]; then
    LIBC="musl"
  fi
  TARGET="${ARCH_TYPE}-unknown-linux-${LIBC}"
else
  TARGET="${ARCH_TYPE}-${OS_TYPE}"
fi

echo "==> Detected target: $TARGET"

# Resolve Version
if [ -z "${VERSION:-}" ]; then
  echo "==> Resolving latest release from GitHub ($REPO)..."
  if command -v curl >/dev/null 2>&1; then
    LATEST_JSON=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || echo "")
  elif command -v wget >/dev/null 2>&1; then
    LATEST_JSON=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || echo "")
  else
    echo "Error: curl or wget is required to install." >&2
    exit 1
  fi

  if [ -n "$LATEST_JSON" ]; then
    VERSION=$(echo "$LATEST_JSON" | grep -m 1 '"tag_name":' | cut -d'"' -f4 || echo "")
  fi

  if [ -z "$VERSION" ]; then
    VERSION="v0.1.0"
    echo "    (Could not query GitHub API; falling back to default $VERSION)"
  else
    echo "    Latest release: $VERSION"
  fi
fi

# Ensure leading 'v'
if [[ ! "$VERSION" =~ ^v ]]; then
  VERSION="v$VERSION"
fi

# Resolve Installation Directory
if [ -z "${INSTALL_DIR:-}" ]; then
  if [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="$HOME/.local/bin"
  fi
fi

mkdir -p "$INSTALL_DIR"

TAR_NAME="omt-${VERSION}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${TAR_NAME}"
SHA_URL="${DOWNLOAD_URL}.sha256"

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "==> Downloading $TAR_NAME..."
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$TAR_NAME"
  curl -fsSL "$SHA_URL" -o "$TMP_DIR/${TAR_NAME}.sha256" || true
elif command -v wget >/dev/null 2>&1; then
  wget -q "$DOWNLOAD_URL" -O "$TMP_DIR/$TAR_NAME"
  wget -q "$SHA_URL" -O "$TMP_DIR/${TAR_NAME}.sha256" || true
fi

# Checksum verification if sha file was downloaded
if [ -f "$TMP_DIR/${TAR_NAME}.sha256" ]; then
  echo "==> Verifying SHA256 checksum..."
  EXPECTED_HASH=$(cut -d' ' -f1 "$TMP_DIR/${TAR_NAME}.sha256")
  if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_HASH=$(sha256sum "$TMP_DIR/$TAR_NAME" | cut -d' ' -f1)
  elif command -v shasum >/dev/null 2>&1; then
    ACTUAL_HASH=$(shasum -a 256 "$TMP_DIR/$TAR_NAME" | cut -d' ' -f1)
  else
    ACTUAL_HASH=""
  fi

  if [ -n "$ACTUAL_HASH" ]; then
    if [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
      echo "Error: Checksum verification failed!" >&2
      echo "  Expected: $EXPECTED_HASH" >&2
      echo "  Actual:   $ACTUAL_HASH" >&2
      exit 1
    fi
    echo "    Checksum verified: OK"
  fi
fi

echo "==> Extracting into $INSTALL_DIR..."
tar -xzf "$TMP_DIR/$TAR_NAME" -C "$TMP_DIR"
cp "$TMP_DIR/omt-${VERSION}-${TARGET}/omt" "$INSTALL_DIR/omt"
chmod +x "$INSTALL_DIR/omt"

echo "==> Successfully installed Open Market Terminal (OMT) to $INSTALL_DIR/omt"

# PATH warning check
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo ""
    echo "Notice: $INSTALL_DIR is not currently in your PATH."
    echo "Add the following line to your ~/.bashrc or ~/.zshrc:"
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
    ;;
esac

echo ""
echo "Quickstart:"
echo "  omt                   # Launch interactive terminal"
echo "  omt BTCUSDT           # Launch with BTCUSDT chart"
echo "  omt --help            # View all options"
