#!/usr/bin/env bash
# build-universal.sh — Assemble a macOS universal binary from arm64 + amd64 slices.
#
# Requires: macOS with Xcode command line tools (lipo is Apple-only).
# Inputs:   dist/vump-darwin-amd64 and dist/vump-darwin-arm64 (from build.sh)
# Output:   dist/vump-darwin-universal
#
# Usage: ./build-universal.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

DIST="$SCRIPT_DIR/dist"
AMD64="$DIST/vump-darwin-amd64"
ARM64="$DIST/vump-darwin-arm64"
UNIVERSAL="$DIST/vump-darwin-universal"

for BIN in "$AMD64" "$ARM64"; do
  if [[ ! -f "$BIN" ]]; then
    echo "ERROR: $BIN not found — run ./build.sh first"
    exit 1
  fi
done

echo "→ Creating universal binary…"
lipo -create -output "$UNIVERSAL" "$AMD64" "$ARM64"
lipo -info "$UNIVERSAL"
echo "  ✓ $UNIVERSAL"
