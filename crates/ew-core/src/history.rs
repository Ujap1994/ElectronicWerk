//! Undo and redo.
//!
//! The number of steps is unlimited; only memory is limited. When the stored
//! transactions exceed the limit, the oldest undo steps are dropped (the
//! newest step is always kept).

use std::collections::VecDeque;

use serde::Serialize;

use crate::change::Change;

/// Default memory limit for undo/redo data (256 MiB).
pub const DEFAULT_LIMIT_BYTES: usize = 256 * 1024 * 1024;

/// A committed group of changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    /// Unique, increasing number.
    pub seq: u64,
    /// What the user did (a label id such as `rename-document`).
    pub label: String,
    /// The changes in the order they were applied.
    pub changes: Vec<Change>,
    /// Approximate memory use (size of the serialized changes).
    pub bytes: usize,
}

/// Undo and redo stacks.
#[derive(Debug, Clone)]
pub struct History {
    undo: VecDeque<Transaction>,
    redo: Vec<Transaction>,
    bytes: usize,
    limit: usize,
    /// State id below the oldest undo step (changes when steps are dropped).
    base: u64,
}

/// Undo/redo state reported to clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryState {
    /// Unsaved changes exist.
    pub dirty: bool,
    /// Something can be undone.
    pub can_undo: bool,
    /// Something can be redone.
    pub can_redo: bool,
    /// Label of the next undo step.
    pub undo_label: Option<String>,
    /// Label of the next redo step.
    pub redo_label: Option<String>,
    /// Number of undo steps.
    pub undo_count: usize,
    /// Number of redo steps.
    pub redo_count: usize,
}

impl History {
    /// Empty history with a memory limit in bytes.
    #[must_use]
    pub const fn new(limit: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            bytes: 0,
            limit,
            base: 0,
        }
    }

    /// Id of the current state: the newest undo step's number, or the base.
    #[must_use]
    pub fn state(&self) -> u64 {
        self.undo
            .back()
            .map_or(self.base, |transaction| transaction.seq)
    }

    /// Records a new transaction and clears the redo stack.
    pub fn push(&mut self, transaction: Transaction) {
        for dropped in self.redo.drain(..) {
            self.bytes = self.bytes.saturating_sub(dropped.bytes);
        }
        self.push_undo(transaction);
    }

    /// Records a transaction without touching the redo stack (used by redo).
    pub fn push_undo(&mut self, transaction: Transaction) {
        self.bytes += transaction.bytes;
        self.undo.push_back(transaction);
        self.evict();
    }

    /// Takes the newest undo step.
    pub fn take_undo(&mut self) -> Option<Transaction> {
        let transaction = self.undo.pop_back()?;
        self.bytes = self.bytes.saturating_sub(transaction.bytes);
        Some(transaction)
    }

    /// Puts an undone step on the redo stack.
    pub fn push_redo(&mut self, transaction: Transaction) {
        self.bytes += transaction.bytes;
        self.redo.push(transaction);
    }

    /// Takes the newest redo step.
    pub fn take_redo(&mut self) -> Option<Transaction> {
        let transaction = self.redo.pop()?;
        self.bytes = self.bytes.saturating_sub(transaction.bytes);
        Some(transaction)
    }

    /// Bytes currently stored.
    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    /// Labels of the undo steps, newest first.
    pub fn undo_labels(&self) -> impl Iterator<Item = &str> {
        self.undo
            .iter()
            .rev()
            .map(|transaction| transaction.label.as_str())
    }

    /// Labels of the redo steps, next first.
    pub fn redo_labels(&self) -> impl Iterator<Item = &str> {
        self.redo
            .iter()
            .rev()
            .map(|transaction| transaction.label.as_str())
    }

    /// The state for clients; `saved` is the state id of the last save.
    #[must_use]
    pub fn report(&self, saved: u64) -> HistoryState {
        HistoryState {
            dirty: self.state() != saved,
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            undo_label: self
                .undo
                .back()
                .map(|transaction| transaction.label.clone()),
            redo_label: self
                .redo
                .last()
                .map(|transaction| transaction.label.clone()),
            undo_count: self.undo.len(),
            redo_count: self.redo.len(),
        }
    }

    fn evict(&mut self) {
        while self.bytes > self.limit && self.undo.len() > 1 {
            if let Some(dropped) = self.undo.pop_front() {
                self.bytes = self.bytes.saturating_sub(dropped.bytes);
                self.base = dropped.seq;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction(seq: u64, bytes: usize) -> Transaction {
        Transaction {
            seq,
            label: format!("t{seq}"),
            changes: Vec::new(),
            bytes,
        }
    }

    #[test]
    fn undo_redo_and_state_ids() {
        let mut history = History::new(1_000);
        assert_eq!(history.state(), 0);
        history.push(transaction(1, 10));
        history.push(transaction(2, 10));
        assert_eq!(history.state(), 2);
        let undone = history.take_undo().unwrap();
        history.push_redo(undone);
        assert_eq!(history.state(), 1);
        assert!(history.report(2).dirty);
        assert_eq!(history.report(1).redo_label.as_deref(), Some("t2"));
        let redone = history.take_redo().unwrap();
        history.push_undo(redone);
        assert_eq!(history.state(), 2);
        assert!(!history.report(2).dirty);

        // A new edit clears the redo stack.
        let undone = history.take_undo().unwrap();
        history.push_redo(undone);
        history.push(transaction(3, 10));
        assert!(!history.report(0).can_redo);
        assert_eq!(history.bytes(), 20);
        assert_eq!(history.undo_labels().collect::<Vec<_>>(), vec!["t3", "t1"]);
    }

    #[test]
    fn memory_limit_drops_oldest_steps() {
        let mut history = History::new(25);
        for seq in 1..=5 {
            history.push(transaction(seq, 10));
        }
        assert_eq!(history.report(0).undo_count, 2);
        assert!(history.bytes() <= 25);
        while let Some(step) = history.take_undo() {
            history.push_redo(step);
        }
        // The base moved past the dropped steps: the original state is gone.
        assert_eq!(history.state(), 3);

        let mut huge = History::new(5);
        huge.push(transaction(1, 100));
        assert_eq!(
            huge.report(0).undo_count,
            1,
            "the newest step is always kept"
        );
        assert_eq!(huge.redo_labels().count(), 0);
    }
}
