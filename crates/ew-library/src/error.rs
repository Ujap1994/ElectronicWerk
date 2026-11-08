//! Errors of the library index.

use std::path::PathBuf;

/// An error of the library index.
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The cause.
        source: std::io::Error,
    },
    /// The database reported an error.
    #[error("library index: {0}")]
    Sqlite(String),
    /// A file has an unexpected content.
    #[error("{0}")]
    Format(String),
    /// Something does not exist.
    #[error("not found: {0}")]
    NotFound(String),
}
