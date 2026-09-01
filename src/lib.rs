//! vump keeps semver version numbers in sync across the files of a repository
//! and verifies, in CI, that a released tag matches what is recorded in source.
//!
//! The crate is organized as ports and adapters. [`domain`] holds pure logic
//! and performs no I/O; everything that touches the filesystem, git, the
//! network, or the terminal lives behind a trait so that the interesting
//! behavior can be tested without any of them.

pub mod config;
pub mod domain;
