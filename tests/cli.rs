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

    let tags = Command::new("git")
        .args(["tag"])
        .current_dir(fx.path())
        .output()
        .expect("cannot list tags");
    assert_eq!(String::from_utf8_lossy(&tags.stdout).trim(), "v1.2.4");
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

    let tags = Command::new("git")
        .args(["tag"])
        .current_dir(fx.path())
        .output()
        .expect("cannot list tags");
    assert_eq!(String::from_utf8_lossy(&tags.stdout).trim(), "v1.2.4");
}

#[test]
fn no_git_overrides_configured_actions() {
    let fx = Fixture::new()
        .write("vump.toml", "files = [\"VERSION\"]\n[git]\ntag = true\n")
        .write("VERSION", "1.2.3\n")
        .with_git();

    assert_eq!(fx.run(&["patch", "--no-git"]).code, 0);
    assert_eq!(fx.read("VERSION"), "1.2.4\n");

    let tags = Command::new("git")
        .args(["tag"])
        .current_dir(fx.path())
        .output()
        .expect("cannot list tags");
    assert!(String::from_utf8_lossy(&tags.stdout).trim().is_empty());
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
