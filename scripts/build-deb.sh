#!/usr/bin/env bash
set -euo pipefail

# Build a standalone Debian package (.deb) without requiring third-party tools.
# Works with dpkg-deb if installed, otherwise falls back to standard tar + ar.

BINARY_PATH="${1:-target/release/omt}"
VERSION="${2:-0.1.0}"
ARCH="${3:-amd64}"
OUTPUT_DIR="${4:-dist}"

# Strip leading 'v' if present
VERSION="${VERSION#v}"

# Normalize architecture names
case "$ARCH" in
  x86_64|amd64|x86_64-unknown-linux-gnu|x86_64-unknown-linux-musl)
    DEB_ARCH="amd64"
    ;;
  aarch64|arm64|aarch64-unknown-linux-gnu|aarch64-unknown-linux-musl)
    DEB_ARCH="arm64"
    ;;
  *)
    DEB_ARCH="$ARCH"
    ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ ! -f "$BINARY_PATH" ]; then
  echo "Error: Binary not found at $BINARY_PATH" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"

STAGE_DIR=$(mktemp -d)
trap 'rm -rf "$STAGE_DIR"' EXIT

DEB_ROOT="$STAGE_DIR/deb-root"
mkdir -p "$DEB_ROOT/usr/bin"
mkdir -p "$DEB_ROOT/usr/share/doc/omt/examples"
mkdir -p "$DEB_ROOT/DEBIAN"

# Install files
install -m 0755 "$BINARY_PATH" "$DEB_ROOT/usr/bin/omt"
install -m 0644 "$WORKSPACE_ROOT/README.md" "$DEB_ROOT/usr/share/doc/omt/README.md"
install -m 0644 "$WORKSPACE_ROOT/LICENSE" "$DEB_ROOT/usr/share/doc/omt/copyright"
if [ -f "$WORKSPACE_ROOT/examples/config.toml" ]; then
  install -m 0644 "$WORKSPACE_ROOT/examples/config.toml" "$DEB_ROOT/usr/share/doc/omt/examples/config.toml"
fi

# Calculate installed size in KB
INSTALLED_SIZE=$(du -sk "$DEB_ROOT" | cut -f1)

# Generate control file
cat <<EOF > "$DEB_ROOT/DEBIAN/control"
Package: omt
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${DEB_ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: Open Market Terminal Contributors <https://github.com/MuhammedZohaib/open-market-terminal>
Homepage: https://github.com/MuhammedZohaib/open-market-terminal
Description: Local-first financial research terminal
 Open Market Terminal (OMT) is a native terminal financial research
 workstation featuring high-density market discovery, multi-period SEC
 financial statements, charting, technical overlays, and screening.
EOF

DEB_FILE="$OUTPUT_DIR/omt_${VERSION}_${DEB_ARCH}.deb"

if command -v dpkg-deb >/dev/null 2>&1; then
  dpkg-deb --build "$DEB_ROOT" "$DEB_FILE"
else
  # Manual deb construction: debian-binary, control.tar.gz, data.tar.gz via ar
  echo "2.0" > "$STAGE_DIR/debian-binary"
  
  (cd "$DEB_ROOT/DEBIAN" && tar --owner=0 --group=0 -czf "$STAGE_DIR/control.tar.gz" .)
  (cd "$DEB_ROOT" && tar --exclude='./DEBIAN' --owner=0 --group=0 -czf "$STAGE_DIR/data.tar.gz" ./usr)
  
  ar rcs "$DEB_FILE" "$STAGE_DIR/debian-binary" "$STAGE_DIR/control.tar.gz" "$STAGE_DIR/data.tar.gz"
fi

# Generate SHA256
DEB_FILENAME=$(basename "$DEB_FILE")
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$OUTPUT_DIR" && sha256sum "$DEB_FILENAME" > "$DEB_FILENAME.sha256")
elif command -v shasum >/dev/null 2>&1; then
  (cd "$OUTPUT_DIR" && shasum -a 256 "$DEB_FILENAME" > "$DEB_FILENAME.sha256")
fi

echo "Successfully built Debian package: $DEB_FILE"
