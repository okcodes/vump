//! What a run will write, and the git side-effects that follow.
//!
//! Bumping and setting differ only in how they arrive at a target version.
//! Once that is decided, the work is identical: write the same version to every
//! tracked file, then optionally commit, tag and push. That shared part lives
//! here, so neither operation has to describe itself in the other's terms.

use std::path::Path;

use semver::Version;
use thiserror::Error;

use crate::app::{AppError, FileVersion, resolve};
use crate::config::TagStyle;
use crate::config::{GitSettings, Project};
use crate::domain::TagPattern;
use crate::domain::TransitionError;
use crate::domain::version_file::Format;
use crate::ports::Annotation;
use crate::ports::{FileSystem, Vcs, VcsError};

/// A tracked file and the version it records today.
///
/// The version it will record is the change set's target, which is the same for
/// every file by construction: keeping the versions in step is the point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Path as declared in configuration.
    pub path: String,
    /// Version currently recorded.
    pub from: Version,
}

/// A tag to create, and how it will be written.
///
/// The annotation travels with the tag rather than beside it, so a run cannot
/// carry a message for a tag it will never create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagPlan {
    /// The tag name, rendered from the project's pattern.
    pub name: String,
    /// The message and signing choice, absent for a lightweight tag.
    pub annotation: Option<Annotation>,
}

impl TagPlan {
    /// How the tag is written, as it appears in configuration.
    #[must_use]
    pub fn style(&self) -> TagStyle {
        match &self.annotation {
            None => TagStyle::Lightweight,
            Some(annotation) if annotation.signed => TagStyle::Signed,
            Some(_) => TagStyle::Annotated,
        }
    }
}

/// The git side-effects a run will perform.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GitPlan {
    /// Commit message, when a commit will be made.
    pub commit: Option<String>,
    /// The tag to create, when one will be.
    pub tag: Option<TagPlan>,
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

/// Everything a run will do, decided before anything is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSet {
    /// Project name, when the repository declares several.
    pub project: Option<String>,
    /// Version every tracked file will record.
    pub target: Version,
    /// Tracked files and what they record now, in declaration order.
    pub files: Vec<FileChange>,
    /// Git side-effects.
    pub git: GitPlan,
}

impl ChangeSet {
    /// The version every tracked file records, when they all agree.
    ///
    /// Derived rather than stored: the per-file versions are the truth, and a
    /// separate field could contradict them. Returns `None` when the files
    /// disagree, which a bump refuses outright and a set exists to repair.
    #[must_use]
    pub fn common_origin(&self) -> Option<&Version> {
        let mut versions = self.files.iter().map(|f| &f.from);
        let first = versions.next()?;
        versions.all(|other| other == first).then_some(first)
    }

    /// Whether any file's version would actually change.
    ///
    /// Writing the version the files already record changes nothing, and git
    /// refuses an empty commit, so callers check this before acting.
    #[must_use]
    pub fn changes_anything(&self) -> bool {
        self.files.iter().any(|f| f.from != self.target)
    }
}

/// What actually happened when a change set was applied.
///
/// Git steps are reported individually because a later step failing does not
/// undo an earlier one: a push that fails after a successful commit and tag
/// must be reported as exactly that, not as a total failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
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
    /// Tag template for the project being changed.
    pub tag: &'a TagPattern,
    /// How the tag object is written.
    pub tag_style: TagStyle,
    /// Message template for an annotated or signed tag.
    pub tag_message: &'a str,
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

/// Why a change could not be planned or applied.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChangeError {
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
    /// `vump set` is the deliberate repair.
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

/// Builds the change set that writing `target` would produce.
#[must_use]
pub fn compose(
    project: &Project,
    target: Version,
    files: Vec<FileVersion>,
    planning: GitPlanning<'_>,
) -> ChangeSet {
    let git = GitPlan {
        commit: planning.intent.commit.then(|| {
            GitSettings::render(planning.commit_message, project.name.as_deref(), &target)
        }),
        tag: planning.intent.tag.then(|| TagPlan {
            name: planning.tag.render(&target),
            annotation: match planning.tag_style {
                TagStyle::Lightweight => None,
                TagStyle::Annotated | TagStyle::Signed => Some(Annotation {
                    message: GitSettings::render(
                        planning.tag_message,
                        project.name.as_deref(),
                        &target,
                    ),
                    signed: planning.tag_style == TagStyle::Signed,
                }),
            },
        }),
        push: planning.intent.push,
    };

    ChangeSet {
        project: project.name.clone(),
        target,
        files: files
            .into_iter()
            .map(|f| FileChange {
                path: f.path,
                from: f.version,
            })
            .collect(),
        git,
    }
}

/// Carries out a change set.
///
/// # Errors
///
/// Returns a [`ChangeError`] when the working tree is dirty and the run would
/// commit, when a file cannot be written, or when staging, committing or
/// tagging fails. A failed push is reported in the returned [`Outcome`] rather
/// than as an error, because the commit and tag it follows have already
/// succeeded.
pub fn apply(
    fs: &dyn FileSystem,
    vcs: &dyn Vcs,
    root: &Path,
    changes: &ChangeSet,
) -> Result<Outcome, ChangeError> {
    // Checked before anything is written: refusing after a partial write would
    // leave the tree in a state the user did not ask for.
    if changes.git.touches_repository() {
        let tree = vcs.status()?;
        if tree.is_dirty() {
            return Err(ChangeError::DirtyTree {
                changed: tree.changed,
            });
        }
    }

    let mut written = Vec::with_capacity(changes.files.len());
    for file in &changes.files {
        let absolute = resolve(root, &file.path);
        let file_name = absolute
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file.path.as_str());
        let format = Format::require(file_name).map_err(AppError::from)?;

        let contents = fs.read(&absolute).map_err(AppError::from)?;
        let updated = format
            .write(&file.path, &contents, &changes.target)
            .map_err(AppError::from)?;
        fs.write(&absolute, &updated).map_err(AppError::from)?;

        written.push(file.path.clone());
    }

    let mut outcome = Outcome {
        written,
        committed: false,
        tagged: false,
        pushed: false,
        push_error: None,
    };

    let Some(message) = changes.git.commit.as_deref() else {
        return Ok(outcome);
    };

    // Only the files vump manages are staged, so an unrelated change can never
    // ride along in a version-bump commit.
    let paths: Vec<String> = changes.files.iter().map(|f| f.path.clone()).collect();
    vcs.stage(&paths)?;
    vcs.commit(message)?;
    outcome.committed = true;

    if let Some(tag) = changes.git.tag.as_ref() {
        vcs.tag(&tag.name, tag.annotation.as_ref())?;
        outcome.tagged = true;
    }

    if changes.git.push {
        match vcs.push(changes.git.tag.as_ref().map(|t| t.name.as_str())) {
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
    use crate::config::DEFAULT_TAG_MESSAGE;
    use crate::config::DEFAULT_TAG_PATTERN;

    fn v(text: &str) -> Version {
        text.parse().unwrap()
    }

    fn project() -> Project {
        Project {
            name: None,
            files: vec!["VERSION".to_owned()],
            tag_pattern: None,
        }
    }

    fn pattern() -> TagPattern {
        TagPattern::parse(DEFAULT_TAG_PATTERN).unwrap()
    }

    fn changeset(files: &[(&str, &str)], target: &str, intent: GitIntent) -> ChangeSet {
        let settings = GitSettings::default();
        let pattern = pattern();
        compose(
            &project(),
            v(target),
            files
                .iter()
                .map(|(path, version)| FileVersion {
                    path: (*path).to_owned(),
                    version: v(version),
                })
                .collect(),
            GitPlanning {
                intent,
                commit_message: &settings.commit_message,
                tag: &pattern,
                tag_style: TagStyle::default(),
                tag_message: DEFAULT_TAG_MESSAGE,
            },
        )
    }

    /// Composes a tagging change set with `style` and `message`.
    fn tagged(style: TagStyle, message: &str, name: Option<&str>) -> ChangeSet {
        let pattern = TagPattern::parse(DEFAULT_TAG_PATTERN).unwrap();
        let settings = GitSettings::default();
        compose(
            &Project {
                name: name.map(ToOwned::to_owned),
                files: vec!["VERSION".to_owned()],
                tag_pattern: None,
            },
            v("1.2.4"),
            vec![FileVersion {
                path: "VERSION".to_owned(),
                version: v("1.2.3"),
            }],
            GitPlanning {
                intent: GitIntent {
                    commit: true,
                    tag: true,
                    push: false,
                },
                commit_message: &settings.commit_message,
                tag: &pattern,
                tag_style: style,
                tag_message: message,
            },
        )
    }

    #[test]
    fn an_annotated_tag_carries_its_rendered_message() {
        let tag = tagged(TagStyle::Annotated, DEFAULT_TAG_MESSAGE, None)
            .git
            .tag
            .unwrap();

        assert_eq!(tag.name, "v1.2.4");
        assert_eq!(
            tag.annotation,
            Some(Annotation {
                message: "Release 1.2.4".to_owned(),
                signed: false,
            })
        );
    }

    #[test]
    fn a_lightweight_tag_carries_no_annotation() {
        let tag = tagged(TagStyle::Lightweight, DEFAULT_TAG_MESSAGE, None)
            .git
            .tag
            .unwrap();
        assert_eq!(tag.annotation, None);
        assert_eq!(tag.style(), TagStyle::Lightweight);
    }

    #[test]
    fn signing_is_a_property_of_the_annotation() {
        // There is no way to ask for a signature without a message to sign:
        // the two travel together or not at all.
        let tag = tagged(TagStyle::Signed, DEFAULT_TAG_MESSAGE, None)
            .git
            .tag
            .unwrap();
        let annotation = tag.annotation.expect("a signed tag is an annotated one");
        assert!(annotation.signed);
        assert_eq!(annotation.message, "Release 1.2.4");
    }

    #[test]
    fn a_tag_message_names_its_project() {
        let tag = tagged(TagStyle::Annotated, "{project} {new_version}", Some("api"))
            .git
            .tag
            .unwrap();
        assert_eq!(tag.annotation.unwrap().message, "api 1.2.4");
    }

    #[test]
    fn a_shared_version_is_derived_from_the_files() {
        let changes = changeset(
            &[("VERSION", "1.2.3"), ("Cargo.toml", "1.2.3")],
            "2.0.0",
            GitIntent::default(),
        );
        assert_eq!(changes.common_origin(), Some(&v("1.2.3")));
    }

    #[test]
    fn files_that_disagree_have_no_shared_version() {
        // Deriving this rather than storing it means it cannot contradict the
        // per-file versions, which are the actual truth.
        let changes = changeset(
            &[("VERSION", "1.2.3"), ("Cargo.toml", "0.9.0")],
            "2.0.0",
            GitIntent::default(),
        );
        assert_eq!(changes.common_origin(), None);
    }

    #[test]
    fn writing_the_recorded_version_changes_nothing() {
        let changes = changeset(&[("VERSION", "1.2.3")], "1.2.3", GitIntent::default());
        assert!(!changes.changes_anything());
    }

    #[test]
    fn one_disagreeing_file_is_enough_to_be_a_change() {
        let changes = changeset(
            &[("VERSION", "1.2.3"), ("Cargo.toml", "0.9.0")],
            "1.2.3",
            GitIntent::default(),
        );
        assert!(changes.changes_anything());
    }

    #[test]
    fn applying_writes_every_file() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        let vcs = MemoryVcs::new();

        let changes = changeset(
            &[("VERSION", "1.2.3"), ("Cargo.toml", "1.2.3")],
            "1.3.0",
            GitIntent::default(),
        );
        let outcome = apply(&fs, &vcs, Path::new("/repo"), &changes).unwrap();

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

        let changes = changeset(
            &[("VERSION", "1.2.3")],
            "1.2.4",
            GitIntent {
                commit: true,
                tag: true,
                push: false,
            },
        );
        let outcome = apply(&fs, &vcs, Path::new("/repo"), &changes).unwrap();

        assert!(outcome.committed && outcome.tagged && !outcome.pushed);
        assert_eq!(
            vcs.calls(),
            [
                VcsCall::Stage(vec!["VERSION".to_owned()]),
                VcsCall::Commit("chore: bump version to v1.2.4".to_owned()),
                VcsCall::Tag(
                    "v1.2.4".to_owned(),
                    // The default style: annotated, with the default message.
                    Some(Annotation {
                        message: "Release 1.2.4".to_owned(),
                        signed: false,
                    }),
                ),
            ]
        );
    }

    #[test]
    fn a_dirty_tree_stops_the_run_before_anything_is_written() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.0.0\n");
        let vcs = MemoryVcs::new().with_changes(&["src/main.rs"]);

        let changes = changeset(
            &[("VERSION", "1.0.0")],
            "1.0.1",
            GitIntent {
                commit: true,
                ..GitIntent::default()
            },
        );

        let err = apply(&fs, &vcs, Path::new("/repo"), &changes).unwrap_err();
        assert!(matches!(err, ChangeError::DirtyTree { .. }));
        // The refusal must leave the file exactly as it was.
        assert_eq!(fs.get("/repo/VERSION").as_deref(), Some("1.0.0\n"));
    }

    #[test]
    fn a_dirty_tree_is_irrelevant_when_no_commit_is_planned() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.0.0\n");
        let vcs = MemoryVcs::new().with_changes(&["src/main.rs"]);

        let changes = changeset(&[("VERSION", "1.0.0")], "1.0.1", GitIntent::default());
        assert!(apply(&fs, &vcs, Path::new("/repo"), &changes).is_ok());
    }

    #[test]
    fn a_failed_push_reports_what_already_succeeded() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.0.0\n");
        let vcs = MemoryVcs::new().failing("push", "no upstream configured");

        let changes = changeset(
            &[("VERSION", "1.0.0")],
            "1.0.1",
            GitIntent {
                commit: true,
                tag: true,
                push: true,
            },
        );

        // The commit and tag really happened, so this is not a failed run.
        let outcome = apply(&fs, &vcs, Path::new("/repo"), &changes).unwrap();
        assert!(outcome.committed);
        assert!(outcome.tagged);
        assert!(!outcome.pushed);
        assert!(outcome.push_error.is_some());
    }

    #[test]
    fn only_tracked_files_are_staged() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.0.0\n")
            .with_file("/repo/unrelated.txt", "untouched");
        let vcs = MemoryVcs::new();

        let changes = changeset(
            &[("VERSION", "1.0.0")],
            "1.0.1",
            GitIntent {
                commit: true,
                ..GitIntent::default()
            },
        );
        apply(&fs, &vcs, Path::new("/repo"), &changes).unwrap();

        assert_eq!(vcs.calls()[0], VcsCall::Stage(vec!["VERSION".to_owned()]));
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

        let changes = changeset(&[("VERSION", "1.0.0")], "1.0.1", intent);
        assert!(!changes.git.touches_repository());
    }
}
