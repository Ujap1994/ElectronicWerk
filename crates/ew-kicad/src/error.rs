//! Import errors.

use std::path::PathBuf;

use crate::sexpr::ParseError;

/// Why an import failed as a whole.
#[derive(Debug, thiserror::Error)]
pub enum KicadError {
    /// A file could not be read.
    #[error("cannot read {path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The cause.
        source: std::io::Error,
    },
    /// A file is not valid S-expression or JSON.
    #[error("{path}: {source}")]
    Parse {
        /// The file.
        path: PathBuf,
        /// The cause.
        source: ParseError,
    },
    /// The file is not what was expected.
    #[error("{path}: {message}")]
    Unsupported {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        message: String,
    },
}
