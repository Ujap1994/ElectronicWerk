//! Render scenes for the 2D viewer (ADR-0003).
//!
//! A scene is everything the viewer draws for one schematic sheet or board:
//! layers with instanced primitives (line segments, arcs, discs and rings,
//! rounded rectangles, triangle meshes) and texts, plus a table of the
//! selectable objects. The primitives live in one little-endian binary
//! buffer that the desktop app passes to the GPU without parsing; the
//! metadata ([`SceneMeta`]) is JSON.
//!
//! Coordinates in the buffer are f32 nanometres relative to
//! [`SceneMeta::origin`], so they keep full precision near the design.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "render coordinates are f32 by design; model coordinates are limited to ±10 m"
)]

mod builder;
mod draw;
mod geom;
mod inspect;
mod pcb;
mod schematic;
mod text;

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::project::Project;
use serde::Deserialize;

pub use builder::{
    CHUNK, FontMetrics, LayerInfo, MESH_HATCH, MeshSection, NONE, ObjType, ObjectSections, Scene,
    SceneMeta, SceneStats, Section,
};
pub use inspect::{ChildRef, ObjectInfo, ObjectRef, Row, describe};
pub use text::{ADVANCE, LINE_PITCH};

/// Labels of the title block, in the language of the client.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TitleLabels {
    /// "File".
    pub file: String,
    /// "Size".
    pub size: String,
    /// "Date".
    pub date: String,
    /// "Rev".
    pub revision: String,
}

impl Default for TitleLabels {
    fn default() -> Self {
        Self {
            file: "File".to_owned(),
            size: "Size".to_owned(),
            date: "Date".to_owned(),
            revision: "Rev".to_owned(),
        }
    }
}

/// Inputs besides the project.
pub struct Options<'a> {
    /// Expands text variables (`${REVISION}`, …) for the document.
    pub resolve: &'a dyn Fn(&str) -> String,
    /// Title block labels.
    pub labels: TitleLabels,
}

impl std::fmt::Debug for Options<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Options")
            .field("labels", &self.labels)
            .finish_non_exhaustive()
    }
}

/// Why a scene could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SceneError {
    /// The document does not exist.
    #[error("no document with id {0}")]
    NotFound(Id),
    /// Only sheets and boards can be viewed.
    #[error("documents of this kind cannot be viewed")]
    NotViewable,
}

/// Builds the scene of a sheet or board.
///
/// # Errors
///
/// Fails if the document does not exist or is not a sheet or board.
pub fn build(project: &Project, doc: Id, options: &Options<'_>) -> Result<Scene, SceneError> {
    let settings = &project.manifest.settings;
    match project.document(doc).ok_or(SceneError::NotFound(doc))? {
        Document::Pcb(pcb) => Ok(pcb::build(pcb, settings.board_grid, options)),
        Document::Schematic(sheet) => Ok(schematic::build(
            project,
            sheet,
            settings.schematic_grid,
            options,
        )),
        _ => Err(SceneError::NotViewable),
    }
}

/// Encodes bytes as standard base64 (with padding), for JSON clients.
#[must_use]
pub fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let symbols = [(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63, n & 63];
        for (i, symbol) in symbols.iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[*symbol as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn options() -> Options<'static> {
        Options {
            resolve: &|text: &str| text.replace("${REVISION}", "B"),
            labels: TitleLabels::default(),
        }
    }

    pub(crate) fn synthetic(size: ew_synth::DesignSize) -> Project {
        ew_synth::generate(size, 7)
    }

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn rejects_unknown_and_unviewable_documents() {
        let project = synthetic(ew_synth::DesignSize::S);
        let missing = Id::from_u128(1);
        assert_eq!(
            build(&project, missing, &options()).unwrap_err(),
            SceneError::NotFound(missing)
        );
        let library = project
            .documents
            .iter()
            .find_map(|(id, doc)| matches!(doc, Document::Library(_)).then_some(*id))
            .unwrap();
        assert_eq!(
            build(&project, library, &options()).unwrap_err(),
            SceneError::NotViewable
        );
    }

    #[test]
    fn describes_tracks_pads_and_symbols() {
        let project = synthetic(ew_synth::DesignSize::S);
        let (pcb_id, pcb) = project
            .documents
            .iter()
            .find_map(|(id, doc)| match doc {
                Document::Pcb(pcb) => Some((*id, pcb)),
                _ => None,
            })
            .unwrap();
        let track = pcb.tracks.iter().next().unwrap();
        let info = describe(
            &project,
            &ObjectRef {
                doc: pcb_id,
                id: track.id,
                child: None,
            },
        )
        .unwrap();
        assert_eq!(info.ty, "track");
        assert!(
            info.rows
                .iter()
                .any(|row| row.key == "width" && row.kind == "length")
        );
        let fp = pcb.footprints.iter().next().unwrap();
        let pad = describe(
            &project,
            &ObjectRef {
                doc: pcb_id,
                id: fp.id,
                child: Some(ChildRef {
                    kind: "pad".into(),
                    index: 0,
                }),
            },
        )
        .unwrap();
        assert_eq!(pad.ty, "pad");
        assert_eq!(pad.title, format!("{}-1", fp.designator));
        let (sheet_id, symbol) = project
            .documents
            .iter()
            .find_map(|(id, doc)| match doc {
                Document::Schematic(sheet) => sheet.symbols.iter().next().map(|s| (*id, s)),
                _ => None,
            })
            .unwrap();
        let info = describe(
            &project,
            &ObjectRef {
                doc: sheet_id,
                id: symbol.id,
                child: None,
            },
        )
        .unwrap();
        assert_eq!(info.title, symbol.designator);
        let missing = ObjectRef {
            doc: sheet_id,
            id: Id::from_u128(5),
            child: None,
        };
        assert!(describe(&project, &missing).is_none());
    }
}
