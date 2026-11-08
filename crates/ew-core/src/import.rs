//! Engine side of the imports: KiCad projects, libraries and netlists.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ew_kicad::netlist::{self, Comparison, Format, Netlist};
use ew_kicad::{ImportReport, KicadError};
use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::pcb::{Net, Pcb};
use ew_model::project::Project;
use serde::Serialize;

use crate::change::Request;
use crate::error::{EngineError, ErrorCode};

/// The file the import report is saved to, in the new project's folder.
pub const REPORT_FILE: &str = "kicad-import-report.md";

/// Maps an import error to an engine error.
#[must_use]
pub fn engine_error(error: &KicadError) -> EngineError {
    match error {
        KicadError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            EngineError::not_found(error.to_string())
        }
        KicadError::Io { .. } => EngineError::new(ErrorCode::IoError, error.to_string()),
        KicadError::Parse { .. } => EngineError::new(ErrorCode::FormatError, error.to_string()),
        KicadError::Unsupported { .. } => EngineError::invalid_params(error.to_string()),
    }
}

/// A quick look at a KiCad project before importing it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    /// Project name (the file name).
    pub name: String,
    /// The folder with the KiCad files.
    pub folder: String,
    /// A `.kicad_pro` exists.
    pub project: bool,
    /// A root schematic exists.
    pub schematic: bool,
    /// A board exists.
    pub board: bool,
}

/// Looks at a KiCad project without importing it.
///
/// # Errors
///
/// Fails if there is no KiCad project at `source`.
pub fn probe(source: &Path) -> Result<Probe, EngineError> {
    let (folder, stem) = ew_kicad::locate(source).map_err(|e| engine_error(&e))?;
    Ok(Probe {
        project: folder.join(format!("{stem}.kicad_pro")).exists(),
        schematic: folder.join(format!("{stem}.kicad_sch")).exists(),
        board: folder.join(format!("{stem}.kicad_pcb")).exists(),
        folder: folder.display().to_string(),
        name: stem,
    })
}

/// The board of a project (`doc`, or the first one).
///
/// # Errors
///
/// Fails if there is no such board.
pub fn board(project: &Project, doc: Option<Id>) -> Result<&Pcb, EngineError> {
    let found = match doc {
        Some(id) => project.document(id).and_then(|d| match d {
            Document::Pcb(pcb) => Some(pcb),
            _ => None,
        }),
        None => project.documents.values().find_map(|d| match d {
            Document::Pcb(pcb) => Some(pcb),
            _ => None,
        }),
    };
    found.ok_or_else(|| EngineError::not_found("the project has no such board"))
}

/// The result of reading a netlist against a board.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetlistResult {
    /// The detected format.
    pub format: Format,
    /// Nets in the file.
    pub nets: usize,
    /// Components in the file.
    pub components: usize,
    /// The comparison before applying.
    pub comparison: Comparison,
    /// Pads whose net changes when the netlist is applied.
    pub changed_pads: usize,
    /// Nets that would be created.
    pub new_nets: usize,
    /// Designators of the netlist without a footprint on the board.
    pub missing_footprints: Vec<String>,
    /// Footprints on the board that the netlist does not mention.
    pub extra_footprints: Vec<String>,
    /// `true` if the changes were applied.
    pub applied: bool,
}

/// Reads a netlist file.
///
/// # Errors
///
/// Fails if the file cannot be read or is not a netlist.
pub fn read_netlist(source: &Path) -> Result<(Netlist, Format), EngineError> {
    let text = std::fs::read_to_string(source).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            EngineError::not_found(format!("{}: {error}", source.display()))
        } else {
            EngineError::new(ErrorCode::IoError, format!("{}: {error}", source.display()))
        }
    })?;
    netlist::parse(&text).map_err(|error| EngineError::new(ErrorCode::FormatError, error.0))
}

/// Compares a netlist with a board and builds the edits that apply it: every
/// pad of a footprint the netlist mentions gets the net of its node (or no
/// net), and missing nets are created.
#[must_use]
pub fn netlist_changes(pcb: &Pcb, list: &Netlist, format: Format) -> (NetlistResult, Vec<Request>) {
    let comparison = netlist::compare(list, &netlist::of_board(pcb));
    let mut net_ids: BTreeMap<&str, Id> = pcb
        .nets
        .iter()
        .map(|net| (net.name.as_str(), net.id))
        .collect();
    let mut requests = Vec::new();
    let mut new_nets = 0;
    for name in list.nets.keys() {
        if !net_ids.contains_key(name.as_str()) {
            let id = Id::random();
            net_ids.insert(name, id);
            requests.push(Request::Insert {
                doc: pcb.id,
                object: Object::Net(Net {
                    id,
                    name: name.clone(),
                    class: String::new(),
                }),
            });
            new_nets += 1;
        }
    }
    let node_net: BTreeMap<(&str, &str), &str> = list
        .nets
        .iter()
        .flat_map(|(name, nodes)| {
            nodes
                .iter()
                .map(move |(r, p)| ((r.as_str(), p.as_str()), name.as_str()))
        })
        .collect();
    let mentioned: BTreeSet<&str> = list
        .components
        .keys()
        .map(String::as_str)
        .chain(list.nets.values().flatten().map(|(r, _)| r.as_str()))
        .collect();
    let on_board: BTreeSet<&str> = pcb
        .footprints
        .iter()
        .map(|fp| fp.designator.as_str())
        .collect();
    let mut changed_pads = 0;
    for fp in &pcb.footprints {
        if !mentioned.contains(fp.designator.as_str()) {
            continue;
        }
        let mut updated = fp.clone();
        let mut changed = false;
        for pad in &mut updated.pads {
            let wanted = node_net
                .get(&(fp.designator.as_str(), pad.number.as_str()))
                .and_then(|name| net_ids.get(name).copied());
            if pad.net != wanted {
                pad.net = wanted;
                changed = true;
                changed_pads += 1;
            }
        }
        if changed {
            requests.push(Request::Update {
                doc: pcb.id,
                object: Object::Footprint(updated),
            });
        }
    }
    let result = NetlistResult {
        format,
        nets: list.nets.len(),
        components: mentioned.len(),
        comparison,
        changed_pads,
        new_nets,
        missing_footprints: mentioned
            .difference(&on_board)
            .map(|s| (*s).to_owned())
            .collect(),
        extra_footprints: on_board
            .difference(&mentioned)
            .map(|s| (*s).to_owned())
            .collect(),
        applied: false,
    };
    (result, requests)
}

/// The edits that add a library's symbols and footprints to a library
/// document; names that exist already are skipped.
#[must_use]
pub fn library_changes(
    project: &Project,
    doc: Id,
    imported: ew_kicad::library::Library,
    report: &mut ImportReport,
) -> (usize, usize, Vec<Request>) {
    let Some(Document::Library(library)) = project.document(doc) else {
        return (0, 0, Vec::new());
    };
    let mut requests = Vec::new();
    let (mut symbols, mut footprints) = (0, 0);
    for symbol in imported.symbols {
        if library.symbol(&symbol.name).is_some() || library.symbols.contains(symbol.id) {
            report.info(
                "already-in-library",
                None,
                format!("`{}` is already in the library", symbol.name),
            );
            continue;
        }
        symbols += 1;
        requests.push(Request::Insert {
            doc,
            object: Object::SymbolDef(symbol),
        });
    }
    for footprint in imported.footprints {
        if library.footprint(&footprint.name).is_some() || library.footprints.contains(footprint.id)
        {
            report.info(
                "already-in-library",
                None,
                format!("`{}` is already in the library", footprint.name),
            );
            continue;
        }
        footprints += 1;
        requests.push(Request::Insert {
            doc,
            object: Object::FootprintDef(footprint),
        });
    }
    (symbols, footprints, requests)
}
