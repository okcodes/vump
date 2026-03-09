#!/usr/bin/env bash
# download-vump.sh — Download the vump binary for the current platform.
#
# Downloads to $RUNNER_TEMP (writable on both GitHub-hosted and self-hosted
# runners, cleaned up after the run, never touches system paths).
# Exports VUMP_BIN to $GITHUB_ENV so subsequent steps reference the exact
# binary path without relying on PATH.
#
# Requires env vars:
#   VUMP_VERSION_INPUT — "latest" or a specific tag like "v0.2.0"
#   RUNNER_TEMP        — set by GitHub Actions runtime (always defined)
#   GITHUB_ENV         — set by GitHub Actions runtime (always defined)

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

DEST="${RUNNER_TEMP}/vump-${TAG}"
echo "Downloading vump ${TAG} (${BINARY}) → ${DEST}"
curl -fsSLo "$DEST" \
  "https://github.com/okcodes/vump/releases/download/${TAG}/${BINARY}"
chmod +x "$DEST"

# Export the full path — next steps use $VUMP_BIN directly, never assume PATH.
echo "VUMP_BIN=${DEST}" >> "$GITHUB_ENV"
"$DEST" --version
