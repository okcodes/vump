//! Verifying that tracked files record an expected version.
//!
//! This is the use case CI depends on: it turns "the tag claims 1.4.0" into a
//! pass or fail against what the source actually says, before any build or
//! publish work is spent on the claim.

use std::path::Path;

use semver::Version;

use crate::app::{AppError, FileVersion, read_project_versions};
use crate::config::Project;
use crate::ports::FileSystem;

/// The result of comparing tracked files against an expected version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    /// The version every file was expected to record.
    pub expected: Version,
    /// Every tracked file and the version it records, in declaration order.
    pub files: Vec<FileVersion>,
}

impl CheckReport {
    /// Whether every tracked file records the expected version.
    #[must_use]
    pub fn is_satisfied(&self) -> bool {
        self.mismatches().next().is_none()
    }

    /// The files that do not record the expected version.
    pub fn mismatches(&self) -> impl Iterator<Item = &FileVersion> {
        self.files.iter().filter(|f| f.version != self.expected)
    }
}

/// Compares every file of `project` against `expected`.
///
/// A mismatch is reported in the returned [`CheckReport`] rather than as an
/// error: failing to match is an expected outcome that the caller renders and
/// turns into an exit code, whereas an error means the check could not be
/// performed at all.
///
/// # Errors
///
/// Returns an [`AppError`] when a declared file is missing, unreadable, or does
/// not carry a usable version.
pub fn check(
    fs: &dyn FileSystem,
    root: &Path,
    project: &Project,
    expected: Version,
) -> Result<CheckReport, AppError> {
    let files = read_project_versions(fs, root, project)?;
    Ok(CheckReport { expected, files })
}

/// Interprets a version as written on the command line or as a git tag.
///
/// A leading `v` is accepted and ignored, so `v1.2.3` and `1.2.3` are the same
/// version. Tags conventionally carry the prefix; the version inside a manifest
/// never does.
///
/// # Errors
///
/// Returns the underlying parse error when `text` is not valid semver.
pub fn parse_expected(text: &str) -> Result<Version, semver::Error> {
    text.strip_prefix('v').unwrap_or(text).parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::MemoryFileSystem;

    fn project(files: &[&str]) -> Project {
        Project {
            name: None,
            files: files.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn v(text: &str) -> Version {
        text.parse().unwrap()
    }

    #[test]
    fn passes_when_every_file_agrees() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.2.3\"\n");

        let report = check(
            &fs,
            Path::new("/repo"),
            &project(&["VERSION", "Cargo.toml"]),
            v("1.2.3"),
        )
        .unwrap();

        assert!(report.is_satisfied());
        assert_eq!(report.mismatches().count(), 0);
    }

    #[test]
    fn reports_each_disagreeing_file() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.2.2\"\n");

        let report = check(
            &fs,
            Path::new("/repo"),
            &project(&["VERSION", "Cargo.toml"]),
            v("1.2.3"),
        )
        .unwrap();

        assert!(!report.is_satisfied());
        let mismatched: Vec<_> = report.mismatches().map(|f| f.path.as_str()).collect();
        assert_eq!(mismatched, ["Cargo.toml"]);
    }

    #[test]
    fn pre_release_versions_must_match_exactly() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.2.3-rc.1\n");

        let satisfied = check(
            &fs,
            Path::new("/repo"),
            &project(&["VERSION"]),
            v("1.2.3-rc.1"),
        )
        .unwrap();
        assert!(satisfied.is_satisfied());

        // A pre-release is not its stable counterpart.
        let stable = check(&fs, Path::new("/repo"), &project(&["VERSION"]), v("1.2.3")).unwrap();
        assert!(!stable.is_satisfied());
    }

    #[test]
    fn a_leading_v_is_accepted_on_the_expected_version() {
        assert_eq!(parse_expected("v1.2.3").unwrap(), v("1.2.3"));
        assert_eq!(parse_expected("1.2.3").unwrap(), v("1.2.3"));
        assert_eq!(
            parse_expected("v0.2.0-alpha.1").unwrap(),
            v("0.2.0-alpha.1")
        );
        assert!(parse_expected("not-a-version").is_err());
    }

    #[test]
    fn build_metadata_is_not_ignored_when_comparing() {
        // Build metadata is excluded from semver precedence, but two artifacts
        // whose recorded versions differ textually should not silently pass.
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.2.3\n");
        let report = check(
            &fs,
            Path::new("/repo"),
            &project(&["VERSION"]),
            v("1.2.3+build.5"),
        )
        .unwrap();
        assert!(!report.is_satisfied());
    }
}
