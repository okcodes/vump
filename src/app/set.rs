//! Writing an exact version to every tracked file.
//!
//! A bump requires the tracked files to agree and refuses when they do not, so
//! until a version can be named outright there is no way out of a
//! disagreement except editing by hand. This is that way out, and it is also
//! how a project whose files never agreed is adopted.
//!
//! There is deliberately no notion of a version being moved *from*. When the
//! files disagree there is no single one, and the per-file versions in the
//! resulting change set are the truth. A caller wanting the shared version,
//! when there is one, asks for it with
//! [`ChangeSet::common_origin`](crate::app::change::ChangeSet::common_origin).

use std::path::Path;

use semver::Version;

use crate::app::change::{ChangeError, ChangeSet, GitPlanning, compose};
use crate::app::read_project_versions;
use crate::config::Project;
use crate::ports::FileSystem;

/// Decides what writing `target` to every tracked file would change.
///
/// Unlike a bump this requires no prior agreement between the files: a
/// disagreement is precisely what naming an exact version repairs. It also does
/// not refuse to move backwards, for the same reason `self update --to` does
/// not — a version written out by hand is consent, and refusing would leave no
/// way to undo a mistaken bump.
///
/// # Errors
///
/// Returns a [`ChangeError`] when a tracked file cannot be read or interpreted.
pub fn set(
    fs: &dyn FileSystem,
    root: &Path,
    project: &Project,
    target: Version,
    planning: GitPlanning<'_>,
) -> Result<ChangeSet, ChangeError> {
    let files = read_project_versions(fs, root, project)?;
    Ok(compose(project, target, files, planning))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{MemoryFileSystem, MemoryVcs, VcsCall};
    use crate::app::change::{GitIntent, apply};
    use crate::config::DEFAULT_TAG_MESSAGE;
    use crate::config::TagStyle;
    use crate::config::{DEFAULT_TAG_PATTERN, GitSettings};
    use crate::domain::TagPattern;
    use crate::ports::Annotation;

    fn project(files: &[&str]) -> Project {
        Project {
            name: None,
            files: files.iter().map(|s| (*s).to_owned()).collect(),
            tag_pattern: None,
        }
    }

    fn v(text: &str) -> Version {
        text.parse().unwrap()
    }

    fn root() -> &'static Path {
        Path::new("/repo")
    }

    fn run(fs: &MemoryFileSystem, files: &[&str], target: &str, intent: GitIntent) -> ChangeSet {
        let settings = GitSettings::default();
        let pattern = TagPattern::parse(DEFAULT_TAG_PATTERN).unwrap();
        set(
            fs,
            root(),
            &project(files),
            v(target),
            GitPlanning {
                intent,
                commit_message: &settings.commit_message,
                tag: &pattern,
                tag_style: TagStyle::default(),
                tag_message: DEFAULT_TAG_MESSAGE,
            },
        )
        .unwrap()
    }

    #[test]
    fn writes_the_exact_version_asked_for() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.2.3\n");
        let vcs = MemoryVcs::new();

        let changes = run(&fs, &["VERSION"], "2.0.0", GitIntent::default());
        apply(&fs, &vcs, root(), &changes).unwrap();

        assert_eq!(fs.get("/repo/VERSION").as_deref(), Some("2.0.0\n"));
    }

    #[test]
    fn repairs_files_that_disagree() {
        // A bump refuses this outright. Repairing it is the whole point.
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"0.9.0\"\n");
        let vcs = MemoryVcs::new();

        let changes = run(
            &fs,
            &["VERSION", "Cargo.toml"],
            "2.0.0",
            GitIntent::default(),
        );

        // There is no single version they are moving from, and each file's own
        // is preserved.
        assert_eq!(changes.common_origin(), None);
        assert_eq!(changes.files[0].from, v("1.2.3"));
        assert_eq!(changes.files[1].from, v("0.9.0"));

        apply(&fs, &vcs, root(), &changes).unwrap();
        assert_eq!(fs.get("/repo/VERSION").as_deref(), Some("2.0.0\n"));
        assert!(fs.get("/repo/Cargo.toml").unwrap().contains("2.0.0"));
    }

    #[test]
    fn a_shared_version_is_still_available_when_files_agree() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"1.2.3\"\n");

        let changes = run(
            &fs,
            &["VERSION", "Cargo.toml"],
            "2.0.0",
            GitIntent::default(),
        );
        assert_eq!(changes.common_origin(), Some(&v("1.2.3")));
    }

    #[test]
    fn moves_backwards_without_complaint() {
        // Matching `self update --to`: a version written out by hand is
        // consent, and refusing would leave no way to undo a mistaken bump.
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "2.0.0\n");
        let vcs = MemoryVcs::new();

        let changes = run(&fs, &["VERSION"], "1.0.0", GitIntent::default());
        apply(&fs, &vcs, root(), &changes).unwrap();

        assert_eq!(fs.get("/repo/VERSION").as_deref(), Some("1.0.0\n"));
    }

    #[test]
    fn writing_the_recorded_version_changes_nothing() {
        // Committing an empty change is an error, so the caller checks this
        // rather than failing at the commit for an unrelated-looking reason.
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.2.3\n");
        let changes = run(&fs, &["VERSION"], "1.2.3", GitIntent::default());

        assert!(!changes.changes_anything());
    }

    #[test]
    fn a_partial_disagreement_still_counts_as_a_change() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "1.2.3\n")
            .with_file("/repo/Cargo.toml", "[package]\nversion = \"0.9.0\"\n");

        // One file already matches; the other does not, so there is work to do.
        let changes = run(
            &fs,
            &["VERSION", "Cargo.toml"],
            "1.2.3",
            GitIntent::default(),
        );
        assert!(changes.changes_anything());
    }

    #[test]
    fn tags_with_the_version_it_wrote() {
        let fs = MemoryFileSystem::new().with_file("/repo/VERSION", "1.0.0\n");
        let vcs = MemoryVcs::new();

        let changes = run(
            &fs,
            &["VERSION"],
            "3.0.0",
            GitIntent {
                commit: true,
                tag: true,
                push: false,
            },
        );
        apply(&fs, &vcs, root(), &changes).unwrap();

        assert_eq!(
            vcs.calls(),
            [
                VcsCall::Stage(vec!["VERSION".to_owned()]),
                VcsCall::Commit("chore: bump version to v3.0.0".to_owned()),
                VcsCall::Tag(
                    "v3.0.0".to_owned(),
                    // The default style: annotated, with the default message.
                    Some(Annotation {
                        message: "Release 3.0.0".to_owned(),
                        signed: false,
                    }),
                ),
            ]
        );
    }
}
