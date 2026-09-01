//! Detecting lock files left stale by a version bump.
//!
//! A lock file records the resolved dependency graph, and several formats
//! record the project's own version alongside it. Bumping a manifest without
//! refreshing its lock leaves the two disagreeing, which build tooling may
//! reject outright rather than merely warn about.
//!
//! vump does not run package managers: doing so would turn a fast, predictable
//! edit into an arbitrarily slow one with network access and its own failure
//! modes. It reports what has gone stale and the command that refreshes it.

use std::path::Path;

use crate::app::resolve;
use crate::ports::FileSystem;

/// A lock file that a bump has likely invalidated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleLock {
    /// Path relative to the configuration's directory.
    pub path: String,
    /// A command that brings it back into agreement.
    pub refresh_with: &'static str,
}

/// Lock files that sit beside a manifest, and what refreshes each.
const SIBLINGS: &[(&str, &[(&str, &str)])] = &[(
    "package.json",
    &[
        ("package-lock.json", "npm install"),
        ("yarn.lock", "yarn install"),
        ("pnpm-lock.yaml", "pnpm install"),
        ("bun.lockb", "bun install"),
    ],
)];

/// How far above a manifest to look for a workspace-level lock file.
const WORKSPACE_SEARCH_DEPTH: usize = 3;

/// Reports lock files invalidated by writing `changed`.
///
/// Paths are relative to `root`, matching how they are declared and displayed.
pub fn detect(fs: &dyn FileSystem, root: &Path, changed: &[String]) -> Vec<StaleLock> {
    let mut stale: Vec<StaleLock> = Vec::new();

    for path in changed {
        let (directory, name) = split(path);

        match name {
            "Cargo.toml" => {
                // A crate in a workspace shares one Cargo.lock at the workspace
                // root, so the search walks upward as well as looking beside.
                if let Some(found) = find_upward(fs, root, directory, "Cargo.lock") {
                    push_unique(
                        &mut stale,
                        StaleLock {
                            path: found,
                            refresh_with: "cargo check",
                        },
                    );
                }
            }
            _ => {
                for (manifest, locks) in SIBLINGS {
                    if *manifest != name {
                        continue;
                    }
                    for (lock, command) in *locks {
                        let candidate = join(directory, lock);
                        if fs.is_file(&resolve(root, &candidate)) {
                            push_unique(
                                &mut stale,
                                StaleLock {
                                    path: candidate,
                                    refresh_with: command,
                                },
                            );
                            // One lock per manifest: a project uses one package
                            // manager, and naming the rest would be noise.
                            break;
                        }
                    }
                }
            }
        }
    }

    stale
}

/// Searches `directory` and its ancestors, up to the configuration root.
fn find_upward(fs: &dyn FileSystem, root: &Path, directory: &str, name: &str) -> Option<String> {
    let mut current = directory;

    for _ in 0..=WORKSPACE_SEARCH_DEPTH {
        let candidate = join(current, name);
        if fs.is_file(&resolve(root, &candidate)) {
            return Some(candidate);
        }
        if current.is_empty() {
            break;
        }
        current = current.rsplit_once('/').map_or("", |(parent, _)| parent);
    }

    None
}

/// Splits a configured path into its directory and file name.
fn split(path: &str) -> (&str, &str) {
    path.rsplit_once('/')
        .map_or(("", path), |(dir, name)| (dir, name))
}

/// Joins a directory and name using the forward slashes configured paths use.
fn join(directory: &str, name: &str) -> String {
    if directory.is_empty() {
        name.to_owned()
    } else {
        format!("{directory}/{name}")
    }
}

fn push_unique(into: &mut Vec<StaleLock>, lock: StaleLock) {
    if !into.iter().any(|existing| existing.path == lock.path) {
        into.push(lock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::MemoryFileSystem;

    fn root() -> &'static Path {
        Path::new("/repo")
    }

    fn changed(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn reports_a_cargo_lock_beside_the_manifest() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/Cargo.toml", "")
            .with_file("/repo/Cargo.lock", "");

        assert_eq!(
            detect(&fs, root(), &changed(&["Cargo.toml"])),
            [StaleLock {
                path: "Cargo.lock".to_owned(),
                refresh_with: "cargo check",
            }]
        );
    }

    #[test]
    fn finds_a_workspace_lock_above_a_member_crate() {
        // Workspace members share one lock at the workspace root.
        let fs = MemoryFileSystem::new()
            .with_file("/repo/crates/api/Cargo.toml", "")
            .with_file("/repo/Cargo.lock", "");

        assert_eq!(
            detect(&fs, root(), &changed(&["crates/api/Cargo.toml"]))[0].path,
            "Cargo.lock"
        );
    }

    #[test]
    fn prefers_the_nearest_lock_file() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/crates/api/Cargo.toml", "")
            .with_file("/repo/crates/api/Cargo.lock", "")
            .with_file("/repo/Cargo.lock", "");

        assert_eq!(
            detect(&fs, root(), &changed(&["crates/api/Cargo.toml"]))[0].path,
            "crates/api/Cargo.lock"
        );
    }

    #[test]
    fn names_the_package_manager_the_lock_belongs_to() {
        let cases = [
            ("pnpm-lock.yaml", "pnpm install"),
            ("yarn.lock", "yarn install"),
            ("package-lock.json", "npm install"),
        ];

        for (lock, command) in cases {
            let fs = MemoryFileSystem::new()
                .with_file("/repo/package.json", "")
                .with_file(format!("/repo/{lock}"), "");

            let found = detect(&fs, root(), &changed(&["package.json"]));
            assert_eq!(found.len(), 1, "{lock}");
            assert_eq!(found[0].refresh_with, command, "{lock}");
        }
    }

    #[test]
    fn reports_nothing_when_no_lock_exists() {
        let fs = MemoryFileSystem::new().with_file("/repo/Cargo.toml", "");
        assert!(detect(&fs, root(), &changed(&["Cargo.toml"])).is_empty());
    }

    #[test]
    fn a_plain_version_file_has_no_lock() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/VERSION", "")
            .with_file("/repo/Cargo.lock", "");

        // A VERSION file belongs to no package manager, so a neighbouring lock
        // is not its concern.
        assert!(detect(&fs, root(), &changed(&["VERSION"])).is_empty());
    }

    #[test]
    fn one_lock_is_reported_once_for_several_manifests() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/crates/a/Cargo.toml", "")
            .with_file("/repo/crates/b/Cargo.toml", "")
            .with_file("/repo/Cargo.lock", "");

        let found = detect(
            &fs,
            root(),
            &changed(&["crates/a/Cargo.toml", "crates/b/Cargo.toml"]),
        );
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn each_project_gets_its_own_lock() {
        let fs = MemoryFileSystem::new()
            .with_file("/repo/api/Cargo.toml", "")
            .with_file("/repo/api/Cargo.lock", "")
            .with_file("/repo/web/package.json", "")
            .with_file("/repo/web/package-lock.json", "");

        let found = detect(
            &fs,
            root(),
            &changed(&["api/Cargo.toml", "web/package.json"]),
        );
        let paths: Vec<&str> = found.iter().map(|l| l.path.as_str()).collect();
        assert_eq!(paths, ["api/Cargo.lock", "web/package-lock.json"]);
    }
}
