//! Release artifact checksums.
//!
//! Self-update downloads a binary over the network and then executes it as the
//! user, and the CI action does the same inside a job holding signing secrets.
//! That is the highest-privilege path in the project, so what arrives is
//! checked against what was published before it is allowed to run.
//!
//! The format is the one `sha256sum` and `shasum -a 256` write, so the same
//! published file verifies from a shell script and from this crate.

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Digests published alongside a release, keyed by artifact name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Checksums {
    entries: Vec<(String, String)>,
}

/// Why an artifact could not be trusted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChecksumError {
    /// The artifact's contents do not match what was published.
    #[error(
        "{asset} does not match its published checksum\n  expected {expected}\n  received {actual}\n\nthe download was corrupted or tampered with; nothing was installed"
    )]
    Mismatch {
        /// The artifact being verified.
        asset: String,
        /// The published digest.
        expected: String,
        /// The digest of what actually arrived.
        actual: String,
    },

    /// The published checksums do not cover this artifact.
    #[error("no published checksum for {asset}; refusing to install an unverified binary")]
    NotListed {
        /// The artifact with no digest.
        asset: String,
    },
}

impl Checksums {
    /// Reads a `sha256sum`-style listing.
    ///
    /// Each line is a hex digest, whitespace, then a filename. The separator is
    /// conventionally two spaces, or a space and an asterisk for binary mode;
    /// both are accepted, as is any surrounding whitespace. Lines that do not
    /// fit are skipped rather than rejected, so a comment or a blank line does
    /// not invalidate an otherwise usable file.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let entries = text
            .lines()
            .filter_map(|line| {
                let (digest, name) = line.trim().split_once(char::is_whitespace)?;
                if digest.is_empty() || !is_hex_digest(digest) {
                    return None;
                }
                // Binary mode marks the name with a leading asterisk.
                let name = name.trim().trim_start_matches('*').trim();
                if name.is_empty() {
                    return None;
                }
                Some((name.to_owned(), digest.to_ascii_lowercase()))
            })
            .collect();

        Self { entries }
    }

    /// The published digest for an artifact.
    #[must_use]
    pub fn get(&self, asset: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(name, _)| name == asset)
            .map(|(_, digest)| digest.as_str())
    }

    /// Whether any digest was read.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Confirms that `bytes` are what was published for `asset`.
    ///
    /// # Errors
    ///
    /// Returns [`ChecksumError::NotListed`] when the artifact has no published
    /// digest, and [`ChecksumError::Mismatch`] when the contents differ from it.
    pub fn verify(&self, asset: &str, bytes: &[u8]) -> Result<(), ChecksumError> {
        let expected = self.get(asset).ok_or_else(|| ChecksumError::NotListed {
            asset: asset.to_owned(),
        })?;

        let actual = sha256_hex(bytes);
        if actual == expected {
            return Ok(());
        }

        Err(ChecksumError::Mismatch {
            asset: asset.to_owned(),
            expected: expected.to_owned(),
            actual,
        })
    }
}

/// The SHA-256 of `bytes`, lowercase hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        // Writing to a String is infallible.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Whether a token looks like a SHA-256 digest.
fn is_hex_digest(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The SHA-256 of the empty input, from the standard test vectors.
    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    /// The SHA-256 of "abc", from the standard test vectors.
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn hashes_match_the_published_test_vectors() {
        // Verifying against known vectors rather than against ourselves: a
        // self-consistent but wrong hash would verify happily and protect
        // nothing.
        assert_eq!(sha256_hex(b""), EMPTY);
        assert_eq!(sha256_hex(b"abc"), ABC);
    }

    #[test]
    fn reads_the_conventional_two_space_format() {
        let text = format!("{ABC}  vump-linux-amd64\n{EMPTY}  vump-darwin-universal\n");
        let sums = Checksums::parse(&text);

        assert_eq!(sums.get("vump-linux-amd64"), Some(ABC));
        assert_eq!(sums.get("vump-darwin-universal"), Some(EMPTY));
        assert_eq!(sums.get("vump-windows-amd64.exe"), None);
    }

    #[test]
    fn reads_binary_mode_and_odd_spacing() {
        let text = format!("  {ABC} *vump-linux-amd64  \n\t{EMPTY}\tvump-linux-arm64\n");
        let sums = Checksums::parse(&text);

        assert_eq!(sums.get("vump-linux-amd64"), Some(ABC));
        assert_eq!(sums.get("vump-linux-arm64"), Some(EMPTY));
    }

    #[test]
    fn digests_are_compared_case_insensitively() {
        let text = format!("{}  vump-linux-amd64\n", ABC.to_ascii_uppercase());
        let sums = Checksums::parse(&text);

        assert!(sums.verify("vump-linux-amd64", b"abc").is_ok());
    }

    #[test]
    fn unusable_lines_do_not_invalidate_the_rest() {
        let text =
            format!("# a comment\n\nnot-a-digest  vump-linux-amd64\n{ABC}  vump-linux-arm64\n");
        let sums = Checksums::parse(&text);

        assert_eq!(sums.get("vump-linux-amd64"), None);
        assert_eq!(sums.get("vump-linux-arm64"), Some(ABC));
    }

    #[test]
    fn an_empty_listing_is_recognized() {
        assert!(Checksums::parse("").is_empty());
        assert!(Checksums::parse("# nothing here\n").is_empty());
        assert!(!Checksums::parse(&format!("{ABC}  a\n")).is_empty());
    }

    #[test]
    fn matching_contents_verify() {
        let sums = Checksums::parse(&format!("{ABC}  vump-linux-amd64\n"));
        assert!(sums.verify("vump-linux-amd64", b"abc").is_ok());
    }

    #[test]
    fn altered_contents_are_refused() {
        let sums = Checksums::parse(&format!("{ABC}  vump-linux-amd64\n"));

        // A single byte's difference must be caught.
        let err = sums.verify("vump-linux-amd64", b"abd").unwrap_err();
        let ChecksumError::Mismatch {
            expected, actual, ..
        } = err
        else {
            panic!("expected a mismatch, got {err:?}");
        };
        assert_eq!(expected, ABC);
        assert_ne!(actual, ABC);
    }

    #[test]
    fn truncated_contents_are_refused() {
        let sums = Checksums::parse(&format!("{ABC}  vump-linux-amd64\n"));
        assert!(sums.verify("vump-linux-amd64", b"ab").is_err());
        assert!(sums.verify("vump-linux-amd64", b"").is_err());
    }

    #[test]
    fn an_artifact_with_no_digest_is_refused_rather_than_allowed() {
        // Silently accepting an unlisted artifact would defeat the point.
        let sums = Checksums::parse(&format!("{ABC}  vump-linux-amd64\n"));
        let err = sums.verify("vump-windows-amd64.exe", b"abc").unwrap_err();
        assert!(matches!(err, ChecksumError::NotListed { .. }));
    }
}
