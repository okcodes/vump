//! Binary entry point.
//!
//! All behavior lives in the library so that it stays reachable from tests;
//! this file only forwards the process exit status.

use std::process::ExitCode;

fn main() -> ExitCode {
    vump::cli::run()
}
