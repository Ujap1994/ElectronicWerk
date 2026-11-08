//! Encoding and decoding of single files, with schema versions and
//! migrations.
//!
//! Every file starts with a header:
//!
//! ```json
//! {
//!   "format": "electronicwerk.pcb",
//!   "schemaVersion": 1,
//!   …
//! }
//! ```

use std::path::Path;

use ew_model::document::{Document, DocumentKind};
use ew_model::project::Manifest;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::FormatError;
use crate::json;

/// The schema version this build writes and the newest it reads.
pub const SCHEMA_VERSION: u32 = 1;

/// What a file contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileKind {
    /// The project manifest (`.ewproj`).
    Manifest,
    /// A document.
    Document(DocumentKind),
}

impl FileKind {
    /// The `format` tag in the file header.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Manifest => "electronicwerk.project",
            Self::Document(DocumentKind::Schematic) => "electronicwerk.schematic",
            Self::Document(DocumentKind::Pcb) => "electronicwerk.pcb",
            Self::Document(DocumentKind::Library) => "electronicwerk.library",
            Self::Document(DocumentKind::Rules) => "electronicwerk.rules",
            Self::Document(DocumentKind::OutputJob) => "electronicwerk.outputJob",
        }
    }
}

/// A step that upgrades a file from `from` to `from + 1`.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// Version the step starts from.
    pub from: u32,
    /// What changes (for the upgrade report).
    pub description: &'static str,
    /// Rewrites the JSON in place.
    pub apply: fn(&mut Value) -> Result<(), String>,
}

/// Registered migrations for a file kind. Schema version 1 is the first
/// published version, so there are none yet; every future format change
/// adds a step here, together with a test fixture of the old version.
#[must_use]
pub const fn migrations(kind: FileKind) -> &'static [Migration] {
    match kind {
        FileKind::Manifest | FileKind::Document(_) => &[],
    }
}

/// A decoded file.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded<T> {
    /// The content.
    pub value: T,
    /// Original schema version if the file was upgraded on load.
    pub migrated_from: Option<u32>,
    /// Descriptions of the applied migration steps.
    pub notes: Vec<&'static str>,
}

/// Serializes a value with the header and the deterministic layout.
///
/// # Errors
///
/// Fails only if serialization fails, which does not happen for model types.
pub fn encode<T: Serialize>(kind: FileKind, value: &T) -> Result<String, serde_json::Error> {
    let body = serde_json::to_string(value)?;
    let mut compact = format!(
        "{{\"format\":\"{}\",\"schemaVersion\":{SCHEMA_VERSION}",
        kind.tag()
    );
    if body.len() > 2 {
        compact.push(',');
        compact.push_str(&body[1..]);
    } else {
        compact.push('}');
    }
    Ok(json::layout(&compact))
}

/// Serializes the manifest.
///
/// # Errors
///
/// See [`encode`].
pub fn encode_manifest(manifest: &Manifest) -> Result<String, serde_json::Error> {
    encode(FileKind::Manifest, manifest)
}

/// Serializes a document.
///
/// # Errors
///
/// See [`encode`].
pub fn encode_document(document: &Document) -> Result<String, serde_json::Error> {
    let kind = FileKind::Document(document.kind());
    match document {
        Document::Schematic(doc) => encode(kind, doc),
        Document::Pcb(doc) => encode(kind, doc),
        Document::Library(doc) => encode(kind, doc),
        Document::Rules(doc) => encode(kind, doc),
        Document::OutputJob(doc) => encode(kind, doc),
    }
}

/// Reads a manifest.
///
/// # Errors
///
/// Fails for invalid JSON, a wrong format tag, a newer schema version or
/// content that does not match the manifest schema.
pub fn decode_manifest(path: &Path, text: &str) -> Result<Decoded<Manifest>, FormatError> {
    decode_with(
        path,
        text,
        FileKind::Manifest,
        SCHEMA_VERSION,
        migrations(FileKind::Manifest),
    )
}

/// Reads a document of a known kind.
///
/// # Errors
///
/// See [`decode_manifest`].
pub fn decode_document(
    path: &Path,
    kind: DocumentKind,
    text: &str,
) -> Result<Decoded<Document>, FormatError> {
    let file_kind = FileKind::Document(kind);
    let chain = migrations(file_kind);
    let map = |decoded: Decoded<Document>| decoded;
    Ok(map(match kind {
        DocumentKind::Schematic => {
            decode_with(path, text, file_kind, SCHEMA_VERSION, chain)?.map(Document::Schematic)
        }
        DocumentKind::Pcb => {
            decode_with(path, text, file_kind, SCHEMA_VERSION, chain)?.map(Document::Pcb)
        }
        DocumentKind::Library => {
            decode_with(path, text, file_kind, SCHEMA_VERSION, chain)?.map(Document::Library)
        }
        DocumentKind::Rules => {
            decode_with(path, text, file_kind, SCHEMA_VERSION, chain)?.map(Document::Rules)
        }
        DocumentKind::OutputJob => {
            decode_with(path, text, file_kind, SCHEMA_VERSION, chain)?.map(Document::OutputJob)
        }
    }))
}

impl<T> Decoded<T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> Decoded<U> {
        Decoded {
            value: f(self.value),
            migrated_from: self.migrated_from,
            notes: self.notes,
        }
    }
}

#[derive(serde::Deserialize)]
struct Header {
    format: Option<String>,
    #[serde(rename = "schemaVersion")]
    schema_version: Option<u32>,
}

fn parse_error(path: &Path, error: &serde_json::Error) -> FormatError {
    FormatError::Parse {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

/// Decodes with an explicit target version and migration chain (tests use
/// this to exercise the migration machinery).
///
/// # Errors
///
/// See [`decode_manifest`].
pub fn decode_with<T: DeserializeOwned>(
    path: &Path,
    text: &str,
    kind: FileKind,
    current: u32,
    chain: &[Migration],
) -> Result<Decoded<T>, FormatError> {
    // Fast path: files written by this version start with a fixed header.
    let expected_start = format!(
        "{{\n  \"format\": \"{}\",\n  \"schemaVersion\": {current},",
        kind.tag()
    );
    if text.starts_with(&expected_start) {
        let value = serde_json::from_str(text).map_err(|error| parse_error(path, &error))?;
        return Ok(Decoded {
            value,
            migrated_from: None,
            notes: Vec::new(),
        });
    }

    let header: Header = serde_json::from_str(text).map_err(|error| parse_error(path, &error))?;
    let found = header.format.unwrap_or_default();
    if found != kind.tag() {
        return Err(FormatError::WrongFormat {
            path: path.to_path_buf(),
            expected: kind.tag(),
            found,
        });
    }
    let version = header.schema_version.unwrap_or(0);
    if version > current {
        return Err(FormatError::NewerVersion {
            path: path.to_path_buf(),
            found: version,
            supported: current,
        });
    }
    if version == current {
        let value = serde_json::from_str(text).map_err(|error| parse_error(path, &error))?;
        return Ok(Decoded {
            value,
            migrated_from: None,
            notes: Vec::new(),
        });
    }

    let mut value: Value = serde_json::from_str(text).map_err(|error| parse_error(path, &error))?;
    let mut notes = Vec::new();
    for step_from in version..current {
        let step = chain
            .iter()
            .find(|migration| migration.from == step_from)
            .ok_or_else(|| FormatError::Migration {
                path: path.to_path_buf(),
                from: step_from,
                message: "no upgrade step is available".to_owned(),
            })?;
        (step.apply)(&mut value).map_err(|message| FormatError::Migration {
            path: path.to_path_buf(),
            from: step_from,
            message,
        })?;
        notes.push(step.description);
    }
    if let Some(object) = value.as_object_mut() {
        object.insert("schemaVersion".to_owned(), Value::from(current));
    }
    let value = serde_json::from_value(value).map_err(|error| parse_error(path, &error))?;
    Ok(Decoded {
        value,
        migrated_from: Some(version),
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::id::Id;
    use ew_model::project::Project;
    use serde::Deserialize;

    fn path() -> &'static Path {
        Path::new("test.json")
    }

    #[test]
    fn header_comes_first() {
        let project = Project::new(Id::from_u128(1), "Demo");
        let text = encode_manifest(&project.manifest).unwrap();
        assert!(text.starts_with(
            "{\n  \"format\": \"electronicwerk.project\",\n  \"schemaVersion\": 1,\n  \"id\": "
        ));
        let decoded = decode_manifest(path(), &text).unwrap();
        assert_eq!(decoded.value, project.manifest);
        assert_eq!(decoded.migrated_from, None);
    }

    #[test]
    fn every_document_kind_round_trips() {
        for kind in DocumentKind::ALL {
            let document = Document::new(kind, Id::random(), "Doc");
            let text = encode_document(&document).unwrap();
            assert!(text.contains(FileKind::Document(kind).tag()));
            let decoded = decode_document(path(), kind, &text).unwrap();
            assert_eq!(decoded.value, document);
            assert_eq!(encode_document(&decoded.value).unwrap(), text);
        }
    }

    #[test]
    fn slow_path_accepts_reordered_headers() {
        let text = r#"{"name":"x","schemaVersion":1,"format":"electronicwerk.library","id":"00000000-0000-0000-0000-000000000005"}"#;
        let decoded = decode_document(path(), DocumentKind::Library, text).unwrap();
        assert_eq!(decoded.value.name(), "x");
    }

    #[test]
    fn rejects_wrong_format_and_newer_versions() {
        let library =
            encode_document(&Document::new(DocumentKind::Library, Id::random(), "L")).unwrap();
        assert!(matches!(
            decode_document(path(), DocumentKind::Pcb, &library),
            Err(FormatError::WrongFormat { .. })
        ));
        let newer = library.replace("\"schemaVersion\": 1", "\"schemaVersion\": 99");
        let error = decode_document(path(), DocumentKind::Library, &newer).unwrap_err();
        assert!(matches!(error, FormatError::NewerVersion { found: 99, .. }));
        assert_eq!(error.code(), "newerFormat");
        assert!(matches!(
            decode_document(path(), DocumentKind::Library, "{oops"),
            Err(FormatError::Parse { .. })
        ));
        assert!(matches!(
            decode_document(
                path(),
                DocumentKind::Library,
                "{\"format\":\"electronicwerk.library\"}"
            ),
            Err(FormatError::Migration { from: 0, .. })
        ));
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct V3 {
        title: String,
        count: u32,
    }

    fn rename_name(value: &mut Value) -> Result<(), String> {
        let object = value.as_object_mut().ok_or("not an object")?;
        let name = object.remove("name").ok_or("missing name")?;
        object.insert("title".into(), name);
        Ok(())
    }

    fn add_count(value: &mut Value) -> Result<(), String> {
        value
            .as_object_mut()
            .ok_or("not an object")?
            .insert("count".into(), Value::from(7));
        Ok(())
    }

    #[test]
    fn migrations_run_step_by_step() {
        let chain = [
            Migration {
                from: 1,
                description: "rename name to title",
                apply: rename_name,
            },
            Migration {
                from: 2,
                description: "add count",
                apply: add_count,
            },
        ];
        let old = r#"{"format":"electronicwerk.project","schemaVersion":1,"name":"Old"}"#;
        let decoded: Decoded<V3> = decode_with(path(), old, FileKind::Manifest, 3, &chain).unwrap();
        assert_eq!(
            decoded.value,
            V3 {
                title: "Old".into(),
                count: 7
            }
        );
        assert_eq!(decoded.migrated_from, Some(1));
        assert_eq!(decoded.notes, vec!["rename name to title", "add count"]);

        let broken = r#"{"format":"electronicwerk.project","schemaVersion":1,"title":"x"}"#;
        let error = decode_with::<V3>(path(), broken, FileKind::Manifest, 3, &chain).unwrap_err();
        assert!(error.to_string().contains("missing name"), "{error}");

        let gap = decode_with::<V3>(path(), old, FileKind::Manifest, 3, &chain[1..]).unwrap_err();
        assert!(matches!(gap, FormatError::Migration { from: 1, .. }));
    }
}
