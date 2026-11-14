//! Engine errors and their machine-readable codes.

use ew_format::FormatError;
use ew_model::error::ModelError;
use serde::Serialize;

/// Machine-readable error category (part of the engine API).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// The request was not valid JSON or did not match the request schema.
    InvalidRequest,
    /// No method with the given name exists.
    MethodNotFound,
    /// The parameters did not match what the method expects.
    InvalidParams,
    /// The method needs an open project.
    NoProject,
    /// A project is already open.
    ProjectOpen,
    /// The project has unsaved changes.
    UnsavedChanges,
    /// Unsaved changes from a crashed session wait for a decision.
    RecoveryPending,
    /// The edit does not fit the current state.
    Conflict,
    /// A file, document or object does not exist.
    NotFound,
    /// The target already exists.
    AlreadyExists,
    /// A file system operation failed.
    IoError,
    /// A file is not a valid ElectronicWerk file.
    FormatError,
    /// A file was written by a newer ElectronicWerk.
    NewerFormat,
    /// Another program changed project files since they were read: saving
    /// would overwrite that work.
    ChangedOnDisk,
    /// An unexpected internal error.
    Internal,
}

/// An error returned by an engine method.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct EngineError {
    /// Category.
    pub code: ErrorCode,
    /// Description (English, for logs and developers).
    pub message: String,
}

impl EngineError {
    /// A new error.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// `invalidParams`.
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidParams, message)
    }

    /// `notFound`.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// `conflict`.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }

    /// `noProject`.
    #[must_use]
    pub fn no_project() -> Self {
        Self::new(ErrorCode::NoProject, "no project is open")
    }

    /// `internal`.
    pub fn internal(error: impl std::fmt::Display) -> Self {
        Self::new(ErrorCode::Internal, error.to_string())
    }

    /// `ioError` for a failed file operation.
    #[must_use]
    pub fn io(context: &str, error: &std::io::Error) -> Self {
        Self::new(ErrorCode::IoError, format!("{context}: {error}"))
    }
}

impl From<FormatError> for EngineError {
    fn from(error: FormatError) -> Self {
        let code = match error.code() {
            "notFound" => ErrorCode::NotFound,
            "alreadyExists" => ErrorCode::AlreadyExists,
            "newerFormat" => ErrorCode::NewerFormat,
            "ioError" => ErrorCode::IoError,
            _ => ErrorCode::FormatError,
        };
        Self::new(code, error.to_string())
    }
}

impl From<ModelError> for EngineError {
    fn from(error: ModelError) -> Self {
        let code = match error {
            ModelError::ObjectNotFound(_) | ModelError::DocumentNotFound(_) => ErrorCode::NotFound,
            _ => ErrorCode::Conflict,
        };
        Self::new(code, error.to_string())
    }
}
