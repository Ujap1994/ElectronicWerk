//! Changes: the unit of every edit.
//!
//! A [`Change`] stores the state before and after, so it can be applied in
//! both directions. Undo applies the changes of a transaction backwards; the
//! autosave journal stores them so a crashed session can be replayed.

use ew_format::store;
use ew_model::document::{DocProps, Document, DocumentKind};
use ew_model::error::ModelError;
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::project::{DocumentRef, Project, ProjectProps};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::EngineError;

/// A document together with its manifest entry and list position (used when
/// documents are added or removed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentEntry {
    /// Manifest entry.
    pub reference: DocumentRef,
    /// Position in the manifest list.
    pub index: usize,
    /// Content.
    pub document: Document,
}

/// One reversible change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[allow(
    clippy::large_enum_variant,
    reason = "changes live in transactions; boxing would only add indirection"
)]
pub enum Change {
    /// Insert (`before` is `None`), update, or delete (`after` is `None`) an
    /// object.
    Object {
        /// Document.
        doc: Id,
        /// Object before the change.
        before: Option<Object>,
        /// Object after the change.
        after: Option<Object>,
    },
    /// Replace document properties.
    Props {
        /// Document.
        doc: Id,
        /// Properties before.
        before: DocProps,
        /// Properties after.
        after: DocProps,
    },
    /// Add (`before` is `None`) or remove (`after` is `None`) a document.
    Document {
        /// Document before.
        before: Option<Box<DocumentEntry>>,
        /// Document after.
        after: Option<Box<DocumentEntry>>,
    },
    /// Replace project properties.
    Project {
        /// Properties before.
        before: ProjectProps,
        /// Properties after.
        after: ProjectProps,
    },
}

impl Change {
    /// The document this change touches (`None` for project properties).
    #[must_use]
    pub fn doc(&self) -> Option<Id> {
        match self {
            Self::Object { doc, .. } | Self::Props { doc, .. } => Some(*doc),
            Self::Document { before, after } => after
                .as_ref()
                .or(before.as_ref())
                .map(|entry| entry.reference.id),
            Self::Project { .. } => None,
        }
    }

    /// `true` if the manifest (document list or project properties) changes.
    #[must_use]
    pub const fn touches_manifest(&self) -> bool {
        matches!(self, Self::Document { .. } | Self::Project { .. })
    }

    /// Applies the change forwards (`forward`) or backwards.
    ///
    /// # Errors
    ///
    /// Fails if the project is not in the expected state (missing document,
    /// duplicate id, …). The project is unchanged in that case.
    pub fn apply(&self, project: &mut Project, forward: bool) -> Result<(), ModelError> {
        match self {
            Self::Object { doc, before, after } => {
                let (from, to) = if forward {
                    (before, after)
                } else {
                    (after, before)
                };
                let document = project
                    .document_mut(*doc)
                    .ok_or(ModelError::DocumentNotFound(*doc))?;
                match (from, to) {
                    (None, Some(object)) => document.insert_object(object.clone()),
                    (Some(_), Some(object)) => document.update_object(object.clone()).map(|_| ()),
                    (Some(object), None) => document.remove_object(object.id()).map(|_| ()),
                    (None, None) => Ok(()),
                }
            }
            Self::Props { doc, before, after } => {
                let props = if forward { after } else { before };
                project
                    .document_mut(*doc)
                    .ok_or(ModelError::DocumentNotFound(*doc))?
                    .set_props(props.clone())
            }
            Self::Document { before, after } => {
                let (from, to) = if forward {
                    (before, after)
                } else {
                    (after, before)
                };
                match (from, to) {
                    (None, Some(entry)) => project.add_document(
                        entry.reference.clone(),
                        entry.index,
                        entry.document.clone(),
                    ),
                    (Some(entry), None) => project.remove_document(entry.reference.id).map(|_| ()),
                    _ => Ok(()),
                }
            }
            Self::Project { before, after } => {
                project.set_props(if forward { after } else { before }.clone());
                Ok(())
            }
        }
    }
}

/// An edit request as sent by a client (`edit.apply`). Requests are turned
/// into [`Change`]s against the current state.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Insert a new object.
    Insert {
        /// Document.
        doc: Id,
        /// Object.
        object: Object,
    },
    /// Replace an object.
    Update {
        /// Document.
        doc: Id,
        /// New object (same id and type).
        object: Object,
    },
    /// Change some fields of an object (JSON merge patch).
    Patch {
        /// Document.
        doc: Id,
        /// Object id.
        id: Id,
        /// Fields to change.
        fields: Map<String, Value>,
    },
    /// Delete an object.
    Delete {
        /// Document.
        doc: Id,
        /// Object id.
        id: Id,
    },
    /// Change document properties (JSON merge patch).
    SetProps {
        /// Document.
        doc: Id,
        /// Properties to change.
        props: Map<String, Value>,
    },
    /// Change project properties (JSON merge patch).
    SetProject {
        /// Properties to change.
        props: Map<String, Value>,
    },
    /// Add an empty document.
    AddDocument {
        /// Kind.
        kind: DocumentKind,
        /// Name.
        name: String,
        /// Path (derived from the name if missing).
        path: Option<String>,
        /// Id (random if missing).
        id: Option<Id>,
    },
    /// Remove a document (sheet symbols that place it are removed too).
    RemoveDocument {
        /// Document.
        doc: Id,
    },
}

fn field<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a Value, EngineError> {
    object
        .get(name)
        .ok_or_else(|| EngineError::invalid_params(format!("missing field `{name}`")))
}

fn id_field(object: &Map<String, Value>, name: &str) -> Result<Id, EngineError> {
    serde_json::from_value(field(object, name)?.clone())
        .map_err(|error| EngineError::invalid_params(format!("`{name}`: {error}")))
}

fn map_field(object: &Map<String, Value>, name: &str) -> Result<Map<String, Value>, EngineError> {
    field(object, name)?
        .as_object()
        .cloned()
        .ok_or_else(|| EngineError::invalid_params(format!("`{name}` must be an object")))
}

fn parse_object(value: &Value, assign_id: bool) -> Result<Object, EngineError> {
    let mut value = value.clone();
    if assign_id
        && let Some(object) = value.as_object_mut()
        && !object.contains_key("id")
    {
        object.insert("id".to_owned(), Value::String(Id::random().to_string()));
    }
    serde_json::from_value(value)
        .map_err(|error| EngineError::invalid_params(format!("object: {error}")))
}

impl Request {
    /// Parses one request (`{"op": "insert", "doc": "…", "object": {…}}`).
    /// Inserted objects without an `id` get a new one.
    ///
    /// # Errors
    ///
    /// Fails for unknown operations or missing/invalid fields.
    pub fn parse(value: &Value) -> Result<Self, EngineError> {
        let object = value
            .as_object()
            .ok_or_else(|| EngineError::invalid_params("a change must be an object"))?;
        let op = field(object, "op")?
            .as_str()
            .ok_or_else(|| EngineError::invalid_params("`op` must be a string"))?;
        Ok(match op {
            "insert" => Self::Insert {
                doc: id_field(object, "doc")?,
                object: parse_object(field(object, "object")?, true)?,
            },
            "update" => Self::Update {
                doc: id_field(object, "doc")?,
                object: parse_object(field(object, "object")?, false)?,
            },
            "patch" => Self::Patch {
                doc: id_field(object, "doc")?,
                id: id_field(object, "id")?,
                fields: map_field(object, "fields")?,
            },
            "delete" => Self::Delete {
                doc: id_field(object, "doc")?,
                id: id_field(object, "id")?,
            },
            "setProps" => Self::SetProps {
                doc: id_field(object, "doc")?,
                props: map_field(object, "props")?,
            },
            "setProject" => Self::SetProject {
                props: map_field(object, "props")?,
            },
            "addDocument" => Self::AddDocument {
                kind: serde_json::from_value(field(object, "kind")?.clone())
                    .map_err(|error| EngineError::invalid_params(format!("`kind`: {error}")))?,
                name: field(object, "name")?
                    .as_str()
                    .ok_or_else(|| EngineError::invalid_params("`name` must be a string"))?
                    .to_owned(),
                path: object
                    .get("path")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                id: match object.get("id") {
                    None | Some(Value::Null) => None,
                    Some(_) => Some(id_field(object, "id")?),
                },
            },
            "removeDocument" => Self::RemoveDocument {
                doc: id_field(object, "doc")?,
            },
            other => {
                return Err(EngineError::invalid_params(format!(
                    "unknown change operation `{other}`"
                )));
            }
        })
    }

    /// Turns the request into changes against the current project state.
    ///
    /// # Errors
    ///
    /// Fails if the request does not fit the current state.
    pub fn resolve(self, project: &Project) -> Result<Vec<Change>, EngineError> {
        let document = |doc: Id| {
            project
                .document(doc)
                .ok_or_else(|| EngineError::not_found(format!("no document with id {doc}")))
        };
        let existing = |doc: Id, id: Id| {
            document(doc)?
                .object(id)
                .ok_or_else(|| EngineError::not_found(format!("no object with id {id}")))
        };
        Ok(match self {
            Self::Insert { doc, object } => {
                document(doc)?;
                vec![Change::Object {
                    doc,
                    before: None,
                    after: Some(object),
                }]
            }
            Self::Update { doc, object } => {
                let before = existing(doc, object.id())?;
                vec![Change::Object {
                    doc,
                    before: Some(before),
                    after: Some(object),
                }]
            }
            Self::Patch { doc, id, fields } => vec![patch_object(doc, existing(doc, id)?, fields)?],
            Self::Delete { doc, id } => vec![Change::Object {
                doc,
                before: Some(existing(doc, id)?),
                after: None,
            }],
            Self::SetProps { doc, props } => {
                let current = document(doc)?;
                let before = current.props();
                let mut value = before.to_json().map_err(EngineError::internal)?;
                merge_patch(&mut value, &Value::Object(props));
                let after = DocProps::from_json(current.kind(), value)
                    .map_err(|error| EngineError::invalid_params(format!("props: {error}")))?;
                check_name(after.name())?;
                vec![Change::Props { doc, before, after }]
            }
            Self::SetProject { props } => vec![set_project(project, props)?],
            Self::AddDocument {
                kind,
                name,
                path,
                id,
            } => vec![add_document(project, kind, &name, path, id)?],
            Self::RemoveDocument { doc } => remove_document(project, doc)?,
        })
    }
}

fn patch_object(
    doc: Id,
    before: Object,
    fields: Map<String, Value>,
) -> Result<Change, EngineError> {
    for locked in ["id", "type"] {
        if fields.contains_key(locked) {
            return Err(EngineError::invalid_params(format!(
                "`{locked}` cannot be changed"
            )));
        }
    }
    let mut value = serde_json::to_value(&before).map_err(EngineError::internal)?;
    merge_patch(&mut value, &Value::Object(fields));
    let after: Object = serde_json::from_value(value)
        .map_err(|error| EngineError::invalid_params(format!("patch: {error}")))?;
    Ok(Change::Object {
        doc,
        before: Some(before),
        after: Some(after),
    })
}

fn set_project(project: &Project, props: Map<String, Value>) -> Result<Change, EngineError> {
    let before = project.props();
    let mut value = serde_json::to_value(&before).map_err(EngineError::internal)?;
    merge_patch(&mut value, &Value::Object(props));
    let after: ProjectProps = serde_json::from_value(value)
        .map_err(|error| EngineError::invalid_params(format!("props: {error}")))?;
    check_name(&after.name)?;
    Ok(Change::Project { before, after })
}

fn add_document(
    project: &Project,
    kind: DocumentKind,
    name: &str,
    path: Option<String>,
    id: Option<Id>,
) -> Result<Change, EngineError> {
    check_name(name)?;
    let id = id.unwrap_or_else(Id::random);
    if project.document(id).is_some() {
        return Err(EngineError::conflict(format!(
            "document {id} already exists"
        )));
    }
    let path = match path {
        Some(path) => {
            store::check_document_path(&path, kind).map_err(EngineError::invalid_params)?;
            path
        }
        None => store::unique_document_path(project, kind, name),
    };
    Ok(Change::Document {
        before: None,
        after: Some(Box::new(DocumentEntry {
            reference: DocumentRef { id, kind, path },
            index: project.manifest.documents.len(),
            document: Document::new(kind, id, name),
        })),
    })
}

/// Removes a document and everything that points at it.
fn remove_document(project: &Project, doc: Id) -> Result<Vec<Change>, EngineError> {
    let index = project
        .position(doc)
        .ok_or_else(|| EngineError::not_found(format!("no document with id {doc}")))?;
    let mut changes = Vec::new();
    // Sheet symbols in other schematics that place this sheet.
    for (other_id, other) in &project.documents {
        if let Document::Schematic(sheet) = other
            && *other_id != doc
        {
            for symbol in sheet.sheets.iter().filter(|symbol| symbol.schematic == doc) {
                changes.push(Change::Object {
                    doc: *other_id,
                    before: Some(Object::Sheet(symbol.clone())),
                    after: None,
                });
            }
        }
    }
    if project.manifest.root_schematic == Some(doc) {
        let before = project.props();
        let mut after = before.clone();
        after.root_schematic = None;
        changes.push(Change::Project { before, after });
    }
    let (Some(reference), Some(document)) = (project.reference(doc), project.document(doc)) else {
        return Err(EngineError::not_found(format!("no document with id {doc}")));
    };
    changes.push(Change::Document {
        before: Some(Box::new(DocumentEntry {
            reference: reference.clone(),
            index,
            document: document.clone(),
        })),
        after: None,
    });
    Ok(changes)
}

/// Checks a project or document name.
///
/// # Errors
///
/// Fails for empty or overlong names.
pub fn check_name(name: &str) -> Result<(), EngineError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(EngineError::invalid_params("the name must not be empty"));
    }
    if trimmed.chars().count() > 100 {
        return Err(EngineError::invalid_params(
            "the name must not exceed 100 characters",
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(EngineError::invalid_params(
            "the name must not contain control characters",
        ));
    }
    Ok(())
}

/// Applies a JSON merge patch (RFC 7386): objects merge recursively, `null`
/// removes a member, everything else replaces.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    if let Value::Object(target) = target {
        for (key, value) in patch {
            if value.is_null() {
                target.remove(key);
            } else {
                merge_patch(target.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_patch_follows_rfc_7386() {
        let mut target = json!({"a": "b", "c": {"d": "e", "f": "g"}});
        merge_patch(&mut target, &json!({"a": "z", "c": {"f": null}}));
        assert_eq!(target, json!({"a": "z", "c": {"d": "e"}}));
        let mut scalar = json!(1);
        merge_patch(&mut scalar, &json!({"x": [1]}));
        assert_eq!(scalar, json!({"x": [1]}));
    }

    #[test]
    fn parses_requests() {
        let doc = Id::random();
        let insert = Request::parse(&json!({
            "op": "insert",
            "doc": doc.to_string(),
            "object": {"type": "junction", "at": [0, 0]}
        }))
        .unwrap();
        assert!(matches!(
            insert,
            Request::Insert {
                object: Object::Junction(_),
                ..
            }
        ));
        for bad in [
            json!(1),
            json!({"op": "fly"}),
            json!({"op": "delete", "doc": "x", "id": "y"}),
            json!({"op": "update", "doc": doc.to_string(), "object": {"type": "junction", "at": [0, 0]}}),
            json!({"op": "setProps", "doc": doc.to_string(), "props": 3}),
        ] {
            assert!(Request::parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn names() {
        assert!(check_name("Power supply").is_ok());
        assert!(check_name("  ").is_err());
        assert!(check_name(&"x".repeat(101)).is_err());
        assert!(check_name("a\nb").is_err());
    }
}
