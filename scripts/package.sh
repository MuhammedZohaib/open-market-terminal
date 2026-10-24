#!/usr/bin/env bash
set -euo pipefail

# Package a release tarball (.tar.gz) and optional Debian package for Linux/macOS.

VERSION=""
TARGET=""
BINARY_PATH=""
OUTPUT_DIR="dist"
BUILD_DEB=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --target)
      TARGET="$2"
      shift 2
      ;;
    --binary)
      BINARY_PATH="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --deb)
      BUILD_DEB=true
      shift
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Resolve version
if [ -z "$VERSION" ]; then
  VERSION=$(grep -m 1 -oE 'version\s*=\s*"[^"]+"' "$WORKSPACE_ROOT/Cargo.toml" | cut -d'"' -f2 || echo "0.1.0")
fi
# Ensure leading 'v'
if [[ ! "$VERSION" =~ ^v ]]; then
  VERSION="v$VERSION"
fi

# Detect default target if not passed
if [ -z "$TARGET" ]; then
  UNAME_S=$(uname -s)
  UNAME_M=$(uname -m)
  case "$UNAME_S" in
    Linux)
      if [ "$UNAME_M" = "aarch64" ]; then
        TARGET="aarch64-unknown-linux-gnu"
      else
        TARGET="x86_64-unknown-linux-gnu"
      fi
      ;;
    Darwin)
      if [ "$UNAME_M" = "arm64" ]; then
        TARGET="aarch64-apple-darwin"
      else
        TARGET="x86_64-apple-darwin"
      fi
      ;;
    *)
      TARGET="unknown-target"
      ;;
  esac
fi

# Locate binary if not passed
if [ -z "$BINARY_PATH" ]; then
  if [ -f "$WORKSPACE_ROOT/target/$TARGET/release/omt" ]; then
    BINARY_PATH="$WORKSPACE_ROOT/target/$TARGET/release/omt"
  elif [ -f "$WORKSPACE_ROOT/target/release/omt" ]; then
    BINARY_PATH="$WORKSPACE_ROOT/target/release/omt"
  else
    echo "Error: Binary 'omt' not found. Specify --binary or compile first." >&2
    exit 1
  fi
fi

mkdir -p "$OUTPUT_DIR"
PACKAGE_NAME="omt-${VERSION}-${TARGET}"
STAGE_DIR="$OUTPUT_DIR/$PACKAGE_NAME"

rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/examples"

echo "Staging release package: $PACKAGE_NAME"
cp "$BINARY_PATH" "$STAGE_DIR/omt"
chmod +x "$STAGE_DIR/omt"
cp "$WORKSPACE_ROOT/README.md" "$STAGE_DIR/README.md"
cp "$WORKSPACE_ROOT/LICENSE" "$STAGE_DIR/LICENSE"
cp "$WORKSPACE_ROOT/THIRD_PARTY_NOTICES.md" "$STAGE_DIR/THIRD_PARTY_NOTICES.md"
if [ -f "$WORKSPACE_ROOT/examples/config.toml" ]; then
  cp "$WORKSPACE_ROOT/examples/config.toml" "$STAGE_DIR/examples/config.toml"
fi

TAR_NAME="${PACKAGE_NAME}.tar.gz"
TAR_PATH="$OUTPUT_DIR/$TAR_NAME"

echo "Creating tarball: $TAR_PATH"
(cd "$OUTPUT_DIR" && tar -czf "$TAR_NAME" "$PACKAGE_NAME")

# Compute SHA256 checksum
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$OUTPUT_DIR" && sha256sum "$TAR_NAME" > "${TAR_NAME}.sha256")
  HASH=$(cut -d' ' -f1 "$OUTPUT_DIR/${TAR_NAME}.sha256")
elif command -v shasum >/dev/null 2>&1; then
  (cd "$OUTPUT_DIR" && shasum -a 256 "$TAR_NAME" > "${TAR_NAME}.sha256")
  HASH=$(cut -d' ' -f1 "$OUTPUT_DIR/${TAR_NAME}.sha256")
else
  echo "Warning: Neither sha256sum nor shasum found, skipping checksum."
  HASH="unknown"
fi

# Clean up stage dir
rm -rf "$STAGE_DIR"

echo "Successfully packaged $TAR_NAME (SHA256: $HASH)"

# Build Debian package if requested
if [ "$BUILD_DEB" = true ] && [[ "$TARGET" =~ linux ]]; then
  bash "$SCRIPT_DIR/build-deb.sh" "$BINARY_PATH" "$VERSION" "$TARGET" "$OUTPUT_DIR"
fi
