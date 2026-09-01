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
