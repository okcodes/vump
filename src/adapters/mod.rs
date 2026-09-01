//! Concrete implementations of the [`crate::ports`] traits.

pub mod filesystem;
pub mod git;
pub mod memory_vcs;

pub use filesystem::{MemoryFileSystem, RealFileSystem};
pub use git::GitCli;
pub use memory_vcs::{MemoryVcs, VcsCall};
