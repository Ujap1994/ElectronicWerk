//! Crash-safe file writes.
//!
//! Files are first written to a temporary file in the same folder, flushed to
//! disk and then renamed over the target. A crash at any moment leaves
//! either the old or the new file, never a half-written one.

use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::FormatError;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 64-bit FNV-1a hash (used to detect changed files).
#[must_use]
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn temp_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    target.with_file_name(format!(".{name}.{}.{count}.tmp", std::process::id()))
}

/// `true` for temporary files created by [`StagedWrites`].
#[must_use]
pub fn is_temp_file(name: &str) -> bool {
    name.starts_with('.')
        && Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("tmp"))
}

/// A group of files that are written together.
///
/// [`stage`](Self::stage) writes and syncs temporary files;
/// [`commit`](Self::commit) renames them all into place. Dropping without
/// committing removes the temporary files.
#[derive(Debug, Default)]
pub struct StagedWrites {
    staged: Vec<(PathBuf, PathBuf)>,
}

impl StagedWrites {
    /// An empty group.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes `bytes` to a temporary file next to `target`.
    ///
    /// # Errors
    ///
    /// Fails if the folder cannot be created or the file cannot be written.
    pub fn stage(&mut self, target: &Path, bytes: &[u8]) -> Result<(), FormatError> {
        let staged = stage_file(target, bytes)?;
        self.adopt(staged);
        Ok(())
    }

    /// Takes over a file staged with [`stage_file`] (for example on another
    /// thread).
    pub fn adopt(&mut self, staged: StagedFile) {
        self.staged.push((staged.temp, staged.target));
    }

    /// Renames every staged file into place.
    ///
    /// # Errors
    ///
    /// Fails if a rename fails; files renamed before stay in place.
    pub fn commit(mut self) -> Result<(), FormatError> {
        let staged = std::mem::take(&mut self.staged);
        let mut folders: Vec<PathBuf> = Vec::new();
        for (index, (temp, target)) in staged.iter().enumerate() {
            if let Err(error) = rename_with_retry(temp, target) {
                for (rest, _) in &staged[index..] {
                    let _ = fs::remove_file(rest);
                }
                return Err(FormatError::io(target, error));
            }
            if let Some(parent) = target.parent()
                && !folders.iter().any(|folder| folder == parent)
            {
                folders.push(parent.to_path_buf());
            }
        }
        for folder in folders {
            sync_dir(&folder);
        }
        Ok(())
    }
}

impl Drop for StagedWrites {
    fn drop(&mut self) {
        for (temp, _) in &self.staged {
            let _ = fs::remove_file(temp);
        }
    }
}

/// A temporary file waiting to be renamed into place.
#[derive(Debug)]
pub struct StagedFile {
    temp: PathBuf,
    target: PathBuf,
}

/// Writes and syncs a temporary file for `target`; see [`StagedWrites`].
///
/// # Errors
///
/// Fails if the folder cannot be created or the file cannot be written.
pub fn stage_file(target: &Path, bytes: &[u8]) -> Result<StagedFile, FormatError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| FormatError::io(parent, error))?;
    }
    let temp = temp_path(target);
    let result = (|| {
        let mut file = File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temp);
        return Err(FormatError::io(target, error));
    }
    Ok(StagedFile {
        temp,
        target: target.to_path_buf(),
    })
}

/// Writes one file atomically.
///
/// # Errors
///
/// See [`StagedWrites`].
pub fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), FormatError> {
    let mut writes = StagedWrites::new();
    writes.stage(target, bytes)?;
    writes.commit()
}

/// Renames, retrying briefly on Windows where virus scanners and indexers
/// may hold the target open for a moment.
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    let attempts = if cfg!(windows) { 10 } else { 1 };
    let mut result = fs::rename(from, to);
    for attempt in 1..attempts {
        match &result {
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                std::thread::sleep(std::time::Duration::from_millis(20 * attempt));
                result = fs::rename(from, to);
            }
            _ => break,
        }
    }
    result
}

/// Flushes a folder entry to disk (Unix only; a no-op elsewhere).
fn sync_dir(folder: &Path) {
    #[cfg(unix)]
    if let Ok(dir) = File::open(folder) {
        let _ = dir.sync_all();
    }
    #[cfg(not(unix))]
    let _ = folder;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_atomically_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("sub/file.txt");
        write_atomic(&target, b"one").unwrap();
        write_atomic(&target, b"two").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"two");
        let leftovers: Vec<_> = fs::read_dir(target.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| is_temp_file(&entry.file_name().to_string_lossy()))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn dropping_uncommitted_writes_removes_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("a.txt");
        {
            let mut writes = StagedWrites::new();
            writes.stage(&target, b"x").unwrap();
        }
        assert!(!target.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_ne!(fnv1a(b"ab"), fnv1a(b"ba"));
    }
}
