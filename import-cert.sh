#!/usr/bin/env bash
# import-cert.sh — Import a Developer ID cert from a base64-encoded P12 into
# an ephemeral Keychain for the duration of a CI job. This keeps the login keychain
# untouched and the cert is automatically gone when the CI job cleans up.
#
# Requires (env vars):
#   APPLE_CERT_P12       — base64-encoded P12 file
#   APPLE_CERT_PASSWORD  — password the P12 was exported with
#   KEYCHAIN_PASSWORD    — password for the new ephemeral keychain
#   KEYCHAIN_NAME        — name of the keychain to create (e.g. ci-signing-123.keychain)
#
# Usage: ./import-cert.sh

set -euo pipefail

: "${APPLE_CERT_P12:?}"
: "${APPLE_CERT_PASSWORD:?}"
: "${KEYCHAIN_PASSWORD:?}"
: "${KEYCHAIN_NAME:?}"

echo "→ Creating ephemeral keychain: $KEYCHAIN_NAME"
security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_NAME"
security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_NAME"
security set-keychain-settings -lut 3600 "$KEYCHAIN_NAME"

# Prepend to the search list so codesign can find the identity.
security list-keychains -d user -s "$KEYCHAIN_NAME" \
  $(security list-keychains -d user | tr -d '"')

echo "→ Importing certificate…"
echo "$APPLE_CERT_P12" | base64 --decode > /tmp/cert.p12
security import /tmp/cert.p12 \
  -k "$KEYCHAIN_NAME" \
  -P "$APPLE_CERT_PASSWORD" \
  -T /usr/bin/codesign \
  -T /usr/bin/security
rm /tmp/cert.p12

# Allow codesign to use the private key without an interactive prompt.
security set-key-partition-list \
  -S apple-tool:,apple: \
  -s \
  -k "$KEYCHAIN_PASSWORD" \
  "$KEYCHAIN_NAME"

echo "→ Identities available:"
security find-identity -v -p codesigning "$KEYCHAIN_NAME"
