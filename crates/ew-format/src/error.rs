//! File format errors.

use std::io;
use std::path::PathBuf;

/// Something went wrong reading or writing project files.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// A file system operation failed.
    #[error("{}: {source}", path.display())]
    Io {
        /// File or folder.
        path: PathBuf,
        /// Cause.
        source: io::Error,
    },
    /// The file is not valid JSON or does not match the schema.
    #[error("{}: {message}", path.display())]
    Parse {
        /// File.
        path: PathBuf,
        /// Description including line and column.
        message: String,
    },
    /// The file holds another kind of data.
    #[error("{}: expected `{expected}` but the file says `{found}`", path.display())]
    WrongFormat {
        /// File.
        path: PathBuf,
        /// Expected format tag.
        expected: &'static str,
        /// Format tag in the file.
        found: String,
    },
    /// The file was written by a newer version of ElectronicWerk.
    #[error(
        "{} was saved by a newer ElectronicWerk (schema version {found}); this version reads up to {supported}",
        path.display()
    )]
    NewerVersion {
        /// File.
        path: PathBuf,
        /// Version in the file.
        found: u32,
        /// Newest version this build understands.
        supported: u32,
    },
    /// No migration path from an old schema version.
    #[error("{}: cannot upgrade from schema version {from}: {message}", path.display())]
    Migration {
        /// File.
        path: PathBuf,
        /// Version in the file.
        from: u32,
        /// Reason.
        message: String,
    },
    /// No project file in a folder.
    #[error("{}: no ElectronicWerk project (.ewproj) found", .0.display())]
    NoManifest(PathBuf),
    /// Several project files in one folder.
    #[error("{}: more than one project file (.ewproj); open the file directly", .0.display())]
    AmbiguousManifest(PathBuf),
    /// The target folder of a new project is not empty.
    #[error("{} already exists and is not empty", .0.display())]
    NotEmpty(PathBuf),
    /// The data is readable but inconsistent.
    #[error("{}: {message}", path.display())]
    Invalid {
        /// File.
        path: PathBuf,
        /// Description.
        message: String,
    },
}

impl FormatError {
    /// Wraps an I/O error with the path it concerns.
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Machine-readable error code (used by the engine API).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { source, .. } if source.kind() == io::ErrorKind::NotFound => "notFound",
            Self::Io { .. } => "ioError",
            Self::NoManifest(_) => "notFound",
            Self::NotEmpty(_) => "alreadyExists",
            Self::NewerVersion { .. } => "newerFormat",
            Self::Parse { .. }
            | Self::WrongFormat { .. }
            | Self::Migration { .. }
            | Self::AmbiguousManifest(_)
            | Self::Invalid { .. } => "formatError",
        }
    }
}
