//! An open project: transactions, undo/redo, dirty tracking, saving and
//! crash recovery.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ew_format::store::{self, LoadedProject, MigratedFile, STATE_DIR, SavePlan, SaveReport};
use ew_format::{fsio, time::DateTime};
use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::outputs::DEFAULT_OUTPUT_DIRECTORY;
use ew_model::project::Project;
use ew_model::variables;
use serde::Serialize;

use crate::change::{Change, Request};
use crate::error::{EngineError, ErrorCode};
use crate::history::{DEFAULT_LIMIT_BYTES, History, HistoryState, Transaction};
use crate::journal::{self, Entry, Journal, Recovery};

/// Why a change set happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Cause {
    /// A new transaction.
    Commit,
    /// Undo.
    Undo,
    /// Redo.
    Redo,
    /// Unsaved changes were restored from the journal.
    Recover,
}

/// What happened to one object, for views that follow the changes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSummary {
    /// Document (`None` for project properties).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<Id>,
    /// `insert`, `update`, `delete`, `props`, `addDocument`, `removeDocument`
    /// or `project`.
    pub op: &'static str,
    /// Object id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Object type.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub object_type: Option<&'static str>,
    /// The object after the change (inserts and updates).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<Object>,
}

/// Maximum number of change summaries in one change set; bigger change sets
/// are marked as truncated and views reload instead.
pub const MAX_SUMMARIES: usize = 1_000;

/// A batch of changes streamed to every view.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet {
    /// Transaction number.
    pub seq: u64,
    /// Cause.
    pub cause: Cause,
    /// Transaction label.
    pub label: String,
    /// What changed (empty if truncated).
    pub changes: Vec<ChangeSummary>,
    /// `true` if `changes` is incomplete and views should reload.
    pub truncated: bool,
    /// Undo/redo state afterwards.
    pub history: HistoryState,
}

fn summarize(changes: &[Change], forward: bool) -> (Vec<ChangeSummary>, bool) {
    if changes.len() > MAX_SUMMARIES {
        return (Vec::new(), true);
    }
    let summaries = changes
        .iter()
        .map(|change| match change {
            Change::Object { doc, before, after } => {
                let (from, to) = if forward {
                    (before, after)
                } else {
                    (after, before)
                };
                let any = to.as_ref().or(from.as_ref());
                ChangeSummary {
                    doc: Some(*doc),
                    op: match (from, to) {
                        (None, _) => "insert",
                        (Some(_), Some(_)) => "update",
                        (Some(_), None) => "delete",
                    },
                    id: any.map(Object::id),
                    object_type: any.map(|object| object.object_type().name()),
                    object: to.clone(),
                }
            }
            Change::Props { doc, .. } => ChangeSummary {
                doc: Some(*doc),
                op: "props",
                id: None,
                object_type: None,
                object: None,
            },
            Change::Document { before, .. } => {
                let added = before.is_none() == forward;
                ChangeSummary {
                    doc: change.doc(),
                    op: if added {
                        "addDocument"
                    } else {
                        "removeDocument"
                    },
                    id: None,
                    object_type: None,
                    object: None,
                }
            }
            Change::Project { .. } => ChangeSummary {
                doc: None,
                op: "project",
                id: None,
                object_type: None,
                object: None,
            },
        })
        .collect();
    (summaries, false)
}

/// An open project.
#[derive(Debug)]
pub struct Session {
    root: PathBuf,
    manifest_file: String,
    project: Project,
    history: History,
    journal: Journal,
    journaling: bool,
    next_seq: u64,
    counter: u64,
    doc_versions: BTreeMap<Id, u64>,
    manifest_version: u64,
    saved_doc_versions: BTreeMap<Id, u64>,
    saved_manifest_version: u64,
    saved_state: u64,
    saved_paths: BTreeSet<String>,
    hashes: BTreeMap<String, u64>,
    recovery: Option<Recovery>,
    migrated: Vec<MigratedFile>,
    journal_error: Option<String>,
}

impl Session {
    fn from_loaded(loaded: LoadedProject, journaling: bool, recovery: Option<Recovery>) -> Self {
        let id = loaded.project.manifest.id;
        let journal = if journaling {
            Journal::new(&loaded.root, id, &loaded.hashes)
        } else {
            Journal::disabled(id)
        };
        let saved_paths = loaded
            .project
            .manifest
            .documents
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        Self {
            root: loaded.root,
            manifest_file: loaded.manifest_file,
            project: loaded.project,
            history: History::new(DEFAULT_LIMIT_BYTES),
            journal,
            journaling,
            next_seq: 1,
            counter: 0,
            doc_versions: BTreeMap::new(),
            manifest_version: 0,
            saved_doc_versions: BTreeMap::new(),
            saved_manifest_version: 0,
            saved_state: 0,
            saved_paths,
            hashes: loaded.hashes,
            recovery,
            migrated: loaded.migrated,
            journal_error: None,
        }
    }

    /// Creates a project on disk and opens it.
    ///
    /// # Errors
    ///
    /// Fails if the folder is not empty or cannot be written.
    pub fn create(root: &Path, project: &Project, journaling: bool) -> Result<Self, EngineError> {
        let loaded = store::create_project(root, project)?;
        Ok(Self::from_loaded(loaded, journaling, None))
    }

    /// Opens a project. Unsaved work from a crashed session is reported by
    /// [`Session::recovery`] and must be recovered or discarded before
    /// editing.
    ///
    /// # Errors
    ///
    /// Fails if the project cannot be read.
    pub fn open(path: &Path, journaling: bool) -> Result<Self, EngineError> {
        let loaded = store::load_project(path)?;
        let recovery = if journaling {
            journal::read_recovery(&loaded.root, loaded.project.manifest.id, &loaded.hashes)
                .map_err(|error| EngineError::io("could not read the autosave journal", &error))?
        } else {
            None
        };
        Ok(Self::from_loaded(loaded, journaling, recovery))
    }

    /// Sets the undo memory limit (for tests and configuration).
    pub fn set_undo_limit(&mut self, bytes: usize) {
        self.history = History::new(bytes);
    }

    /// The project.
    #[must_use]
    pub const fn project(&self) -> &Project {
        &self.project
    }

    /// The project folder.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The manifest file name.
    #[must_use]
    pub fn manifest_file(&self) -> &str {
        &self.manifest_file
    }

    /// Full path of the manifest.
    #[must_use]
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join(&self.manifest_file)
    }

    /// Undo/redo state.
    #[must_use]
    pub fn history_state(&self) -> HistoryState {
        self.history.report(self.saved_state)
    }

    /// The undo history.
    #[must_use]
    pub const fn history(&self) -> &History {
        &self.history
    }

    /// `true` if there are unsaved changes.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.history.state() != self.saved_state
    }

    /// `true` if this document changed since the last save.
    #[must_use]
    pub fn is_document_dirty(&self, id: Id) -> bool {
        self.doc_versions.get(&id) != self.saved_doc_versions.get(&id)
            || self
                .project
                .reference(id)
                .is_some_and(|entry| !self.saved_paths.contains(&entry.path))
    }

    /// Unsaved work from a crashed session, waiting for a decision.
    #[must_use]
    pub const fn recovery(&self) -> Option<&Recovery> {
        self.recovery.as_ref()
    }

    /// Files that were read in an older format and will be upgraded on save.
    #[must_use]
    pub fn migrated_files(&self) -> &[MigratedFile] {
        &self.migrated
    }

    /// The last autosave journal error, if writing the journal failed.
    #[must_use]
    pub fn journal_error(&self) -> Option<&str> {
        self.journal_error.as_deref()
    }

    /// The journal file, if journaling is enabled.
    #[must_use]
    pub fn journal_path(&self) -> Option<&Path> {
        self.journal.path()
    }

    /// Output folders of the output jobs (relative to the project).
    #[must_use]
    pub fn generated_dirs(&self) -> Vec<String> {
        let mut dirs: BTreeSet<String> = self
            .project
            .documents
            .values()
            .filter_map(|document| match document {
                Document::OutputJob(job) => Some(job.directory.trim_matches('/').to_owned()),
                _ => None,
            })
            .filter(|dir| !dir.is_empty())
            .collect();
        dirs.insert(DEFAULT_OUTPUT_DIRECTORY.to_owned());
        dirs.into_iter().collect()
    }

    fn ensure_editable(&self) -> Result<(), EngineError> {
        if self.recovery.is_some() {
            return Err(EngineError::new(
                ErrorCode::RecoveryPending,
                "unsaved changes from an earlier session must be recovered or discarded first",
            ));
        }
        Ok(())
    }

    fn rollback(&mut self, applied: &[Change]) {
        for change in applied.iter().rev() {
            // Reverting changes that were just applied cannot fail.
            let _ = change.apply(&mut self.project, false);
        }
    }

    fn touch(&mut self, changes: &[Change]) {
        self.counter += 1;
        for change in changes {
            if let Some(doc) = change.doc() {
                self.doc_versions.insert(doc, self.counter);
            }
            if change.touches_manifest() {
                self.manifest_version = self.counter;
            }
        }
    }

    fn write_journal(&mut self, line: &str) {
        if let Err(error) = self.journal.append(line) {
            self.journal_error = Some(error.to_string());
        }
    }

    /// Applies a list of requests as one transaction. Either every request
    /// succeeds or nothing changes.
    ///
    /// # Errors
    ///
    /// Fails if a request does not fit the current state.
    pub fn commit(
        &mut self,
        label: &str,
        requests: Vec<Request>,
    ) -> Result<ChangeSet, EngineError> {
        self.ensure_editable()?;
        if requests.is_empty() {
            return Err(EngineError::invalid_params(
                "a transaction needs at least one change",
            ));
        }
        let mut applied: Vec<Change> = Vec::new();
        for request in requests {
            let changes = match request.resolve(&self.project) {
                Ok(changes) => changes,
                Err(error) => {
                    self.rollback(&applied);
                    return Err(error);
                }
            };
            for change in changes {
                if let Err(error) = change.apply(&mut self.project, true) {
                    self.rollback(&applied);
                    return Err(error.into());
                }
                applied.push(change);
            }
        }
        self.record(label, applied, true)
    }

    fn record(
        &mut self,
        label: &str,
        changes: Vec<Change>,
        journal: bool,
    ) -> Result<ChangeSet, EngineError> {
        let seq = self.next_seq;
        self.next_seq += 1;
        let line = journal::commit_line(seq, label, &changes).map_err(EngineError::internal)?;
        if journal {
            self.write_journal(&line);
        }
        self.touch(&changes);
        let (summaries, truncated) = summarize(&changes, true);
        self.history.push(Transaction {
            seq,
            label: label.to_owned(),
            changes,
            bytes: line.len(),
        });
        Ok(ChangeSet {
            seq,
            cause: Cause::Commit,
            label: label.to_owned(),
            changes: summaries,
            truncated,
            history: self.history_state(),
        })
    }

    /// Undoes the newest transaction. Returns `None` if there is nothing to
    /// undo.
    ///
    /// # Errors
    ///
    /// Fails if recovery is pending or the project is inconsistent.
    pub fn undo(&mut self) -> Result<Option<ChangeSet>, EngineError> {
        self.ensure_editable()?;
        self.undo_inner(true)
    }

    fn undo_inner(&mut self, journal: bool) -> Result<Option<ChangeSet>, EngineError> {
        let Some(transaction) = self.history.take_undo() else {
            return Ok(None);
        };
        for (index, change) in transaction.changes.iter().enumerate().rev() {
            if let Err(error) = change.apply(&mut self.project, false) {
                for redo in &transaction.changes[index + 1..] {
                    let _ = redo.apply(&mut self.project, true);
                }
                self.history.push_undo(transaction);
                return Err(error.into());
            }
        }
        if journal {
            self.write_journal(r#"{"op":"undo"}"#);
        }
        self.touch(&transaction.changes);
        let (summaries, truncated) = summarize(&transaction.changes, false);
        let change_set = ChangeSet {
            seq: transaction.seq,
            cause: Cause::Undo,
            label: transaction.label.clone(),
            changes: summaries,
            truncated,
            history: HistoryState {
                dirty: false,
                can_undo: false,
                can_redo: false,
                undo_label: None,
                redo_label: None,
                undo_count: 0,
                redo_count: 0,
            },
        };
        self.history.push_redo(transaction);
        Ok(Some(ChangeSet {
            history: self.history_state(),
            ..change_set
        }))
    }

    /// Redoes the next undone transaction. Returns `None` if there is
    /// nothing to redo.
    ///
    /// # Errors
    ///
    /// Fails if recovery is pending or the project is inconsistent.
    pub fn redo(&mut self) -> Result<Option<ChangeSet>, EngineError> {
        self.ensure_editable()?;
        self.redo_inner(true)
    }

    fn redo_inner(&mut self, journal: bool) -> Result<Option<ChangeSet>, EngineError> {
        let Some(transaction) = self.history.take_redo() else {
            return Ok(None);
        };
        for (index, change) in transaction.changes.iter().enumerate() {
            if let Err(error) = change.apply(&mut self.project, true) {
                for undo in transaction.changes[..index].iter().rev() {
                    let _ = undo.apply(&mut self.project, false);
                }
                self.history.push_redo(transaction);
                return Err(error.into());
            }
        }
        if journal {
            self.write_journal(r#"{"op":"redo"}"#);
        }
        self.touch(&transaction.changes);
        let (summaries, truncated) = summarize(&transaction.changes, true);
        let seq = transaction.seq;
        let label = transaction.label.clone();
        self.history.push_undo(transaction);
        Ok(Some(ChangeSet {
            seq,
            cause: Cause::Redo,
            label,
            changes: summaries,
            truncated,
            history: self.history_state(),
        }))
    }

    /// Replays the journal of a crashed session. On failure nothing changes
    /// and the journal stays for another attempt.
    ///
    /// # Errors
    ///
    /// Fails if there is nothing to recover or the journal does not fit the
    /// saved files.
    pub fn recover(&mut self) -> Result<ChangeSet, EngineError> {
        let Some(recovery) = self.recovery.take() else {
            return Err(EngineError::conflict(
                "there are no unsaved changes to recover",
            ));
        };
        let project = self.project.clone();
        let history = self.history.clone();
        let (next_seq, counter) = (self.next_seq, self.counter);
        let versions = (self.doc_versions.clone(), self.manifest_version);
        for entry in &recovery.entries {
            let result = match entry {
                Entry::Commit {
                    seq,
                    label,
                    changes,
                } => self.replay_commit(*seq, label, changes),
                Entry::Undo => self.undo_inner(false).map(|_| ()),
                Entry::Redo => self.redo_inner(false).map(|_| ()),
            };
            if let Err(error) = result {
                self.project = project;
                self.history = history;
                self.next_seq = next_seq;
                self.counter = counter;
                (self.doc_versions, self.manifest_version) = versions;
                self.recovery = Some(recovery);
                return Err(EngineError::conflict(format!(
                    "the unsaved changes do not fit the saved project any more: {}",
                    error.message
                )));
            }
        }
        self.journal.continue_existing();
        Ok(ChangeSet {
            seq: self.history.state(),
            cause: Cause::Recover,
            label: "recover".to_owned(),
            changes: Vec::new(),
            truncated: true,
            history: self.history_state(),
        })
    }

    fn replay_commit(
        &mut self,
        seq: u64,
        label: &str,
        changes: &[Change],
    ) -> Result<(), EngineError> {
        for (index, change) in changes.iter().enumerate() {
            if let Err(error) = change.apply(&mut self.project, true) {
                self.rollback(&changes[..index]);
                return Err(error.into());
            }
        }
        self.next_seq = self.next_seq.max(seq);
        self.record(label, changes.to_vec(), false).map(|_| ())
    }

    /// Discards the unsaved work of a crashed session. The journal is kept
    /// as `.ewerk/journal.discarded.jsonl` until the next discard.
    ///
    /// # Errors
    ///
    /// Fails if there is nothing to discard or the file cannot be moved.
    pub fn discard_recovery(&mut self) -> Result<(), EngineError> {
        if self.recovery.take().is_none() {
            return Err(EngineError::conflict(
                "there are no unsaved changes to discard",
            ));
        }
        self.journal
            .discard()
            .map_err(|error| EngineError::io("could not discard the autosave journal", &error))
    }

    /// Saves changed files (or every file with `all`). Files of removed
    /// documents are deleted. Files read in an older format are backed up to
    /// `.ewerk/backup/schema-v<N>/` first.
    ///
    /// # Errors
    ///
    /// Fails if recovery is pending or a file cannot be written.
    pub fn save(&mut self, all: bool) -> Result<SaveReport, EngineError> {
        self.ensure_editable()?;
        let migrated: BTreeSet<&str> = self
            .migrated
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        let current: BTreeSet<String> = self
            .project
            .manifest
            .documents
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        let plan = SavePlan {
            manifest: all
                || self.manifest_version != self.saved_manifest_version
                || migrated.contains(self.manifest_file.as_str()),
            documents: self
                .project
                .manifest
                .documents
                .iter()
                .filter(|entry| {
                    all || self.is_document_dirty(entry.id)
                        || migrated.contains(entry.path.as_str())
                })
                .map(|entry| entry.id)
                .collect(),
            delete: self.saved_paths.difference(&current).cloned().collect(),
        };
        self.check_unchanged_on_disk(&plan)?;
        for file in &self.migrated {
            let backup = self
                .root
                .join(STATE_DIR)
                .join("backup")
                .join(format!("schema-v{}", file.from));
            fsio::write_atomic(&store::resolve(&backup, &file.path), &file.original)?;
        }
        let report = store::save_project(&self.root, &self.manifest_file, &self.project, &plan)?;
        for path in &report.deleted {
            self.hashes.remove(path);
        }
        self.hashes.extend(report.hashes.clone());
        self.saved_doc_versions.clone_from(&self.doc_versions);
        self.saved_manifest_version = self.manifest_version;
        self.saved_state = self.history.state();
        self.saved_paths = current;
        self.migrated.clear();
        self.journal_error = None;
        self.journal
            .reset(&self.hashes)
            .map_err(|error| EngineError::io("could not reset the autosave journal", &error))?;
        Ok(report)
    }

    /// Fails with `conflict` if a file this save would write or delete was
    /// changed by another program (another ElectronicWerk window, the
    /// command line, an AI agent, git) since the project was read or saved:
    /// saving would throw that work away.
    fn check_unchanged_on_disk(&self, plan: &SavePlan) -> Result<(), EngineError> {
        let mut paths: Vec<String> = Vec::new();
        if plan.manifest {
            paths.push(self.manifest_file.clone());
        }
        for id in &plan.documents {
            if let Some(entry) = self.project.manifest.documents.iter().find(|e| e.id == *id) {
                paths.push(entry.path.clone());
            }
        }
        paths.extend(plan.delete.iter().cloned());
        let mut changed = Vec::new();
        for path in paths {
            let Ok(bytes) = std::fs::read(store::resolve(&self.root, &path)) else {
                // Missing files are written again; nothing is lost.
                continue;
            };
            match self.hashes.get(&path) {
                Some(hash) if *hash == fsio::fnv1a(&bytes) => {}
                // A file of a new document that already exists on disk, or
                // a file that another program changed.
                _ => changed.push(path),
            }
        }
        if changed.is_empty() {
            return Ok(());
        }
        Err(EngineError::new(
            ErrorCode::ChangedOnDisk,
            format!("changed by another program: {}", changed.join(", ")),
        ))
    }

    /// Writes the project to another folder (which must be new or empty),
    /// including other files such as notes, and continues working there.
    ///
    /// # Errors
    ///
    /// Fails if the target is not empty or cannot be written.
    pub fn save_as(&mut self, target: &Path) -> Result<(), EngineError> {
        self.ensure_editable()?;
        let loaded = self.write_copy(target)?;
        // The unsaved changes now live in the new folder.
        let _ = self.journal.delete_file();
        self.root = loaded.root;
        self.manifest_file = loaded.manifest_file;
        self.hashes = loaded.hashes;
        self.saved_doc_versions.clone_from(&self.doc_versions);
        self.saved_manifest_version = self.manifest_version;
        self.saved_state = self.history.state();
        self.saved_paths = self
            .project
            .manifest
            .documents
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        self.migrated.clear();
        self.journal = if self.journaling {
            Journal::new(&self.root, self.project.manifest.id, &self.hashes)
        } else {
            Journal::disabled(self.project.manifest.id)
        };
        Ok(())
    }

    /// Writes a copy of the current state to another folder without
    /// switching to it. Returns the manifest path of the copy.
    ///
    /// # Errors
    ///
    /// Fails if the target is not empty or cannot be written.
    pub fn save_copy(&self, target: &Path) -> Result<PathBuf, EngineError> {
        let loaded = self.write_copy(target)?;
        Ok(loaded.root.join(loaded.manifest_file))
    }

    fn write_copy(&self, target: &Path) -> Result<LoadedProject, EngineError> {
        if target.starts_with(&self.root) {
            return Err(EngineError::invalid_params(
                "the copy cannot be placed inside the project folder",
            ));
        }
        let loaded = store::create_project(target, &self.project)?;
        store::copy_other_files(
            &self.root,
            target,
            &self.manifest_file,
            &self.project,
            &self.generated_dirs(),
        )?;
        Ok(loaded)
    }

    /// Prepares closing. Without `discard`, unsaved changes are an error.
    /// With `discard`, they are dropped (the journal is kept as a discarded
    /// copy). A pending recovery is left in place for the next time.
    ///
    /// # Errors
    ///
    /// Fails with `unsavedChanges` if there are unsaved changes and
    /// `discard` is not set.
    pub fn close(&mut self, discard: bool) -> Result<(), EngineError> {
        if self.recovery.is_some() {
            return Ok(());
        }
        if self.is_dirty() {
            if !discard {
                return Err(EngineError::new(
                    ErrorCode::UnsavedChanges,
                    "the project has unsaved changes",
                ));
            }
            return self.journal.discard().map_err(|error| {
                EngineError::io("could not discard the autosave journal", &error)
            });
        }
        self.journal
            .delete_file()
            .map_err(|error| EngineError::io("could not remove the autosave journal", &error))
    }

    /// Replaces `${NAME}` variables in a text. `doc` provides `SHEET_NAME`
    /// and `FILENAME`.
    #[must_use]
    pub fn resolve_text(&self, text: &str, doc: Option<Id>) -> String {
        let manifest = &self.project.manifest;
        let document = doc.and_then(|id| self.project.document(id));
        let file_name = doc
            .and_then(|id| self.project.reference(id))
            .map_or(self.manifest_file.as_str(), |entry| {
                entry.path.rsplit('/').next().unwrap_or(&entry.path)
            });
        let lookup = |name: &str| -> Option<String> {
            match name {
                "PROJECT_NAME" => Some(manifest.name.clone()),
                "REVISION" => Some(manifest.revision.clone()),
                "CURRENT_DATE" => Some(DateTime::now().date_string()),
                "SHEET_NAME" => document.map(|document| document.name().to_owned()),
                "FILENAME" => Some(file_name.to_owned()),
                _ => manifest.variables.get(name).cloned(),
            }
        };
        variables::expand(text, &lookup)
    }
}
