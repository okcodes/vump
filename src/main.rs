//! vump keeps semver version numbers in sync across the files of a repository
//! and verifies, in CI, that a released tag matches what is recorded in source.

use clap::Parser;

/// Top-level command-line interface.
///
/// `about` and `long_about` are set explicitly rather than inherited from this
/// doc comment: doc comments target maintainers, whereas help text targets
/// users, and the two should be free to diverge.
///
/// The version reported by `--version` comes from `Cargo.toml` at compile time,
/// making the manifest the single source of truth for the binary's identity.
#[derive(Debug, Parser)]
#[command(
    name = "vump",
    version,
    about = "Keep semver version numbers in sync across a repository",
    long_about = "Keep semver version numbers in sync across the files of a repository, \
                  and verify in CI that a released tag matches what is recorded in source."
)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();

    println!("vump {}", env!("CARGO_PKG_VERSION"));
}
