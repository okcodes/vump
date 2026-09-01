//! Rendering results for humans and for machines.
//!
//! Both renderings are produced from the same structured result, so neither can
//! carry information the other lacks. Nothing here computes anything: if a
//! value needs deriving, it belongs in a use case.

use std::io::IsTerminal;

use serde_json::{Value, json};

use crate::app::bump::{BumpOutcome, BumpPlan};
use crate::app::check::CheckReport;
use crate::app::status::ProjectStatus;
use crate::cli::exit::Exit;

/// Symbols used to mark pass and fail states.
///
/// Symbols are decorative, so they are replaced by words when output is being
/// captured rather than read by a person.
struct Marks {
    ok: &'static str,
    fail: &'static str,
}

impl Marks {
    fn detect() -> Self {
        if std::io::stdout().is_terminal() {
            Self {
                ok: "✓", fail: "✗"
            }
        } else {
            Self {
                ok: "OK  ",
                fail: "FAIL",
            }
        }
    }
}

/// Renders the outcome of a `check`.
pub fn check(report: &CheckReport, json: bool) {
    if json {
        let files: Vec<Value> = report
            .files
            .iter()
            .map(|f| {
                json!({
                    "path": f.path,
                    "version": f.version.to_string(),
                    "matches": f.version == report.expected,
                })
            })
            .collect();

        print(&json!({
            "command": "check",
            "ok": report.is_satisfied(),
            "expected": report.expected.to_string(),
            "files": files,
        }));
        return;
    }

    let marks = Marks::detect();

    if report.is_satisfied() {
        let count = report.files.len();
        let subject = if count == 1 {
            "file matches"
        } else {
            "files match"
        };
        println!("{} {count} {subject} {}", marks.ok, report.expected);
        return;
    }

    let width = report
        .files
        .iter()
        .map(|f| f.path.len())
        .max()
        .unwrap_or_default();

    eprintln!(
        "{} version mismatch: expected {}",
        marks.fail, report.expected
    );
    eprintln!();
    for file in &report.files {
        let mark = if file.version == report.expected {
            marks.ok
        } else {
            marks.fail
        };
        eprintln!("  {mark}  {:<width$}  {}", file.path, file.version);
    }
    eprintln!();
}

/// Renders the outcome of a `status`.
pub fn status(projects: &[ProjectStatus], json: bool) {
    if json {
        let rendered: Vec<Value> = projects
            .iter()
            .map(|p| {
                json!({
                    "name": p.name,
                    "in_sync": p.is_in_sync(),
                    "version": p.agreed_version().map(ToString::to_string),
                    "files": p.files.iter().map(|f| json!({
                        "path": f.path,
                        "version": f.version.to_string(),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();

        print(&json!({
            "command": "status",
            "ok": projects.iter().all(ProjectStatus::is_in_sync),
            "projects": rendered,
        }));
        return;
    }

    let marks = Marks::detect();

    for (i, project) in projects.iter().enumerate() {
        if i > 0 {
            println!();
        }

        let label = project.name.as_deref().unwrap_or("(this repository)");
        match project.agreed_version() {
            Some(version) => println!("{}  {label}  {version}", marks.ok),
            None => println!("{}  {label}  files disagree", marks.fail),
        }

        // Listing every file is noise when they agree; when they do not, the
        // disagreement is the whole point of the report.
        if !project.is_in_sync() {
            let width = project
                .files
                .iter()
                .map(|f| f.path.len())
                .max()
                .unwrap_or_default();
            for file in &project.files {
                println!("      {:<width$}  {}", file.path, file.version);
            }
        }
    }
}

/// Renders a plan as a block of text for a confirmation prompt.
#[must_use]
pub fn summary(plan: &BumpPlan) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(out);
    if let Some(project) = plan.project.as_deref() {
        let _ = writeln!(out, "  Project:  {project}");
    }
    let _ = writeln!(out, "  Bumping:  {}  ->  {}", plan.current, plan.next);

    for change in &plan.changes {
        let _ = writeln!(out, "  File:     {}", change.path);
    }
    if let Some(message) = plan.git.commit.as_deref() {
        let _ = writeln!(out, "  Commit:   {message}");
    }
    if let Some(tag) = plan.git.tag.as_deref() {
        let _ = writeln!(out, "  Tag:      {tag}");
    }
    let _ = writeln!(
        out,
        "  Push:     {}",
        if plan.git.push { "yes" } else { "no" }
    );

    out
}

/// Renders a plan that will not be carried out.
pub fn plan(plan: &BumpPlan, json: bool) {
    if json {
        print(&json!({
            "command": "plan",
            "ok": true,
            "dry_run": true,
            "project": plan.project,
            "current": plan.current.to_string(),
            "next": plan.next.to_string(),
            "changes": changes_json(plan),
            "git": git_json(plan),
        }));
        return;
    }

    println!("{} -> {}  (dry run)", plan.current, plan.next);
    println!();
    for change in &plan.changes {
        println!("  would write  {}", change.path);
    }
    describe_git_plan(plan);
    println!();
    println!("Nothing was written.");
}

/// Renders a completed bump.
pub fn bump(plan: &BumpPlan, outcome: &BumpOutcome, json: bool) {
    if json {
        print(&json!({
            "command": "bump",
            "ok": outcome.push_error.is_none(),
            "project": plan.project,
            "previous": plan.current.to_string(),
            "version": plan.next.to_string(),
            "changes": changes_json(plan),
            "git": {
                "committed": outcome.committed,
                "commit_message": plan.git.commit,
                "tagged": outcome.tagged,
                "tag": plan.git.tag,
                "pushed": outcome.pushed,
                "push_error": outcome.push_error,
            },
        }));
        return;
    }

    let marks = Marks::detect();

    println!("{} {} -> {}", marks.ok, plan.current, plan.next);
    for path in &outcome.written {
        println!("  {path}");
    }

    if let Some(message) = plan.git.commit.as_deref().filter(|_| outcome.committed) {
        println!("{} committed  {message}", marks.ok);
    }
    if let Some(tag) = plan.git.tag.as_deref().filter(|_| outcome.tagged) {
        println!("{} tagged     {tag}", marks.ok);
    }

    if let Some(detail) = outcome.push_error.as_deref() {
        // The commit and tag survived; only the push did not. Saying what
        // remains to be done is more useful than reporting a failed run.
        eprintln!();
        eprintln!("{} push failed: {detail}", marks.fail);
        eprintln!();
        eprintln!("Everything else succeeded. To finish:");
        eprintln!("  {}", push_command(plan.git.tag.as_deref()));
    } else if outcome.pushed {
        println!("{} pushed", marks.ok);
    } else if outcome.committed {
        println!();
        println!("To push:");
        println!("  {}", push_command(plan.git.tag.as_deref()));
    }
}

fn push_command(tag: Option<&str>) -> String {
    match tag {
        Some(tag) => format!("git push && git push origin {tag}"),
        None => "git push".to_owned(),
    }
}

fn describe_git_plan(plan: &BumpPlan) {
    if let Some(message) = plan.git.commit.as_deref() {
        println!("  would commit  {message}");
    }
    if let Some(tag) = plan.git.tag.as_deref() {
        println!("  would tag     {tag}");
    }
    if plan.git.push {
        println!("  would push");
    }
}

fn changes_json(plan: &BumpPlan) -> Vec<Value> {
    plan.changes
        .iter()
        .map(|c| {
            json!({
                "path": c.path,
                "from": c.from.to_string(),
                "to": c.to.to_string(),
            })
        })
        .collect()
}

fn git_json(plan: &BumpPlan) -> Value {
    json!({
        "commit_message": plan.git.commit,
        "tag": plan.git.tag,
        "push": plan.git.push,
    })
}

/// Renders a failure.
pub fn error(message: &str, exit: Exit, json: bool) {
    if json {
        print(&json!({
            "ok": false,
            "error": {
                "kind": exit.as_str(),
                "message": message,
            },
        }));
    } else {
        eprintln!("error: {message}");
    }
}

fn print(value: &Value) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        // Serializing a value built from `json!` cannot fail in practice; if it
        // somehow does, say so rather than exiting silently.
        Err(e) => eprintln!("error: cannot render JSON output: {e}"),
    }
}
