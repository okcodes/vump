//! Concrete implementations of the [`crate::ports`] traits.

pub mod filesystem;

pub use filesystem::{MemoryFileSystem, RealFileSystem};
