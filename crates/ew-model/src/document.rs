//! Documents: the typed contents of a project's files.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ModelError;
use crate::id::Id;
use crate::library::{Library, LibraryProps};
use crate::object::{Object, ObjectType};
use crate::outputs::{OutputJob, OutputJobProps};
use crate::pcb::{Pcb, PcbProps};
use crate::rules::{Rules, RulesProps};
use crate::schematic::{Schematic, SchematicProps};

/// Kind of document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentKind {
    /// Schematic sheet (`.ewsch`).
    Schematic,
    /// Board (`.ewpcb`).
    Pcb,
    /// Library (`.ewlib`).
    Library,
    /// Design rules (`.ewrules`).
    Rules,
    /// Output job (`.ewout`).
    OutputJob,
}

impl DocumentKind {
    /// Every kind, in project tree order.
    pub const ALL: [Self; 5] = [
        Self::Schematic,
        Self::Pcb,
        Self::Library,
        Self::Rules,
        Self::OutputJob,
    ];

    /// Name used in JSON and messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Schematic => "schematic",
            Self::Pcb => "pcb",
            Self::Library => "library",
            Self::Rules => "rules",
            Self::OutputJob => "outputJob",
        }
    }

    /// File extension (without the dot).
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Schematic => "ewsch",
            Self::Pcb => "ewpcb",
            Self::Library => "ewlib",
            Self::Rules => "ewrules",
            Self::OutputJob => "ewout",
        }
    }

    /// Default folder inside the project.
    #[must_use]
    pub const fn folder(self) -> &'static str {
        match self {
            Self::Schematic => "schematics",
            Self::Pcb => "pcb",
            Self::Library => "library",
            Self::Rules => "rules",
            Self::OutputJob => "outputs",
        }
    }

    /// The kind with this file extension.
    #[must_use]
    pub fn from_extension(extension: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.extension().eq_ignore_ascii_case(extension))
    }
}

impl fmt::Display for DocumentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A document's content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "docKind", rename_all = "camelCase")]
#[allow(
    clippy::large_enum_variant,
    reason = "documents are few and long-lived; boxing would only add indirection"
)]
pub enum Document {
    /// A schematic sheet.
    Schematic(Schematic),
    /// A board.
    Pcb(Pcb),
    /// A library.
    Library(Library),
    /// Design rules.
    Rules(Rules),
    /// An output job.
    OutputJob(OutputJob),
}

/// Editable document properties (everything except the objects).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "docKind", rename_all = "camelCase")]
pub enum DocProps {
    /// Schematic properties.
    Schematic(SchematicProps),
    /// Board properties.
    Pcb(PcbProps),
    /// Library properties.
    Library(LibraryProps),
    /// Rules.
    Rules(RulesProps),
    /// Output job.
    OutputJob(OutputJobProps),
}

impl DocProps {
    /// Kind of document these properties belong to.
    #[must_use]
    pub const fn kind(&self) -> DocumentKind {
        match self {
            Self::Schematic(_) => DocumentKind::Schematic,
            Self::Pcb(_) => DocumentKind::Pcb,
            Self::Library(_) => DocumentKind::Library,
            Self::Rules(_) => DocumentKind::Rules,
            Self::OutputJob(_) => DocumentKind::OutputJob,
        }
    }

    /// The document name in these properties.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Schematic(props) => &props.name,
            Self::Pcb(props) => &props.name,
            Self::Library(props) => &props.name,
            Self::Rules(props) => &props.name,
            Self::OutputJob(props) => &props.name,
        }
    }

    /// Parses properties of a known kind from JSON.
    ///
    /// # Errors
    ///
    /// Fails if the JSON does not match the properties of `kind`.
    pub fn from_json(kind: DocumentKind, value: serde_json::Value) -> serde_json::Result<Self> {
        Ok(match kind {
            DocumentKind::Schematic => Self::Schematic(serde_json::from_value(value)?),
            DocumentKind::Pcb => Self::Pcb(serde_json::from_value(value)?),
            DocumentKind::Library => Self::Library(serde_json::from_value(value)?),
            DocumentKind::Rules => Self::Rules(serde_json::from_value(value)?),
            DocumentKind::OutputJob => Self::OutputJob(serde_json::from_value(value)?),
        })
    }

    /// The properties as JSON without the kind tag.
    ///
    /// # Errors
    ///
    /// Fails only if serialization fails (it does not for these types).
    pub fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        match self {
            Self::Schematic(props) => serde_json::to_value(props),
            Self::Pcb(props) => serde_json::to_value(props),
            Self::Library(props) => serde_json::to_value(props),
            Self::Rules(props) => serde_json::to_value(props),
            Self::OutputJob(props) => serde_json::to_value(props),
        }
    }
}

impl Document {
    /// A new, empty document of `kind`.
    #[must_use]
    pub fn new(kind: DocumentKind, id: Id, name: &str) -> Self {
        match kind {
            DocumentKind::Schematic => Self::Schematic(Schematic::new(id, name)),
            DocumentKind::Pcb => Self::Pcb(Pcb::new(
                id,
                name,
                crate::pcb::Stackup::standard(2, crate::pcb::STANDARD_THICKNESS),
            )),
            DocumentKind::Library => Self::Library(Library::new(id, name)),
            DocumentKind::Rules => Self::Rules(Rules::new(id, name)),
            DocumentKind::OutputJob => Self::OutputJob(OutputJob::new(id, name)),
        }
    }

    /// The document kind.
    #[must_use]
    pub const fn kind(&self) -> DocumentKind {
        match self {
            Self::Schematic(_) => DocumentKind::Schematic,
            Self::Pcb(_) => DocumentKind::Pcb,
            Self::Library(_) => DocumentKind::Library,
            Self::Rules(_) => DocumentKind::Rules,
            Self::OutputJob(_) => DocumentKind::OutputJob,
        }
    }

    /// The document id.
    #[must_use]
    pub const fn id(&self) -> Id {
        match self {
            Self::Schematic(doc) => doc.id,
            Self::Pcb(doc) => doc.id,
            Self::Library(doc) => doc.id,
            Self::Rules(doc) => doc.id,
            Self::OutputJob(doc) => doc.id,
        }
    }

    /// The document name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Schematic(doc) => &doc.name,
            Self::Pcb(doc) => &doc.name,
            Self::Library(doc) => &doc.name,
            Self::Rules(doc) => &doc.name,
            Self::OutputJob(doc) => &doc.name,
        }
    }

    /// The editable properties.
    #[must_use]
    pub fn props(&self) -> DocProps {
        match self {
            Self::Schematic(doc) => DocProps::Schematic(doc.props()),
            Self::Pcb(doc) => DocProps::Pcb(doc.props()),
            Self::Library(doc) => DocProps::Library(doc.props()),
            Self::Rules(doc) => DocProps::Rules(doc.props()),
            Self::OutputJob(doc) => DocProps::OutputJob(doc.props()),
        }
    }

    /// Replaces the editable properties.
    ///
    /// # Errors
    ///
    /// Fails if the properties belong to another document kind.
    pub fn set_props(&mut self, props: DocProps) -> Result<(), ModelError> {
        match (self, props) {
            (Self::Schematic(doc), DocProps::Schematic(props)) => doc.set_props(props),
            (Self::Pcb(doc), DocProps::Pcb(props)) => doc.set_props(props),
            (Self::Library(doc), DocProps::Library(props)) => doc.set_props(props),
            (Self::Rules(doc), DocProps::Rules(props)) => doc.set_props(props),
            (Self::OutputJob(doc), DocProps::OutputJob(props)) => doc.set_props(props),
            (doc, props) => {
                return Err(ModelError::PropsMismatch {
                    given: props.kind().name(),
                    document: doc.kind().name(),
                });
            }
        }
        Ok(())
    }

    /// The object with this id.
    #[must_use]
    pub fn object(&self, id: Id) -> Option<Object> {
        match self {
            Self::Schematic(doc) => doc.get_object(id),
            Self::Pcb(doc) => doc.get_object(id),
            Self::Library(doc) => doc.get_object(id),
            Self::Rules(_) | Self::OutputJob(_) => None,
        }
    }

    /// Type of the object with this id.
    #[must_use]
    pub fn object_type(&self, id: Id) -> Option<ObjectType> {
        match self {
            Self::Schematic(doc) => doc.type_of(id),
            Self::Pcb(doc) => doc.type_of(id),
            Self::Library(doc) => doc.type_of(id),
            Self::Rules(_) | Self::OutputJob(_) => None,
        }
    }

    /// Adds a new object.
    ///
    /// # Errors
    ///
    /// Fails if the id is taken or the type does not belong here.
    pub fn insert_object(&mut self, object: Object) -> Result<(), ModelError> {
        let id = object.id();
        if self.object_type(id).is_some() {
            return Err(ModelError::DuplicateId(id));
        }
        self.put(object).map(|_| ())
    }

    /// Replaces an existing object of the same type and returns the old one.
    ///
    /// # Errors
    ///
    /// Fails if there is no such object or its type differs.
    pub fn update_object(&mut self, object: Object) -> Result<Object, ModelError> {
        let id = object.id();
        let existing = self.object_type(id).ok_or(ModelError::ObjectNotFound(id))?;
        let requested = object.object_type();
        if existing != requested {
            return Err(ModelError::TypeMismatch {
                id,
                existing: existing.name(),
                requested: requested.name(),
            });
        }
        self.put(object)?.ok_or(ModelError::ObjectNotFound(id))
    }

    /// Removes an object.
    ///
    /// # Errors
    ///
    /// Fails if there is no such object.
    pub fn remove_object(&mut self, id: Id) -> Result<Object, ModelError> {
        let removed = match self {
            Self::Schematic(doc) => doc.take_object(id),
            Self::Pcb(doc) => doc.take_object(id),
            Self::Library(doc) => doc.take_object(id),
            Self::Rules(_) | Self::OutputJob(_) => None,
        };
        removed.ok_or(ModelError::ObjectNotFound(id))
    }

    /// Number of objects in the document's collections.
    #[must_use]
    pub fn object_count(&self) -> usize {
        match self {
            Self::Schematic(doc) => doc.count_objects(),
            Self::Pcb(doc) => doc.count_objects(),
            Self::Library(doc) => doc.count_objects(),
            Self::Rules(_) | Self::OutputJob(_) => 0,
        }
    }

    /// Calls `visit` with every object id and type.
    pub fn visit_ids(&self, visit: &mut dyn FnMut(Id, ObjectType)) {
        match self {
            Self::Schematic(doc) => doc.visit_ids(visit),
            Self::Pcb(doc) => doc.visit_ids(visit),
            Self::Library(doc) => doc.visit_ids(visit),
            Self::Rules(_) | Self::OutputJob(_) => {}
        }
    }

    fn put(&mut self, object: Object) -> Result<Option<Object>, ModelError> {
        let kind = self.kind();
        let result = match self {
            Self::Schematic(doc) => doc.put_object(object),
            Self::Pcb(doc) => doc.put_object(object),
            Self::Library(doc) => doc.put_object(object),
            Self::Rules(_) | Self::OutputJob(_) => Err(object.object_type()),
        };
        result.map_err(|object_type| ModelError::NotAllowed {
            object: object_type.name(),
            document: kind.name(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::Wire;
    use crate::units::Point;

    fn wire(id: Id) -> Object {
        Object::Wire(Wire {
            id,
            start: Point::ORIGIN,
            end: Point::mm(1, 0),
        })
    }

    #[test]
    fn object_lifecycle() {
        let mut doc = Document::new(DocumentKind::Schematic, Id::random(), "Root");
        let id = Id::random();
        doc.insert_object(wire(id)).unwrap();
        assert_eq!(
            doc.insert_object(wire(id)),
            Err(ModelError::DuplicateId(id))
        );
        assert_eq!(doc.object_type(id), Some(ObjectType::Wire));
        assert_eq!(doc.object_count(), 1);

        let old = doc.update_object(wire(id)).unwrap();
        assert_eq!(old.id(), id);
        let junction = Object::Junction(crate::schematic::Junction {
            id,
            at: Point::ORIGIN,
        });
        assert!(matches!(
            doc.update_object(junction),
            Err(ModelError::TypeMismatch { .. })
        ));
        assert_eq!(doc.remove_object(id).unwrap().id(), id);
        assert_eq!(doc.remove_object(id), Err(ModelError::ObjectNotFound(id)));
    }

    #[test]
    fn rejects_objects_of_other_documents() {
        let mut pcb = Document::new(DocumentKind::Pcb, Id::random(), "Board");
        assert!(matches!(
            pcb.insert_object(wire(Id::random())),
            Err(ModelError::NotAllowed {
                object: "wire",
                document: "pcb"
            })
        ));
        let mut rules = Document::new(DocumentKind::Rules, Id::random(), "Rules");
        assert!(rules.insert_object(wire(Id::random())).is_err());
    }

    #[test]
    fn props_must_match_kind() {
        let mut doc = Document::new(DocumentKind::Library, Id::random(), "Lib");
        let schematic_props = Document::new(DocumentKind::Schematic, Id::random(), "S").props();
        assert!(doc.set_props(schematic_props).is_err());
        let mut props = doc.props();
        if let DocProps::Library(library) = &mut props {
            library.name = "Renamed".into();
        }
        doc.set_props(props).unwrap();
        assert_eq!(doc.name(), "Renamed");
    }

    #[test]
    fn kinds_and_extensions() {
        for kind in DocumentKind::ALL {
            assert_eq!(DocumentKind::from_extension(kind.extension()), Some(kind));
            let doc = Document::new(kind, Id::from_u128(1), "x");
            assert_eq!(doc.kind(), kind);
            let props = doc.props();
            let json = props.to_json().unwrap();
            assert_eq!(DocProps::from_json(kind, json).unwrap(), props);
            let tagged = serde_json::to_string(&doc).unwrap();
            assert_eq!(serde_json::from_str::<Document>(&tagged).unwrap(), doc);
        }
        assert_eq!(DocumentKind::from_extension("txt"), None);
    }
}
