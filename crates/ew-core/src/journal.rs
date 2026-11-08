//! Autosave journal and crash recovery.
//!
//! Every committed transaction, undo and redo is appended as one JSON line
//! to `.ewerk/journal.jsonl` in the project folder. Saving deletes the
//! journal. If ElectronicWerk or the engine stops without saving, the
//! journal is still there the next time the project is opened, and replaying
//! it restores the unsaved work exactly, including the undo history.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ew_format::store::STATE_DIR;
use ew_format::time::DateTime;
use ew_model::id::Id;
use serde::{Deserialize, Serialize};

use crate::change::Change;

/// Journal file name inside the state folder.
pub const JOURNAL_FILE: &str = "journal.jsonl";
/// Where a discarded journal is kept (one generation).
pub const DISCARDED_FILE: &str = "journal.discarded.jsonl";
/// Journal format version.
pub const JOURNAL_VERSION: u32 = 1;
/// How often the journal is flushed to the disk itself (it is written to
/// the operating system after every entry, which survives a process crash).
const SYNC_INTERVAL: Duration = Duration::from_secs(1);

/// First line of a journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalHeader {
    /// Journal format version.
    pub journal: u32,
    /// Project id.
    pub project: Id,
    /// Hashes (hex) of the saved files the journal builds on.
    pub base: BTreeMap<String, String>,
    /// When the journal was started (UTC).
    pub started: String,
}

/// One journal line after the header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Entry {
    /// A committed transaction.
    Commit {
        /// Transaction number.
        seq: u64,
        /// Label.
        label: String,
        /// Changes.
        changes: Vec<Change>,
    },
    /// Undo of the newest step.
    Undo,
    /// Redo of the next step.
    Redo,
}

/// Serialization helper that borrows the changes.
#[derive(Serialize)]
struct CommitRef<'a> {
    op: &'static str,
    seq: u64,
    label: &'a str,
    changes: &'a [Change],
}

/// Serializes a commit entry without cloning the changes.
///
/// # Errors
///
/// Fails only if serialization fails.
pub fn commit_line(seq: u64, label: &str, changes: &[Change]) -> serde_json::Result<String> {
    serde_json::to_string(&CommitRef {
        op: "commit",
        seq,
        label,
        changes,
    })
}

/// Hex form of the file hashes for the header.
#[must_use]
pub fn hex_hashes(hashes: &BTreeMap<String, u64>) -> BTreeMap<String, String> {
    hashes
        .iter()
        .map(|(path, hash)| (path.clone(), format!("{hash:016x}")))
        .collect()
}

/// The journal of an open project.
#[derive(Debug)]
pub struct Journal {
    path: Option<PathBuf>,
    header: JournalHeader,
    file: Option<File>,
    last_sync: Instant,
}

impl Journal {
    /// A journal for the project in `root` (created on the first entry).
    #[must_use]
    pub fn new(root: &Path, project: Id, hashes: &BTreeMap<String, u64>) -> Self {
        Self {
            path: Some(root.join(STATE_DIR).join(JOURNAL_FILE)),
            header: JournalHeader {
                journal: JOURNAL_VERSION,
                project,
                base: hex_hashes(hashes),
                started: String::new(),
            },
            file: None,
            last_sync: Instant::now(),
        }
    }

    /// A journal that writes nothing (one-shot command-line use).
    #[must_use]
    pub fn disabled(project: Id) -> Self {
        Self {
            path: None,
            header: JournalHeader {
                journal: JOURNAL_VERSION,
                project,
                base: BTreeMap::new(),
                started: String::new(),
            },
            file: None,
            last_sync: Instant::now(),
        }
    }

    /// The journal file, if journaling is enabled.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Appends one line, creating the file with its header first if needed.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be written.
    pub fn append(&mut self, line: &str) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if self.file.is_none() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let exists = path.is_file();
            let mut file = OpenOptions::new().create(true).append(true).open(path)?;
            if !exists {
                self.header.started = DateTime::now().iso_string();
                let header = serde_json::to_string(&self.header).map_err(io::Error::other)?;
                file.write_all(header.as_bytes())?;
                file.write_all(b"\n")?;
            }
            self.file = Some(file);
        }
        if let Some(file) = &mut self.file {
            let mut buffer = Vec::with_capacity(line.len() + 1);
            buffer.extend_from_slice(line.as_bytes());
            buffer.push(b'\n');
            file.write_all(&buffer)?;
            if self.last_sync.elapsed() >= SYNC_INTERVAL {
                file.sync_data()?;
                self.last_sync = Instant::now();
            }
        }
        Ok(())
    }

    /// Continues an existing journal file (after a recovery replay).
    pub fn continue_existing(&mut self) {
        // The next `append` opens the existing file in append mode and does
        // not write a second header.
        self.file = None;
    }

    /// Starts over after a save: deletes the file and records new base
    /// hashes.
    ///
    /// # Errors
    ///
    /// Fails if the file exists but cannot be deleted.
    pub fn reset(&mut self, hashes: &BTreeMap<String, u64>) -> io::Result<()> {
        self.file = None;
        self.header.base = hex_hashes(hashes);
        self.delete_file()
    }

    /// Deletes the journal file (the project was saved or changes were
    /// discarded on purpose).
    ///
    /// # Errors
    ///
    /// Fails if the file exists but cannot be deleted.
    pub fn delete_file(&mut self) -> io::Result<()> {
        self.file = None;
        match &self.path {
            Some(path) => match fs::remove_file(path) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            },
            None => Ok(()),
        }
    }

    /// Moves the journal to [`DISCARDED_FILE`], so a wrong decision can still
    /// be undone by hand.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be renamed.
    pub fn discard(&mut self) -> io::Result<()> {
        self.file = None;
        if let Some(path) = &self.path
            && path.is_file()
        {
            fs::rename(path, path.with_file_name(DISCARDED_FILE))?;
        }
        Ok(())
    }
}

/// Unsaved work found in a journal.
#[derive(Debug, Clone, PartialEq)]
pub struct Recovery {
    /// Header.
    pub header: JournalHeader,
    /// Entries in order.
    pub entries: Vec<Entry>,
    /// When the journal was last written (UTC), if known.
    pub modified: Option<String>,
    /// `true` if the saved files are still the ones the journal builds on.
    pub matches_files: bool,
    /// `true` if an incomplete last line was ignored.
    pub damaged_tail: bool,
}

impl Recovery {
    /// Number of committed transactions in the journal.
    #[must_use]
    pub fn commits(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| matches!(entry, Entry::Commit { .. }))
            .count()
    }
}

/// Reads the journal of the project in `root`, if there is one with at
/// least one entry. `hashes` are the hashes of the files as loaded.
///
/// # Errors
///
/// Fails if the file exists but cannot be read.
pub fn read_recovery(
    root: &Path,
    project: Id,
    hashes: &BTreeMap<String, u64>,
) -> io::Result<Option<Recovery>> {
    let path = root.join(STATE_DIR).join(JOURNAL_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut lines = text.split('\n');
    let Some(header) = lines
        .next()
        .and_then(|line| serde_json::from_str::<JournalHeader>(line).ok())
    else {
        return Ok(None);
    };
    if header.project != project || header.journal != JOURNAL_VERSION {
        return Ok(None);
    }
    let mut entries = Vec::new();
    let mut damaged_tail = false;
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let Ok(entry) = serde_json::from_str::<Entry>(line) else {
            // Only the last line can be incomplete (crash while writing).
            damaged_tail = true;
            break;
        };
        entries.push(entry);
    }
    if entries.is_empty() {
        return Ok(None);
    }
    let modified = fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .map(|seconds| DateTime::from_unix(seconds).iso_string());
    let matches_files = header.base == hex_hashes(hashes);
    Ok(Some(Recovery {
        header,
        entries,
        modified,
        matches_files,
        damaged_tail,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_header_once_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let project = Id::random();
        let hashes = BTreeMap::from([("a.ewproj".to_owned(), 0xabc_u64)]);
        let mut journal = Journal::new(dir.path(), project, &hashes);
        assert!(
            read_recovery(dir.path(), project, &hashes)
                .unwrap()
                .is_none()
        );
        journal
            .append(&commit_line(1, "first", &[]).unwrap())
            .unwrap();
        journal.append(r#"{"op":"undo"}"#).unwrap();
        journal.continue_existing();
        journal.append(r#"{"op":"redo"}"#).unwrap();

        let recovery = read_recovery(dir.path(), project, &hashes)
            .unwrap()
            .unwrap();
        assert_eq!(recovery.entries.len(), 3);
        assert_eq!(recovery.commits(), 1);
        assert!(recovery.matches_files);
        assert!(!recovery.damaged_tail);
        assert!(recovery.modified.is_some());
        assert_eq!(recovery.header.base["a.ewproj"], "0000000000000abc");

        let other = BTreeMap::from([("a.ewproj".to_owned(), 1_u64)]);
        assert!(
            !read_recovery(dir.path(), project, &other)
                .unwrap()
                .unwrap()
                .matches_files
        );
        assert!(
            read_recovery(dir.path(), Id::random(), &hashes)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn ignores_a_torn_last_line() {
        let dir = tempfile::tempdir().unwrap();
        let project = Id::random();
        let mut journal = Journal::new(dir.path(), project, &BTreeMap::new());
        journal.append(&commit_line(1, "a", &[]).unwrap()).unwrap();
        let path = journal.path().unwrap().to_path_buf();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(br#"{"op":"commit","seq":2,"lab"#).unwrap();
        let recovery = read_recovery(dir.path(), project, &BTreeMap::new())
            .unwrap()
            .unwrap();
        assert_eq!(recovery.entries.len(), 1);
        assert!(recovery.damaged_tail);
    }

    #[test]
    fn reset_discard_and_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let project = Id::random();
        let mut journal = Journal::new(dir.path(), project, &BTreeMap::new());
        journal.append(r#"{"op":"undo"}"#).unwrap();
        journal.discard().unwrap();
        assert!(dir.path().join(STATE_DIR).join(DISCARDED_FILE).is_file());
        assert!(
            read_recovery(dir.path(), project, &BTreeMap::new())
                .unwrap()
                .is_none()
        );
        journal.append(r#"{"op":"undo"}"#).unwrap();
        journal.reset(&BTreeMap::new()).unwrap();
        assert!(!journal.path().unwrap().exists());
        journal.delete_file().unwrap();

        let mut disabled = Journal::disabled(project);
        disabled.append("x").unwrap();
        assert!(disabled.path().is_none());
    }
}
