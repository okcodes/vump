//! Replacing the running binary with the newest release.
//!
//! The comparison and asset-naming rules are pure and tested here. Fetching and
//! replacing are behind the [`ReleaseSource`] port, so the interesting decisions
//! — is this newer, which asset applies, should anything happen at all — are
//! exercised without a network.

use semver::Version;
use thiserror::Error;

/// Where releases are published and how the running binary is replaced.
pub trait ReleaseSource {
    /// Reports the newest published release.
    ///
    /// # Errors
    ///
    /// Returns [`UpdateError`] when the release cannot be determined.
    fn latest(&self) -> Result<Release, UpdateError>;

    /// Downloads `asset` from `release` and replaces the running binary.
    ///
    /// # Errors
    ///
    /// Returns [`UpdateError`] when the download or replacement fails.
    fn install(&self, release: &Release, asset: &str) -> Result<(), UpdateError>;
}

/// A published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The tag, as published, which may carry a leading `v`.
    pub tag: String,
    /// The version parsed from the tag.
    pub version: Version,
}

/// Why an update could not be performed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UpdateError {
    /// The release list could not be reached or understood.
    #[error("cannot determine the latest release: {detail}")]
    Unreachable {
        /// Underlying error detail.
        detail: String,
    },

    /// A published tag is not a version.
    #[error("published tag {tag:?} is not a version: {detail}")]
    UnreadableTag {
        /// The tag that could not be parsed.
        tag: String,
        /// Parser-supplied detail.
        detail: String,
    },

    /// This platform has no published binary.
    #[error("no published binary for {os} {arch}")]
    UnsupportedPlatform {
        /// Operating system.
        os: String,
        /// Architecture.
        arch: String,
    },

    /// The download or replacement failed.
    #[error("cannot install {asset}: {detail}")]
    InstallFailed {
        /// Asset being installed.
        asset: String,
        /// Underlying error detail.
        detail: String,
    },
}

/// What an update run concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    /// The running version is the newest published one.
    UpToDate {
        /// The version in use.
        current: Version,
    },
    /// A newer release exists and was not installed, because only a check was
    /// requested.
    Available {
        /// The version in use.
        current: Version,
        /// The newer published version.
        latest: Version,
    },
    /// A newer release was installed.
    Installed {
        /// The version replaced.
        previous: Version,
        /// The version now in use.
        installed: Version,
    },
    /// The published release is older than what is running.
    ///
    /// Normal for a development build, and never acted upon: replacing a newer
    /// binary with an older one is not an update.
    Ahead {
        /// The version in use.
        current: Version,
        /// The newest published version.
        latest: Version,
    },
}

/// Compares the running version against the newest release and, unless
/// `check_only`, installs it when it is newer.
///
/// # Errors
///
/// Returns an [`UpdateError`] when the latest release cannot be determined,
/// this platform has no published binary, or installation fails.
pub fn update(
    source: &dyn ReleaseSource,
    current: &Version,
    target: (&str, &str),
    check_only: bool,
) -> Result<UpdateOutcome, UpdateError> {
    let release = source.latest()?;

    if release.version == *current {
        return Ok(UpdateOutcome::UpToDate {
            current: current.clone(),
        });
    }

    if release.version < *current {
        return Ok(UpdateOutcome::Ahead {
            current: current.clone(),
            latest: release.version,
        });
    }

    if check_only {
        return Ok(UpdateOutcome::Available {
            current: current.clone(),
            latest: release.version,
        });
    }

    let (os, arch) = target;
    let asset = asset_name(os, arch).ok_or_else(|| UpdateError::UnsupportedPlatform {
        os: os.to_owned(),
        arch: arch.to_owned(),
    })?;

    source.install(&release, &asset)?;

    Ok(UpdateOutcome::Installed {
        previous: current.clone(),
        installed: release.version,
    })
}

/// The release asset for a platform.
///
/// These names are a contract shared with the release workflow and the CI
/// action; changing one without the others breaks installation.
///
/// macOS publishes only a universal binary, which is also the only notarized
/// artifact, so both architectures resolve to it.
#[must_use]
pub fn asset_name(os: &str, arch: &str) -> Option<String> {
    match (os, arch) {
        ("macos", _) => Some("vump-darwin-universal".to_owned()),
        ("linux", "x86_64") => Some("vump-linux-amd64".to_owned()),
        ("linux", "aarch64") => Some("vump-linux-arm64".to_owned()),
        ("windows", "x86_64") => Some("vump-windows-amd64.exe".to_owned()),
        ("windows", "aarch64") => Some("vump-windows-arm64.exe".to_owned()),
        _ => None,
    }
}

/// Interprets a published tag, which conventionally carries a leading `v`.
///
/// # Errors
///
/// Returns [`UpdateError::UnreadableTag`] when the tag is not a version.
pub fn parse_tag(tag: &str) -> Result<Release, UpdateError> {
    let version = tag
        .strip_prefix('v')
        .unwrap_or(tag)
        .parse()
        .map_err(|e: semver::Error| UpdateError::UnreadableTag {
            tag: tag.to_owned(),
            detail: e.to_string(),
        })?;

    Ok(Release {
        tag: tag.to_owned(),
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn v(text: &str) -> Version {
        text.parse().unwrap()
    }

    /// A release source that answers with a fixed tag and records installs.
    struct Fake {
        tag: String,
        installed: RefCell<Vec<String>>,
    }

    impl Fake {
        fn at(tag: &str) -> Self {
            Self {
                tag: tag.to_owned(),
                installed: RefCell::new(Vec::new()),
            }
        }
    }

    impl ReleaseSource for Fake {
        fn latest(&self) -> Result<Release, UpdateError> {
            parse_tag(&self.tag)
        }

        fn install(&self, _: &Release, asset: &str) -> Result<(), UpdateError> {
            self.installed.borrow_mut().push(asset.to_owned());
            Ok(())
        }
    }

    const LINUX: (&str, &str) = ("linux", "x86_64");

    #[test]
    fn installs_a_newer_release() {
        let source = Fake::at("v1.5.0");
        let outcome = update(&source, &v("1.4.0"), LINUX, false).unwrap();

        assert_eq!(
            outcome,
            UpdateOutcome::Installed {
                previous: v("1.4.0"),
                installed: v("1.5.0"),
            }
        );
        assert_eq!(*source.installed.borrow(), ["vump-linux-amd64"]);
    }

    #[test]
    fn does_nothing_when_already_current() {
        let source = Fake::at("v1.4.0");
        let outcome = update(&source, &v("1.4.0"), LINUX, false).unwrap();

        assert_eq!(
            outcome,
            UpdateOutcome::UpToDate {
                current: v("1.4.0")
            }
        );
        assert!(source.installed.borrow().is_empty());
    }

    #[test]
    fn never_installs_an_older_release() {
        // A development build ahead of the last release must not be downgraded.
        let source = Fake::at("v1.0.0");
        let outcome = update(&source, &v("1.4.0"), LINUX, false).unwrap();

        assert_eq!(
            outcome,
            UpdateOutcome::Ahead {
                current: v("1.4.0"),
                latest: v("1.0.0"),
            }
        );
        assert!(
            source.installed.borrow().is_empty(),
            "an older release must never overwrite a newer binary"
        );
    }

    #[test]
    fn checking_reports_without_installing() {
        let source = Fake::at("v2.0.0");
        let outcome = update(&source, &v("1.0.0"), LINUX, true).unwrap();

        assert_eq!(
            outcome,
            UpdateOutcome::Available {
                current: v("1.0.0"),
                latest: v("2.0.0"),
            }
        );
        assert!(source.installed.borrow().is_empty());
    }

    #[test]
    fn a_pre_release_is_older_than_its_release() {
        let source = Fake::at("v1.0.0");
        let outcome = update(&source, &v("1.0.0-rc.1"), LINUX, true).unwrap();

        assert_eq!(
            outcome,
            UpdateOutcome::Available {
                current: v("1.0.0-rc.1"),
                latest: v("1.0.0"),
            }
        );
    }

    #[test]
    fn an_unsupported_platform_is_reported_before_downloading() {
        let source = Fake::at("v2.0.0");
        let err = update(&source, &v("1.0.0"), ("freebsd", "x86_64"), false).unwrap_err();

        assert!(matches!(err, UpdateError::UnsupportedPlatform { .. }));
        assert!(source.installed.borrow().is_empty());
    }

    #[test]
    fn asset_names_match_the_published_artifacts() {
        // These strings are shared with the release workflow; a change here
        // without a matching one there breaks installation for everyone.
        assert_eq!(asset_name("linux", "x86_64").unwrap(), "vump-linux-amd64");
        assert_eq!(asset_name("linux", "aarch64").unwrap(), "vump-linux-arm64");
        assert_eq!(
            asset_name("windows", "x86_64").unwrap(),
            "vump-windows-amd64.exe"
        );
        assert_eq!(
            asset_name("windows", "aarch64").unwrap(),
            "vump-windows-arm64.exe"
        );
        // Both macOS architectures resolve to the one notarized artifact.
        assert_eq!(
            asset_name("macos", "aarch64").unwrap(),
            "vump-darwin-universal"
        );
        assert_eq!(
            asset_name("macos", "x86_64").unwrap(),
            "vump-darwin-universal"
        );
    }

    #[test]
    fn tags_are_read_with_or_without_a_leading_v() {
        assert_eq!(parse_tag("v1.2.3").unwrap().version, v("1.2.3"));
        assert_eq!(parse_tag("1.2.3").unwrap().version, v("1.2.3"));
        // The tag is preserved as published, since it forms the download URL.
        assert_eq!(parse_tag("v1.2.3").unwrap().tag, "v1.2.3");
        assert!(parse_tag("nightly").is_err());
    }
}
