//! Process exit codes.
//!
//! These are part of vump's public contract. Callers — CI steps, scripts, and
//! other tools — branch on them, so a code's meaning must not change once
//! published.

/// A process exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Exit {
    /// The operation completed and any verification passed.
    Success = 0,
    /// An unexpected failure with no more specific code.
    Failure = 1,
    /// The command line could not be interpreted.
    ///
    /// Clap uses this code for its own argument errors, so vump matches it.
    Usage = 2,
    /// Configuration is missing, invalid, or refers to unusable files.
    Config = 3,
    /// Tracked files do not record the expected version.
    VersionMismatch = 4,
    /// Tracked files of one project disagree with each other.
    OutOfSync = 5,
    /// The working tree has uncommitted changes.
    DirtyTree = 6,
    /// The requested version transition is not meaningful.
    InvalidTransition = 7,
    /// A git operation failed.
    Git = 8,
    /// A release artifact could not be trusted.
    ///
    /// Either the release publishes no checksums, or what was downloaded does
    /// not match them. Distinct from a generic failure because it warrants
    /// looking into rather than retrying: a mismatch means the artifact was
    /// corrupted or tampered with.
    Unverifiable = 9,
    /// A release could not be obtained.
    ///
    /// The release list was unreachable, the requested version is not
    /// published, or this platform has no published binary. Distinct from
    /// verification failure because retrying, or naming a different version,
    /// may well succeed.
    ReleaseUnavailable = 10,
}

impl From<Exit> for std::process::ExitCode {
    fn from(exit: Exit) -> Self {
        Self::from(exit as u8)
    }
}

impl Exit {
    /// A stable machine-readable name, used in JSON output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Usage => "usage",
            Self::Config => "config",
            Self::VersionMismatch => "version_mismatch",
            Self::OutOfSync => "out_of_sync",
            Self::DirtyTree => "dirty_tree",
            Self::InvalidTransition => "invalid_transition",
            Self::Git => "git",
            Self::Unverifiable => "unverifiable",
            Self::ReleaseUnavailable => "release_unavailable",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        // Changing any of these numbers breaks callers that branch on them.
        assert_eq!(Exit::Success as u8, 0);
        assert_eq!(Exit::Failure as u8, 1);
        assert_eq!(Exit::Usage as u8, 2);
        assert_eq!(Exit::Config as u8, 3);
        assert_eq!(Exit::VersionMismatch as u8, 4);
        assert_eq!(Exit::OutOfSync as u8, 5);
        assert_eq!(Exit::DirtyTree as u8, 6);
        assert_eq!(Exit::InvalidTransition as u8, 7);
        assert_eq!(Exit::Git as u8, 8);
        assert_eq!(Exit::Unverifiable as u8, 9);
        assert_eq!(Exit::ReleaseUnavailable as u8, 10);
    }
}
