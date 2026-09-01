//! Traits describing what use cases need from the outside world.
//!
//! Use cases depend on these traits, never on their implementations. That is
//! what allows the interesting behavior to be exercised in tests without a
//! filesystem, a git repository, or a terminal.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Filesystem access required by use cases.
pub trait FileSystem {
    /// Reads a file as UTF-8 text.
    ///
    /// # Errors
    ///
    /// Returns [`FsError::NotFound`] when the file does not exist, and
    /// [`FsError::Io`] for any other failure, including invalid UTF-8.
    fn read(&self, path: &Path) -> Result<String, FsError>;

    /// Writes UTF-8 text, replacing any existing contents.
    ///
    /// # Errors
    ///
    /// Returns [`FsError::Io`] when the write cannot be completed.
    fn write(&self, path: &Path, contents: &str) -> Result<(), FsError>;

    /// Whether a regular file exists at `path`.
    fn is_file(&self, path: &Path) -> bool;
}

/// Version control operations required by use cases.
///
/// Implementations act on one repository, fixed at construction, so that no
/// caller has to thread a working directory through every call.
pub trait Vcs {
    /// Reports whether the working tree has uncommitted changes.
    ///
    /// # Errors
    ///
    /// Returns [`VcsError`] when the repository cannot be inspected.
    fn status(&self) -> Result<WorkingTree, VcsError>;

    /// Stages the given paths, which are relative to the repository root.
    ///
    /// # Errors
    ///
    /// Returns [`VcsError`] when staging fails.
    fn stage(&self, paths: &[String]) -> Result<(), VcsError>;

    /// Commits the staged changes.
    ///
    /// # Errors
    ///
    /// Returns [`VcsError`] when the commit fails.
    fn commit(&self, message: &str) -> Result<(), VcsError>;

    /// Creates a tag at `HEAD`.
    ///
    /// # Errors
    ///
    /// Returns [`VcsError`] when the tag cannot be created, including when one
    /// of that name already exists.
    fn tag(&self, name: &str) -> Result<(), VcsError>;

    /// Pushes the current branch, and `tag` when one is given.
    ///
    /// # Errors
    ///
    /// Returns [`VcsError`] when either push fails.
    fn push(&self, tag: Option<&str>) -> Result<(), VcsError>;
}

/// The state of a working tree.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkingTree {
    /// Paths with uncommitted changes, as reported by the VCS.
    pub changed: Vec<String>,
}

impl WorkingTree {
    /// Whether anything is uncommitted.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        !self.changed.is_empty()
    }
}

/// A version control operation that could not be completed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VcsError {
    /// The command could not be run at all.
    #[error("cannot run git: {detail}")]
    Unavailable {
        /// Underlying error detail.
        detail: String,
    },

    /// The command ran and failed.
    #[error("git {operation} failed: {detail}")]
    Failed {
        /// The operation being attempted, for example `commit`.
        operation: String,
        /// Output produced by git.
        detail: String,
    },
}

/// A filesystem operation that could not be completed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FsError {
    /// The path does not exist.
    #[error("{path} does not exist")]
    NotFound {
        /// The path that was looked for.
        path: PathBuf,
    },

    /// Any other failure, including permission errors and invalid UTF-8.
    #[error("cannot access {path}: {detail}")]
    Io {
        /// The path being accessed.
        path: PathBuf,
        /// Underlying error detail.
        detail: String,
    },
}
