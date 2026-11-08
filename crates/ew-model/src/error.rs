//! Model errors.

use crate::id::Id;

/// An edit that does not fit the model.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// An object with this id already exists in the document.
    #[error("an object with id {0} already exists")]
    DuplicateId(Id),
    /// There is no object with this id.
    #[error("no object with id {0}")]
    ObjectNotFound(Id),
    /// The object type does not belong in this kind of document.
    #[error("a {object} cannot be placed in a {document} document")]
    NotAllowed {
        /// Object type name.
        object: &'static str,
        /// Document kind name.
        document: &'static str,
    },
    /// An update tried to change an object's type.
    #[error("object {id} is a {existing}, not a {requested}")]
    TypeMismatch {
        /// Object id.
        id: Id,
        /// Type of the existing object.
        existing: &'static str,
        /// Type in the update.
        requested: &'static str,
    },
    /// Document properties of the wrong kind.
    #[error("{given} properties cannot be applied to a {document} document")]
    PropsMismatch {
        /// Kind of the properties.
        given: &'static str,
        /// Kind of the document.
        document: &'static str,
    },
    /// There is no document with this id.
    #[error("no document with id {0}")]
    DocumentNotFound(Id),
    /// A document with this id already exists.
    #[error("a document with id {0} already exists")]
    DuplicateDocument(Id),
    /// Another document already uses this file path.
    #[error("another document already uses the path `{0}`")]
    DuplicatePath(String),
}
