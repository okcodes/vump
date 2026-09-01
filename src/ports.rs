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

/// Questions a run may need to put to a person.
///
/// Only the interactive entry point uses this. Naming a subcommand selects an
/// implementation that cannot ask anything, which is how the non-interactive
/// guarantee is enforced by construction rather than by discipline.
pub trait Interaction {
    /// Asks which project to operate on.
    ///
    /// # Errors
    ///
    /// Returns [`InteractionError`] when the question cannot be asked or is
    /// declined.
    fn choose_project(&self, names: &[String]) -> Result<String, InteractionError>;

    /// Asks which version to treat as current when tracked files disagree.
    ///
    /// # Errors
    ///
    /// Returns [`InteractionError`] when the question cannot be asked or is
    /// declined.
    fn choose_base(&self, candidates: &[(String, String)]) -> Result<String, InteractionError>;

    /// Asks which transition to apply, given those valid from here.
    ///
    /// `options` pairs each choice with the version it would produce, and
    /// contains only transitions that will succeed.
    ///
    /// # Errors
    ///
    /// Returns [`InteractionError`] when the question cannot be asked or is
    /// declined.
    fn choose_transition(
        &self,
        current: &str,
        options: &[(String, String)],
    ) -> Result<usize, InteractionError>;

    /// Asks which git side-effects to perform.
    ///
    /// Asked only when configuration has not already decided.
    ///
    /// # Errors
    ///
    /// Returns [`InteractionError`] when the question cannot be asked or is
    /// declined.
    fn choose_git(&self) -> Result<GitChoice, InteractionError>;

    /// Asks for final approval of a rendered summary.
    ///
    /// # Errors
    ///
    /// Returns [`InteractionError`] when the question cannot be asked or is
    /// declined.
    fn confirm(&self, summary: &str) -> Result<bool, InteractionError>;
}

/// Git side-effects offered as a single question.
///
/// Combining them avoids asking twice for what is really one decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitChoice {
    /// Write the files and stop.
    None,
    /// Commit the change.
    Commit,
    /// Commit and tag.
    Tag,
    /// Commit, tag and push.
    TagAndPush,
}

/// Why a question could not be answered.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InteractionError {
    /// The person declined, by cancelling the prompt.
    #[error("cancelled")]
    Cancelled,

    /// There is nobody to ask.
    #[error("cannot prompt: {detail}")]
    Unavailable {
        /// Why prompting is impossible, for example no terminal is attached.
        detail: String,
    },
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
