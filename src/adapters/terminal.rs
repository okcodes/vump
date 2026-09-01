//! Terminal implementation of the [`Interaction`] port.

use std::io::IsTerminal;

use inquire::{Confirm, Select};

use crate::ports::{GitChoice, Interaction, InteractionError};

/// Asks questions on the terminal.
#[derive(Debug, Default, Clone, Copy)]
pub struct TerminalInteraction;

impl TerminalInteraction {
    /// Creates an interaction bound to the current terminal.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Fails when there is no terminal to prompt on.
    ///
    /// Checked before every question so that a piped or redirected run reports
    /// why it cannot continue instead of failing obscurely inside the prompt.
    fn require_terminal() -> Result<(), InteractionError> {
        if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
            return Ok(());
        }
        Err(InteractionError::Unavailable {
            detail: "no terminal is attached; name a subcommand to run without prompting"
                .to_owned(),
        })
    }
}

/// Labels for the git question. Matching on these is why they are named.
const GIT_NOTHING: &str = "Nothing    — just write the files";
const GIT_COMMIT: &str = "Commit     — stage and commit";
const GIT_TAG: &str = "Tag        — commit and tag";
const GIT_PUSH: &str = "Push       — commit, tag and push";

/// Translates a prompt failure, distinguishing a deliberate cancel from a
/// genuine fault.
fn translate(error: &inquire::InquireError) -> InteractionError {
    match error {
        inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted => {
            InteractionError::Cancelled
        }
        other => InteractionError::Unavailable {
            detail: other.to_string(),
        },
    }
}

impl Interaction for TerminalInteraction {
    fn choose_project(&self, names: &[String]) -> Result<String, InteractionError> {
        Self::require_terminal()?;
        Select::new("Which project?", names.to_vec())
            .prompt()
            .map_err(|e| translate(&e))
    }

    fn choose_base(&self, candidates: &[(String, String)]) -> Result<String, InteractionError> {
        Self::require_terminal()?;

        let labels: Vec<String> = candidates
            .iter()
            .map(|(version, where_seen)| format!("{version}  ({where_seen})"))
            .collect();

        let chosen = Select::new(
            "Tracked files disagree. Which version is correct?",
            labels.clone(),
        )
        .prompt()
        .map_err(|e| translate(&e))?;

        let index = labels
            .iter()
            .position(|l| *l == chosen)
            .ok_or(InteractionError::Cancelled)?;

        Ok(candidates[index].0.clone())
    }

    fn choose_transition(
        &self,
        current: &str,
        options: &[(String, String)],
    ) -> Result<usize, InteractionError> {
        Self::require_terminal()?;

        // Padding the name column keeps the resulting versions aligned, which is
        // what the reader is actually comparing.
        let width = options
            .iter()
            .map(|(n, _)| n.len())
            .max()
            .unwrap_or_default();
        let labels: Vec<String> = options
            .iter()
            .map(|(name, result)| format!("{name:<width$}  ->  {result}"))
            .collect();

        let chosen = Select::new(
            &format!("Current version {current}. Bump to:"),
            labels.clone(),
        )
        .with_page_size(12)
        .prompt()
        .map_err(|e| translate(&e))?;

        labels
            .iter()
            .position(|l| *l == chosen)
            .ok_or(InteractionError::Cancelled)
    }

    fn choose_git(&self) -> Result<GitChoice, InteractionError> {
        Self::require_terminal()?;

        let chosen = Select::new(
            "Git actions after bumping:",
            vec![GIT_NOTHING, GIT_COMMIT, GIT_TAG, GIT_PUSH],
        )
        .prompt()
        .map_err(|e| translate(&e))?;

        Ok(match chosen {
            GIT_COMMIT => GitChoice::Commit,
            GIT_TAG => GitChoice::Tag,
            GIT_PUSH => GitChoice::TagAndPush,
            _ => GitChoice::None,
        })
    }

    fn confirm(&self, summary: &str) -> Result<bool, InteractionError> {
        Self::require_terminal()?;
        println!("{summary}");
        Confirm::new("Proceed?")
            .with_default(true)
            .prompt()
            .map_err(|e| translate(&e))
    }
}

/// An interaction that refuses every question.
///
/// Selected whenever a subcommand is named. A prompt reaching this
/// implementation is a bug, and it fails loudly rather than blocking on input
/// that will never arrive.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoInteraction;

impl NoInteraction {
    fn refuse<T>() -> Result<T, InteractionError> {
        Err(InteractionError::Unavailable {
            detail: "this command never prompts; every choice must be given as an argument"
                .to_owned(),
        })
    }
}

impl Interaction for NoInteraction {
    fn choose_project(&self, _: &[String]) -> Result<String, InteractionError> {
        Self::refuse()
    }

    fn choose_base(&self, _: &[(String, String)]) -> Result<String, InteractionError> {
        Self::refuse()
    }

    fn choose_transition(
        &self,
        _: &str,
        _: &[(String, String)],
    ) -> Result<usize, InteractionError> {
        Self::refuse()
    }

    fn choose_git(&self) -> Result<GitChoice, InteractionError> {
        Self::refuse()
    }

    fn confirm(&self, _: &str) -> Result<bool, InteractionError> {
        Self::refuse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_non_interactive_implementation_refuses_every_question() {
        let i = NoInteraction;
        assert!(i.choose_project(&["api".to_owned()]).is_err());
        assert!(i.choose_base(&[]).is_err());
        assert!(i.choose_transition("1.0.0", &[]).is_err());
        assert!(i.choose_git().is_err());
        assert!(i.confirm("summary").is_err());
    }

    #[test]
    fn prompting_without_a_terminal_is_reported_clearly() {
        // The test harness runs without a terminal attached, so this exercises
        // the guard that keeps a redirected run from hanging.
        let err = TerminalInteraction::new()
            .choose_git()
            .expect_err("must refuse without a terminal");
        assert!(matches!(err, InteractionError::Unavailable { .. }));
    }
}
