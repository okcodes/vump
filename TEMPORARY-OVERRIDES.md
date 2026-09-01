# Temporary overrides

Protections that are currently relaxed to keep development moving. Each entry
records what was weakened, why, and exactly how to restore it.

**Delete this file once the list is empty.**

---

## 1. macOS notarization is skipped

**Where:** `.github/workflows/release.yml` sets `VUMP_SKIP_NOTARIZATION: '1'`
on the *Sign and notarize* step.

**Effect:** Release binaries are still code-signed with the Developer ID
certificate, but are not submitted to Apple's notary service. Gatekeeper falls
back to an online check and warns the user on first launch. Released macOS
binaries are therefore **not suitable for distribution to others** while this
is in place.

**Why:** Apple's notary service rejects submissions for this team:

```
HTTP status code: 403. A required agreement is missing or has expired.
```

**Action required (account holder only):**

1. Sign in at <https://developer.apple.com/account>.
2. Accept any pending agreement shown in the banner or under *Membership* —
   this is normally the Apple Developer Program License Agreement, which Apple
   revises periodically and which blocks notarization until accepted.
3. Check <https://appstoreconnect.apple.com> → *Business* → *Agreements* for
   anything still marked as pending.
4. Confirm the membership itself has not lapsed; it renews annually.

**To restore:** delete the `VUMP_SKIP_NOTARIZATION` line from
`.github/workflows/release.yml`, then re-run a release and confirm the
*Sign and notarize* step reports `✓ Notarization accepted`.

---

## 2. Release environment approval is disabled

**Where:** repository settings — the `Production-Signing` environment no longer
requires a reviewer.

**Effect:** Any tag push signs and publishes with no human in the loop.

**Why:** Removed to avoid a manual approval on every iteration during the
rewrite.

**To restore:** repository *Settings* → *Environments* → `Production-Signing` →
enable *Required reviewers* and add the account holder.
