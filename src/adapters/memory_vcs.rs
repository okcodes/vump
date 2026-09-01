//! An in-memory version control adapter for tests.
//!
//! Records what it was asked to do and can be told to fail a specific
//! operation, which is what makes partial-failure paths — a commit that
//! succeeds followed by a push that does not — testable without a network or a
//! real repository.

use std::sync::Mutex;

use crate::ports::{Vcs, VcsError, WorkingTree};

/// One operation performed against the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VcsCall {
    /// Paths were staged.
    Stage(Vec<String>),
    /// A commit was created with this message.
    Commit(String),
    /// A tag was created with this name.
    Tag(String),
    /// A push was performed, optionally including this tag.
    Push(Option<String>),
}

/// A scriptable [`Vcs`] that records calls instead of performing them.
#[derive(Debug, Default)]
pub struct MemoryVcs {
    tree: Mutex<WorkingTree>,
    calls: Mutex<Vec<VcsCall>>,
    failing: Mutex<Option<(String, String)>>,
}

impl MemoryVcs {
    /// Creates a repository with a clean working tree.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the working tree as carrying uncommitted changes.
    #[must_use]
    pub fn with_changes(self, paths: &[&str]) -> Self {
        *lock(&self.tree) = WorkingTree {
            changed: paths.iter().map(|p| (*p).to_owned()).collect(),
        };
        self
    }

    /// Makes `operation` fail with `detail` when it is attempted.
    #[must_use]
    pub fn failing(self, operation: &str, detail: &str) -> Self {
        *lock(&self.failing) = Some((operation.to_owned(), detail.to_owned()));
        self
    }

    /// The operations performed so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<VcsCall> {
        lock(&self.calls).clone()
    }

    fn record(&self, call: VcsCall) {
        lock(&self.calls).push(call);
    }

    fn guard(&self, operation: &str) -> Result<(), VcsError> {
        let failing = lock(&self.failing);
        match failing.as_ref() {
            Some((op, detail)) if op == operation => Err(VcsError::Failed {
                operation: operation.to_owned(),
                detail: detail.clone(),
            }),
            _ => Ok(()),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Vcs for MemoryVcs {
    fn status(&self) -> Result<WorkingTree, VcsError> {
        self.guard("status")?;
        Ok(lock(&self.tree).clone())
    }

    fn stage(&self, paths: &[String]) -> Result<(), VcsError> {
        self.guard("add")?;
        self.record(VcsCall::Stage(paths.to_vec()));
        Ok(())
    }

    fn commit(&self, message: &str) -> Result<(), VcsError> {
        self.guard("commit")?;
        self.record(VcsCall::Commit(message.to_owned()));
        Ok(())
    }

    fn tag(&self, name: &str) -> Result<(), VcsError> {
        self.guard("tag")?;
        self.record(VcsCall::Tag(name.to_owned()));
        Ok(())
    }

    fn push(&self, tag: Option<&str>) -> Result<(), VcsError> {
        self.guard("push")?;
        self.record(VcsCall::Push(tag.map(ToOwned::to_owned)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_calls_in_order() {
        let vcs = MemoryVcs::new();
        vcs.stage(&["VERSION".to_owned()]).unwrap();
        vcs.commit("bump").unwrap();
        vcs.tag("v1.0.0").unwrap();

        assert_eq!(
            vcs.calls(),
            [
                VcsCall::Stage(vec!["VERSION".to_owned()]),
                VcsCall::Commit("bump".to_owned()),
                VcsCall::Tag("v1.0.0".to_owned()),
            ]
        );
    }

    #[test]
    fn a_scripted_failure_affects_only_its_operation() {
        let vcs = MemoryVcs::new().failing("push", "no upstream");
        vcs.commit("bump").unwrap();
        assert!(vcs.push(None).is_err());
    }

    #[test]
    fn a_dirty_tree_is_reported() {
        let vcs = MemoryVcs::new().with_changes(&["src/main.rs"]);
        let tree = vcs.status().unwrap();
        assert!(tree.is_dirty());
        assert_eq!(tree.changed, ["src/main.rs"]);
    }
}
