//! Filesystem adapters.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::ports::{FileSystem, FsError};

/// Reads and writes the real filesystem.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn read(&self, path: &Path) -> Result<String, FsError> {
        std::fs::read_to_string(path).map_err(|e| classify(path, &e))
    }

    fn write(&self, path: &Path, contents: &str) -> Result<(), FsError> {
        std::fs::write(path, contents).map_err(|e| classify(path, &e))
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
}

fn classify(path: &Path, error: &io::Error) -> FsError {
    if error.kind() == io::ErrorKind::NotFound {
        FsError::NotFound {
            path: path.to_path_buf(),
        }
    } else {
        FsError::Io {
            path: path.to_path_buf(),
            detail: error.to_string(),
        }
    }
}

/// An in-memory filesystem for tests.
///
/// Keeping this beside the real adapter rather than inside a test module lets
/// integration tests drive use cases without touching disk.
#[derive(Debug, Default)]
pub struct MemoryFileSystem {
    files: Mutex<BTreeMap<PathBuf, String>>,
}

impl MemoryFileSystem {
    /// Creates an empty filesystem.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Seeds a file, replacing any existing contents.
    #[must_use]
    pub fn with_file(self, path: impl Into<PathBuf>, contents: impl Into<String>) -> Self {
        self.insert(path, contents);
        self
    }

    /// Inserts a file, replacing any existing contents.
    pub fn insert(&self, path: impl Into<PathBuf>, contents: impl Into<String>) {
        self.lock().insert(path.into(), contents.into());
    }

    /// Returns the current contents of a file, if present.
    #[must_use]
    pub fn get(&self, path: impl AsRef<Path>) -> Option<String> {
        self.lock().get(path.as_ref()).cloned()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<PathBuf, String>> {
        // A poisoned lock means a test already panicked. Recovering the guard
        // keeps that original panic as the reported failure instead of burying
        // it under a second one.
        self.files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl FileSystem for MemoryFileSystem {
    fn read(&self, path: &Path) -> Result<String, FsError> {
        self.lock()
            .get(path)
            .cloned()
            .ok_or_else(|| FsError::NotFound {
                path: path.to_path_buf(),
            })
    }

    fn write(&self, path: &Path, contents: &str) -> Result<(), FsError> {
        self.lock().insert(path.to_path_buf(), contents.to_owned());
        Ok(())
    }

    fn is_file(&self, path: &Path) -> bool {
        self.lock().contains_key(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_filesystem_round_trips() {
        let fs = MemoryFileSystem::new().with_file("a/VERSION", "1.0.0\n");

        assert!(fs.is_file(Path::new("a/VERSION")));
        assert_eq!(fs.read(Path::new("a/VERSION")).unwrap(), "1.0.0\n");

        fs.write(Path::new("a/VERSION"), "2.0.0\n").unwrap();
        assert_eq!(fs.get("a/VERSION").as_deref(), Some("2.0.0\n"));
    }

    #[test]
    fn missing_files_are_reported_as_not_found() {
        let fs = MemoryFileSystem::new();
        assert!(!fs.is_file(Path::new("nope")));
        assert!(matches!(
            fs.read(Path::new("nope")).unwrap_err(),
            FsError::NotFound { .. }
        ));
    }
}
