//! Use cases.
//!
//! Each use case orchestrates the domain and the ports to carry out one
//! complete operation. Use cases depend on port traits only, so they can be
//! driven entirely from memory in tests.

pub mod bump;
pub mod check;
pub mod init;
pub mod status;

use std::path::{Path, PathBuf};

use semver::Version;
use thiserror::Error;

use crate::config::Project;
use crate::domain::version_file::{Format, VersionFileError};
use crate::ports::{FileSystem, FsError};

/// A version file and the version currently recorded in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileVersion {
    /// Path as declared in configuration, relative to the repository root.
    pub path: String,
    /// The version read from the file.
    pub version: Version,
}

/// Why a use case could not complete.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AppError {
    /// A declared file does not exist.
    #[error("{path} is declared in {config} but does not exist", config = crate::config::FILE_NAME)]
    MissingFile {
        /// The declared path.
        path: String,
    },

    /// A declared file could not be read.
    #[error("{0}")]
    Filesystem(#[from] FsError),

    /// A declared file could not be interpreted.
    #[error("{0}")]
    VersionFile(#[from] VersionFileError),
}

/// Reads the version recorded in every file of `project`.
///
/// Paths are resolved relative to `root`, the directory containing the
/// configuration file.
///
/// # Errors
///
/// Returns an [`AppError`] when any declared file is missing, unreadable, or
/// does not carry a usable version.
pub fn read_project_versions(
    fs: &dyn FileSystem,
    root: &Path,
    project: &Project,
) -> Result<Vec<FileVersion>, AppError> {
    let mut versions = Vec::with_capacity(project.files.len());

    for declared in &project.files {
        let absolute = resolve(root, declared);

        let file_name = absolute
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(declared.as_str());
        let format = Format::require(file_name)?;

        if !fs.is_file(&absolute) {
            return Err(AppError::MissingFile {
                path: declared.clone(),
            });
        }

        let contents = fs.read(&absolute)?;
        let version = format.read(declared, &contents)?;

        versions.push(FileVersion {
            path: declared.clone(),
            version,
        });
    }

    Ok(versions)
}

/// Resolves a configured path against the configuration's directory.
///
/// Configured paths always use forward slashes, so they are split explicitly
/// rather than handed to the platform's path parser.
#[must_use]
pub fn resolve(root: &Path, declared: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for segment in declared.split('/').filter(|s| !s.is_empty() && *s != ".") {
        path.push(segment);
    }
    path
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

    #[test]
    fn reads_every_declared_file() {
        let root = Path::new("/repo");
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/ui/package.json", r#"{"version":"1.2.3"}"#);

        let versions =
            read_project_versions(&fs, root, &project(&["VERSION", "ui/package.json"])).unwrap();

        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].path, "VERSION");
        assert_eq!(versions[1].version, "1.2.3".parse().unwrap());
    }

    #[test]
    fn a_missing_file_is_reported_with_its_declared_path() {
        let fs = MemoryFileSystem::new();
        let err =
            read_project_versions(&fs, Path::new("/repo"), &project(&["VERSION"])).unwrap_err();

        // The declared path is what the user wrote in configuration, so that is
        // what the message must name.
        assert!(matches!(err, AppError::MissingFile { path } if path == "VERSION"));
    }

    #[test]
    fn an_unsupported_filename_is_rejected_before_any_read() {
        let fs = MemoryFileSystem::new();
        let err = read_project_versions(&fs, Path::new("/repo"), &project(&["pyproject.toml"]))
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::VersionFile(VersionFileError::UnsupportedFile { .. })
        ));
    }

    #[test]
    fn resolves_configured_paths_relative_to_the_config_directory() {
        let resolved = resolve(Path::new("/repo"), "services/api/Cargo.toml");
        assert_eq!(
            resolved,
            Path::new("/repo")
                .join("services")
                .join("api")
                .join("Cargo.toml")
        );
    }
}
