//! Concrete implementations of the [`crate::ports`] traits.

pub mod filesystem;
pub mod git;
pub mod github;
pub mod memory_vcs;
pub mod terminal;

pub use filesystem::{MemoryFileSystem, RealFileSystem};
pub use git::GitCli;
pub use github::GitHubReleases;
pub use memory_vcs::{MemoryVcs, VcsCall};
pub use terminal::{NoInteraction, TerminalInteraction};
