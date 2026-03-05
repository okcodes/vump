#!/usr/bin/env bash
# notarize.sh — Sign and notarize macOS binaries for distribution.
#
# Requires:
#   APPLE_ID                   — Your Apple ID email
#   APP_SPECIFIC_PASSWORD_VUMP — App-specific password from appleid.apple.com
#   APPLE_TEAM_ID              — Your 10-char Apple team ID
#
# Produces: signed + notarized arm64, amd64, and universal binaries.
#
# Usage:
#   ./notarize.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# ── Config ────────────────────────────────────────────────────────────────────
APPLE_ID="${APPLE_ID:?'APPLE_ID env var is required'}"
PASSWORD="${APP_SPECIFIC_PASSWORD_VUMP:?'APP_SPECIFIC_PASSWORD_VUMP env var is required'}"
TEAM_ID="${APPLE_TEAM_ID:?'APPLE_TEAM_ID env var is required'}"

# Derive the signing identity from Keychain — no name is hardcoded.
# Looks for the first valid Developer ID Application cert matching your team.
IDENTITY=$(security find-identity -v -p codesigning \
  | grep "Developer ID Application" \
  | grep "($TEAM_ID)" \
  | head -1 \
  | sed 's/.*"\(.*\)"/\1/')

if [[ -z "$IDENTITY" ]]; then
  echo "ERROR: No 'Developer ID Application' cert found in Keychain for team $TEAM_ID"
  echo "       Download it from developer.apple.com → Certificates"
  exit 1
fi

DIST="$SCRIPT_DIR/dist"
AMD64="$DIST/vump-darwin-amd64"
ARM64="$DIST/vump-darwin-arm64"
UNIVERSAL="$DIST/vump-darwin-universal"

# ── Helpers ───────────────────────────────────────────────────────────────────
sign_binary() {
  local bin="$1"
  echo "→ Signing: $(basename "$bin")"
  codesign \
    --sign "$IDENTITY" \
    --options runtime \
    --timestamp \
    --force \
    "$bin"
  codesign --verify --strict --verbose=2 "$bin"
  echo "  ✓ Signature verified"
}

notarize_binary() {
  local bin="$1"
  local zip="${bin}.zip"

  echo "→ Zipping for notarization: $(basename "$zip")"
  # Remove previous zip if any.
  # Note: do NOT use --keepParent on bare files — it embeds the full path into
  # the zip, which can confuse Apple's scanner. Use -k only (store paths as-is).
  rm -f "$zip"
  ditto -c -k "$bin" "$zip"

  echo "→ Submitting to Apple notarization (this can take 1-5 min)…"
  xcrun notarytool submit "$zip" \
    --apple-id "$APPLE_ID" \
    --password "$PASSWORD" \
    --team-id "$TEAM_ID" \
    --wait \
    --output-format json | tee /tmp/notary-result.json

  # Check submission result
  local status
  status=$(jq -r '.status' /tmp/notary-result.json)
  if [[ "$status" != "Accepted" ]]; then
    echo "  ✗ Notarization FAILED (status: $status)"
    echo "  Fetching full log…"
    local sub_id
    sub_id=$(jq -r '.id' '/tmp/notary-result.json')
    if [[ -n "$sub_id" ]]; then
      xcrun notarytool log "$sub_id" \
        --apple-id "$APPLE_ID" \
        --password "$PASSWORD" \
        --team-id "$TEAM_ID"
    fi
    exit 1
  fi
  echo "  ✓ Notarization accepted"
  rm -f "$zip"

  echo "→ Stapling ticket to: $(basename "$bin")"
  # Note: Apple stapling of bare binaries (non-.app/.pkg) often fails —
  # this is expected. The binary is still notarized; Gatekeeper verifies
  # online. Stapling just embeds the ticket so offline verification works.
  #
  # Since stapling binaries is not supported in bare binaries we don't even try it.
  # So, stapling command bellow is commented out.
  #
  # if xcrun stapler staple "$bin" 2>&1; then
  #   echo "  ✓ Stapled successfully"
  # else
  #   echo "  ⚠ Staple failed (normal for bare CLI binaries — notarization still valid)"
  # fi
}

# ── Main ──────────────────────────────────────────────────────────────────────
echo ""
echo "========================================"
echo "  vump macOS Notarization"
echo "  Team:     ${TEAM_ID}"
echo "  Identity: ${IDENTITY}"
echo "========================================"
echo ""

# Verify individual binaries exist before doing anything else.
for BIN in "$AMD64" "$ARM64"; do
  if [[ ! -f "$BIN" ]]; then
    echo "ERROR: Binary not found: $BIN"
    echo "       Run ./build.sh first"
    exit 1
  fi
done

# ── Build Universal Binary ────────────────────────────────────────────────────
echo "→ Creating universal binary with lipo…"
lipo -create -output "$UNIVERSAL" "$AMD64" "$ARM64"
lipo -info "$UNIVERSAL"

# ── Code sign and notarize ────────────────────────────────────────────────────
TARGETS=(
  # Since Apple allows only 75 notarizations submissions per day per Team ID
  # we're only notarizing the universal app version.
  # So we comment out amd64 and arm64 so they don't get notarized.
  # "$AMD64"
  # "$ARM64"
  "$UNIVERSAL"
)

for BIN in "${TARGETS[@]}"; do
  echo ""
  sign_binary "$BIN"
  notarize_binary "$BIN"
done

echo ""
echo "========================================"
echo "  All done! ✓"
echo "========================================"
echo ""
echo "Final verification (codesign chain):"
for BIN in "${TARGETS[@]}"; do
  if [[ ! -f "$BIN" ]]; then continue; fi
  echo ""
  echo "  $(basename "$BIN"):"
  codesign -dv --verbose=4 "$BIN" 2>&1 \
    | grep -E "TeamIdentifier|Authority|flags|Timestamp" \
    | sed 's/^/    /'
  codesign --verify --strict "$BIN" && echo "    ✓ Signature OK"
done
