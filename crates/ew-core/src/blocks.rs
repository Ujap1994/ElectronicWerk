//! Design blocks: a part of a schematic with its board layout, kept as a
//! file (`.ewblock`) for reuse and placed again in this or another project.
//! The blocks live in a folder beside the base library
//! (`ElectronicWerk/blocks`, or `EW_BLOCKS_DIR`).
//!
//! A block holds the schematic objects (parts, wires, labels, ports, texts),
//! the library symbols and footprints they use and, when the parts are on a
//! board, their footprints with the tracks and vias that belong to them.
//!
//! Placing a block pastes the schematic (parts whose designators are taken
//! are numbered on), brings missing library items into the project library,
//! adds the footprints the way the board update does (so the nets are the
//! schematic's) at the block's positions, copies the tracks and vias onto
//! the new nets and makes the board part one group. All in one undo step.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::library::{FootprintDef, SymbolDef};
use ew_model::object::Object;
use ew_model::pcb::{FootprintInstance, Group, Pcb, Track, Via};
use ew_model::project::Project;
use ew_model::schematic::{PlacedSymbol, Schematic};
use ew_model::units::{Nm, Point};
use ew_pcb::sync::Change as SyncChange;
use ew_schematic::connectivity::symbol_def;
use ew_schematic::edit::Change as EditChange;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::change::Request;
use crate::design::{Outcome, Stage, first_library, on_sheet_grid};
use crate::error::{EngineError, ErrorCode};

/// File extension of a design block.
pub const EXTENSION: &str = "ewblock";
/// The `format` of a block file.
const FORMAT: &str = "electronicwerk-block";
/// The newest block file version this build reads and writes.
const VERSION: u32 = 1;
/// Undo label of placing a block.
pub const PLACE_BLOCK: &str = "place-block";
/// The schematic grid of the free-place search.
const SHEET_GRID: Nm = 2_540_000;
/// How far around its pads the copper of a block's parts may reach.
const REACH: f64 = 1_000_000.0;

/// A design block file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    /// Always `electronicwerk-block`.
    pub format: String,
    /// File version.
    pub version: u32,
    /// Name.
    pub name: String,
    /// What the block is for.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The top-left corner of the schematic part, on the grid of its sheet:
    /// placing the block puts this point where the user asks.
    pub origin: Point,
    /// Parts, wires, labels, ports, texts and drawings.
    pub schematic: Vec<Object>,
    /// The library items the parts use.
    #[serde(default)]
    pub library: BlockLibrary,
    /// The layout, if the parts were on a board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<BlockBoard>,
}

/// The library items of a block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockLibrary {
    /// Symbols.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<SymbolDef>,
    /// Footprints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub footprints: Vec<FootprintDef>,
}

/// The layout of a block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockBoard {
    /// The top-left corner of the footprint positions.
    pub origin: Point,
    /// The footprints of the parts.
    pub footprints: Vec<FootprintInstance>,
    /// Tracks of their nets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<Track>,
    /// Vias of their nets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vias: Vec<Via>,
}

/// The design block folder of this system: `EW_BLOCKS_DIR`, or `blocks`
/// beside the base library folder.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("EW_BLOCKS_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    crate::library::default_dir().and_then(|dir| dir.parent().map(|p| p.join("blocks")))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FolderParams {
    #[serde(default)]
    folder: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NameParams {
    name: String,
    #[serde(default)]
    folder: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveParams {
    /// The schematic sheet.
    doc: Id,
    /// The objects of the block.
    ids: Vec<Id>,
    name: String,
    #[serde(default)]
    description: String,
    /// The board with the layout (default: the project's first board).
    #[serde(default)]
    board: Option<Id>,
    #[serde(default)]
    folder: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlaceParams {
    /// A block in the folder.
    #[serde(default)]
    name: Option<String>,
    /// Or a block file anywhere.
    #[serde(default)]
    file: Option<PathBuf>,
    /// The schematic sheet.
    doc: Id,
    /// Where the top-left corner goes (default: beside the sheet's content).
    #[serde(default)]
    at: Option<Point>,
    /// The board (default: the project's first board).
    #[serde(default)]
    board: Option<Id>,
    /// Where the top-left footprint corner goes (default: below the board).
    #[serde(default)]
    board_at: Option<Point>,
    #[serde(default)]
    folder: Option<PathBuf>,
}

fn parse<T: DeserializeOwned>(params: &Value) -> Result<T, EngineError> {
    serde_json::from_value(params.clone()).map_err(|e| EngineError::invalid_params(e.to_string()))
}

fn folder(given: Option<PathBuf>) -> Result<PathBuf, EngineError> {
    given
        .or_else(default_dir)
        .ok_or_else(|| EngineError::not_found("no folder for design blocks (set EW_BLOCKS_DIR)"))
}

/// Executes a `block.*` method. `block.list` and `block.delete` need no
/// project.
///
/// # Errors
///
/// Fails with `invalidParams` for wrong parameters or files, `notFound` for
/// missing blocks, sheets or boards, `noProject` without a project.
pub fn call(
    project: Option<&Project>,
    method: &str,
    params: &Value,
) -> Result<Outcome, EngineError> {
    match method {
        "block.list" => {
            let params: FolderParams = parse(params)?;
            Ok(Outcome::Read(list(&folder(params.folder)?)))
        }
        "block.delete" => {
            let params: NameParams = parse(params)?;
            let path = folder(params.folder)?.join(file_name(&params.name)?);
            std::fs::remove_file(&path).map_err(|e| {
                EngineError::not_found(format!("no design block `{}` ({e})", params.name))
            })?;
            Ok(Outcome::Read(json!({"deleted": params.name})))
        }
        "block.save" => {
            let project = project.ok_or_else(EngineError::no_project)?;
            let params: SaveParams = parse(params)?;
            let block = make(project, &params)?;
            let dir = folder(params.folder.clone())?;
            std::fs::create_dir_all(&dir).map_err(EngineError::internal)?;
            let path = dir.join(file_name(&block.name)?);
            let mut text = serde_json::to_string_pretty(&block).map_err(EngineError::internal)?;
            text.push('\n');
            ew_format::fsio::write_atomic(&path, text.as_bytes())
                .map_err(|e| EngineError::internal(format!("{}: {e}", path.display())))?;
            Ok(Outcome::Read(summary(&block, &path)))
        }
        "block.place" => {
            let project = project.ok_or_else(EngineError::no_project)?;
            place(project, &parse(params)?)
        }
        other => Err(EngineError::new(
            ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

/// The file name of a block: its name with only safe characters.
fn file_name(name: &str) -> Result<String, EngineError> {
    let clean: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')' | '+') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let clean = clean.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if clean.is_empty() {
        return Err(EngineError::invalid_params("a design block needs a name"));
    }
    Ok(format!("{clean}.{EXTENSION}"))
}

/// Reads a block file.
///
/// # Errors
///
/// Fails if the file is missing or not a block this build can read.
pub fn read(path: &Path) -> Result<Block, EngineError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| EngineError::not_found(format!("{}: {e}", path.display())))?;
    let block: Block = serde_json::from_str(&text).map_err(|e| {
        EngineError::invalid_params(format!("{}: not a design block ({e})", path.display()))
    })?;
    if block.format != FORMAT {
        return Err(EngineError::invalid_params(format!(
            "{}: not a design block",
            path.display()
        )));
    }
    if block.version > VERSION {
        return Err(EngineError::invalid_params(format!(
            "{}: made by a newer ElectronicWerk (block version {})",
            path.display(),
            block.version
        )));
    }
    Ok(block)
}

/// The blocks of a folder, by name.
fn list(dir: &Path) -> Value {
    let mut out: Vec<(String, Value)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some(EXTENSION) {
                continue;
            }
            if let Ok(block) = read(&path) {
                out.push((block.name.to_lowercase(), summary(&block, &path)));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Value::Array(out.into_iter().map(|(_, v)| v).collect())
}

/// The designators of the parts of a block (no power symbols).
fn designators(objects: &[Object]) -> BTreeSet<String> {
    objects
        .iter()
        .filter_map(|o| match o {
            Object::Symbol(s) if !s.designator.starts_with('#') => Some(s.designator.clone()),
            _ => None,
        })
        .collect()
}

fn summary(block: &Block, path: &Path) -> Value {
    let parts = designators(&block.schematic);
    let board = block.board.as_ref();
    json!({
        "name": block.name,
        "description": block.description,
        "file": path,
        "parts": parts.len(),
        "designators": parts,
        "layout": board.is_some(),
        "footprints": board.map_or(0, |b| b.footprints.len()),
        "tracks": board.map_or(0, |b| b.tracks.len()),
        "vias": board.map_or(0, |b| b.vias.len()),
    })
}

/// The points that place a schematic object.
fn anchors(object: &Object) -> Vec<Point> {
    match object {
        Object::Symbol(o) => vec![o.at],
        Object::Wire(o) => vec![o.start, o.end],
        Object::Bus(o) => vec![o.start, o.end],
        Object::BusEntry(o) => vec![o.at, o.at + o.size],
        Object::Junction(o) => vec![o.at],
        Object::Label(o) => vec![o.at],
        Object::PowerPort(o) => vec![o.at],
        Object::NoConnect(o) => vec![o.at],
        Object::Text(o) => vec![o.at],
        _ => Vec::new(),
    }
}

/// The top-left corner and the size of points (schematic y up).
fn corner(points: &[Point]) -> Option<(Point, Point)> {
    let min_x = points.iter().map(|p| p.x).min()?;
    let max_x = points.iter().map(|p| p.x).max()?;
    let min_y = points.iter().map(|p| p.y).min()?;
    let max_y = points.iter().map(|p| p.y).max()?;
    Some((
        Point::new(min_x, max_y),
        Point::new(max_x - min_x, max_y - min_y),
    ))
}

/// Makes a block from objects of a sheet and their layout.
fn make(project: &Project, params: &SaveParams) -> Result<Block, EngineError> {
    let name = params.name.trim();
    file_name(name)?;
    let document = project
        .document(params.doc)
        .filter(|d| matches!(d, Document::Schematic(_)))
        .ok_or_else(|| EngineError::not_found(format!("no schematic {}", params.doc)))?;
    let objects: Vec<Object> = params
        .ids
        .iter()
        .filter_map(|id| document.object(*id))
        .filter(|o| {
            matches!(
                o,
                Object::Symbol(_)
                    | Object::Wire(_)
                    | Object::Bus(_)
                    | Object::BusEntry(_)
                    | Object::Junction(_)
                    | Object::Label(_)
                    | Object::PowerPort(_)
                    | Object::NoConnect(_)
                    | Object::Text(_)
                    | Object::Shape(_)
            )
        })
        .collect();
    let symbols: Vec<&PlacedSymbol> = objects
        .iter()
        .filter_map(|o| match o {
            Object::Symbol(s) => Some(s),
            _ => None,
        })
        .collect();
    if symbols.is_empty() {
        return Err(EngineError::invalid_params(
            "select at least one part for a design block",
        ));
    }
    let points: Vec<Point> = objects.iter().flat_map(anchors).collect();
    let (top_left, _) = corner(&points).unwrap_or_default();
    let origin = on_sheet_grid(project, params.doc, top_left);
    let mut library = BlockLibrary::default();
    let mut seen = BTreeSet::new();
    for symbol in &symbols {
        if seen.insert(("symbol", symbol.symbol.clone()))
            && let Some(def) = symbol_def(project, &symbol.symbol)
        {
            library.symbols.push(def.clone());
        }
        if let Some(footprint) = &symbol.footprint
            && seen.insert(("footprint", footprint.clone()))
            && let Some(def) = ew_pcb::sync::footprint_def(project, footprint)
        {
            library.footprints.push(def.clone());
        }
    }
    let board_doc = params
        .board
        .or_else(|| project.documents_of(DocumentKind::Pcb).next().map(|e| e.id));
    let board = board_doc.and_then(|doc| match project.document(doc) {
        Some(Document::Pcb(pcb)) => layout(pcb, &designators(&objects)),
        _ => None,
    });
    Ok(Block {
        format: FORMAT.to_owned(),
        version: VERSION,
        name: name.to_owned(),
        description: params.description.trim().to_owned(),
        origin,
        schematic: objects,
        library,
        board,
    })
}

/// The layout of some parts: their footprints, the tracks and vias of nets
/// only they use, and of their other nets what lies near their pads.
#[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
fn layout(pcb: &Pcb, parts: &BTreeSet<String>) -> Option<BlockBoard> {
    let footprints: Vec<FootprintInstance> = pcb
        .footprints
        .iter()
        .filter(|f| parts.contains(&f.designator))
        .cloned()
        .collect();
    let (origin, _) = corner(&footprints.iter().map(|f| f.at).collect::<Vec<_>>())?;
    let own: BTreeSet<Id> = footprints.iter().map(|f| f.id).collect();
    let nets: BTreeSet<Id> = footprints
        .iter()
        .flat_map(|f| f.pads.iter().filter_map(|p| p.net))
        .collect();
    let shared: BTreeSet<Id> = pcb
        .footprints
        .iter()
        .filter(|f| !own.contains(&f.id))
        .flat_map(|f| f.pads.iter().filter_map(|p| p.net))
        .collect();
    let mut area: Option<ew_pcb::geom::BBox> = None;
    for fp in &footprints {
        let xf = ew_pcb::items::footprint_xform(fp);
        for pad in &fp.pads {
            let bbox = ew_pcb::items::pad_shape(pad, &xf, 0.0).bbox();
            area = Some(area.map_or(bbox, |a| a.union(bbox)));
        }
    }
    let area = area.map(|a| a.grow(REACH));
    let near = |p: Point| {
        area.is_some_and(|a| {
            let (x, y) = (p.x as f64, p.y as f64);
            x >= a.min.x && x <= a.max.x && y >= a.min.y && y <= a.max.y
        })
    };
    let belongs = |net: Option<Id>, points: &[Point]| {
        net.is_some_and(|n| {
            nets.contains(&n) && (!shared.contains(&n) || points.iter().all(|p| near(*p)))
        })
    };
    let tracks = pcb
        .tracks
        .iter()
        .filter(|t| belongs(t.net, &[t.start, t.end]))
        .cloned()
        .collect();
    let vias = pcb
        .vias
        .iter()
        .filter(|v| belongs(v.net, &[v.at]))
        .cloned()
        .collect();
    Some(BlockBoard {
        origin,
        footprints,
        tracks,
        vias,
    })
}

/// A free place for a block on a sheet: right of the content, or below it
/// when the block does not fit beside it.
fn free_place(sheet: &Schematic, block: &Block) -> Point {
    let mut taken: Vec<Point> = sheet.symbols.iter().map(|s| s.at).collect();
    taken.extend(sheet.wires.iter().flat_map(|w| [w.start, w.end]));
    taken.extend(sheet.labels.iter().map(|l| l.at));
    taken.extend(sheet.power_ports.iter().map(|p| p.at));
    taken.extend(sheet.texts.iter().map(|t| t.at));
    taken.extend(sheet.sheets.iter().map(|s| s.at));
    let page = sheet.page.dimensions();
    let points: Vec<Point> = block.schematic.iter().flat_map(anchors).collect();
    let (_, size) = corner(&points).unwrap_or_default();
    let margin = 12 * SHEET_GRID;
    let Some((content, extent)) = corner(&taken) else {
        return Point::new(margin, page.h - margin);
    };
    let right = Point::new(content.x + extent.x + 10 * SHEET_GRID, content.y);
    if right.x + size.x <= page.w - margin {
        return right;
    }
    Point::new(content.x, content.y - extent.y - 10 * SHEET_GRID)
}

/// Places a block: library, schematic and layout as one undo step.
fn place(project: &Project, params: &PlaceParams) -> Result<Outcome, EngineError> {
    let path = match (&params.file, &params.name) {
        (Some(file), _) => file.clone(),
        (None, Some(name)) => folder(params.folder.clone())?.join(file_name(name)?),
        (None, None) => {
            return Err(EngineError::invalid_params(
                "give the name or the file of a design block",
            ));
        }
    };
    let block = read(&path)?;
    let Some(Document::Schematic(sheet)) = project.document(params.doc) else {
        return Err(EngineError::not_found(format!(
            "no schematic {}",
            params.doc
        )));
    };
    let mut stage = Stage::new(project);
    let (added, existing) = bring_library(&mut stage, &block.library)?;
    let at = params.at.unwrap_or_else(|| free_place(sheet, &block));
    let at = on_sheet_grid(project, params.doc, at);
    let (edit, renamed) = ew_schematic::edit::paste_renamed(
        &stage.project,
        params.doc,
        &block.schematic,
        at - block.origin,
        true,
    )
    .map_err(|e| EngineError::invalid_params(e.to_string()))?;
    let select = edit.select.clone();
    stage.push_edit(&edit)?;
    let mut board = Value::Null;
    if let Some(layout) = &block.board {
        let pcb_doc = params
            .board
            .or_else(|| project.documents_of(DocumentKind::Pcb).next().map(|e| e.id));
        if let Some(pcb_doc) = pcb_doc {
            board = place_layout(
                &mut stage,
                pcb_doc,
                layout,
                &block.name,
                &renamed,
                params.board_at,
            )?;
        }
    }
    let parts: BTreeMap<&String, &String> = renamed
        .iter()
        .filter(|(old, _)| !old.starts_with('#'))
        .collect();
    let reply = json!({
        "name": block.name,
        "select": select,
        "parts": parts.len(),
        "designators": parts,
        "library": {"added": added, "existing": existing},
        "board": board,
    });
    Ok(stage.commit(PLACE_BLOCK, reply))
}

/// Brings the block's library items the project lacks into its first
/// library. Returns how many were added and how many the project had
/// already (by name; the project's own are kept).
fn bring_library(stage: &mut Stage, library: &BlockLibrary) -> Result<(usize, usize), EngineError> {
    if library.symbols.is_empty() && library.footprints.is_empty() {
        return Ok((0, 0));
    }
    let doc = first_library(&stage.project)?;
    let mut requests = Vec::new();
    let mut existing = 0;
    for def in &library.symbols {
        if symbol_def(&stage.project, &def.name).is_some() {
            existing += 1;
        } else {
            let mut def = def.clone();
            def.id = Id::random();
            requests.push(Request::Insert {
                doc,
                object: Object::SymbolDef(def),
            });
        }
    }
    for def in &library.footprints {
        if ew_pcb::sync::footprint_def(&stage.project, &def.name).is_some() {
            existing += 1;
        } else {
            let mut def = def.clone();
            def.id = Id::random();
            requests.push(Request::Insert {
                doc,
                object: Object::FootprintDef(def),
            });
        }
    }
    let added = requests.len();
    stage.push(requests)?;
    Ok((added, existing))
}

/// Puts the block's layout onto a board: the footprints the board update
/// adds for the new parts, at the block's positions, then the tracks and
/// vias on the new nets, all in one group.
fn place_layout(
    stage: &mut Stage,
    pcb_doc: Id,
    layout: &BlockBoard,
    name: &str,
    renamed: &BTreeMap<String, String>,
    at: Option<Point>,
) -> Result<Value, EngineError> {
    let pcb = match stage.project.document(pcb_doc) {
        Some(Document::Pcb(pcb)) => pcb.clone(),
        _ => return Err(EngineError::not_found(format!("no board {pcb_doc}"))),
    };
    let connectivity = ew_schematic::analyze(&stage.project);
    let diff = ew_pcb::sync::diff(&stage.project, &connectivity, &pcb);
    let by_new: HashMap<&str, &FootprintInstance> = layout
        .footprints
        .iter()
        .filter_map(|f| renamed.get(&f.designator).map(|d| (d.as_str(), f)))
        .collect();
    let selected: Vec<usize> = diff
        .changes
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            matches!(c, SyncChange::AddFootprint { designator, .. } if by_new.contains_key(designator.as_str()))
        })
        .map(|(i, _)| i)
        .collect();
    if selected.is_empty() {
        return Ok(
            json!({"doc": pcb_doc, "select": [], "footprints": 0, "tracks": 0, "vias": 0, "skipped": 0}),
        );
    }
    let mut edit = ew_pcb::sync::apply(
        &stage.project,
        &connectivity,
        pcb_doc,
        &pcb,
        &diff,
        &selected,
    );
    // Below the board, at its left edge (board y up).
    let at = at.unwrap_or_else(|| {
        pcb.outline_bounds()
            .map_or(Point::ORIGIN, |r| Point::new(r.min.x, r.min.y - 10_000_000))
    });
    let shift = |p: Point| p - layout.origin + at;
    let mut nets: HashMap<Id, Id> = HashMap::new();
    let mut members: Vec<Id> = Vec::new();
    for change in &mut edit.changes {
        let EditChange::Insert {
            object: Object::Footprint(fp),
            ..
        } = change
        else {
            continue;
        };
        let Some(old) = by_new.get(fp.designator.as_str()) else {
            continue;
        };
        fp.at = shift(old.at);
        fp.rotation = old.rotation;
        fp.side = old.side;
        if old.footprint == fp.footprint && old.texts.len() == fp.texts.len() {
            fp.texts.clone_from(&old.texts);
        }
        for pad in &old.pads {
            let new = fp
                .pads
                .iter()
                .find(|p| p.number == pad.number)
                .and_then(|p| p.net);
            if let (Some(from), Some(to)) = (pad.net, new) {
                nets.entry(from).or_insert(to);
            }
        }
        members.push(fp.id);
    }
    let footprints = members.len();
    stage.push_edit(&edit)?;
    let routing = copy_routing(pcb_doc, layout, &nets, &pcb, &shift);
    members.extend(routing.ids.iter().copied());
    let mut requests = routing.requests;
    let mut group = None;
    if members.len() > 1 {
        let id = Id::random();
        group = Some(id);
        requests.push(Request::Insert {
            doc: pcb_doc,
            object: Object::Group(Group {
                id,
                name: name.to_owned(),
                members: members.clone(),
            }),
        });
    }
    stage.push(requests)?;
    Ok(json!({
        "doc": pcb_doc,
        "select": members,
        "group": group,
        "footprints": footprints,
        "tracks": routing.tracks,
        "vias": routing.vias,
        "skipped": routing.skipped,
    }))
}

/// The tracks and vias of a layout, copied onto new nets.
struct Routing {
    requests: Vec<Request>,
    ids: Vec<Id>,
    tracks: usize,
    vias: usize,
    /// Left out: no net on the board, or a layer the board lacks.
    skipped: usize,
}

fn copy_routing(
    pcb_doc: Id,
    layout: &BlockBoard,
    nets: &HashMap<Id, Id>,
    pcb: &Pcb,
    shift: &dyn Fn(Point) -> Point,
) -> Routing {
    let layers: BTreeSet<_> = pcb.stackup.copper_layers().collect();
    let mut out = Routing {
        requests: Vec::new(),
        ids: Vec::new(),
        tracks: 0,
        vias: 0,
        skipped: 0,
    };
    for track in &layout.tracks {
        let net = track.net.and_then(|n| nets.get(&n));
        let Some(net) = net.filter(|_| layers.contains(&track.layer)) else {
            out.skipped += 1;
            continue;
        };
        let id = Id::random();
        out.ids.push(id);
        out.tracks += 1;
        out.requests.push(Request::Insert {
            doc: pcb_doc,
            object: Object::Track(Track {
                id,
                net: Some(*net),
                start: shift(track.start),
                end: shift(track.end),
                mid: track.mid.map(shift),
                locked: false,
                ..track.clone()
            }),
        });
    }
    for via in &layout.vias {
        let net = via.net.and_then(|n| nets.get(&n));
        let Some(net) = net.filter(|_| layers.contains(&via.from) && layers.contains(&via.to))
        else {
            out.skipped += 1;
            continue;
        };
        let id = Id::random();
        out.ids.push(id);
        out.vias += 1;
        out.requests.push(Request::Insert {
            doc: pcb_doc,
            object: Object::Via(Via {
                id,
                net: Some(*net),
                at: shift(via.at),
                locked: false,
                ..via.clone()
            }),
        });
    }
    out
}
