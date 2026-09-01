#!/usr/bin/env bash
# notarize.sh — Sign and notarize macOS binaries for distribution.
#
# Requires:
#   APPLE_ID                   — Your Apple ID email
#   APP_SPECIFIC_PASSWORD_VUMP — App-specific password from appleid.apple.com
#   APPLE_TEAM_ID              — Your 10-char Apple team ID
#
# Signs/notarizes:
#   - dist/vump-darwin-universal (signed + notarized)
#
# Does not sign/notarize:
#   - dist/vump-darwin-arm64
#   - dist/vump-darwin-amd64
#
# Run ./build.sh then ./build-universal.sh before this script to create the binaries.
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

# UTC timestamp + PID: unique per invocation, shared across all binaries in this run.
RUN_ID="$(date -u +%Y%m%dT%H%M%S)-$$"

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
  local zip="$DIST/notary-$(basename "$bin")-${RUN_ID}.zip"
  local result_file="$DIST/notary-result-$(basename "$bin")-${RUN_ID}.json"

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
    --output-format json | tee "$result_file"

  # Check submission result.
  local status
  status=$(jq -r '.status' "$result_file")
  if [[ "$status" != "Accepted" ]]; then
    echo "  ✗ Notarization FAILED (status: $status)"
    echo "  Fetching full log…"
    local sub_id
    sub_id=$(jq -r '.id' "$result_file")
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

  # Stapling embeds the notarization ticket directly into the distributed file so
  # Gatekeeper can verify offline without phoning Apple. However, stapling is only
  # supported for .app bundles, .dmg disk images, and .pkg installers — NOT for
  # bare Mach-O executables. Attempting it returns Error 73 (errSecCSNotSupported).
  # For bare binaries, Gatekeeper falls back to an online check on first launch.
  # No action needed here. Notarization is still valid.
  # echo "→ Stapling ticket to: $(basename "$bin")"
  # if xcrun stapler staple "$bin" 2>&1; then
  #   echo "  ✓ Stapled successfully"
  # else
  #   echo "  ⚠ Staple failed"
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

# ── Code sign and notarize ────────────────────────────────────────────────────
# Only the universal binary is signed and notarized. The arm64 and amd64 binaries
# were already merged into it by lipo and are not processed further.
# If you ever need to notarize them independently, uncomment them below.
# Note: Apple enforces a rate limit of 75 notarization submissions per day per Team ID.
# So, that's why amd64 and arm64 are commented-out targets as notarizing universal only
# is more convenient as it only requires one notarization submission.
TARGETS=(
  # "$AMD64"
  # "$ARM64"
  "$UNIVERSAL"
)

# Binaries must exist
for BIN in "${TARGETS[@]}"; do
  if [[ ! -f "$BIN" ]]; then
    echo "ERROR: $BIN not found"
    echo "       Run ./build.sh then ./build-universal.sh first"
    exit 1
  fi
done

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
