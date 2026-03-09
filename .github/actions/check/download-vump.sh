#!/usr/bin/env bash
# download-vump.sh — Download the vump binary for the current platform and
# install it to /usr/local/bin. Called by the vump composite action.
#
# Requires env vars (set by action.yml):
#   VUMP_VERSION_INPUT — "latest" or a specific tag like "v0.2.0"

set -euo pipefail

OS=$(uname -s | tr '[:upper:]' '[:lower:]')

case "$(uname -m)" in
  x86_64)        ARCH="amd64" ;;
  aarch64|arm64) ARCH="arm64" ;;
  *) echo "::error::Unsupported architecture: $(uname -m)"; exit 1 ;;
esac

# Darwin releases only the universal binary (contains both arm64 + amd64).
if [[ "$OS" == "darwin" ]]; then
  BINARY="vump-darwin-universal"
else
  BINARY="vump-${OS}-${ARCH}"
fi

if [[ "$VUMP_VERSION_INPUT" == "latest" ]]; then
  TAG=$(curl -fsSL https://api.github.com/repos/okcodes/vump/releases/latest \
        | grep '"tag_name"' | head -1 | cut -d'"' -f4)
else
  TAG="$VUMP_VERSION_INPUT"
fi

echo "Downloading vump ${TAG} (${BINARY})…"
curl -fsSLo /usr/local/bin/vump \
  "https://github.com/okcodes/vump/releases/download/${TAG}/${BINARY}"
chmod +x /usr/local/bin/vump
vump --version
