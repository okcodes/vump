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

use crate::adapters::{GitCli, RealFileSystem};
use crate::app::bump::{BumpError, GitFlags, GitIntent};
use crate::app::{self, AppError};
use crate::config::{Config, ConfigError};
use crate::domain::{PreLabel, StableBump, Transition};
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
///
/// Naming a bump subcommand is what makes a run non-interactive. Anything a
/// subcommand would otherwise have to ask about is a required flag or an
/// error, never a prompt.
#[derive(Debug, Subcommand)]
enum Command {
    /// Bump the patch component (1.2.3 -> 1.2.4).
    Patch(BumpArgs),

    /// Bump the minor component (1.2.3 -> 1.3.0).
    Minor(BumpArgs),

    /// Bump the major component (1.2.3 -> 2.0.0).
    Major(BumpArgs),

    /// Start or advance an alpha pre-release.
    Alpha(PreReleaseArgs),

    /// Start or advance a beta pre-release.
    Beta(PreReleaseArgs),

    /// Start or advance a release candidate.
    Rc(PreReleaseArgs),

    /// Drop the pre-release suffix (1.2.3-rc.1 -> 1.2.3).
    Release(BumpArgs),

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

/// Options shared by every bump.
#[derive(Debug, clap::Args)]
struct BumpArgs {
    /// Report what would change without writing anything.
    #[arg(long)]
    dry_run: bool,

    #[command(flatten)]
    git: GitArgs,
}

/// Options for starting or advancing a pre-release.
#[derive(Debug, clap::Args)]
struct PreReleaseArgs {
    /// Which release the pre-release leads to.
    ///
    /// Required when the current version is stable, because a pre-release must
    /// know which future release it precedes. Ignored when already on one.
    #[arg(long, value_name = "BUMP")]
    from: Option<StableBumpArg>,

    /// Report what would change without writing anything.
    #[arg(long)]
    dry_run: bool,

    #[command(flatten)]
    git: GitArgs,
}

/// Git side-effects selectable on the command line.
///
/// The usual objection to several booleans in one struct is unreadable call
/// sites, which does not apply here: every field is a distinct, independently
/// meaningful flag, named at the point of use both on the command line and in
/// code.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, clap::Args)]
struct GitArgs {
    /// Stage and commit the changed files.
    #[arg(long)]
    commit: bool,

    /// Commit and tag. Implies --commit.
    #[arg(long)]
    tag: bool,

    /// Push the commit and tag. Implies --commit.
    #[arg(long)]
    push: bool,

    /// Perform no git actions, overriding vump.toml for this run.
    #[arg(long, conflicts_with_all = ["commit", "tag", "push"])]
    no_git: bool,
}

/// The stable bump a pre-release is based on.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum StableBumpArg {
    /// 1.2.3 -> 1.2.4
    Patch,
    /// 1.2.3 -> 1.3.0
    Minor,
    /// 1.2.3 -> 2.0.0
    Major,
}

impl From<StableBumpArg> for StableBump {
    fn from(arg: StableBumpArg) -> Self {
        match arg {
            StableBumpArg::Patch => Self::Patch,
            StableBumpArg::Minor => Self::Minor,
            StableBumpArg::Major => Self::Major,
        }
    }
}

impl GitArgs {
    /// Resolves configuration and flags into the side-effects to perform.
    fn intent(&self, settings: &crate::config::GitSettings) -> GitIntent {
        if self.no_git {
            return GitIntent::default();
        }
        GitIntent::resolve(
            settings,
            GitFlags {
                commit: self.commit,
                tag: self.tag,
                push: self.push,
            },
        )
    }
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

/// Everything a command needs from the environment, resolved once.
struct Context {
    fs: RealFileSystem,
    root: std::path::PathBuf,
    config: Config,
    json: bool,
    project: Option<String>,
}

fn execute(cli: &Cli) -> Result<Exit, CliError> {
    let cwd = std::env::current_dir().map_err(|e| CliError::WorkingDirectory(e.to_string()))?;
    let (root, config) = Config::discover(&cwd)?;

    let ctx = Context {
        fs: RealFileSystem,
        root,
        config,
        json: cli.json,
        project: cli.project.clone(),
    };

    let pre = |label, args: &PreReleaseArgs| Transition::PreRelease {
        label,
        from: args.from.map(Into::into),
    };

    match &cli.command {
        Command::Patch(a) => bump(
            &ctx,
            Transition::Stable(StableBump::Patch),
            a.dry_run,
            &a.git,
        ),
        Command::Minor(a) => bump(
            &ctx,
            Transition::Stable(StableBump::Minor),
            a.dry_run,
            &a.git,
        ),
        Command::Major(a) => bump(
            &ctx,
            Transition::Stable(StableBump::Major),
            a.dry_run,
            &a.git,
        ),
        Command::Release(a) => bump(&ctx, Transition::Release, a.dry_run, &a.git),
        Command::Alpha(a) => bump(&ctx, pre(PreLabel::Alpha, a), a.dry_run, &a.git),
        Command::Beta(a) => bump(&ctx, pre(PreLabel::Beta, a), a.dry_run, &a.git),
        Command::Rc(a) => bump(&ctx, pre(PreLabel::Rc, a), a.dry_run, &a.git),
        Command::Check { version } => check(&ctx, version),
        Command::Status => status(&ctx),
    }
}

fn bump(
    ctx: &Context,
    transition: Transition,
    dry_run: bool,
    git_args: &GitArgs,
) -> Result<Exit, CliError> {
    let project = ctx.config.select(ctx.project.as_deref())?;
    let intent = git_args.intent(&ctx.config.git);

    let plan = app::bump::plan(
        &ctx.fs,
        &ctx.root,
        project,
        transition,
        &ctx.config.git,
        intent,
    )?;

    if dry_run {
        render::plan(&plan, ctx.json);
        return Ok(Exit::Success);
    }

    let vcs = GitCli::new(&ctx.root);
    let outcome = app::bump::apply(&ctx.fs, &vcs, &ctx.root, &plan)?;
    render::bump(&plan, &outcome, ctx.json);

    // A failed push leaves real, recoverable work behind, so it is reported as
    // a git failure rather than as a successful run.
    Ok(if outcome.push_error.is_some() {
        Exit::Git
    } else {
        Exit::Success
    })
}

fn check(ctx: &Context, version: &str) -> Result<Exit, CliError> {
    let expected =
        app::check::parse_expected(version).map_err(|e| CliError::InvalidVersionArgument {
            text: version.to_owned(),
            detail: e.to_string(),
        })?;

    let project = ctx.config.select(ctx.project.as_deref())?;
    let report = app::check::check(&ctx.fs, &ctx.root, project, expected)?;

    render::check(&report, ctx.json);

    Ok(if report.is_satisfied() {
        Exit::Success
    } else {
        Exit::VersionMismatch
    })
}

fn status(ctx: &Context) -> Result<Exit, CliError> {
    // Selecting a project narrows the report; without one, every declared
    // project is reported.
    let scoped;
    let target = match ctx.project.as_deref() {
        Some(name) => {
            scoped = Config {
                git: ctx.config.git.clone(),
                projects: vec![ctx.config.select(Some(name))?.clone()],
            };
            &scoped
        }
        None => &ctx.config,
    };

    let statuses = app::status::status(&ctx.fs, &ctx.root, target)?;
    render::status(&statuses, ctx.json);

    Ok(
        if statuses.iter().all(app::status::ProjectStatus::is_in_sync) {
            Exit::Success
        } else {
            Exit::OutOfSync
        },
    )
}

/// Maps a use-case failure to its documented exit code.
///
/// An unreadable file is an environment failure; every other way a declared
/// file fails to yield a version means configuration points at something
/// unusable.
fn app_exit(error: &AppError) -> Exit {
    match error {
        AppError::Filesystem(FsError::Io { .. }) => Exit::Failure,
        AppError::MissingFile { .. }
        | AppError::Filesystem(FsError::NotFound { .. })
        | AppError::VersionFile(_) => Exit::Config,
    }
}

/// A failure that prevents a command from completing.
#[derive(Debug, Error)]
enum CliError {
    #[error("{0}")]
    Config(#[from] ConfigError),

    #[error("{0}")]
    App(#[from] AppError),

    #[error("{0}")]
    Bump(#[from] BumpError),

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
            Self::App(app) => app_exit(app),
            Self::Bump(bump) => match bump {
                BumpError::App(app) => app_exit(app),
                BumpError::Transition(_) => Exit::InvalidTransition,
                BumpError::OutOfSync { .. } => Exit::OutOfSync,
                BumpError::DirtyTree { .. } => Exit::DirtyTree,
                BumpError::Vcs(_) => Exit::Git,
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
