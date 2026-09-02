//! Computing and applying a version bump.
//!
//! The work is split in two. [`plan`] is a pure decision: it reads what is
//! recorded, decides what the new version is, and describes every change that
//! would follow — without writing anything. [`apply`] carries a plan out.
//!
//! That split is what lets `--dry-run`, the JSON renderer, and the human
//! renderer all describe exactly what a real run would do, because all three
//! read the same [`BumpPlan`] the real run executes.

use std::path::Path;

use semver::Version;
use thiserror::Error;

use crate::app::{AppError, read_project_versions, resolve};
use crate::config::{GitSettings, Project};
use crate::domain::TagPattern;
use crate::domain::version_file::Format;
use crate::domain::{Transition, TransitionError, apply as transition_apply};

pub use crate::domain::bump::valid_transitions as valid_transitions_for;
use crate::ports::{FileSystem, Vcs, VcsError};

/// A single file's version change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Path as declared in configuration.
    pub path: String,
    /// Version currently recorded.
    pub from: Version,
    /// Version that will be recorded.
    pub to: Version,
}

/// The git side-effects a bump will perform.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GitPlan {
    /// Commit message, when a commit will be made.
    pub commit: Option<String>,
    /// Tag name, when a tag will be created.
    pub tag: Option<String>,
    /// Whether the commit and tag will be pushed.
    pub push: bool,
}

impl GitPlan {
    /// Whether the plan touches the repository at all.
    #[must_use]
    pub fn touches_repository(&self) -> bool {
        self.commit.is_some()
    }
}

/// Everything a bump will do, decided before anything is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BumpPlan {
    /// Project name, when the repository declares several.
    pub project: Option<String>,
    /// Version every tracked file currently records.
    ///
    /// `None` when they do not agree, which only a plan produced by [`set`] can
    /// be: a bump requires agreement, whereas setting an exact version is how
    /// disagreement is repaired. Each file's own previous version is always
    /// available in `changes`.
    pub current: Option<Version>,
    /// Version that will be recorded.
    pub next: Version,
    /// Per-file changes, in declaration order.
    pub changes: Vec<FileChange>,
    /// Git side-effects.
    pub git: GitPlan,
}

impl BumpPlan {
    /// Whether any file's version would actually change.
    ///
    /// Setting the version files already record changes nothing, and committing
    /// an empty change is an error rather than a no-op, so callers check this
    /// before acting.
    #[must_use]
    pub fn changes_anything(&self) -> bool {
        self.changes.iter().any(|c| c.from != c.to)
    }
}

/// What actually happened when a plan was applied.
///
/// Git steps are reported individually because a later step failing does not
/// undo an earlier one: a push that fails after a successful commit and tag
/// must be reported as exactly that, not as a total failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BumpOutcome {
    /// Files that were written.
    pub written: Vec<String>,
    /// Whether a commit was created.
    pub committed: bool,
    /// Whether a tag was created.
    pub tagged: bool,
    /// Whether the push succeeded.
    pub pushed: bool,
    /// Why the push failed, when it did.
    pub push_error: Option<String>,
}

/// Which git side-effects a run should perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GitIntent {
    /// Stage and commit.
    pub commit: bool,
    /// Create a tag.
    pub tag: bool,
    /// Push.
    pub push: bool,
}

/// How a release is named, and which side-effects to perform.
///
/// Naming travels with the intent because the tag template is resolved per
/// project: two independently-versioned projects must not produce the same tag.
#[derive(Debug, Clone, Copy)]
pub struct GitPlanning<'a> {
    /// Side-effects to perform.
    pub intent: GitIntent,
    /// Commit message template.
    pub commit_message: &'a str,
    /// Tag template for the project being bumped.
    pub tag: &'a TagPattern,
}

/// Git side-effects requested on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GitFlags {
    /// `--commit`
    pub commit: bool,
    /// `--tag`
    pub tag: bool,
    /// `--push`
    pub push: bool,
}

impl GitIntent {
    /// Combines configuration with command-line flags.
    ///
    /// Configuration is authoritative: a setting present in `vump.toml` is a
    /// decision already made, and is acted upon without being re-asked. Flags
    /// add to it for a single run.
    ///
    /// Tagging and pushing both require a commit to exist, so either implies
    /// one.
    ///
    /// Suppressing configured behavior for one run is the caller's decision,
    /// expressed by passing [`GitIntent::default`] instead of calling this.
    #[must_use]
    pub fn resolve(settings: &GitSettings, flags: GitFlags) -> Self {
        let tag = settings.tag || flags.tag;
        let push = settings.push || flags.push;
        Self {
            commit: settings.commit || flags.commit || tag || push,
            tag,
            push,
        }
    }
}

/// Why a bump could not be planned or applied.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BumpError {
    /// A tracked file could not be read or interpreted.
    #[error("{0}")]
    App(#[from] AppError),

    /// The requested transition is not meaningful from the current version.
    #[error("{0}")]
    Transition(#[from] TransitionError),

    /// Tracked files record different versions.
    ///
    /// There is deliberately no flag to resolve this without a human: a source
    /// of truth contradicting itself is an exceptional state, not a parameter.
    #[error("tracked files disagree about the current version:\n{}", format_disagreement(.found))]
    OutOfSync {
        /// Each file and the version it records.
        found: Vec<(String, Version)>,
    },

    /// The working tree has uncommitted changes and the run would commit.
    #[error(
        "the working tree has uncommitted changes, so a version bump would sweep them into its commit:\n{}\n\ncommit or stash them first, or re-run with --no-git",
        .changed.iter().map(|c| format!("  {c}")).collect::<Vec<_>>().join("\n")
    )]
    DirtyTree {
        /// Paths with uncommitted changes.
        changed: Vec<String>,
    },

    /// A git operation failed.
    #[error("{0}")]
    Vcs(#[from] VcsError),
}

fn format_disagreement(found: &[(String, Version)]) -> String {
    let width = found.iter().map(|(p, _)| p.len()).max().unwrap_or_default();
    found
        .iter()
        .map(|(path, version)| format!("  {path:<width$}  {version}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Decides what a bump would change, without writing anything.
///
/// # Errors
///
/// Returns a [`BumpError`] when files cannot be read, disagree about the
/// current version, or the transition is not meaningful.
pub fn plan(
    fs: &dyn FileSystem,
    root: &Path,
    project: &Project,
    transition: Transition,
    planning: GitPlanning<'_>,
) -> Result<BumpPlan, BumpError> {
    plan_from(fs, root, project, None, transition, planning)
}

/// Decides what a bump would change, treating `base` as the current version.
///
/// Passing a `base` skips the requirement that tracked files already agree,
/// which is how an interactive run repairs a disagreement the person has just
/// been shown and resolved. Passing `None` requires agreement, as [`plan`]
/// does.
///
/// # Errors
///
/// Returns a [`BumpError`] when files cannot be read, disagree with no `base`
/// given, or the transition is not meaningful.
pub fn plan_from(
    fs: &dyn FileSystem,
    root: &Path,
    project: &Project,
    base: Option<Version>,
    transition: Transition,
    planning: GitPlanning<'_>,
) -> Result<BumpPlan, BumpError> {
    let files = read_project_versions(fs, root, project)?;

    let current = if let Some(base) = base {
        base
    } else {
        let mut distinct: Vec<Version> = Vec::new();
        for file in &files {
            if !distinct.contains(&file.version) {
                distinct.push(file.version.clone());
            }
        }

        if distinct.len() > 1 {
            return Err(BumpError::OutOfSync {
                found: files
                    .iter()
                    .map(|f| (f.path.clone(), f.version.clone()))
                    .collect(),
            });
        }

        // `read_project_versions` returns at least one entry, because
        // configuration rejects a project with no files.
        distinct
            .into_iter()
            .next()
            .ok_or(BumpError::OutOfSync { found: Vec::new() })?
    };

    let next = transition_apply(&current, transition)?;

    Ok(compose(project, Some(current), next, files, planning))
}

/// Decides what writing an exact version would change.
///
/// Unlike a bump, this does not require the tracked files to agree beforehand:
/// a disagreement is precisely what naming an exact version repairs. It also
/// does not refuse to move backwards, for the same reason `self update --to`
/// does not — a version written out by hand is consent.
///
/// # Errors
///
/// Returns a [`BumpError`] when a tracked file cannot be read or interpreted.
pub fn set(
    fs: &dyn FileSystem,
    root: &Path,
    project: &Project,
    target: Version,
    planning: GitPlanning<'_>,
) -> Result<BumpPlan, BumpError> {
    let files = read_project_versions(fs, root, project)?;

    // Reported only when the files already agree; otherwise there is no single
    // version they are moving from, and each file's own is in `changes`.
    let mut distinct: Vec<&Version> = Vec::new();
    for file in &files {
        if !distinct.contains(&&file.version) {
            distinct.push(&file.version);
        }
    }
    let current = match distinct.as_slice() {
        [only] => Some((*only).clone()),
        _ => None,
    };

    Ok(compose(project, current, target, files, planning))
}

/// Builds the plan shared by bumping and setting.
fn compose(
    project: &Project,
    current: Option<Version>,
    next: Version,
    files: Vec<crate::app::FileVersion>,
    planning: GitPlanning<'_>,
) -> BumpPlan {
    let changes = files
        .into_iter()
        .map(|f| FileChange {
            path: f.path,
            from: f.version,
            to: next.clone(),
        })
        .collect();

    let git = GitPlan {
        commit: planning
            .intent
            .commit
            .then(|| GitSettings::render(planning.commit_message, project.name.as_deref(), &next)),
        tag: planning.intent.tag.then(|| planning.tag.render(&next)),
        push: planning.intent.push,
    };

    BumpPlan {
        project: project.name.clone(),
        current,
        next,
        changes,
        git,
    }
}

/// Carries out a plan.
///
/// # Errors
///
/// Returns a [`BumpError`] when the working tree is dirty and the plan would
/// commit, when a file cannot be written, or when staging, committing or
/// tagging fails. A failed push is reported in the returned [`BumpOutcome`]
/// rather than as an error, because the commit and tag it follows have already
/// succeeded.
pub fn apply(
    fs: &dyn FileSystem,
    vcs: &dyn Vcs,
    root: &Path,
    plan: &BumpPlan,
) -> Result<BumpOutcome, BumpError> {
    // Checked before anything is written: refusing after a partial write would
    // leave the tree in a state the user did not ask for.
    if plan.git.touches_repository() {
        let tree = vcs.status()?;
        if tree.is_dirty() {
            return Err(BumpError::DirtyTree {
                changed: tree.changed,
            });
        }
    }

    let mut written = Vec::with_capacity(plan.changes.len());
    for change in &plan.changes {
        let absolute = resolve(root, &change.path);
        let file_name = absolute
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(change.path.as_str());
        let format = Format::require(file_name).map_err(AppError::from)?;

        let contents = fs.read(&absolute).map_err(AppError::from)?;
        let updated = format
            .write(&change.path, &contents, &change.to)
            .map_err(AppError::from)?;
        fs.write(&absolute, &updated).map_err(AppError::from)?;

        written.push(change.path.clone());
    }

    let mut outcome = BumpOutcome {
        written,
        committed: false,
        tagged: false,
        pushed: false,
        push_error: None,
    };

    let Some(message) = plan.git.commit.as_deref() else {
        return Ok(outcome);
    };

    // Only the files vump manages are staged, so an unrelated change can never
    // ride along in a version-bump commit.
    let paths: Vec<String> = plan.changes.iter().map(|c| c.path.clone()).collect();
    vcs.stage(&paths)?;
    vcs.commit(message)?;
    outcome.committed = true;

    if let Some(tag) = plan.git.tag.as_deref() {
        vcs.tag(tag)?;
        outcome.tagged = true;
    }

    if plan.git.push {
        match vcs.push(plan.git.tag.as_deref()) {
            Ok(()) => outcome.pushed = true,
            Err(e) => outcome.push_error = Some(e.to_string()),
        }
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{MemoryFileSystem, MemoryVcs, VcsCall};
    use crate::domain::StableBump;

    fn project(files: &[&str]) -> Project {
        Project {
            name: None,
            files: files.iter().map(|s| (*s).to_owned()).collect(),
            tag_pattern: None,
        }
    }

    fn settings() -> GitSettings {
        GitSettings::default()
    }

    /// The repository-wide tag pattern, which is what a single-project
    /// repository uses.
    fn default_pattern() -> TagPattern {
        TagPattern::parse(crate::config::DEFAULT_TAG_PATTERN).expect("default pattern is valid")
    }

    fn planning<'a>(
        intent: GitIntent,
        settings: &'a GitSettings,
        tag: &'a TagPattern,
    ) -> GitPlanning<'a> {
        GitPlanning {
            intent,
            commit_message: &settings.commit_message,
            tag,
        }
    }

    fn v(text: &str) -> Version {
        text.parse().unwrap()
    }

    fn root() -> &'static Path {
        Path::new("/repo")
    }

    #[test]
    fn plans_a_patch_across_every_file() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/ui/package.json", r#"{"version":"1.2.3"}"#);

        let plan = plan(
            &fs,
            root(),
            &project(&["VERSION", "ui/package.json"]),
            Transition::Stable(StableBump::Patch),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap();

        assert_eq!(plan.current, Some(v("1.2.3")));
        assert_eq!(plan.next, v("1.2.4"));
        assert_eq!(plan.changes.len(), 2);
        assert!(!plan.git.touches_repository());
    }

    #[test]
    fn planning_writes_nothing() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.2.3\n");

        plan(
            &fs,
            root(),
            &project(&["VERSION"]),
            Transition::Stable(StableBump::Major),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap();

        assert_eq!(fs.get("/repo/VERSION").as_deref(), Some("1.2.3\n"));
    }

    #[test]
    fn disagreeing_files_are_refused() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.0.0\"\n");

        let err = plan(
            &fs,
            root(),
            &project(&["VERSION", "Cargo.toml"]),
            Transition::Stable(StableBump::Patch),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap_err();

        let BumpError::OutOfSync { found } = err else {
            panic!("expected OutOfSync, got {err:?}");
        };
        // Every file is listed, not just the odd one out: which is "wrong"
        // is the user's call.
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn applying_writes_every_file() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        let vcs = MemoryVcs::new();

        let plan = plan(
            &fs,
            root(),
            &project(&["VERSION", "Cargo.toml"]),
            Transition::Stable(StableBump::Minor),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap();

        let outcome = apply(&fs, &vcs, root(), &plan).unwrap();

        assert_eq!(outcome.written.len(), 2);
        assert_eq!(fs.get("/repo/VERSION").as_deref(), Some("1.3.0\n"));
        assert!(fs.get("/repo/Cargo.toml").unwrap().contains("1.3.0"));
        // With no git intent, the repository is untouched.
        assert!(vcs.calls().is_empty());
    }

    #[test]
    fn commit_and_tag_use_the_configured_templates() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.2.3\n");
        let vcs = MemoryVcs::new();

        let plan = plan(
            &fs,
            root(),
            &project(&["VERSION"]),
            Transition::Stable(StableBump::Patch),
            planning(
                GitIntent {
                    commit: true,
                    tag: true,
                    push: false,
                },
                &settings(),
                &default_pattern(),
            ),
        )
        .unwrap();

        let outcome = apply(&fs, &vcs, root(), &plan).unwrap();

        assert!(outcome.committed && outcome.tagged && !outcome.pushed);
        assert_eq!(
            vcs.calls(),
            [
                VcsCall::Stage(vec!["VERSION".to_owned()]),
                VcsCall::Commit("chore: bump version to v1.2.4".to_owned()),
                VcsCall::Tag("v1.2.4".to_owned()),
            ]
        );
    }

    #[test]
    fn only_tracked_files_are_staged() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.0.0\n")
            .with_file("/repo/unrelated.txt", "untouched");
        let vcs = MemoryVcs::new();

        let plan = plan(
            &fs,
            root(),
            &project(&["VERSION"]),
            Transition::Stable(StableBump::Patch),
            planning(
                GitIntent {
                    commit: true,
                    ..GitIntent::default()
                },
                &settings(),
                &default_pattern(),
            ),
        )
        .unwrap();
        apply(&fs, &vcs, root(), &plan).unwrap();

        assert_eq!(vcs.calls()[0], VcsCall::Stage(vec!["VERSION".to_owned()]));
    }

    #[test]
    fn a_dirty_tree_stops_the_run_before_anything_is_written() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.0.0\n");
        let vcs = MemoryVcs::new().with_changes(&["src/main.rs"]);

        let plan = plan(
            &fs,
            root(),
            &project(&["VERSION"]),
            Transition::Stable(StableBump::Patch),
            planning(
                GitIntent {
                    commit: true,
                    ..GitIntent::default()
                },
                &settings(),
                &default_pattern(),
            ),
        )
        .unwrap();

        let err = apply(&fs, &vcs, root(), &plan).unwrap_err();
        assert!(matches!(err, BumpError::DirtyTree { .. }));
        // The refusal must leave the file exactly as it was.
        assert_eq!(fs.get("/repo/VERSION").as_deref(), Some("1.0.0\n"));
    }

    #[test]
    fn a_dirty_tree_is_irrelevant_when_no_commit_is_planned() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.0.0\n");
        let vcs = MemoryVcs::new().with_changes(&["src/main.rs"]);

        let plan = plan(
            &fs,
            root(),
            &project(&["VERSION"]),
            Transition::Stable(StableBump::Patch),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap();

        assert!(apply(&fs, &vcs, root(), &plan).is_ok());
    }

    #[test]
    fn a_failed_push_reports_what_already_succeeded() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.0.0\n");
        let vcs = MemoryVcs::new().failing("push", "no upstream configured");

        let plan = plan(
            &fs,
            root(),
            &project(&["VERSION"]),
            Transition::Stable(StableBump::Patch),
            planning(
                GitIntent {
                    commit: true,
                    tag: true,
                    push: true,
                },
                &settings(),
                &default_pattern(),
            ),
        )
        .unwrap();

        // The commit and tag really happened, so this is not a failed run.
        let outcome = apply(&fs, &vcs, root(), &plan).unwrap();
        assert!(outcome.committed);
        assert!(outcome.tagged);
        assert!(!outcome.pushed);
        assert!(outcome.push_error.is_some());
    }

    #[test]
    fn an_invalid_transition_is_refused_at_planning_time() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.2.3-rc.1\n");

        let err = plan(
            &fs,
            root(),
            &project(&["VERSION"]),
            Transition::Stable(StableBump::Patch),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap_err();

        assert!(matches!(err, BumpError::Transition(_)));
    }

    // ─── set ─────────────────────────────────────────────────────────────────

    #[test]
    fn set_writes_the_exact_version_asked_for() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.2.3\n");
        let vcs = MemoryVcs::new();

        let plan = set(
            &fs,
            root(),
            &project(&["VERSION"]),
            v("2.0.0"),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap();

        apply(&fs, &vcs, root(), &plan).unwrap();
        assert_eq!(fs.get("/repo/VERSION").as_deref(), Some("2.0.0\n"));
    }

    #[test]
    fn set_repairs_files_that_disagree() {
        // A bump refuses this outright. Repairing it is the whole point of
        // naming an exact version.
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"0.9.0\"\n");
        let vcs = MemoryVcs::new();

        let plan = set(
            &fs,
            root(),
            &project(&["VERSION", "Cargo.toml"]),
            v("2.0.0"),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap();

        // There is no single version they are moving from, and saying otherwise
        // would misreport one of them.
        assert!(plan.current.is_none());
        assert_eq!(plan.changes[0].from, v("1.2.3"));
        assert_eq!(plan.changes[1].from, v("0.9.0"));

        apply(&fs, &vcs, root(), &plan).unwrap();
        assert_eq!(fs.get("/repo/VERSION").as_deref(), Some("2.0.0\n"));
        assert!(fs.get("/repo/Cargo.toml").unwrap().contains("2.0.0"));
    }

    #[test]
    fn set_reports_the_shared_version_when_files_already_agree() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.2.3\"\n");

        let plan = set(
            &fs,
            root(),
            &project(&["VERSION", "Cargo.toml"]),
            v("2.0.0"),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap();

        assert_eq!(plan.current, Some(v("1.2.3")));
    }

    #[test]
    fn set_moves_backwards_without_complaint() {
        // Matching `self update --to`: a version written out by hand is
        // consent, and refusing would leave no way to undo a mistaken bump.
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "2.0.0\n");
        let vcs = MemoryVcs::new();

        let plan = set(
            &fs,
            root(),
            &project(&["VERSION"]),
            v("1.0.0"),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap();

        apply(&fs, &vcs, root(), &plan).unwrap();
        assert_eq!(fs.get("/repo/VERSION").as_deref(), Some("1.0.0\n"));
    }

    #[test]
    fn setting_the_recorded_version_changes_nothing() {
        // Committing an empty change is an error, so the caller checks this
        // rather than failing at the commit for an unrelated-looking reason.
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.2.3\n");

        let plan = set(
            &fs,
            root(),
            &project(&["VERSION"]),
            v("1.2.3"),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap();

        assert!(!plan.changes_anything());
    }

    #[test]
    fn a_partial_disagreement_still_counts_as_a_change() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"0.9.0\"\n");

        let plan = set(
            &fs,
            root(),
            &project(&["VERSION", "Cargo.toml"]),
            v("1.2.3"),
            planning(GitIntent::default(), &settings(), &default_pattern()),
        )
        .unwrap();

        // One file already matches; the other does not, so there is work to do.
        assert!(plan.changes_anything());
    }

    #[test]
    fn set_tags_with_the_version_it_wrote() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.0.0\n");
        let vcs = MemoryVcs::new();

        let plan = set(
            &fs,
            root(),
            &project(&["VERSION"]),
            v("3.0.0"),
            planning(
                GitIntent {
                    commit: true,
                    tag: true,
                    push: false,
                },
                &settings(),
                &default_pattern(),
            ),
        )
        .unwrap();

        apply(&fs, &vcs, root(), &plan).unwrap();
        assert_eq!(
            vcs.calls(),
            [
                VcsCall::Stage(vec!["VERSION".to_owned()]),
                VcsCall::Commit("chore: bump version to v3.0.0".to_owned()),
                VcsCall::Tag("v3.0.0".to_owned()),
            ]
        );
    }

    #[test]
    fn configuration_is_authoritative_for_git_intent() {
        let configured = GitSettings {
            commit: true,
            tag: true,
            ..GitSettings::default()
        };

        // Nothing on the command line: configuration alone decides.
        let intent = GitIntent::resolve(&configured, GitFlags::default());
        assert_eq!(
            intent,
            GitIntent {
                commit: true,
                tag: true,
                push: false
            }
        );
    }

    #[test]
    fn tagging_or_pushing_implies_committing() {
        let none = GitSettings::default();

        let tagged = GitIntent::resolve(
            &none,
            GitFlags {
                tag: true,
                ..GitFlags::default()
            },
        );
        assert!(tagged.commit, "a tag needs a commit to point at");

        let pushed = GitIntent::resolve(
            &none,
            GitFlags {
                push: true,
                ..GitFlags::default()
            },
        );
        assert!(pushed.commit, "there is nothing to push without a commit");
    }

    #[test]
    fn the_default_intent_touches_nothing() {
        // This is what `--no-git` selects, bypassing `resolve` entirely.
        let intent = GitIntent::default();
        assert!(!intent.commit && !intent.tag && !intent.push);

        let plan = GitPlan {
            commit: None,
            tag: None,
            push: false,
        };
        assert!(!plan.touches_repository());
    }
}
