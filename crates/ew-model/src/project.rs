//! Projects and the project manifest (`.ewproj`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::document::{Document, DocumentKind};
use crate::error::ModelError;
use crate::id::Id;
use crate::units::{Nm, um};

/// Units shown in the user interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Units {
    /// Millimetres.
    #[default]
    Mm,
    /// Mils (1/1000 inch).
    Mil,
    /// Inches.
    Inch,
}

/// Project-wide editor settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectSettings {
    /// Display units.
    pub units: Units,
    /// Schematic grid.
    pub schematic_grid: Nm,
    /// Board grid.
    pub board_grid: Nm,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            units: Units::Mm,
            schematic_grid: um(1_270),
            board_grid: um(250),
        }
    }
}

/// An entry in the manifest's document list.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRef {
    /// Document id (also stored inside the document).
    pub id: Id,
    /// Document kind.
    pub kind: DocumentKind,
    /// File path relative to the project folder, with `/` separators.
    pub path: String,
}

/// The project manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Project id.
    pub id: Id,
    /// Project name.
    pub name: String,
    /// Revision, for example `A` or `1.2`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub revision: String,
    /// Description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// User text variables, usable as `${NAME}` in texts and title blocks.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, String>,
    /// Documents in project tree order.
    pub documents: Vec<DocumentRef>,
    /// The top sheet of the schematic hierarchy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_schematic: Option<Id>,
    /// Editor settings.
    #[serde(default)]
    pub settings: ProjectSettings,
}

/// Editable project properties (the manifest without its document list).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProps {
    /// Project name.
    pub name: String,
    /// Revision.
    #[serde(default)]
    pub revision: String,
    /// Description.
    #[serde(default)]
    pub description: String,
    /// Text variables.
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    /// Top sheet.
    #[serde(default)]
    pub root_schematic: Option<Id>,
    /// Editor settings.
    #[serde(default)]
    pub settings: ProjectSettings,
}

/// A project in memory: the manifest and every document.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    /// The manifest.
    pub manifest: Manifest,
    /// Documents by id. Every manifest entry has a document here.
    pub documents: BTreeMap<Id, Document>,
}

impl Project {
    /// An empty project.
    #[must_use]
    pub fn new(id: Id, name: impl Into<String>) -> Self {
        Self {
            manifest: Manifest {
                id,
                name: name.into(),
                revision: String::new(),
                description: String::new(),
                variables: BTreeMap::new(),
                documents: Vec::new(),
                root_schematic: None,
                settings: ProjectSettings::default(),
            },
            documents: BTreeMap::new(),
        }
    }

    /// The document with this id.
    #[must_use]
    pub fn document(&self, id: Id) -> Option<&Document> {
        self.documents.get(&id)
    }

    /// Mutable access to a document.
    pub fn document_mut(&mut self, id: Id) -> Option<&mut Document> {
        self.documents.get_mut(&id)
    }

    /// The manifest entry of a document.
    #[must_use]
    pub fn reference(&self, id: Id) -> Option<&DocumentRef> {
        self.manifest.documents.iter().find(|entry| entry.id == id)
    }

    /// Position of a document in the manifest list.
    #[must_use]
    pub fn position(&self, id: Id) -> Option<usize> {
        self.manifest
            .documents
            .iter()
            .position(|entry| entry.id == id)
    }

    /// Manifest entries of one kind, in tree order.
    pub fn documents_of(&self, kind: DocumentKind) -> impl Iterator<Item = &DocumentRef> + '_ {
        self.manifest
            .documents
            .iter()
            .filter(move |entry| entry.kind == kind)
    }

    /// Adds a document at `index` (clamped to the list length).
    ///
    /// # Errors
    ///
    /// Fails if the id or path is already used or the kinds do not match.
    pub fn add_document(
        &mut self,
        reference: DocumentRef,
        index: usize,
        document: Document,
    ) -> Result<(), ModelError> {
        if self.documents.contains_key(&reference.id) || document.id() != reference.id {
            return Err(ModelError::DuplicateDocument(reference.id));
        }
        if self
            .manifest
            .documents
            .iter()
            .any(|entry| entry.path.eq_ignore_ascii_case(&reference.path))
        {
            return Err(ModelError::DuplicatePath(reference.path));
        }
        if document.kind() != reference.kind {
            return Err(ModelError::PropsMismatch {
                given: document.kind().name(),
                document: reference.kind.name(),
            });
        }
        let index = index.min(self.manifest.documents.len());
        self.documents.insert(reference.id, document);
        self.manifest.documents.insert(index, reference);
        Ok(())
    }

    /// Removes a document and returns its entry, position and content.
    ///
    /// # Errors
    ///
    /// Fails if there is no such document.
    pub fn remove_document(
        &mut self,
        id: Id,
    ) -> Result<(DocumentRef, usize, Document), ModelError> {
        let index = self.position(id).ok_or(ModelError::DocumentNotFound(id))?;
        let document = self
            .documents
            .remove(&id)
            .ok_or(ModelError::DocumentNotFound(id))?;
        let reference = self.manifest.documents.remove(index);
        Ok((reference, index, document))
    }

    /// The editable project properties.
    #[must_use]
    pub fn props(&self) -> ProjectProps {
        let manifest = &self.manifest;
        ProjectProps {
            name: manifest.name.clone(),
            revision: manifest.revision.clone(),
            description: manifest.description.clone(),
            variables: manifest.variables.clone(),
            root_schematic: manifest.root_schematic,
            settings: manifest.settings,
        }
    }

    /// Replaces the editable project properties.
    pub fn set_props(&mut self, props: ProjectProps) {
        let manifest = &mut self.manifest;
        manifest.name = props.name;
        manifest.revision = props.revision;
        manifest.description = props.description;
        manifest.variables = props.variables;
        manifest.root_schematic = props.root_schematic;
        manifest.settings = props.settings;
    }

    /// Total number of objects in all documents.
    #[must_use]
    pub fn object_count(&self) -> usize {
        self.documents.values().map(Document::object_count).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::Schematic;

    fn schematic_ref(id: Id, path: &str) -> DocumentRef {
        DocumentRef {
            id,
            kind: DocumentKind::Schematic,
            path: path.into(),
        }
    }

    #[test]
    fn add_and_remove_documents() {
        let mut project = Project::new(Id::random(), "Demo");
        let a = Id::random();
        let b = Id::random();
        project
            .add_document(
                schematic_ref(a, "schematics/a.ewsch"),
                0,
                Document::Schematic(Schematic::new(a, "A")),
            )
            .unwrap();
        project
            .add_document(
                schematic_ref(b, "schematics/b.ewsch"),
                0,
                Document::Schematic(Schematic::new(b, "B")),
            )
            .unwrap();
        assert_eq!(project.position(b), Some(0));
        assert_eq!(project.documents_of(DocumentKind::Schematic).count(), 2);

        let duplicate_path = project.add_document(
            schematic_ref(Id::random(), "Schematics/A.ewsch"),
            9,
            Document::Schematic(Schematic::new(Id::NIL, "C")),
        );
        assert!(duplicate_path.is_err());

        let (reference, index, document) = project.remove_document(b).unwrap();
        assert_eq!((reference.id, index, document.id()), (b, 0, b));
        assert!(project.remove_document(b).is_err());
        assert_eq!(project.manifest.documents.len(), 1);
    }

    #[test]
    fn props_round_trip() {
        let mut project = Project::new(Id::random(), "Demo");
        let mut props = project.props();
        props.revision = "B".into();
        props.variables.insert("AUTHOR".into(), "Lab".into());
        project.set_props(props.clone());
        assert_eq!(project.props(), props);
    }
}
