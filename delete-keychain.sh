#!/usr/bin/env bash
# delete-keychain.sh — Delete an ephemeral Keychain created by import-cert.sh.
# Safe to run even if the keychain was never created (e.g. on early failure).
#
# Requires (env vars):
#   KEYCHAIN_NAME — name of the keychain to delete
#
# Usage: ./delete-keychain.sh

set -euo pipefail

: "${KEYCHAIN_NAME:?}"

security delete-keychain "$KEYCHAIN_NAME" 2>/dev/null && \
  echo "→ Deleted keychain: $KEYCHAIN_NAME" || \
  echo "→ Keychain not found (already deleted or never created): $KEYCHAIN_NAME"
