//! A scriptable [`Interaction`] for driving a guided run from a test.
//!
//! Answers are queued in the order the questions are asked, and every question
//! is recorded. That makes both halves of a guided run observable: what it
//! decided, and what it thought worth asking to get there.

use std::sync::Mutex;

use crate::config::GitThrough;
use crate::ports::{Interaction, InteractionError, TransitionChoice};

/// One answer a scripted run will give.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Pick the project with this name.
    Project(String),
    /// Treat this version as the current one.
    Base(String),
    /// Pick the transition at this index.
    Transition(usize),
    /// Carry the release this far.
    Git(GitThrough),
    /// Approve, or decline, the final summary.
    Confirm(bool),
}

/// A question a guided run asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// Which project, out of these.
    Project(Vec<String>),
    /// Which version is current, given these candidates.
    Base(Vec<String>),
    /// Which bump, out of these choices.
    Transition(Vec<TransitionChoice>),
    /// How far to carry the release.
    Git,
    /// Approve this summary.
    Confirm(String),
    /// Not a question: something the run reported along the way.
    Notice(String),
}

/// An [`Interaction`] that answers from a script.
#[derive(Debug, Default)]
pub struct MemoryInteraction {
    answers: Mutex<Vec<Answer>>,
    asked: Mutex<Vec<Question>>,
}

impl MemoryInteraction {
    /// Creates an interaction that will give `answers`, in order.
    #[must_use]
    pub fn new(answers: &[Answer]) -> Self {
        Self {
            answers: Mutex::new(answers.to_vec()),
            asked: Mutex::new(Vec::new()),
        }
    }

    /// Everything the run asked or reported, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<Question> {
        lock(&self.asked).clone()
    }

    /// The choices offered for the bump, or `None` if it was never asked.
    #[must_use]
    pub fn offered(&self) -> Option<Vec<TransitionChoice>> {
        self.asked().into_iter().find_map(|q| match q {
            Question::Transition(choices) => Some(choices),
            _ => None,
        })
    }

    /// Everything reported rather than asked.
    #[must_use]
    pub fn notices(&self) -> Vec<String> {
        self.asked()
            .into_iter()
            .filter_map(|q| match q {
                Question::Notice(text) => Some(text),
                _ => None,
            })
            .collect()
    }

    /// Takes the next answer, failing when the script has run out.
    ///
    /// Running out is reported as a cancellation rather than a panic: a run
    /// that asks more than the test expected is a result worth asserting on,
    /// not a crash in the harness.
    fn next(&self, question: Question) -> Result<Answer, InteractionError> {
        lock(&self.asked).push(question);
        let mut answers = lock(&self.answers);
        if answers.is_empty() {
            return Err(InteractionError::Cancelled);
        }
        Ok(answers.remove(0))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Interaction for MemoryInteraction {
    fn choose_project(&self, names: &[String]) -> Result<String, InteractionError> {
        match self.next(Question::Project(names.to_vec()))? {
            Answer::Project(name) => Ok(name),
            _ => Err(InteractionError::Cancelled),
        }
    }

    fn choose_base(&self, candidates: &[(String, String)]) -> Result<String, InteractionError> {
        let listed = candidates.iter().map(|(v, _)| v.clone()).collect();
        match self.next(Question::Base(listed))? {
            Answer::Base(version) => Ok(version),
            _ => Err(InteractionError::Cancelled),
        }
    }

    fn choose_transition(
        &self,
        _: &str,
        options: &[TransitionChoice],
    ) -> Result<usize, InteractionError> {
        match self.next(Question::Transition(options.to_vec()))? {
            Answer::Transition(index) => Ok(index),
            _ => Err(InteractionError::Cancelled),
        }
    }

    fn choose_git(&self) -> Result<GitThrough, InteractionError> {
        match self.next(Question::Git)? {
            Answer::Git(through) => Ok(through),
            _ => Err(InteractionError::Cancelled),
        }
    }

    fn notice(&self, message: &str) {
        lock(&self.asked).push(Question::Notice(message.to_owned()));
    }

    fn confirm(&self, summary: &str) -> Result<bool, InteractionError> {
        match self.next(Question::Confirm(summary.to_owned()))? {
            Answer::Confirm(yes) => Ok(yes),
            _ => Err(InteractionError::Cancelled),
        }
    }
}
