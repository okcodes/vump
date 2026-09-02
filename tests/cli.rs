//! End-to-end tests driving the compiled binary.
//!
//! This layer owns the two contracts that cannot be proven anywhere else: that
//! naming a subcommand never results in a prompt, and that each failure mode
//! exits with its documented code. Both are observable only from outside the
//! process.

use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

/// A scratch repository to run vump against.
struct Fixture {
    dir: TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            dir: TempDir::new().expect("cannot create temp dir"),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(self, name: &str, contents: &str) -> Self {
        let target = self.dir.path().join(name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("cannot create parent directory");
        }
        std::fs::write(target, contents).expect("cannot write fixture file");
        self
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(name)).expect("cannot read fixture file")
    }

    /// Runs vump with stdin closed.
    ///
    /// Closed stdin is the point: a command that tried to prompt would have no
    /// terminal to prompt on, so any command that completes here is proven not
    /// to depend on interactive input.
    fn run(&self, args: &[&str]) -> Run {
        let output = Command::new(env!("CARGO_BIN_EXE_vump"))
            .args(args)
            .current_dir(self.dir.path())
            .stdin(Stdio::null())
            .output()
            .expect("cannot run vump");
        Run::from(output)
    }

    /// Tags present in the fixture repository, in git's own order.
    fn tags(&self) -> Vec<String> {
        let out = Command::new("git")
            .args(["tag"])
            .current_dir(self.dir.path())
            .output()
            .expect("cannot list tags");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }

    fn git(&self, args: &[&str]) -> &Self {
        let status = Command::new("git")
            .args(args)
            .current_dir(self.dir.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("cannot run git");
        assert!(status.success(), "git {args:?} failed");
        self
    }

    /// Creates a git repository with an initial commit.
    fn with_git(self) -> Self {
        self.git(&["init", "-q", "."])
            .git(&["config", "user.email", "test@example.com"])
            .git(&["config", "user.name", "Test"])
            .git(&["config", "commit.gpgsign", "false"])
            .git(&["add", "-A"])
            .git(&["commit", "-qm", "initial"]);
        self
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl From<Output> for Run {
    fn from(output: Output) -> Self {
        Self {
            code: output.status.code().expect("process was terminated"),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

impl Run {
    fn output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{}", self.stdout))
    }
}

const SINGLE: &str = "files = [\"VERSION\"]\n";
const SINGLE_PACKAGE: &str = "files = [\"package.json\"]\n";

// ─── Exit codes ──────────────────────────────────────────────────────────────

#[test]
fn check_exits_zero_when_versions_match() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n");

    let run = fx.run(&["check", "1.2.3"]);
    assert_eq!(run.code, 0, "{}", run.output());
}

#[test]
fn check_accepts_a_v_prefixed_tag() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "0.2.0-alpha.1\n");

    assert_eq!(fx.run(&["check", "v0.2.0-alpha.1"]).code, 0);
}

#[test]
fn check_exits_four_on_mismatch() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n");

    let run = fx.run(&["check", "9.9.9"]);
    assert_eq!(run.code, 4, "{}", run.output());
    assert!(run.stderr.contains("9.9.9"));
}

#[test]
fn a_malformed_version_argument_exits_two() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n");

    assert_eq!(fx.run(&["check", "not-a-version"]).code, 2);
}

#[test]
fn missing_configuration_exits_three() {
    let fx = Fixture::new();
    let run = fx.run(&["status"]);
    assert_eq!(run.code, 3, "{}", run.output());
    assert!(run.stderr.contains("vump.toml"));
}

#[test]
fn disagreeing_files_exit_five() {
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"VERSION\", \"Cargo.toml\"]\n")
        .write("VERSION", "1.2.3\n")
        .write("Cargo.toml", "[package]\nversion = \"1.0.0\"\n");

    let run = fx.run(&["patch", "--no-git"]);
    assert_eq!(run.code, 5, "{}", run.output());
}

#[test]
fn a_dirty_tree_exits_six() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n")
        .with_git()
        .write("stray.txt", "uncommitted");

    let run = fx.run(&["patch", "--commit"]);
    assert_eq!(run.code, 6, "{}", run.output());
    // Refusing must leave the version untouched.
    assert_eq!(fx.read("VERSION"), "1.2.3\n");
}

#[test]
fn an_impossible_transition_exits_seven() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3-rc.1\n");

    let run = fx.run(&["patch", "--no-git"]);
    assert_eq!(run.code, 7, "{}", run.output());
    // The message must point at the command that resolves the ambiguity.
    assert!(run.stderr.contains("release"), "{}", run.stderr);
}

#[test]
fn starting_a_pre_release_without_a_base_exits_seven() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n");

    let run = fx.run(&["alpha", "--no-git"]);
    assert_eq!(run.code, 7, "{}", run.output());
    assert!(run.stderr.contains("--from"), "{}", run.stderr);
}

// ─── The non-interactive contract ────────────────────────────────────────────

#[test]
fn no_subcommand_ever_waits_for_input() {
    // Each of these would have needed a question from the user under a design
    // that falls back to prompting. With stdin closed, any of them hanging or
    // consuming input would fail this test.
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"VERSION\", \"Cargo.toml\"]\n")
        .write("VERSION", "1.2.3\n")
        .write("Cargo.toml", "[package]\nversion = \"1.0.0\"\n");

    // Files disagree: resolved by erroring, not by asking which to trust.
    assert_eq!(fx.run(&["patch", "--no-git"]).code, 5);

    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n");

    // Pre-release from stable: resolved by requiring a flag, not by asking.
    assert_eq!(fx.run(&["beta", "--no-git"]).code, 7);

    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3-beta.2\n");

    // Stable bump on a pre-release: resolved by erroring, not by asking.
    assert_eq!(fx.run(&["minor", "--no-git"]).code, 7);
}

#[test]
fn the_interactive_path_refuses_without_a_terminal_instead_of_hanging() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n");

    // Bare vump is the interactive entry point. With no terminal attached it
    // must say so and stop, never block waiting for input that cannot arrive.
    let run = fx.run(&[]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.stderr.contains("terminal"), "{}", run.stderr);
    assert_eq!(fx.read("VERSION"), "1.2.3\n");
}

// ─── Bumping ─────────────────────────────────────────────────────────────────

#[test]
fn a_bump_writes_every_tracked_file() {
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"VERSION\", \"ui/package.json\"]\n")
        .write("VERSION", "1.2.3\n")
        .write("ui/package.json", "{\n  \"version\": \"1.2.3\"\n}\n");

    let run = fx.run(&["minor", "--no-git"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert_eq!(fx.read("VERSION"), "1.3.0\n");
    assert!(fx.read("ui/package.json").contains("\"1.3.0\""));
}

#[test]
fn dry_run_writes_nothing() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n");

    let run = fx.run(&["major", "--dry-run", "--no-git"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert_eq!(fx.read("VERSION"), "1.2.3\n");
    assert!(run.stdout.contains("2.0.0"));
}

#[test]
fn a_pre_release_sequence_advances_and_finalizes() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n");

    assert_eq!(fx.run(&["alpha", "--from", "minor", "--no-git"]).code, 0);
    assert_eq!(fx.read("VERSION"), "1.3.0-alpha.0\n");

    assert_eq!(fx.run(&["alpha", "--no-git"]).code, 0);
    assert_eq!(fx.read("VERSION"), "1.3.0-alpha.1\n");

    assert_eq!(fx.run(&["rc", "--no-git"]).code, 0);
    assert_eq!(fx.read("VERSION"), "1.3.0-rc.0\n");

    assert_eq!(fx.run(&["release", "--no-git"]).code, 0);
    assert_eq!(fx.read("VERSION"), "1.3.0\n");
}

#[test]
fn moving_to_a_less_mature_channel_is_refused() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3-rc.2\n");

    let run = fx.run(&["alpha", "--no-git"]);
    assert_eq!(run.code, 7, "{}", run.output());
    assert_eq!(fx.read("VERSION"), "1.2.3-rc.2\n");
}

#[test]
fn a_bump_commits_and_tags_when_asked() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n")
        .with_git();

    let run = fx.run(&["patch", "--tag"]);
    assert_eq!(run.code, 0, "{}", run.output());

    assert_eq!(fx.tags(), ["v1.2.4"]);
}

#[test]
fn configuration_alone_drives_git_actions() {
    // No git flags are passed: settings in vump.toml are decisions already
    // made, and must be acted upon rather than ignored.
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"VERSION\"]\n[git]\ntag = true\n")
        .write("VERSION", "1.2.3\n")
        .with_git();

    assert_eq!(fx.run(&["patch"]).code, 0);

    assert_eq!(fx.tags(), ["v1.2.4"]);
}

#[test]
fn no_git_overrides_configured_actions() {
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"VERSION\"]\n[git]\ntag = true\n")
        .write("VERSION", "1.2.3\n")
        .with_git();

    assert_eq!(fx.run(&["patch", "--no-git"]).code, 0);
    assert_eq!(fx.read("VERSION"), "1.2.4\n");

    assert!(fx.tags().is_empty());
}

#[test]
fn a_bump_reports_lock_files_it_leaves_stale() {
    // A stale Cargo.lock is not cosmetic: it fails a --locked build, which is
    // what release pipelines run.
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"crates/api/Cargo.toml\"]\n")
        .write("crates/api/Cargo.toml", "[package]\nversion = \"1.0.0\"\n")
        .write("Cargo.lock", "# workspace lock\n");

    let run = fx.run(&["patch", "--no-git"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(run.stdout.contains("Cargo.lock"), "{}", run.stdout);
    assert!(run.stdout.contains("cargo check"), "{}", run.stdout);

    // The lock itself is never rewritten; vump does not run package managers.
    assert_eq!(fx.read("Cargo.lock"), "# workspace lock\n");
}

#[test]
fn stale_locks_appear_in_json_output() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE_PACKAGE)
        .write("package.json", "{\"version\":\"1.0.0\"}")
        .write("package-lock.json", "{}");

    let value = fx.run(&["patch", "--no-git", "--json"]).json();
    let locks = value["stale_locks"]
        .as_array()
        .expect("stale_locks must be an array");

    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0]["path"], "package-lock.json");
    assert_eq!(locks[0]["refresh_with"], "npm install");
}

#[test]
fn a_bump_with_no_lock_files_reports_none() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.0.0\n");

    let value = fx.run(&["patch", "--no-git", "--json"]).json();
    assert!(
        value["stale_locks"]
            .as_array()
            .expect("stale_locks must be present")
            .is_empty()
    );
}

// ─── Setting an exact version ────────────────────────────────────────────────

#[test]
fn set_repairs_files_that_a_bump_refuses() {
    // The dead end this command exists for: `patch` exits 5 here, and the only
    // other repair was editing by hand.
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"VERSION\", \"Cargo.toml\"]\n")
        .write("VERSION", "1.2.3\n")
        .write("Cargo.toml", "[package]\nversion = \"0.9.0\"\n");

    assert_eq!(fx.run(&["patch", "--no-git"]).code, 5);

    let run = fx.run(&["set", "2.0.0", "--no-git"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert_eq!(fx.read("VERSION"), "2.0.0\n");
    assert!(fx.read("Cargo.toml").contains("2.0.0"));

    // Repaired, so an ordinary bump works again.
    assert_eq!(fx.run(&["patch", "--no-git"]).code, 0);
    assert_eq!(fx.read("VERSION"), "2.0.1\n");
}

#[test]
fn set_accepts_a_v_prefix() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.0.0\n");

    assert_eq!(fx.run(&["set", "v2.5.0", "--no-git"]).code, 0);
    assert_eq!(fx.read("VERSION"), "2.5.0\n");
}

#[test]
fn set_moves_backwards_without_complaint() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "2.0.0\n");

    assert_eq!(fx.run(&["set", "1.0.0", "--no-git"]).code, 0);
    assert_eq!(fx.read("VERSION"), "1.0.0\n");
}

#[test]
fn setting_the_recorded_version_is_a_no_op_not_a_failure() {
    // Configuration asks for a commit, and git refuses an empty one. The run
    // must report success rather than fail for a reason unrelated to the ask.
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"VERSION\"]\n[git]\ncommit = true\n")
        .write("VERSION", "1.2.3\n")
        .with_git();

    let run = fx.run(&["set", "1.2.3"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(run.stdout.contains("nothing to do"), "{}", run.stdout);
    assert!(fx.tags().is_empty());
}

#[test]
fn set_commits_and_tags_like_a_bump() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.0.0\n")
        .with_git();

    assert_eq!(fx.run(&["set", "4.5.6", "--tag"]).code, 0);
    assert_eq!(fx.tags(), ["v4.5.6"]);
}

#[test]
fn set_honours_dry_run() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.0.0\n");

    let run = fx.run(&["set", "9.9.9", "--dry-run", "--no-git"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert_eq!(fx.read("VERSION"), "1.0.0\n");
    assert!(run.stdout.contains("9.9.9"));
}

#[test]
fn set_reports_each_files_previous_version_as_json() {
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"VERSION\", \"Cargo.toml\"]\n")
        .write("VERSION", "1.2.3\n")
        .write("Cargo.toml", "[package]\nversion = \"0.9.0\"\n");

    let value = fx.run(&["set", "2.0.0", "--no-git", "--json"]).json();

    assert_eq!(value["version"], "2.0.0");
    // Files disagreed, so there is no single version they moved from.
    assert!(value["previous"].is_null());
    assert_eq!(value["changes"][0]["from"], "1.2.3");
    assert_eq!(value["changes"][1]["from"], "0.9.0");
}

#[test]
fn a_malformed_set_argument_exits_two() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.0.0\n");

    assert_eq!(fx.run(&["set", "not-a-version", "--no-git"]).code, 2);
    assert_eq!(fx.read("VERSION"), "1.0.0\n");
}

// ─── Initialization ──────────────────────────────────────────────────────────

#[test]
fn init_writes_a_config_the_next_command_can_use() {
    let fx = Fixture::new()
        .write("VERSION", "0.1.0\n")
        .write("apps/web/package.json", "{\"version\":\"0.1.0\"}")
        .write("node_modules/dep/package.json", "{\"version\":\"9.9.9\"}");

    let run = fx.run(&["init"]);
    assert_eq!(run.code, 0, "{}", run.output());

    let config = fx.read("vump.toml");
    assert!(config.contains("VERSION"));
    assert!(config.contains("apps/web/package.json"));
    assert!(
        !config.contains("node_modules"),
        "a dependency's manifest is not this project's version:\n{config}"
    );

    // The generated file must be usable immediately, without editing.
    assert_eq!(fx.run(&["check", "0.1.0"]).code, 0);
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let fx = Fixture::new()
        .write("VERSION", "0.1.0\n")
        .write("vump.toml", SINGLE);

    assert_eq!(fx.run(&["init"]).code, 3);
    assert_eq!(fx.run(&["init", "--force"]).code, 0);
}

#[test]
fn init_reports_when_there_is_nothing_to_track() {
    let fx = Fixture::new().write("README.md", "nothing versioned here");
    let run = fx.run(&["init"]);
    assert_eq!(run.code, 3, "{}", run.output());
}

// ─── Multi-project repositories ──────────────────────────────────────────────

const MULTI: &str = "\
[[project]]
name = \"api\"
files = [\"api/Cargo.toml\"]

[[project]]
name = \"web\"
files = [\"web/package.json\"]
";

#[test]
fn projects_are_versioned_independently() {
    let fx = Fixture::new()
        .write("vump.toml", MULTI)
        .write("api/Cargo.toml", "[package]\nversion = \"1.0.0\"\n")
        .write("web/package.json", "{\"version\":\"3.4.5\"}");

    assert_eq!(fx.run(&["patch", "--project", "api", "--no-git"]).code, 0);

    assert!(fx.read("api/Cargo.toml").contains("1.0.1"));
    assert!(
        fx.read("web/package.json").contains("3.4.5"),
        "the other project must be untouched"
    );
}

/// A monorepo where each project carries its own tag pattern.
const MULTI_TAGGED: &str = "\
[[project]]
name = \"api\"
files = [\"api/Cargo.toml\"]
tag_pattern = \"api-v{new_version}\"

[[project]]
name = \"web\"
files = [\"web/package.json\"]
tag_pattern = \"web-v{new_version}\"
";

fn tagged_monorepo() -> Fixture {
    Fixture::new()
        .write("vump.toml", MULTI_TAGGED)
        .write("api/Cargo.toml", "[package]\nversion = \"1.0.0\"\n")
        .write("web/package.json", "{\"version\":\"3.4.5\"}")
}

#[test]
fn a_tag_identifies_its_own_project() {
    // This is the CI case: a workflow passes the pushed tag straight through,
    // without knowing or saying which project it belongs to.
    let fx = tagged_monorepo();

    assert_eq!(fx.run(&["check", "api-v1.0.0"]).code, 0);
    assert_eq!(fx.run(&["check", "web-v3.4.5"]).code, 0);

    // The version is compared against the right project, not just any.
    assert_eq!(fx.run(&["check", "api-v3.4.5"]).code, 4);
    assert_eq!(fx.run(&["check", "web-v1.0.0"]).code, 4);
}

#[test]
fn a_bump_tags_with_the_projects_own_pattern() {
    let fx = tagged_monorepo().with_git();

    let run = fx.run(&["patch", "--project", "api", "--tag"]);
    assert_eq!(run.code, 0, "{}", run.output());

    assert_eq!(fx.tags(), ["api-v1.0.1"]);
}

#[test]
fn independently_versioned_projects_do_not_collide_on_tags() {
    // Both projects reaching the same version is exactly the case a single
    // repository-wide pattern could not express.
    let fx = Fixture::new()
        .write("vump.toml", MULTI_TAGGED)
        .write("api/Cargo.toml", "[package]\nversion = \"2.0.0\"\n")
        .write("web/package.json", "{\"version\":\"2.0.0\"}")
        .with_git();

    assert_eq!(fx.run(&["patch", "--project", "api", "--tag"]).code, 0);
    assert_eq!(fx.run(&["patch", "--project", "web", "--tag"]).code, 0);

    let mut listed = fx.tags();
    listed.sort();
    assert_eq!(listed, ["api-v2.0.1", "web-v2.0.1"]);
}

#[test]
fn a_tag_conflicting_with_an_explicit_project_is_refused() {
    let fx = tagged_monorepo();
    let run = fx.run(&["check", "api-v1.0.0", "--project", "web"]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.stderr.contains("api"), "{}", run.stderr);
}

#[test]
fn projects_sharing_a_tag_pattern_report_the_ambiguity() {
    // Without distinct patterns there is no way to attribute a tag, and
    // guessing would defeat the point of checking it.
    let fx = Fixture::new()
        .write("vump.toml", MULTI)
        .write("api/Cargo.toml", "[package]\nversion = \"1.0.0\"\n")
        .write("web/package.json", "{\"version\":\"3.4.5\"}");

    let run = fx.run(&["check", "v1.0.0"]);
    assert_eq!(run.code, 3, "{}", run.output());
    assert!(run.stderr.contains("tag_pattern"), "{}", run.stderr);
}

#[test]
fn a_single_project_repository_keeps_accepting_bare_versions() {
    // The tag pattern now matches "v1.2.3", but "1.2.3" must keep working.
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n");

    assert_eq!(fx.run(&["check", "1.2.3"]).code, 0);
    assert_eq!(fx.run(&["check", "v1.2.3"]).code, 0);
    assert_eq!(fx.run(&["check", "v9.9.9"]).code, 4);
}

#[test]
fn a_bump_without_a_project_is_refused_when_several_exist() {
    let fx = Fixture::new()
        .write("vump.toml", MULTI)
        .write("api/Cargo.toml", "[package]\nversion = \"1.0.0\"\n")
        .write("web/package.json", "{\"version\":\"3.4.5\"}");

    let run = fx.run(&["patch", "--no-git"]);
    assert_eq!(run.code, 3, "{}", run.output());
    assert!(run.stderr.contains("--project"), "{}", run.stderr);
}

#[test]
fn configuration_is_found_from_a_subdirectory() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n")
        .write("deep/nested/placeholder", "");

    let output = Command::new(env!("CARGO_BIN_EXE_vump"))
        .args(["check", "1.2.3"])
        .current_dir(fx.path().join("deep").join("nested"))
        .stdin(Stdio::null())
        .output()
        .expect("cannot run vump");

    assert_eq!(Run::from(output).code, 0);
}

// ─── Machine-readable output ─────────────────────────────────────────────────

#[test]
fn check_reports_each_file_as_json() {
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"VERSION\", \"Cargo.toml\"]\n")
        .write("VERSION", "1.2.3\n")
        .write("Cargo.toml", "[package]\nversion = \"1.0.0\"\n");

    let run = fx.run(&["check", "1.2.3", "--json"]);
    let value = run.json();

    assert_eq!(value["ok"], false);
    assert_eq!(value["expected"], "1.2.3");
    let files = value["files"].as_array().expect("files must be an array");
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["matches"], true);
    assert_eq!(files[1]["matches"], false);
}

#[test]
fn a_bump_reports_the_resulting_version_as_json() {
    let fx = Fixture::new()
        .write("vump.toml", SINGLE)
        .write("VERSION", "1.2.3\n");

    let value = fx.run(&["minor", "--no-git", "--json"]).json();

    assert_eq!(value["ok"], true);
    assert_eq!(value["previous"], "1.2.3");
    assert_eq!(value["version"], "1.3.0");
    assert_eq!(value["changes"][0]["path"], "VERSION");
}

#[test]
fn failures_are_reported_as_json_too() {
    let fx = Fixture::new().write("vump.toml", SINGLE);

    let run = fx.run(&["check", "1.0.0", "--json"]);
    let value = run.json();

    assert_eq!(value["ok"], false);
    // The error kind mirrors the exit code, so a caller can branch on either.
    assert_eq!(value["error"]["kind"], "config");
    assert_eq!(run.code, 3);
}
