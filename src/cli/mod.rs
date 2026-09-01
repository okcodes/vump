//! Command-line interface.
//!
//! This layer owns argument parsing, adapter selection, and presentation. It
//! contains no rules of its own: it translates a parsed command into a use
//! case, then renders whatever comes back.

pub mod exit;
pub mod render;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use thiserror::Error;

use crate::adapters::RealFileSystem;
use crate::app::{self, AppError};
use crate::config::{Config, ConfigError};
use crate::ports::FsError;

use exit::Exit;

/// Keep semver version numbers in sync across a repository.
///
/// `about` and `long_about` are set explicitly rather than inherited from this
/// doc comment: doc comments target maintainers, whereas help text targets
/// users, and the two should be free to diverge.
#[derive(Debug, Parser)]
#[command(
    name = "vump",
    version,
    about = "Keep semver version numbers in sync across a repository",
    long_about = "Keep semver version numbers in sync across the files of a repository, \
                  and verify in CI that a released tag matches what is recorded in source.",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Emit machine-readable JSON instead of human-readable text.
    #[arg(long, global = true)]
    json: bool,

    /// Select a project in a repository that declares several.
    #[arg(long, global = true, value_name = "NAME")]
    project: Option<String>,
}

/// The operation to perform.
///
/// Every bump type is a real subcommand rather than a matched string, so that
/// help text, validation, and typo diagnostics come from one source.
#[derive(Debug, Subcommand)]
enum Command {
    /// Verify that tracked files record the given version.
    ///
    /// Intended for CI: run it against a pushed tag to abort before any build
    /// or publish work when the tag disagrees with source. A leading "v" is
    /// accepted and ignored.
    Check {
        /// Version to verify, with or without a leading "v".
        version: String,
    },

    /// Report the versions currently recorded, and whether they agree.
    Status,
}

/// Parses arguments, runs the requested command, and returns a process status.
#[must_use]
pub fn run() -> ExitCode {
    let cli = Cli::parse();

    match execute(&cli) {
        Ok(exit) => exit.into(),
        Err(error) => {
            let exit = error.exit();
            render::error(&error.to_string(), exit, cli.json);
            exit.into()
        }
    }
}

fn execute(cli: &Cli) -> Result<Exit, CliError> {
    let cwd = std::env::current_dir().map_err(|e| CliError::WorkingDirectory(e.to_string()))?;
    let fs = RealFileSystem;
    let (root, config) = Config::discover(&cwd)?;

    match &cli.command {
        Command::Check { version } => {
            let expected = app::check::parse_expected(version).map_err(|e| {
                CliError::InvalidVersionArgument {
                    text: version.clone(),
                    detail: e.to_string(),
                }
            })?;

            let project = config.select(cli.project.as_deref())?;
            let report = app::check::check(&fs, &root, project, expected)?;

            render::check(&report, cli.json);

            Ok(if report.is_satisfied() {
                Exit::Success
            } else {
                Exit::VersionMismatch
            })
        }

        Command::Status => {
            // Selecting a project narrows the report; without one, every
            // declared project is reported.
            let scoped;
            let target = match cli.project.as_deref() {
                Some(name) => {
                    scoped = Config {
                        git: config.git.clone(),
                        projects: vec![config.select(Some(name))?.clone()],
                    };
                    &scoped
                }
                None => &config,
            };

            let statuses = app::status::status(&fs, &root, target)?;
            render::status(&statuses, cli.json);

            Ok(
                if statuses.iter().all(app::status::ProjectStatus::is_in_sync) {
                    Exit::Success
                } else {
                    Exit::OutOfSync
                },
            )
        }
    }
}

/// A failure that prevents a command from completing.
#[derive(Debug, Error)]
enum CliError {
    #[error("{0}")]
    Config(#[from] ConfigError),

    #[error("{0}")]
    App(#[from] AppError),

    #[error("{text:?} is not a valid version: {detail}")]
    InvalidVersionArgument { text: String, detail: String },

    #[error("cannot determine the current directory: {0}")]
    WorkingDirectory(String),
}

impl CliError {
    /// Maps a failure to its documented exit code.
    fn exit(&self) -> Exit {
        match self {
            Self::InvalidVersionArgument { .. } => Exit::Usage,
            Self::WorkingDirectory(_) => Exit::Failure,
            Self::Config(_) => Exit::Config,
            Self::App(app) => match app {
                // An unreadable file is an environment failure; every other way
                // a declared file fails to yield a version means configuration
                // points at something unusable.
                AppError::Filesystem(FsError::Io { .. }) => Exit::Failure,
                AppError::MissingFile { .. }
                | AppError::Filesystem(FsError::NotFound { .. })
                | AppError::VersionFile(_) => Exit::Config,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        // Catches conflicting flags, duplicate names and malformed help at test
        // time rather than on first run.
        Cli::command().debug_assert();
    }

    #[test]
    fn check_accepts_a_version_argument() {
        let cli = Cli::try_parse_from(["vump", "check", "v1.2.3"]).unwrap();
        assert!(matches!(cli.command, Command::Check { version } if version == "v1.2.3"));
    }

    #[test]
    fn global_flags_are_accepted_after_the_subcommand() {
        let cli = Cli::try_parse_from(["vump", "check", "1.0.0", "--json"]).unwrap();
        assert!(cli.json);

        let cli = Cli::try_parse_from(["vump", "--json", "status"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn project_selection_is_available_to_every_command() {
        let cli = Cli::try_parse_from(["vump", "status", "--project", "api"]).unwrap();
        assert_eq!(cli.project.as_deref(), Some("api"));
    }

    #[test]
    fn check_requires_a_version() {
        assert!(Cli::try_parse_from(["vump", "check"]).is_err());
    }

    #[test]
    fn an_unknown_subcommand_is_rejected() {
        assert!(Cli::try_parse_from(["vump", "bogus"]).is_err());
    }
}
