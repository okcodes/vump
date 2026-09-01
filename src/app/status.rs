//! Reporting what versions are currently recorded, and whether they agree.

use std::path::Path;

use semver::Version;

use crate::app::{AppError, FileVersion, read_project_versions};
use crate::config::Config;
use crate::ports::FileSystem;

/// The recorded state of one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectStatus {
    /// `None` for a single-project repository.
    pub name: Option<String>,
    /// Every tracked file and the version it records, in declaration order.
    pub files: Vec<FileVersion>,
}

impl ProjectStatus {
    /// Whether every tracked file records the same version.
    #[must_use]
    pub fn is_in_sync(&self) -> bool {
        self.distinct_versions().len() <= 1
    }

    /// The distinct versions recorded across the project's files, in the order
    /// first encountered.
    #[must_use]
    pub fn distinct_versions(&self) -> Vec<Version> {
        let mut seen: Vec<Version> = Vec::new();
        for file in &self.files {
            if !seen.contains(&file.version) {
                seen.push(file.version.clone());
            }
        }
        seen
    }

    /// The project's version, when its files agree on one.
    #[must_use]
    pub fn agreed_version(&self) -> Option<&Version> {
        match self.distinct_versions().len() {
            1 => self.files.first().map(|f| &f.version),
            _ => None,
        }
    }
}

/// Reads the recorded state of every project in `config`.
///
/// # Errors
///
/// Returns an [`AppError`] when any declared file is missing, unreadable, or
/// does not carry a usable version.
pub fn status(
    fs: &dyn FileSystem,
    root: &Path,
    config: &Config,
) -> Result<Vec<ProjectStatus>, AppError> {
    config
        .projects
        .iter()
        .map(|project| {
            Ok(ProjectStatus {
                name: project.name.clone(),
                files: read_project_versions(fs, root, project)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::MemoryFileSystem;
    use std::path::Path;

    fn config(text: &str) -> Config {
        Config::parse(Path::new("vump.toml"), text).unwrap()
    }

    #[test]
    fn reports_a_single_project_in_sync() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "2.0.0\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"2.0.0\"\n");

        let report = status(
            &fs,
            Path::new("/repo"),
            &config(r#"files = ["VERSION", "Cargo.toml"]"#),
        )
        .unwrap();

        assert_eq!(report.len(), 1);
        assert!(report[0].is_in_sync());
        assert_eq!(
            report[0].agreed_version().unwrap(),
            &"2.0.0".parse().unwrap()
        );
    }

    #[test]
    fn detects_files_that_disagree() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "2.0.0\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.9.0\"\n");

        let report = status(
            &fs,
            Path::new("/repo"),
            &config(r#"files = ["VERSION", "Cargo.toml"]"#),
        )
        .unwrap();

        assert!(!report[0].is_in_sync());
        assert_eq!(report[0].distinct_versions().len(), 2);
        assert!(
            report[0].agreed_version().is_none(),
            "there is no single version to report when files disagree"
        );
    }

    #[test]
    fn reports_each_project_independently() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/api/Cargo.toml", "[package]\nversion = \"1.0.0\"\n")
            .with_file("/repo/web/package.json", r#"{"version":"3.4.5"}"#);

        let report = status(
            &fs,
            Path::new("/repo"),
            &config(
                r#"
                [[project]]
                name = "api"
                files = ["api/Cargo.toml"]

                [[project]]
                name = "web"
                files = ["web/package.json"]
                "#,
            ),
        )
        .unwrap();

        assert_eq!(report.len(), 2);
        assert_eq!(report[0].name.as_deref(), Some("api"));
        // Independently-versioned projects differing is normal, not a mismatch.
        assert_eq!(
            report[0].agreed_version().unwrap(),
            &"1.0.0".parse().unwrap()
        );
        assert_eq!(
            report[1].agreed_version().unwrap(),
            &"3.4.5".parse().unwrap()
        );
    }
}
