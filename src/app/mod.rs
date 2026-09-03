//! Use cases.
//!
//! Each use case orchestrates the domain and the ports to carry out one
//! complete operation. Use cases depend on port traits only, so they can be
//! driven entirely from memory in tests.

pub mod bump;
pub mod change;
pub mod check;
pub mod init;
pub mod set;
pub mod status;
pub mod update;

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

    /// A lock file records this project's version but is not tracked.
    #[error(
        "{lock} records this project's version but is not declared in {config}; \
         add it to files, or writing {manifest} will leave the two disagreeing",
        config = crate::config::FILE_NAME
    )]
    UndeclaredLock {
        /// The lock file found next to a declared manifest.
        lock: String,
        /// The manifest it belongs to.
        manifest: String,
    },
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

    check_declared_locks(fs, root, project)?;

    Ok(versions)
}

/// Refuses a lock file that records this project's version but is not declared.
///
/// Writing a manifest without its lock leaves the two disagreeing, and
/// `cargo build --locked` rejects that outright — so a tag would be published
/// for a tree that cannot be built from it. Catching it here means the refusal
/// arrives before anything is written, rather than as advice after a commit and
/// tag already exist.
///
/// A lock vump cannot write is passed over rather than demanded: a workspace
/// lock covers several packages, and asking for it to be declared would only
/// produce a second refusal.
fn check_declared_locks(
    fs: &dyn FileSystem,
    root: &Path,
    project: &Project,
) -> Result<(), AppError> {
    for declared in &project.files {
        let Some(lock) = companion_lock(declared) else {
            continue;
        };
        if project.files.contains(&lock) {
            continue;
        }

        let absolute = resolve(root, &lock);
        if !fs.is_file(&absolute) {
            continue;
        }

        // Reading it decides whether it is one vump could keep in step.
        let contents = fs.read(&absolute)?;
        if Format::CargoLock.read(&lock, &contents).is_ok() {
            return Err(AppError::UndeclaredLock {
                lock,
                manifest: declared.clone(),
            });
        }
    }

    Ok(())
}

/// Names the lock file that sits beside `declared` and records its version.
///
/// Only Cargo is covered: a `yarn.lock` or `pnpm-lock.yaml` carries no version
/// for the project itself, so a bump never invalidates one.
fn companion_lock(declared: &str) -> Option<String> {
    let (directory, name) = declared
        .rsplit_once('/')
        .map_or(("", declared), |(dir, name)| (dir, name));

    if name != "Cargo.toml" {
        return None;
    }

    Some(if directory.is_empty() {
        "Cargo.lock".to_owned()
    } else {
        format!("{directory}/Cargo.lock")
    })
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
            tag_pattern: None,
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

    /// A lock naming each of `members` as built from this repository.
    fn lock(members: &[&str]) -> String {
        let mut out = String::from(
            "version = 4\n\n\
             [[package]]\nname = \"semver\"\nversion = \"1.0.23\"\n\
             source = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
        );
        for name in members {
            out.push_str("\n[[package]]\nname = \"");
            out.push_str(name);
            out.push_str("\"\nversion = \"1.2.3\"\n");
        }
        out
    }

    #[test]
    fn a_lock_recording_this_version_must_be_declared() {
        // Writing the manifest without it leaves `cargo build --locked` failing
        // on a tree a tag already claims is correct.
        let fs = MemoryFileSystem::new()
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.2.3\"\n")
            .with_file("/repo/Cargo.lock", lock(&["demo"]));

        let err =
            read_project_versions(&fs, Path::new("/repo"), &project(&["Cargo.toml"])).unwrap_err();

        assert!(
            matches!(err, AppError::UndeclaredLock { ref lock, .. } if lock == "Cargo.lock"),
            "got {err:?}"
        );
    }

    #[test]
    fn a_declared_lock_is_read_like_any_other_file() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.2.3\"\n")
            .with_file("/repo/Cargo.lock", lock(&["demo"]));

        let versions = read_project_versions(
            &fs,
            Path::new("/repo"),
            &project(&["Cargo.toml", "Cargo.lock"]),
        )
        .unwrap();

        assert_eq!(versions.len(), 2);
        assert_eq!(versions[1].path, "Cargo.lock");
        assert_eq!(versions[1].version, "1.2.3".parse().unwrap());
    }

    #[test]
    fn a_lock_vump_cannot_write_is_not_demanded() {
        // A workspace lock records no single project version, so asking for it
        // to be declared would only produce a second refusal.
        let fs = MemoryFileSystem::new()
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.2.3\"\n")
            .with_file("/repo/Cargo.lock", lock(&["demo", "demo-cli"]));

        assert!(read_project_versions(&fs, Path::new("/repo"), &project(&["Cargo.toml"])).is_ok());
    }

    #[test]
    fn a_manifest_with_no_lock_beside_it_is_fine() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.2.3\"\n");

        assert!(read_project_versions(&fs, Path::new("/repo"), &project(&["Cargo.toml"])).is_ok());
    }

    #[test]
    fn only_cargo_manifests_have_a_companion_lock() {
        // yarn and pnpm locks carry no version for the project itself, so a
        // bump never invalidates one.
        assert_eq!(companion_lock("Cargo.toml").as_deref(), Some("Cargo.lock"));
        assert_eq!(
            companion_lock("crates/api/Cargo.toml").as_deref(),
            Some("crates/api/Cargo.lock")
        );
        assert_eq!(companion_lock("package.json"), None);
        assert_eq!(companion_lock("VERSION"), None);
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
