//! Release source backed by GitHub Releases.

use std::time::Duration;

use crate::app::update::{Release, ReleaseSource, UpdateError, parse_tag};

/// Where vump publishes its own binaries.
const REPOSITORY: &str = "okcodes/vump";

/// Requests are given a bound so that an unreachable network fails rather than
/// hanging a command the user is waiting on.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Reads releases from the GitHub API and replaces the running binary.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitHubReleases;

impl GitHubReleases {
    /// Creates a source pointing at vump's own repository.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .user_agent(concat!("vump/", env!("CARGO_PKG_VERSION")))
            .build()
            .into()
    }
}

impl ReleaseSource for GitHubReleases {
    fn latest(&self) -> Result<Release, UpdateError> {
        // This endpoint excludes pre-releases, so an alpha never presents
        // itself as an upgrade to someone running a stable build.
        let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");

        let mut response = Self::agent()
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| UpdateError::Unreachable {
                detail: e.to_string(),
            })?;

        let body: serde_json::Value =
            response
                .body_mut()
                .read_json()
                .map_err(|e| UpdateError::Unreachable {
                    detail: e.to_string(),
                })?;

        let tag = body
            .get("tag_name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| UpdateError::Unreachable {
                detail: "response carried no tag_name".to_owned(),
            })?;

        parse_tag(tag)
    }

    fn install(&self, release: &Release, asset: &str) -> Result<(), UpdateError> {
        let url = format!(
            "https://github.com/{REPOSITORY}/releases/download/{}/{asset}",
            release.tag
        );

        let fail = |detail: String| UpdateError::InstallFailed {
            asset: asset.to_owned(),
            detail,
        };

        let mut response = Self::agent()
            .get(&url)
            .call()
            .map_err(|e| fail(e.to_string()))?;

        let bytes = response
            .body_mut()
            .with_config()
            .limit(256 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| fail(e.to_string()))?;

        // The replacement is staged next to the running binary so that the
        // final move is within one filesystem, and therefore atomic.
        let exe = std::env::current_exe().map_err(|e| fail(e.to_string()))?;
        let directory = exe.parent().unwrap_or_else(|| std::path::Path::new("."));
        let staged = directory.join(format!(".{asset}.incoming"));

        std::fs::write(&staged, &bytes).map_err(|e| fail(e.to_string()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| fail(e.to_string()))?;
        }

        // Replacing a running executable differs sharply between platforms:
        // Unix can rename over an open file, Windows must move the original
        // aside first. That is delegated rather than reimplemented.
        let result = self_replace::self_replace(&staged).map_err(|e| fail(e.to_string()));

        // Best-effort: a leftover staging file is untidy but harmless, and
        // must not mask the outcome of the replacement itself.
        let _ = std::fs::remove_file(&staged);

        result
    }
}
