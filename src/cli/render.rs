//! Rendering results for humans and for machines.
//!
//! Both renderings are produced from the same structured result, so neither can
//! carry information the other lacks. Nothing here computes anything: if a
//! value needs deriving, it belongs in a use case.

use std::io::IsTerminal;

use serde_json::{Value, json};

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
