//! Project snapshots for clients (project tree, status bar, CLI `info`).

use std::collections::BTreeMap;

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::project::ProjectSettings;
use ew_model::units::{Nm, Size};
use serde::Serialize;

use crate::history::HistoryState;
use crate::session::Session;

/// Key figures of one document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentStats {
    /// Objects in the document.
    pub objects: usize,
    /// Schematic: placed symbols. Library: symbol definitions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<usize>,
    /// Schematic: wires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wires: Option<usize>,
    /// Schematic: net labels and power ports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<usize>,
    /// Schematic: child sheets placed on this sheet (document ids).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Id>>,
    /// Schematic: paper size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<Size>,
    /// Board and library: footprints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footprints: Option<usize>,
    /// Board: track segments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracks: Option<usize>,
    /// Board: vias.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vias: Option<usize>,
    /// Board: pours and keep-outs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zones: Option<usize>,
    /// Board: nets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nets: Option<usize>,
    /// Board: copper layers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copper_layers: Option<u8>,
    /// Board: thickness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness: Option<Nm>,
    /// Board: outline size, if an outline exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_size: Option<Size>,
    /// Rules: net classes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net_classes: Option<usize>,
    /// Output job: outputs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<usize>,
}

/// One document in the snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    /// Document id.
    pub id: Id,
    /// Kind.
    pub kind: DocumentKind,
    /// Name.
    pub name: String,
    /// Project-relative path.
    pub path: String,
    /// Changed since the last save.
    pub dirty: bool,
    /// Key figures.
    pub stats: DocumentStats,
}

/// Unsaved work found in the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryInfo {
    /// Committed transactions.
    pub commits: usize,
    /// All journal entries (including undo and redo).
    pub entries: usize,
    /// When the journal was last written (UTC).
    pub modified: Option<String>,
    /// `false` if the saved files changed since the journal was started.
    pub matches_files: bool,
}

/// The state of the open project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    /// Project id.
    pub id: Id,
    /// Name.
    pub name: String,
    /// Revision.
    pub revision: String,
    /// Description.
    pub description: String,
    /// Text variables.
    pub variables: BTreeMap<String, String>,
    /// Project folder.
    pub path: String,
    /// Manifest file.
    pub manifest_path: String,
    /// Top sheet.
    pub root_schematic: Option<Id>,
    /// Editor settings.
    pub settings: ProjectSettings,
    /// Documents in tree order.
    pub documents: Vec<DocumentSummary>,
    /// Undo/redo state.
    pub history: HistoryState,
    /// Files will be upgraded to the current format on the next save.
    pub needs_upgrade: bool,
    /// Unsaved work from a crashed session.
    pub recovery: Option<RecoveryInfo>,
    /// Problem writing the autosave journal, if any.
    pub journal_error: Option<String>,
}

/// Key figures of a document.
#[must_use]
pub fn stats(document: &Document) -> DocumentStats {
    let objects = document.object_count();
    match document {
        Document::Schematic(sheet) => DocumentStats {
            objects,
            symbols: Some(sheet.symbols.len()),
            wires: Some(sheet.wires.len()),
            labels: Some(sheet.labels.len() + sheet.power_ports.len()),
            children: Some(sheet.child_schematics().collect()),
            page: Some(sheet.page.dimensions()),
            ..DocumentStats::default()
        },
        Document::Pcb(pcb) => DocumentStats {
            objects,
            footprints: Some(pcb.footprints.len()),
            tracks: Some(pcb.tracks.len()),
            vias: Some(pcb.vias.len()),
            zones: Some(pcb.zones.len()),
            nets: Some(pcb.nets.len()),
            copper_layers: Some(pcb.stackup.copper_count()),
            thickness: Some(pcb.stackup.thickness()),
            board_size: pcb
                .outline_bounds()
                .map(|bounds| Size::new(bounds.width(), bounds.height())),
            ..DocumentStats::default()
        },
        Document::Library(library) => DocumentStats {
            objects,
            symbols: Some(library.symbols.len()),
            footprints: Some(library.footprints.len()),
            ..DocumentStats::default()
        },
        Document::Rules(rules) => DocumentStats {
            objects,
            net_classes: Some(rules.net_classes.len()),
            ..DocumentStats::default()
        },
        Document::OutputJob(job) => DocumentStats {
            objects,
            outputs: Some(job.outputs.len()),
            ..DocumentStats::default()
        },
    }
}

/// A snapshot of the open project.
#[must_use]
pub fn snapshot(session: &Session) -> ProjectSnapshot {
    let project = session.project();
    let manifest = &project.manifest;
    let documents = manifest
        .documents
        .iter()
        .filter_map(|entry| {
            let document = project.document(entry.id)?;
            Some(DocumentSummary {
                id: entry.id,
                kind: entry.kind,
                name: document.name().to_owned(),
                path: entry.path.clone(),
                dirty: session.is_document_dirty(entry.id),
                stats: stats(document),
            })
        })
        .collect();
    ProjectSnapshot {
        id: manifest.id,
        name: manifest.name.clone(),
        revision: manifest.revision.clone(),
        description: manifest.description.clone(),
        variables: manifest.variables.clone(),
        path: session.root().to_string_lossy().into_owned(),
        manifest_path: session.manifest_path().to_string_lossy().into_owned(),
        root_schematic: manifest.root_schematic,
        settings: manifest.settings,
        documents,
        history: session.history_state(),
        needs_upgrade: !session.migrated_files().is_empty(),
        recovery: session.recovery().map(|recovery| RecoveryInfo {
            commits: recovery.commits(),
            entries: recovery.entries.len(),
            modified: recovery.modified.clone(),
            matches_files: recovery.matches_files,
        }),
        journal_error: session.journal_error().map(str::to_owned),
    }
}
