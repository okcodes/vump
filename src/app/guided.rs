//! The guided run: a bump whose decisions are asked for rather than passed in.
//!
//! It lives here rather than in the CLI because it is the operation carrying
//! the most decisions, and a decision that cannot be driven from memory cannot
//! be tested. Every question and every side-effect goes through a port, so the
//! whole run is exercisable without a terminal or a repository.
//!
//! Nothing here formats output. The confirmation text is rendered by the
//! caller and passed in, so presentation stays where the rest of it lives.

use std::path::Path;

use semver::Version;
use thiserror::Error;

use crate::app::bump::BumpPlan;
use crate::app::change::{BranchCheck, ChangeError, GitPlanning, Outcome};
use crate::app::{self, AppError, FileVersion};
use crate::config::{Config, ConfigError};
use crate::domain::{Transition, TransitionError};
use crate::ports::{
    Availability, FileSystem, Interaction, InteractionError, TransitionChoice, Vcs,
};

/// A guided run that went through.
#[derive(Debug)]
pub struct Application {
    /// What the run planned to do.
    pub plan: BumpPlan,
    /// What the repository did about it.
    pub outcome: Outcome,
}

/// What a guided run settled on.
///
/// The applied case is boxed because declining carries nothing: without it,
/// every caller would pay for the larger variant to say "nothing happened".
#[derive(Debug)]
pub enum Guided {
    /// Declined at the confirmation. Nothing was written.
    Declined,
    /// Approved, and applied.
    Applied(Box<Application>),
}

/// What a guided run is told before it starts asking.
#[derive(Debug, Clone, Copy)]
pub struct Guidance<'a> {
    /// Project named by the caller, when one was.
    pub project: Option<&'a str>,
    /// Whether the release-branch policy is enforced or waived.
    pub branch: BranchCheck,
    /// Renders a plan for the final confirmation.
    ///
    /// Passed in rather than called directly: which words a person sees is the
    /// caller's business, and a test needs neither.
    pub summary: fn(&BumpPlan) -> String,
    /// Names a bump for the menu, in the caller's own vocabulary.
    pub label: fn(Transition) -> String,
}

/// Why a guided run could not complete.
#[derive(Debug, Error)]
pub enum GuidedError {
    /// Configuration does not describe what was asked for.
    #[error("{0}")]
    Config(#[from] ConfigError),

    /// A tracked file could not be read or interpreted.
    #[error("{0}")]
    App(#[from] AppError),

    /// The change could not be planned or applied.
    #[error("{0}")]
    Change(#[from] ChangeError),

    /// No version transition is possible from here.
    #[error("{0}")]
    Transition(#[from] TransitionError),

    /// A question could not be asked, or was declined.
    #[error("{0}")]
    Interaction(#[from] InteractionError),
}

/// Guides a bump, asking only what has not already been decided.
///
/// Configuration is consulted first: a repository that declares its git
/// settings is never asked about them again. That leaves two questions in the
/// common case — which bump, and whether to proceed.
///
/// # Errors
///
/// Returns [`GuidedError`] when configuration, the files, the repository or the
/// person answering prevents the run from completing.
pub fn run(
    fs: &dyn FileSystem,
    vcs: &dyn Vcs,
    ask: &dyn Interaction,
    root: &Path,
    config: &Config,
    guidance: Guidance<'_>,
) -> Result<Guided, GuidedError> {
    // Which project.
    let project = match guidance.project {
        Some(name) => config.select(Some(name))?.clone(),
        None if config.projects.len() == 1 => config.projects[0].clone(),
        None => {
            let names: Vec<String> = config
                .projects
                .iter()
                .filter_map(|p| p.name.clone())
                .collect();
            let chosen = ask.choose_project(&names)?;
            config.select(Some(&chosen))?.clone()
        }
    };

    // What the current version is, resolving a disagreement if there is one.
    let files = app::read_project_versions(fs, root, &project)?;
    let base = resolve_base(ask, &files)?;

    // Which transition, offering only those that would succeed as versions.
    let offered = app::bump::valid_transitions_for(&base)?;
    let targets: Vec<Version> = offered.iter().map(|(_, next)| next.clone()).collect();

    // What the branch policy makes of each, settled before the question is
    // asked. A run that would be refused must not collect answers first, and a
    // bump that cannot be taken should say so in the list rather than be
    // discovered afterwards.
    //
    // With `through` unset the run counts as reaching git, because that is what
    // it does unless the next question says otherwise.
    let reaches_commit = config
        .git
        .through
        .is_none_or(crate::config::GitThrough::commits);
    let availability =
        app::change::offer(vcs, &targets, reaches_commit, &config.git, guidance.branch)?;

    // Whichever bump the policy has something to say about explains it for all
    // of them: the branch and the list are the same either way.
    if let Some(qualified) = availability.iter().position(|a| *a != Availability::Free) {
        if let Some(off) = app::change::check_branch(
            vcs,
            &targets[qualified],
            reaches_commit,
            &config.git,
            BranchCheck::Skipped,
        )? {
            ask.notice(&off.describe(guidance.branch));
        }
    }

    // Nothing selectable is a refusal, not a menu. Raised through the same
    // check, so the wording is the one every other refusal uses.
    if availability.iter().all(|a| *a == Availability::Blocked) {
        app::change::check_branch(
            vcs,
            &targets[0],
            reaches_commit,
            &config.git,
            BranchCheck::Enforce,
        )?;
    }

    let choices: Vec<TransitionChoice> = offered
        .iter()
        .zip(&availability)
        .map(|((transition, next), availability)| TransitionChoice {
            label: (guidance.label)(*transition),
            result: next.to_string(),
            availability: *availability,
        })
        .collect();

    let index = ask.choose_transition(&base.to_string(), &choices)?;
    let (transition, _) = offered.get(index).ok_or(InteractionError::Cancelled)?;

    // How far to carry the release, asked only when configuration has not said.
    let through = match config.git.through {
        Some(through) => through,
        None => ask.choose_git()?,
    };

    let tag_pattern = config.tag_pattern_for(&project)?;
    let plan = app::bump::plan_from(
        fs,
        root,
        &project,
        Some(base),
        *transition,
        GitPlanning {
            through,
            commit_message: &config.git.commit_message,
            tag: &tag_pattern,
            tag_style: config.git.tag_style,
            tag_message: &config.git.tag_message,
        },
    )?;

    if !ask.confirm(&(guidance.summary)(&plan))? {
        return Ok(Guided::Declined);
    }

    let outcome = app::change::apply(fs, vcs, root, &plan.changes)?;
    Ok(Guided::Applied(Box::new(Application { plan, outcome })))
}

/// Determines the version to bump from, asking only if the files disagree.
fn resolve_base(ask: &dyn Interaction, files: &[FileVersion]) -> Result<Version, GuidedError> {
    let mut distinct: Vec<Version> = Vec::new();
    for file in files {
        if !distinct.contains(&file.version) {
            distinct.push(file.version.clone());
        }
    }

    match distinct.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(ChangeError::OutOfSync { found: Vec::new() }.into()),
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
                .map_err(|_| InteractionError::Cancelled.into())
        }
    }
}
