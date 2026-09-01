# Temporary overrides

Protections that are currently relaxed to keep development moving. Each entry
records what was weakened, why, and exactly how to restore it.

**Delete this file once the list is empty.**

---

## 1. Release environment approval is disabled

**Where:** repository settings — the `Production-Signing` environment no longer
requires a reviewer.

**Effect:** Any tag push signs and publishes with no human in the loop.

**Why:** Removed to avoid a manual approval on every iteration during the
rewrite.

**To restore:** repository *Settings* → *Environments* → `Production-Signing` →
enable *Required reviewers* and add the account holder.
