#!/usr/bin/env bash
# build.sh — Cross-platform release builds using Docker.
#
# Produces binaries in dist/ for all supported platforms using
# golang:1.26-alpine3.23. Docker must be running.
#
# Usage: ./build.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Read version from VERSION file (first line, trimmed).
VERSION="$(head -1 VERSION | tr -d '[:space:]')"
if [[ -z "$VERSION" ]]; then
  echo "ERROR: VERSION file is empty or missing" >&2
  exit 1
fi

BINARY_NAME="vump"
IMAGE="golang:1.26-alpine3.23"
DIST_DIR="$SCRIPT_DIR/dist"
LDFLAGS="-s -w -X main.Version=${VERSION}"

mkdir -p "$DIST_DIR"

# Targets: OS ARCH [output suffix]
TARGETS=(
  "linux   amd64  "
  "linux   arm64  "
  "darwin  amd64  "
  "darwin  arm64  "
  "windows amd64  .exe"
  "windows arm64  .exe"
)

echo "Building vump ${VERSION} for all platforms..."
echo ""

for target in "${TARGETS[@]}"; do
  read -r GOOS GOARCH EXT <<< "$target"

  OUTPUT="${DIST_DIR}/${BINARY_NAME}-${GOOS}-${GOARCH}${EXT}"

  printf "  %-20s → %s\n" "${GOOS}/${GOARCH}" "$(basename "$OUTPUT")"

  docker run --rm \
    -v "$SCRIPT_DIR":/src \
    -w /src \
    -e GOOS="$GOOS" \
    -e GOARCH="$GOARCH" \
    -e CGO_ENABLED=0 \
    "$IMAGE" \
    go build -trimpath -ldflags "$LDFLAGS" -o "/src/dist/${BINARY_NAME}-${GOOS}-${GOARCH}${EXT}" .

done

echo ""
echo "Done. Binaries in dist/:"
ls -lh "$DIST_DIR"
