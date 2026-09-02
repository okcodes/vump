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

use crate::adapters::{GitCli, GitHubReleases, RealFileSystem, TerminalInteraction};
use crate::app::bump::{BumpError, GitFlags, GitIntent};
use crate::app::update::Channel;
use crate::app::{self, AppError};
use crate::config::{Config, ConfigError};
use crate::domain::{PreLabel, StableBump, Transition, TransitionError};
use crate::ports::{FsError, GitChoice, Interaction, InteractionError};

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
                  and verify in CI that a released tag matches what is recorded in source.\n\n\
                  Run without a subcommand to be guided through a bump. Naming a subcommand \
                  runs without prompting, always."
)]
pub struct Cli {
    /// Omitted to run interactively.
    #[command(subcommand)]
    command: Option<Command>,

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

    /// Create a vump.toml tracking every version file found here.
    ///
    /// Writes a configuration that keeps all discovered files at one version,
    /// with git actions off. Edit the result to split the repository into
    /// independently-versioned projects or to enable git actions.
    Init {
        /// Overwrite an existing vump.toml.
        #[arg(long)]
        force: bool,
    },

    /// Manage this installation of vump.
    ///
    /// These act on the binary rather than on a project, and so need no
    /// vump.toml.
    ///
    /// The variant cannot be called `Self`, which is a reserved word, so the
    /// command name is set explicitly rather than derived from it.
    #[command(subcommand, name = "self")]
    SelfCmd(SelfCommand),
}

/// Operations on the vump installation itself.
#[derive(Debug, Subcommand)]
enum SelfCommand {
    /// Replace this binary with a published release.
    Update {
        /// Install this exact version, newer or older.
        ///
        /// Naming a version bypasses the channel and the refusal to downgrade,
        /// which is how a rollback is expressed.
        #[arg(long, value_name = "VERSION")]
        to: Option<String>,

        /// Least mature kind of release to accept.
        #[arg(long, value_name = "CHANNEL", default_value = "stable")]
        channel: ChannelArg,
    },

    /// Report the running version and whether a newer one is published.
    Status {
        /// Least mature kind of release to consider.
        #[arg(long, value_name = "CHANNEL", default_value = "stable")]
        channel: ChannelArg,
    },

    /// List published releases, marking the running one.
    List {
        /// Least mature kind of release to include.
        #[arg(long, value_name = "CHANNEL", default_value = "stable")]
        channel: ChannelArg,

        /// Show at most this many, newest first.
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
    },
}

/// The least mature kind of release to accept.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ChannelArg {
    /// Finished releases only.
    Stable,
    /// Release candidates and finished releases.
    Rc,
    /// Betas and anything more mature.
    Beta,
    /// Everything, including alphas.
    Alpha,
}

impl From<ChannelArg> for Channel {
    fn from(arg: ChannelArg) -> Self {
        match arg {
            ChannelArg::Stable => Self::Stable,
            ChannelArg::Rc => Self::Rc,
            ChannelArg::Beta => Self::Beta,
            ChannelArg::Alpha => Self::Alpha,
        }
    }
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

    // init and update operate on the installation rather than on a project, so
    // they run before configuration is looked for. init exists precisely
    // because there is none yet.
    match &cli.command {
        Some(Command::Init { force }) => {
            let written = app::init::init(&RealFileSystem, &cwd, *force)?;
            render::init(&written, cli.json);
            return Ok(Exit::Success);
        }
        Some(Command::SelfCmd(command)) => return self_command(command, cli.json),
        _ => {}
    }

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

    let Some(command) = &cli.command else {
        return interactive(&ctx);
    };

    match command {
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
        // Handled before configuration discovery.
        Command::Init { .. } | Command::SelfCmd(_) => Ok(Exit::Success),
    }
}

/// The version of the running binary.
///
/// Taken from the manifest it was built from, which is the same value `check`
/// verifies against the release tag.
fn running_version() -> Result<semver::Version, CliError> {
    env!("CARGO_PKG_VERSION")
        .parse()
        .map_err(|e: semver::Error| CliError::InvalidVersionArgument {
            text: env!("CARGO_PKG_VERSION").to_owned(),
            detail: e.to_string(),
        })
}

/// Runs a command that acts on the installation rather than on a project.
fn self_command(command: &SelfCommand, json: bool) -> Result<Exit, CliError> {
    let current = running_version()?;
    let source = GitHubReleases::new();

    match command {
        SelfCommand::Status { channel } => {
            let outcome = app::update::status(&source, &current, (*channel).into())?;
            render::update(&outcome, json);

            // Scripts ask this to decide whether to act, so the answer is
            // carried by the exit status as well as by the output.
            Ok(match outcome {
                app::update::UpdateOutcome::Available { .. } => Exit::Failure,
                _ => Exit::Success,
            })
        }

        SelfCommand::List { channel, limit } => {
            let listing = app::update::list(&source, &current, (*channel).into())?;
            render::releases(&listing, *limit, json);
            Ok(Exit::Success)
        }

        SelfCommand::Update { to, channel } => {
            let requested = to
                .as_deref()
                .map(|text| {
                    app::check::parse_expected(text).map_err(|e| CliError::InvalidVersionArgument {
                        text: text.to_owned(),
                        detail: e.to_string(),
                    })
                })
                .transpose()?;

            let outcome = app::update::update(
                &source,
                &current,
                (std::env::consts::OS, std::env::consts::ARCH),
                (*channel).into(),
                requested.as_ref(),
            )?;

            render::update(&outcome, json);
            Ok(Exit::Success)
        }
    }
}

/// Guides a bump, asking only what has not already been decided.
///
/// Configuration is consulted first: a repository that declares its git
/// settings is never asked about them again. That leaves two questions in the
/// common case — which bump, and whether to proceed.
fn interactive(ctx: &Context) -> Result<Exit, CliError> {
    let ask = TerminalInteraction::new();

    // Which project.
    let project = match ctx.project.as_deref() {
        Some(name) => ctx.config.select(Some(name))?.clone(),
        None if ctx.config.projects.len() == 1 => ctx.config.projects[0].clone(),
        None => {
            let names: Vec<String> = ctx
                .config
                .projects
                .iter()
                .filter_map(|p| p.name.clone())
                .collect();
            let chosen = ask.choose_project(&names)?;
            ctx.config.select(Some(&chosen))?.clone()
        }
    };

    // What the current version is, resolving a disagreement if there is one.
    let files = app::read_project_versions(&ctx.fs, &ctx.root, &project)?;
    let base = resolve_base(&ask, &files)?;

    // Which transition, offering only those that would succeed.
    let offered = app::bump::valid_transitions_for(&base)?;
    let labelled: Vec<(String, String)> = offered
        .iter()
        .map(|(t, next)| (describe_transition(*t), next.to_string()))
        .collect();
    let index = ask.choose_transition(&base.to_string(), &labelled)?;
    let (transition, _) = offered
        .get(index)
        .ok_or(CliError::Interaction(InteractionError::Cancelled))?;

    // Which git actions, asked only when configuration has not decided.
    let intent = match configured_intent(&ctx.config.git) {
        Some(intent) => intent,
        None => intent_from(ask.choose_git()?),
    };

    let tag_pattern = ctx.config.tag_pattern_for(&project)?;
    let plan = app::bump::plan_from(
        &ctx.fs,
        &ctx.root,
        &project,
        Some(base),
        *transition,
        app::bump::GitPlanning {
            intent,
            commit_message: &ctx.config.git.commit_message,
            tag: &tag_pattern,
        },
    )?;

    if !ask.confirm(&render::summary(&plan))? {
        println!("Nothing was changed.");
        return Ok(Exit::Success);
    }

    let vcs = GitCli::new(&ctx.root);
    let outcome = app::bump::apply(&ctx.fs, &vcs, &ctx.root, &plan)?;
    let stale = app::lockfile::detect(&ctx.fs, &ctx.root, &outcome.written);
    render::bump(&plan, &outcome, &stale, ctx.json);

    Ok(if outcome.push_error.is_some() {
        Exit::Git
    } else {
        Exit::Success
    })
}

/// Determines the version to bump from, asking only if the files disagree.
fn resolve_base(
    ask: &dyn Interaction,
    files: &[app::FileVersion],
) -> Result<semver::Version, CliError> {
    let mut distinct: Vec<semver::Version> = Vec::new();
    for file in files {
        if !distinct.contains(&file.version) {
            distinct.push(file.version.clone());
        }
    }

    match distinct.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(CliError::Bump(BumpError::OutOfSync { found: Vec::new() })),
        _ => {
            // Each candidate is shown with the files recording it, so the
            // choice is made on evidence rather than on a bare version string.
            let candidates: Vec<(String, String)> = distinct
                .iter()
                .map(|version| {
                    let where_seen = files
                        .iter()
                        .filter(|f| f.version == *version)
                        .map(|f| f.path.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    (version.to_string(), where_seen)
                })
                .collect();

            let chosen = ask.choose_base(&candidates)?;
            chosen
                .parse()
                .map_err(|_| CliError::Interaction(InteractionError::Cancelled))
        }
    }
}

/// The git side-effects configuration has already decided, if any.
///
/// Returning `None` means configuration is silent on the matter, which is the
/// only case where asking is warranted.
fn configured_intent(settings: &crate::config::GitSettings) -> Option<GitIntent> {
    if settings.commit || settings.tag || settings.push {
        Some(GitIntent::resolve(settings, GitFlags::default()))
    } else {
        None
    }
}

fn intent_from(choice: GitChoice) -> GitIntent {
    match choice {
        GitChoice::None => GitIntent::default(),
        GitChoice::Commit => GitIntent {
            commit: true,
            tag: false,
            push: false,
        },
        GitChoice::Tag => GitIntent {
            commit: true,
            tag: true,
            push: false,
        },
        GitChoice::TagAndPush => GitIntent {
            commit: true,
            tag: true,
            push: true,
        },
    }
}

/// Names a transition the way the corresponding subcommand is spelled, so the
/// menu teaches the non-interactive equivalent.
fn describe_transition(transition: Transition) -> String {
    match transition {
        Transition::Stable(bump) => bump.to_string(),
        Transition::PreRelease {
            label,
            from: Some(base),
        } => format!("{label} --from {base}"),
        Transition::PreRelease { label, from: None } => label.to_string(),
        Transition::Release => "release".to_owned(),
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
    let tag_pattern = ctx.config.tag_pattern_for(project)?;

    let plan = app::bump::plan(
        &ctx.fs,
        &ctx.root,
        project,
        transition,
        app::bump::GitPlanning {
            intent,
            commit_message: &ctx.config.git.commit_message,
            tag: &tag_pattern,
        },
    )?;

    if dry_run {
        render::plan(&plan, ctx.json);
        return Ok(Exit::Success);
    }

    let vcs = GitCli::new(&ctx.root);
    let outcome = app::bump::apply(&ctx.fs, &vcs, &ctx.root, &plan)?;
    let stale = app::lockfile::detect(&ctx.fs, &ctx.root, &outcome.written);
    render::bump(&plan, &outcome, &stale, ctx.json);

    // A failed push leaves real, recoverable work behind, so it is reported as
    // a git failure rather than as a successful run.
    Ok(if outcome.push_error.is_some() {
        Exit::Git
    } else {
        Exit::Success
    })
}

/// Verifies that a project's files record the version a tag claims.
///
/// The argument may be a full tag (`api-v1.2.3`) or a bare version (`1.2.3`).
/// A tag identifies its own project, which is what lets a CI job pass
/// `github.ref_name` straight through without knowing which project it names.
fn check(ctx: &Context, version: &str) -> Result<Exit, CliError> {
    let (project, expected) =
        if let Some((project, from_tag)) = ctx.config.project_for_tag(version)? {
            // An explicit selection must not silently disagree with the tag.
            if let Some(requested) = ctx.project.as_deref() {
                let selected = ctx.config.select(Some(requested))?;
                if selected.name != project.name {
                    return Err(CliError::TagProjectMismatch {
                        tag: version.to_owned(),
                        from_tag: project.name.clone().unwrap_or_default(),
                        requested: requested.to_owned(),
                    });
                }
            }
            (project, from_tag)
        } else {
            // Not a tag this repository would produce, so it is read as a bare
            // version and the project selected the usual way.
            let expected = app::check::parse_expected(version).map_err(|e| {
                // Reporting only "not a version" is unhelpful for something that
                // clearly looks like a tag, so the patterns that were tried are
                // named.
                let patterns = ctx.config.tag_patterns();
                if version.contains('-') && !patterns.is_empty() {
                    CliError::UnrecognizedTag {
                        tag: version.to_owned(),
                        patterns: patterns.join(", "),
                    }
                } else {
                    CliError::InvalidVersionArgument {
                        text: version.to_owned(),
                        detail: e.to_string(),
                    }
                }
            })?;
            (ctx.config.select(ctx.project.as_deref())?, expected)
        };

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

    #[error("{0}")]
    Transition(#[from] TransitionError),

    #[error("{0}")]
    Interaction(#[from] InteractionError),

    #[error("{0}")]
    Init(#[from] app::init::InitError),

    #[error("{0}")]
    Update(#[from] app::update::UpdateError),

    #[error("{text:?} is not a valid version: {detail}")]
    InvalidVersionArgument { text: String, detail: String },

    #[error("tag {tag:?} belongs to project {from_tag:?}, but --project {requested:?} was given")]
    TagProjectMismatch {
        tag: String,
        from_tag: String,
        requested: String,
    },

    #[error("{tag:?} is neither a version nor a tag any project produces ({patterns})")]
    UnrecognizedTag { tag: String, patterns: String },

    #[error("cannot determine the current directory: {0}")]
    WorkingDirectory(String),
}

impl CliError {
    /// Maps a failure to its documented exit code.
    fn exit(&self) -> Exit {
        match self {
            Self::InvalidVersionArgument { .. }
            | Self::TagProjectMismatch { .. }
            | Self::UnrecognizedTag { .. } => Exit::Usage,
            Self::Config(_) => Exit::Config,
            Self::App(app) => app_exit(app),
            Self::Transition(_) => Exit::InvalidTransition,
            // Updating touches the installation rather than a project, so
            // nothing about it maps onto the project-shaped codes. Neither does
            // failing to resolve the working directory.
            Self::Update(_) | Self::WorkingDirectory(_) => Exit::Failure,
            Self::Init(init) => match init {
                app::init::InitError::Filesystem(FsError::Io { .. }) => Exit::Failure,
                app::init::InitError::AlreadyExists { .. }
                | app::init::InitError::NothingFound { .. }
                | app::init::InitError::Filesystem(FsError::NotFound { .. }) => Exit::Config,
            },
            Self::Interaction(interaction) => match interaction {
                // Declining is a decision, not a fault, but nothing was done,
                // so it must not look like a successful run either.
                InteractionError::Cancelled => Exit::Failure,
                // Reaching a prompt with nobody to answer it means the command
                // was invoked the wrong way for its environment.
                InteractionError::Unavailable { .. } => Exit::Usage,
            },
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
        assert!(matches!(
            cli.command,
            Some(Command::Check { version }) if version == "v1.2.3"
        ));
    }

    #[test]
    fn omitting_a_subcommand_selects_the_interactive_path() {
        let cli = Cli::try_parse_from(["vump"]).expect("bare vump must parse");
        assert!(cli.command.is_none());
    }

    #[test]
    fn every_bump_subcommand_is_reachable() {
        for name in ["patch", "minor", "major", "alpha", "beta", "rc", "release"] {
            Cli::try_parse_from(["vump", name])
                .unwrap_or_else(|e| panic!("{name} must be a subcommand: {e}"));
        }
    }

    #[test]
    fn no_git_cannot_be_combined_with_a_git_action() {
        // Asking for no git actions and for a commit in the same breath is a
        // contradiction, caught at parse time rather than silently resolved.
        assert!(Cli::try_parse_from(["vump", "patch", "--no-git", "--commit"]).is_err());
        assert!(Cli::try_parse_from(["vump", "patch", "--no-git", "--tag"]).is_err());
    }

    #[test]
    fn from_only_accepts_stable_bumps() {
        assert!(Cli::try_parse_from(["vump", "alpha", "--from", "minor"]).is_ok());
        assert!(Cli::try_parse_from(["vump", "alpha", "--from", "beta"]).is_err());
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

    #[test]
    fn installation_commands_live_under_self() {
        for args in [
            vec!["vump", "self", "update"],
            vec!["vump", "self", "status"],
            vec!["vump", "self", "list"],
        ] {
            Cli::try_parse_from(&args).unwrap_or_else(|e| panic!("{args:?} must parse: {e}"));
        }

        // `self` alone is a group, not an action.
        assert!(Cli::try_parse_from(["vump", "self"]).is_err());
    }

    #[test]
    fn every_channel_is_accepted_and_others_are_not() {
        for channel in ["stable", "rc", "beta", "alpha"] {
            Cli::try_parse_from(["vump", "self", "status", "--channel", channel])
                .unwrap_or_else(|e| panic!("{channel} must parse: {e}"));
        }
        assert!(Cli::try_parse_from(["vump", "self", "status", "--channel", "nightly"]).is_err());
    }

    #[test]
    fn the_channel_defaults_to_stable() {
        let cli = Cli::try_parse_from(["vump", "self", "status"]).unwrap();
        let Some(Command::SelfCmd(SelfCommand::Status { channel })) = cli.command else {
            panic!("expected self status");
        };
        assert!(matches!(channel, ChannelArg::Stable));
    }

    #[test]
    fn an_exact_version_can_be_requested() {
        let cli = Cli::try_parse_from(["vump", "self", "update", "--to", "1.2.3"]).unwrap();
        let Some(Command::SelfCmd(SelfCommand::Update { to, .. })) = cli.command else {
            panic!("expected self update");
        };
        assert_eq!(to.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn listing_accepts_a_limit() {
        let cli = Cli::try_parse_from(["vump", "self", "list", "--limit", "5"]).unwrap();
        let Some(Command::SelfCmd(SelfCommand::List { limit, .. })) = cli.command else {
            panic!("expected self list");
        };
        assert_eq!(limit, Some(5));
    }
}
