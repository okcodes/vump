#!/usr/bin/env bash
# test.sh — Run vet and all tests inside Docker.
#
# Usage: ./test.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

IMAGE="golang:1.26-alpine3.23"

echo "Running go vet and go test inside ${IMAGE}..."
echo ""

docker run --rm \
  -v "$SCRIPT_DIR":/src \
  -v vump-mod-cache:/go/pkg/mod \
  -v vump-build-cache:/root/.cache/go-build \
  -w /src \
  "$IMAGE" \
  sh -c "go vet ./... && go test ./... -v"
