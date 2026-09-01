//! vump keeps semver version numbers in sync across the files of a repository
//! and verifies, in CI, that a released tag matches what is recorded in source.
//!
//! The crate is organized as ports and adapters. [`domain`] holds pure logic
//! and performs no I/O; [`ports`] declares what the outside world must provide;
//! [`adapters`] implements those traits; [`app`] orchestrates them into use
//! cases; and [`cli`] parses arguments and renders results.

pub mod adapters;
pub mod app;
pub mod cli;
pub mod config;
pub mod domain;
pub mod ports;
