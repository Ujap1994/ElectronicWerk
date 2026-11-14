//! `design.*`: design methods for scripts, the command line and AI agents.
//!
//! The other engine methods work with object ids and nanometres, like the
//! editors that call them. These methods address what a designer names:
//! parts by designator (`R1`), pins as `R1.2` or `U1.VCC`, sheets and boards
//! by name, nets by name, lengths in millimetres and angles in degrees.
//! Every call that changes the design is one undo step.

use std::collections::{BTreeMap, BTreeSet};

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Side};
use ew_model::object::Object;
use ew_model::pcb::{FootprintInstance, Pcb, Track};
use ew_model::project::Project;
use ew_model::rules::Rules;
use ew_model::schematic::{LabelKind, PlacedSymbol, PowerStyle, Schematic};
use ew_model::units::{Angle, Nm, Point};
use ew_pcb::edit::BoardEdit;
use ew_pcb::geom::{BBox, V};
use ew_pcb::route::{Corners, Router};
use ew_schematic::Connectivity;
use ew_schematic::connectivity::symbol_def;
use ew_schematic::edit::{self as sch, AddItem, PlaceSymbol};
use ew_schematic::geometry::pin_direction;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::change::{Change, Request};
use crate::error::EngineError;

/// Labels of the undo steps of the design methods.
pub mod labels {
    /// `design.place`.
    pub const PLACE: &str = "place-symbol";
    /// `design.connect`.
    pub const CONNECT: &str = "connect-pins";
    /// `design.noConnect`.
    pub const NO_CONNECT: &str = "add-no-connect";
    /// `design.set`.
    pub const SET: &str = "edit-part";
    /// `design.footprint`.
    pub const FOOTPRINT: &str = "add-footprint";
    /// `design.updateBoard`.
    pub const UPDATE_BOARD: &str = "update-board";
    /// `design.placeFootprint` and `design.arrange`.
    pub const PLACE_FOOTPRINTS: &str = "place-footprints";
    /// `design.route`.
    pub const ROUTE: &str = "auto-route";
}

/// The methods of this module (for `engine.methods`).
pub const METHODS: &[&str] = &[
    "design.arrange",
    "design.board",
    "design.components",
    "design.connect",
    "design.footprint",
    "design.nets",
    "design.noConnect",
    "design.place",
    "design.placeFootprint",
    "design.presets",
    "design.route",
    "design.set",
    "design.updateBoard",
];

/// What a design method does.
pub enum Outcome {
    /// It only read the design.
    Read(Value),
    /// It changed the design: one undo step.
    Commit {
        /// Undo label.
        label: &'static str,
        /// The changes.
        requests: Vec<Request>,
        /// The reply (the engine adds `seq` and `history`).
        reply: Value,
    },
}

/// Looks up parts in the base library (provided by the engine).
pub trait BaseLibrary {
    /// A symbol of the base library by its full name (`Device:R`), with the
    /// footprints it names.
    fn symbol(&mut self, name: &str) -> Option<ew_kicad::library::Library>;
    /// A footprint of the base library by its full name.
    fn footprint(&mut self, name: &str) -> Option<ew_kicad::library::Library>;
}

const MM: f64 = 1_000_000.0;
/// Length of the wire stub drawn from a pin to its label.
const STUB: Nm = 2_540_000;
/// Placement grid on sheets.
const SHEET_GRID: Nm = 2_540_000;

#[allow(
    clippy::cast_possible_truncation,
    reason = "board and sheet sizes fit i64 nanometres"
)]
fn nm(mm: f64) -> Nm {
    (mm * MM).round() as Nm
}

#[allow(
    clippy::cast_precision_loss,
    reason = "lengths are reported in millimetres"
)]
fn mm(nm: Nm) -> f64 {
    (nm as f64 / MM * 10_000.0).round() / 10_000.0
}

#[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
const fn fl(value: Nm) -> f64 {
    value as f64
}

#[allow(clippy::cast_possible_truncation, reason = "board coordinates fit i64")]
fn rnd(value: f64) -> Nm {
    value.round() as Nm
}

fn mm_f(value: f64) -> f64 {
    (value / MM * 10_000.0).round() / 10_000.0
}

fn point(at: [f64; 2]) -> Point {
    Point::new(nm(at[0]), nm(at[1]))
}

fn angle(degrees: f64) -> Result<Angle, EngineError> {
    if !degrees.is_finite() {
        return Err(EngineError::invalid_params("the rotation must be a number"));
    }
    #[allow(clippy::cast_possible_truncation, reason = "rounded millidegrees")]
    Ok(Angle::from_millidegrees((degrees * 1000.0).round() as i64))
}

fn degrees(angle: Angle) -> f64 {
    f64::from(angle.millidegrees()) / 1000.0
}

fn parse<T: DeserializeOwned>(params: &Value) -> Result<T, EngineError> {
    let value = if params.is_null() {
        json!({})
    } else {
        params.clone()
    };
    serde_json::from_value(value).map_err(|error| EngineError::invalid_params(error.to_string()))
}

fn edit_failed(error: impl std::fmt::Display) -> EngineError {
    EngineError::invalid_params(error.to_string())
}

/// Changes applied to a copy of the project, so the next step sees the
/// earlier ones; committed together as one undo step.
struct Stage {
    project: Project,
    requests: Vec<Request>,
}

impl Stage {
    fn new(project: &Project) -> Self {
        Self {
            project: project.clone(),
            requests: Vec::new(),
        }
    }

    fn push(&mut self, requests: Vec<Request>) -> Result<(), EngineError> {
        for request in requests {
            let changes: Vec<Change> = request.clone().resolve(&self.project)?;
            for change in changes {
                change.apply(&mut self.project, true)?;
            }
            self.requests.push(request);
        }
        Ok(())
    }

    fn push_edit(&mut self, edit: &sch::Edit) -> Result<(), EngineError> {
        self.push(crate::schematic::requests(edit))
    }

    fn commit(self, label: &'static str, reply: Value) -> Outcome {
        if self.requests.is_empty() {
            return Outcome::Read(reply);
        }
        Outcome::Commit {
            label,
            requests: self.requests,
            reply,
        }
    }
}

/// Executes a `design.*` method.
///
/// # Errors
///
/// Fails with `invalidParams` for wrong parameters, `notFound` for unknown
/// parts, pins, sheets or boards.
pub fn call(
    project: &Project,
    library: &mut dyn BaseLibrary,
    method: &str,
    params: &Value,
) -> Result<Outcome, EngineError> {
    match method {
        "design.components" => Ok(Outcome::Read(components(project))),
        "design.nets" => Ok(Outcome::Read(nets(project))),
        "design.presets" => Ok(Outcome::Read(presets())),
        "design.place" => place(project, library, &parse(params)?),
        "design.set" => set(project, library, &parse(params)?),
        "design.connect" => connect(project, &parse(params)?),
        "design.noConnect" => no_connect(project, &parse(params)?),
        "design.footprint" => {
            let params: FootprintParams = parse(params)?;
            let mut stage = Stage::new(project);
            let name = ensure_footprint(&mut stage, library, &params.name)?;
            Ok(stage.commit(labels::FOOTPRINT, json!({ "footprint": name })))
        }
        "design.board" => board_summary(project, &parse(params)?),
        "design.updateBoard" => update_board(project, &parse(params)?),
        "design.placeFootprint" => place_footprint(project, &parse(params)?),
        "design.arrange" => arrange(project, &parse(params)?),
        "design.route" => route(project, &parse(params)?),
        other => Err(EngineError::new(
            crate::error::ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

// --- Reading -----------------------------------------------------------------

/// The footprint wizard presets usable as footprints (no project needed).
#[must_use]
pub fn presets() -> Value {
    json!({
        "footprints": ew_libedit::wizard::presets()
            .into_iter()
            .map(|preset| preset.name)
            .collect::<Vec<_>>(),
    })
}

/// Sheet names by document id.
fn sheet_names(project: &Project) -> BTreeMap<Id, String> {
    project
        .documents_of(DocumentKind::Schematic)
        .filter_map(|entry| match project.document(entry.id) {
            Some(Document::Schematic(sheet)) => Some((entry.id, sheet.name.clone())),
            _ => None,
        })
        .collect()
}

fn components(project: &Project) -> Value {
    let connectivity = ew_schematic::analyze(project);
    let names = sheet_names(project);
    let mut parts: BTreeMap<String, Value> = BTreeMap::new();
    for pin in &connectivity.pins {
        if pin.power_symbol {
            continue;
        }
        let sheet = connectivity
            .instances
            .get(pin.instance)
            .map(|instance| instance.sheet);
        let symbol = sheet.and_then(|sheet| placed(project, sheet, pin.symbol));
        let entry = parts.entry(pin.designator.clone()).or_insert_with(|| {
            json!({
                "designator": pin.designator,
                "value": pin.value,
                "symbol": symbol.map(|s| s.symbol.clone()),
                "footprint": symbol.and_then(|s| s.footprint.clone()),
                "sheet": sheet.and_then(|id| names.get(&id).cloned()),
                "pins": [],
            })
        });
        let net = connectivity.nets.get(pin.net).map(|net| net.name.clone());
        if let Some(pins) = entry["pins"].as_array_mut() {
            pins.push(json!({ "number": pin.number, "name": pin.name, "net": net }));
        }
    }
    // Parts without pins (mechanical) still count.
    for (sheet_id, name) in &names {
        let Some(Document::Schematic(sheet)) = project.document(*sheet_id) else {
            continue;
        };
        for symbol in &sheet.symbols {
            parts.entry(symbol.designator.clone()).or_insert_with(|| {
                json!({
                    "designator": symbol.designator,
                    "value": symbol.value,
                    "symbol": symbol.symbol,
                    "footprint": symbol.footprint,
                    "sheet": name,
                    "pins": [],
                })
            });
        }
    }
    let mut list: Vec<Value> = parts.into_values().collect();
    list.sort_by(|a, b| {
        ew_schematic::connectivity::natural_cmp(
            a["designator"].as_str().unwrap_or(""),
            b["designator"].as_str().unwrap_or(""),
        )
    });
    json!({ "components": list })
}

fn nets(project: &Project) -> Value {
    let connectivity = ew_schematic::analyze(project);
    let list: Vec<Value> = connectivity
        .nets
        .iter()
        .filter(|net| !net.pins.is_empty())
        .map(|net| {
            let pins: BTreeSet<String> = net
                .pins
                .iter()
                .filter_map(|index| connectivity.pins.get(*index))
                .filter(|pin| !pin.power_symbol)
                .map(|pin| format!("{}.{}", pin.designator, pin.number))
                .collect();
            json!({ "name": net.name, "named": net.named, "pins": pins })
        })
        .collect();
    json!({ "nets": list })
}

fn placed(project: &Project, sheet: Id, symbol: Id) -> Option<&PlacedSymbol> {
    match project.document(sheet) {
        Some(Document::Schematic(sheet)) => sheet.symbols.get(symbol),
        _ => None,
    }
}

// --- Placing symbols ----------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlaceParams {
    /// Symbol name (project library or base library, e.g. `Device:R`).
    #[serde(default)]
    symbol: Option<String>,
    /// Component name of a project library.
    #[serde(default)]
    component: Option<String>,
    #[serde(default)]
    designator: Option<String>,
    #[serde(default)]
    value: Option<String>,
    /// Footprint name, wizard preset (`0805`, `SOIC-8`) or base library name.
    #[serde(default)]
    footprint: Option<String>,
    /// Sheet name or id (default: the top sheet).
    #[serde(default)]
    sheet: Option<String>,
    /// Position in millimetres (default: a free place on the sheet).
    #[serde(default)]
    at: Option<[f64; 2]>,
    /// Rotation in degrees (counter-clockwise).
    #[serde(default)]
    rotation: f64,
    #[serde(default)]
    mirror: bool,
    #[serde(default = "one")]
    unit: u16,
}

const fn one() -> u16 {
    1
}

fn schematic_doc(project: &Project, sheet: Option<&str>) -> Result<Id, EngineError> {
    let names = sheet_names(project);
    match sheet {
        None => project
            .manifest
            .root_schematic
            .filter(|id| names.contains_key(id))
            .or_else(|| names.keys().next().copied())
            .ok_or_else(|| EngineError::not_found("the project has no schematic sheet")),
        Some(wanted) => names
            .iter()
            .find(|(id, name)| name.as_str() == wanted || id.to_string() == wanted)
            .or_else(|| {
                names
                    .iter()
                    .find(|(_, name)| name.eq_ignore_ascii_case(wanted))
            })
            .map(|(id, _)| *id)
            .ok_or_else(|| EngineError::not_found(format!("no sheet `{wanted}`"))),
    }
}

fn first_library(project: &Project) -> Result<Id, EngineError> {
    project
        .documents_of(DocumentKind::Library)
        .next()
        .map(|entry| entry.id)
        .ok_or_else(|| EngineError::not_found("the project has no library"))
}

/// Brings a symbol into the project library from the base library if
/// needed; returns its name in the project.
fn ensure_symbol(
    stage: &mut Stage,
    library: &mut dyn BaseLibrary,
    name: &str,
) -> Result<String, EngineError> {
    if symbol_def(&stage.project, name).is_some() {
        return Ok(name.to_owned());
    }
    let imported = library.symbol(name).ok_or_else(|| {
        EngineError::not_found(format!(
            "symbol `{name}` is not in the project library or the base library \
             (search with library.search)"
        ))
    })?;
    let imported_name = imported
        .symbols
        .first()
        .map_or_else(|| name.to_owned(), |def| def.name.clone());
    let doc = first_library(&stage.project)?;
    let mut report = ew_kicad::ImportReport::new("library");
    let (_, _, requests) =
        crate::import::library_changes(&stage.project, doc, imported, &mut report);
    stage.push(requests)?;
    if symbol_def(&stage.project, &imported_name).is_none() {
        return Err(EngineError::not_found(format!(
            "symbol `{name}` could not be added to the project library"
        )));
    }
    Ok(imported_name)
}

/// Makes sure a footprint is in the project library: as it is, from a
/// wizard preset or from the base library. Returns its name.
fn ensure_footprint(
    stage: &mut Stage,
    library: &mut dyn BaseLibrary,
    name: &str,
) -> Result<String, EngineError> {
    if ew_pcb::sync::footprint_def(&stage.project, name).is_some() {
        return Ok(name.to_owned());
    }
    let wanted = name.strip_prefix("preset:").unwrap_or(name);
    if let Some(preset) = ew_libedit::wizard::presets()
        .into_iter()
        .find(|preset| preset.name.eq_ignore_ascii_case(wanted))
    {
        let generated = ew_libedit::wizard::suggest_name(&preset.wizard);
        if ew_pcb::sync::footprint_def(&stage.project, &generated).is_none() {
            let def =
                ew_libedit::wizard::generate(&preset.wizard, &generated).map_err(edit_failed)?;
            let doc = first_library(&stage.project)?;
            stage.push(vec![Request::Insert {
                doc,
                object: Object::FootprintDef(def),
            }])?;
        }
        return Ok(generated);
    }
    let imported = library.footprint(name).ok_or_else(|| {
        EngineError::not_found(format!(
            "footprint `{name}` is not in the project library, not a wizard preset \
             (see design.presets) and not in the base library"
        ))
    })?;
    let imported_name = imported
        .footprints
        .first()
        .map_or_else(|| name.to_owned(), |def| def.name.clone());
    let doc = first_library(&stage.project)?;
    let mut report = ew_kicad::ImportReport::new("library");
    let (_, _, requests) =
        crate::import::library_changes(&stage.project, doc, imported, &mut report);
    stage.push(requests)?;
    Ok(imported_name)
}

/// A free place on a sheet: left to right, top to bottom, inside the frame
/// and above the title block.
fn free_spot(sheet: &Schematic) -> Point {
    let page = sheet.page.dimensions();
    let taken: Vec<Point> = sheet.symbols.iter().map(|s| s.at).collect();
    let step_x = 10 * SHEET_GRID;
    let step_y = 10 * SHEET_GRID;
    let left = 12 * SHEET_GRID;
    let right = page.w - 12 * SHEET_GRID;
    let top = page.h - 12 * SHEET_GRID;
    let bottom = 24 * SHEET_GRID;
    let clear = 5 * SHEET_GRID;
    let mut y = top;
    while y >= bottom {
        let mut x = left;
        while x <= right {
            let spot = Point::new(x, y);
            if taken
                .iter()
                .all(|p| (p.x - spot.x).abs() >= clear || (p.y - spot.y).abs() >= clear)
            {
                return spot;
            }
            x += step_x;
        }
        y -= step_y;
    }
    Point::new(left, top)
}

/// Snaps a symbol position to the sheet's grid (as the editor does: the
/// grid runs through the top-left corner of the page), so its pins and the
/// wires drawn to them are on the grid too.
fn on_sheet_grid(project: &Project, doc: Id, at: Point) -> Point {
    let Some(Document::Schematic(sheet)) = project.document(doc) else {
        return at;
    };
    let grid = project.manifest.settings.schematic_grid;
    let grid = if grid > 0 { grid } else { SHEET_GRID / 2 };
    ew_schematic::geometry::snap(at, grid, ew_schematic::geometry::grid_origin(sheet))
}

#[allow(clippy::too_many_lines, reason = "one step after the other")]
fn place(
    project: &Project,
    library: &mut dyn BaseLibrary,
    params: &PlaceParams,
) -> Result<Outcome, EngineError> {
    let mut stage = Stage::new(project);
    let doc = schematic_doc(project, params.sheet.as_deref())?;
    let symbol = match (&params.symbol, &params.component) {
        (_, Some(_)) => String::new(),
        (Some(name), None) => ensure_symbol(&mut stage, library, name)?,
        (None, None) => {
            return Err(EngineError::invalid_params("give a symbol or a component"));
        }
    };
    let footprint = match &params.footprint {
        Some(name) => Some(ensure_footprint(&mut stage, library, name)?),
        None => None,
    };
    if let Some(designator) = &params.designator {
        check_free_designator(&stage.project, designator)?;
    }
    let at = match params.at {
        Some(at) => point(at),
        None => match stage.project.document(doc) {
            Some(Document::Schematic(sheet)) => free_spot(sheet),
            _ => return Err(EngineError::not_found("no such sheet")),
        },
    };
    let at = on_sheet_grid(&stage.project, doc, at);
    let edit = sch::place_symbol(
        &stage.project,
        doc,
        &PlaceSymbol {
            symbol,
            unit: params.unit,
            at,
            rotation: angle(params.rotation)?,
            mirror: params.mirror,
            annotate: params.designator.is_none(),
            component: params.component.clone(),
        },
    )
    .map_err(edit_failed)?;
    let id = edit
        .select
        .first()
        .copied()
        .ok_or_else(|| EngineError::internal("no symbol was placed"))?;
    stage.push_edit(&edit)?;
    let mut fields = serde_json::Map::new();
    if let Some(designator) = &params.designator {
        fields.insert("designator".into(), json!(designator));
    }
    if let Some(value) = &params.value {
        fields.insert("value".into(), json!(value));
    }
    if let Some(footprint) = &footprint {
        fields.insert("footprint".into(), json!(footprint));
    }
    if !fields.is_empty() {
        stage.push(vec![Request::Patch { doc, id, fields }])?;
    }
    let Some(placed) = placed(&stage.project, doc, id) else {
        return Err(EngineError::internal("the placed symbol is missing"));
    };
    let reply = json!({
        "designator": placed.designator,
        "value": placed.value,
        "symbol": placed.symbol,
        "footprint": placed.footprint,
        "sheet": sheet_names(&stage.project).get(&doc),
        "at": [mm(placed.at.x), mm(placed.at.y)],
        "id": id,
        "pins": pin_list(&stage.project, placed),
    });
    Ok(stage.commit(labels::PLACE, reply))
}

fn pin_list(project: &Project, symbol: &PlacedSymbol) -> Vec<Value> {
    let Some(def) = symbol_def(project, &symbol.symbol) else {
        return Vec::new();
    };
    let Some(body) = def.body(symbol.unit, symbol.alternate) else {
        return Vec::new();
    };
    body.pins
        .iter()
        .map(|pin| json!({ "number": pin.number, "name": pin.name }))
        .collect()
}

fn check_free_designator(project: &Project, designator: &str) -> Result<(), EngineError> {
    for (id, _) in sheet_names(project) {
        if let Some(Document::Schematic(sheet)) = project.document(id)
            && sheet.symbols.iter().any(|s| s.designator == designator)
        {
            return Err(EngineError::invalid_params(format!(
                "the designator {designator} is taken"
            )));
        }
    }
    Ok(())
}

/// Every placed symbol (all units) with a designator.
fn symbols_of(project: &Project, designator: &str) -> Vec<(Id, Id)> {
    let mut out = Vec::new();
    for (doc, _) in sheet_names(project) {
        if let Some(Document::Schematic(sheet)) = project.document(doc) {
            for symbol in &sheet.symbols {
                if symbol.designator == designator {
                    out.push((doc, symbol.id));
                }
            }
        }
    }
    out
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetParams {
    designator: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    footprint: Option<String>,
    /// New designator.
    #[serde(default)]
    rename: Option<String>,
    /// Properties to set (`MPN`, `Manufacturer`, …; empty text removes).
    #[serde(default)]
    properties: BTreeMap<String, String>,
}

fn set(
    project: &Project,
    library: &mut dyn BaseLibrary,
    params: &SetParams,
) -> Result<Outcome, EngineError> {
    let mut stage = Stage::new(project);
    let symbols = symbols_of(project, &params.designator);
    if symbols.is_empty() {
        return Err(EngineError::not_found(format!(
            "no part {}",
            params.designator
        )));
    }
    if let Some(rename) = &params.rename {
        check_free_designator(project, rename)?;
    }
    let footprint = match &params.footprint {
        Some(name) => Some(ensure_footprint(&mut stage, library, name)?),
        None => None,
    };
    for (doc, id) in symbols {
        let Some(symbol) = placed(&stage.project, doc, id).cloned() else {
            continue;
        };
        let mut fields = serde_json::Map::new();
        if let Some(value) = &params.value {
            fields.insert("value".into(), json!(value));
        }
        if let Some(footprint) = &footprint {
            fields.insert("footprint".into(), json!(footprint));
        }
        if let Some(rename) = &params.rename {
            fields.insert("designator".into(), json!(rename));
        }
        if !params.properties.is_empty() {
            let mut properties = symbol.properties.clone();
            for (key, value) in &params.properties {
                if value.is_empty() {
                    properties.remove(key);
                } else {
                    properties.insert(key.clone(), value.clone());
                }
            }
            fields.insert("properties".into(), json!(properties));
        }
        if !fields.is_empty() {
            stage.push(vec![Request::Patch { doc, id, fields }])?;
        }
    }
    let designator = params.rename.as_ref().unwrap_or(&params.designator);
    Ok(stage.commit(
        labels::SET,
        json!({ "designator": designator, "footprint": footprint }),
    ))
}

// --- Connecting pins ------------------------------------------------------------

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
enum NetStyle {
    /// A local label, or a global label when the net spans sheets.
    #[default]
    Auto,
    Label,
    Global,
    Power,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConnectParams {
    net: String,
    pins: Vec<String>,
    #[serde(default)]
    style: NetStyle,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PinsParams {
    pins: Vec<String>,
}

/// A pin found by its reference.
struct FoundPin {
    reference: String,
    sheet: Id,
    at: Point,
    /// Unit vector from the pin into the body.
    inward: Point,
    net: Option<usize>,
}

fn split_pin(reference: &str) -> Result<(&str, &str), EngineError> {
    reference
        .rsplit_once('.')
        .filter(|(part, pin)| !part.is_empty() && !pin.is_empty())
        .ok_or_else(|| {
            EngineError::invalid_params(format!(
                "`{reference}` is not a pin: write designator.pin, for example R1.2 or U1.VCC"
            ))
        })
}

fn find_pins(
    project: &Project,
    connectivity: &Connectivity,
    reference: &str,
) -> Result<Vec<FoundPin>, EngineError> {
    let (designator, wanted) = split_pin(reference)?;
    let candidates: Vec<&ew_schematic::connectivity::PinInfo> = connectivity
        .pins
        .iter()
        .filter(|pin| !pin.power_symbol && pin.designator == designator)
        .collect();
    if candidates.is_empty() {
        return Err(EngineError::not_found(format!("no part {designator}")));
    }
    let mut chosen: Vec<_> = candidates
        .iter()
        .filter(|pin| pin.number == wanted)
        .collect();
    if chosen.is_empty() {
        chosen = candidates.iter().filter(|pin| pin.name == wanted).collect();
    }
    if chosen.is_empty() {
        chosen = candidates
            .iter()
            .filter(|pin| pin.name.eq_ignore_ascii_case(wanted))
            .collect();
    }
    if chosen.is_empty() {
        // KiCad marks inverted pins as `~{RST}`: `RST` finds them.
        chosen = candidates
            .iter()
            .filter(|pin| plain_name(&pin.name).eq_ignore_ascii_case(&plain_name(wanted)))
            .collect();
    }
    if chosen.is_empty() {
        let known: Vec<String> = candidates
            .iter()
            .map(|pin| {
                if pin.name.is_empty() || pin.name == "~" || pin.name == pin.number {
                    pin.number.clone()
                } else {
                    format!("{} ({})", pin.number, pin.name)
                }
            })
            .collect();
        return Err(EngineError::not_found(format!(
            "{designator} has no pin {wanted}; its pins are {}",
            known.join(", ")
        )));
    }
    let mut out = Vec::new();
    for pin in chosen {
        let sheet = connectivity
            .instances
            .get(pin.instance)
            .map(|instance| instance.sheet)
            .ok_or_else(|| EngineError::internal("pin without sheet"))?;
        let symbol = placed(project, sheet, pin.symbol)
            .ok_or_else(|| EngineError::internal("pin without symbol"))?;
        let def = symbol_def(project, &symbol.symbol)
            .ok_or_else(|| EngineError::not_found(format!("no symbol `{}`", symbol.symbol)))?;
        let body = def
            .body(symbol.unit, symbol.alternate)
            .ok_or_else(|| EngineError::internal("symbol without unit"))?;
        let pin_def = body
            .pins
            .get(pin.index)
            .ok_or_else(|| EngineError::internal("pin index out of range"))?;
        out.push(FoundPin {
            reference: format!("{designator}.{}", pin.number),
            sheet,
            at: pin.at,
            inward: pin_direction(symbol, pin_def),
            net: Some(pin.net),
        });
    }
    Ok(out)
}

/// A pin name without KiCad's overbar markup (`~{RST}` → `RST`).
fn plain_name(name: &str) -> String {
    name.replace("~{", "").replace('}', "")
}

/// Rotation of a label at the end of a stub pointing in `outward`.
fn label_rotation(outward: Point) -> Angle {
    if outward.x < 0 {
        Angle::DEG_180
    } else if outward.y > 0 {
        Angle::DEG_90
    } else if outward.y < 0 {
        Angle::DEG_270
    } else {
        Angle::ZERO
    }
}

fn power_style(net: &str) -> PowerStyle {
    let upper = net.to_ascii_uppercase();
    if upper.starts_with("GND") || upper.starts_with("VSS") || upper == "0V" {
        PowerStyle::Ground
    } else {
        PowerStyle::Bar
    }
}

#[allow(clippy::too_many_lines, reason = "one pass over the pins")]
fn connect(project: &Project, params: &ConnectParams) -> Result<Outcome, EngineError> {
    let net = params.net.trim();
    if net.is_empty() {
        return Err(EngineError::invalid_params("the net needs a name"));
    }
    if params.pins.is_empty() {
        return Err(EngineError::invalid_params("give at least one pin"));
    }
    let connectivity = ew_schematic::analyze(project);
    let mut found = Vec::new();
    for reference in &params.pins {
        found.extend(find_pins(project, &connectivity, reference)?);
    }
    // Sheets where the net is already, and where it will be.
    let mut sheets: BTreeSet<Id> = found.iter().map(|pin| pin.sheet).collect();
    let mut global_exists = false;
    for existing in connectivity.nets.iter().filter(|n| n.names.contains(net)) {
        for index in &existing.pins {
            if let Some(pin) = connectivity.pins.get(*index)
                && let Some(instance) = connectivity.instances.get(pin.instance)
            {
                sheets.insert(instance.sheet);
            }
        }
        if existing.pieces.len() > 1 {
            global_exists = true;
        }
    }
    let style = match params.style {
        NetStyle::Auto if sheets.len() > 1 || global_exists => NetStyle::Global,
        NetStyle::Auto => NetStyle::Label,
        other => other,
    };
    let mut stage = Stage::new(project);
    let mut connected = Vec::new();
    let mut already = Vec::new();
    for pin in &found {
        if let Some(existing) = pin.net.and_then(|n| connectivity.nets.get(n)) {
            if existing.names.contains(net) {
                already.push(pin.reference.clone());
                continue;
            }
            if existing.named {
                return Err(EngineError::invalid_params(format!(
                    "{} is on net {} already",
                    pin.reference, existing.name
                )));
            }
        }
        let outward = Point::new(-pin.inward.x, -pin.inward.y);
        let end = Point::new(pin.at.x + outward.x * STUB, pin.at.y + outward.y * STUB);
        let wire = sch::draw_wire(&stage.project, pin.sheet, &[pin.at, end], false)
            .map_err(edit_failed)?;
        stage.push_edit(&wire)?;
        let item = match style {
            NetStyle::Power => AddItem::PowerPort {
                net: net.to_owned(),
                style: power_style(net),
            },
            NetStyle::Global => AddItem::Label {
                label: LabelKind::Global,
                text: net.to_owned(),
            },
            NetStyle::Auto | NetStyle::Label => AddItem::Label {
                label: LabelKind::Local,
                text: net.to_owned(),
            },
        };
        let rotation = if style == NetStyle::Power {
            Angle::ZERO
        } else {
            label_rotation(outward)
        };
        let added =
            sch::add_item(&stage.project, pin.sheet, &item, end, rotation).map_err(edit_failed)?;
        stage.push_edit(&added)?;
        connected.push(pin.reference.clone());
    }
    let style_name = match style {
        NetStyle::Power => "power",
        NetStyle::Global => "global",
        NetStyle::Auto | NetStyle::Label => "label",
    };
    let reply = json!({
        "net": net,
        "connected": connected,
        "already": already,
        "style": style_name,
    });
    Ok(stage.commit(labels::CONNECT, reply))
}

fn no_connect(project: &Project, params: &PinsParams) -> Result<Outcome, EngineError> {
    let connectivity = ew_schematic::analyze(project);
    let mut stage = Stage::new(project);
    let mut marked = Vec::new();
    for reference in &params.pins {
        for pin in find_pins(project, &connectivity, reference)? {
            let added = sch::add_item(
                &stage.project,
                pin.sheet,
                &AddItem::NoConnect,
                pin.at,
                Angle::ZERO,
            )
            .map_err(edit_failed)?;
            stage.push_edit(&added)?;
            marked.push(pin.reference);
        }
    }
    Ok(stage.commit(labels::NO_CONNECT, json!({ "marked": marked })))
}

// --- The board --------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FootprintParams {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BoardParams {
    /// Board name or id (default: the first board).
    #[serde(default)]
    board: Option<String>,
}

fn board_doc<'a>(project: &'a Project, board: Option<&str>) -> Result<(Id, &'a Pcb), EngineError> {
    let mut boards = project.documents_of(DocumentKind::Pcb).filter_map(|entry| {
        match project.document(entry.id) {
            Some(Document::Pcb(pcb)) => Some((entry.id, pcb)),
            _ => None,
        }
    });
    match board {
        None => boards
            .next()
            .ok_or_else(|| EngineError::not_found("the project has no board")),
        Some(wanted) => boards
            .find(|(id, pcb)| pcb.name == wanted || id.to_string() == wanted)
            .ok_or_else(|| EngineError::not_found(format!("no board `{wanted}`"))),
    }
}

fn rules(project: &Project) -> Option<&Rules> {
    let entry = project.documents_of(DocumentKind::Rules).next()?;
    match project.document(entry.id) {
        Some(Document::Rules(rules)) => Some(rules),
        _ => None,
    }
}

/// The board outline's box (`None` without an outline).
fn outline_box(pcb: &Pcb) -> Option<BBox> {
    let outline = ew_pcb::items::outline(pcb);
    let ring = outline.rings.first()?;
    Some(BBox::of_points(ring))
}

fn board_summary(project: &Project, params: &BoardParams) -> Result<Outcome, EngineError> {
    let (doc, pcb) = board_doc(project, params.board.as_deref())?;
    let outline = outline_box(pcb);
    let footprints: Vec<Value> = {
        let mut list: Vec<&FootprintInstance> = pcb.footprints.iter().collect();
        list.sort_by(|a, b| ew_schematic::connectivity::natural_cmp(&a.designator, &b.designator));
        list.into_iter()
            .map(|fp| {
                let on_board = outline.is_none_or(|b| {
                    let c = V::new(fl(fp.at.x), fl(fp.at.y));
                    c.x >= b.min.x && c.x <= b.max.x && c.y >= b.min.y && c.y <= b.max.y
                });
                json!({
                    "designator": fp.designator,
                    "value": fp.value,
                    "footprint": fp.footprint,
                    "at": [mm(fp.at.x), mm(fp.at.y)],
                    "rotation": degrees(fp.rotation),
                    "side": if fp.side == Side::Bottom { "bottom" } else { "top" },
                    "onBoard": on_board,
                })
            })
            .collect()
    };
    let board = ew_pcb::items::Board::new(pcb);
    let connectivity = ew_pcb::connectivity::analyze(&board);
    let layers: Vec<String> = pcb
        .stackup
        .copper_layers()
        .map(|layer| layer.to_string())
        .collect();
    Ok(Outcome::Read(json!({
        "board": pcb.name,
        "id": doc,
        "outline": outline.map(|b| json!({
            "min": [mm_f(b.min.x), mm_f(b.min.y)],
            "max": [mm_f(b.max.x), mm_f(b.max.y)],
        })),
        "copperLayers": layers,
        "footprints": footprints,
        "tracks": pcb.tracks.len(),
        "vias": pcb.vias.len(),
        "unrouted": connectivity.airwires.len(),
    })))
}

fn update_board(project: &Project, params: &BoardParams) -> Result<Outcome, EngineError> {
    let (doc, pcb) = board_doc(project, params.board.as_deref())?;
    let connectivity = ew_schematic::analyze(project);
    let diff = ew_pcb::sync::diff(project, &connectivity, pcb);
    let changes: Vec<Value> = diff
        .changes
        .iter()
        .map(|change| serde_json::to_value(change).unwrap_or(Value::Null))
        .collect();
    let selected: Vec<usize> = (0..diff.changes.len()).collect();
    let edit = ew_pcb::sync::apply(project, &connectivity, doc, pcb, &diff, &selected);
    let reply = json!({ "changes": changes });
    if edit.changes.is_empty() {
        return Ok(Outcome::Read(reply));
    }
    Ok(Outcome::Commit {
        label: labels::UPDATE_BOARD,
        requests: crate::schematic::requests(&edit),
        reply,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlaceFootprintParams {
    #[serde(default)]
    board: Option<String>,
    designator: String,
    /// Position in millimetres.
    at: [f64; 2],
    /// Rotation in degrees (counter-clockwise).
    #[serde(default)]
    rotation: Option<f64>,
    /// `top` or `bottom`.
    #[serde(default)]
    side: Option<Side>,
}

/// Applies a board edit to the stage.
fn board_edit(stage: &mut Stage, doc: Id, edit: &BoardEdit) -> Result<(), EngineError> {
    let Some(Document::Pcb(pcb)) = stage.project.document(doc) else {
        return Err(EngineError::not_found("no such board"));
    };
    let edit = ew_pcb::edit::apply(doc, pcb, edit).map_err(edit_failed)?;
    stage.push(crate::schematic::requests(&edit))
}

fn footprint_by_designator(
    project: &Project,
    doc: Id,
    designator: &str,
) -> Result<FootprintInstance, EngineError> {
    match project.document(doc) {
        Some(Document::Pcb(pcb)) => pcb
            .footprints
            .iter()
            .find(|fp| fp.designator == designator)
            .cloned()
            .ok_or_else(|| {
                EngineError::not_found(format!(
                    "no footprint {designator} on the board (update the board from the schematic first)"
                ))
            }),
        _ => Err(EngineError::not_found("no such board")),
    }
}

/// Moves, turns and flips a footprint to an exact place.
fn put(
    stage: &mut Stage,
    doc: Id,
    designator: &str,
    at: Point,
    rotation: Option<Angle>,
    side: Option<Side>,
) -> Result<(), EngineError> {
    let fp = footprint_by_designator(&stage.project, doc, designator)?;
    if fp.locked {
        return Err(EngineError::invalid_params(format!(
            "{designator} is locked"
        )));
    }
    if let Some(side) = side
        && side != fp.side
    {
        board_edit(
            stage,
            doc,
            &BoardEdit::Flip {
                ids: vec![fp.id],
                center_x: Some(fp.at.x),
            },
        )?;
    }
    let fp = footprint_by_designator(&stage.project, doc, designator)?;
    if let Some(rotation) = rotation {
        let turn = Angle::from_millidegrees(
            i64::from(rotation.millidegrees()) - i64::from(fp.rotation.millidegrees()),
        );
        if turn != Angle::ZERO {
            board_edit(
                stage,
                doc,
                &BoardEdit::Rotate {
                    ids: vec![fp.id],
                    center: Some(fp.at),
                    angle: turn,
                },
            )?;
        }
    }
    let fp = footprint_by_designator(&stage.project, doc, designator)?;
    let offset = Point::new(at.x - fp.at.x, at.y - fp.at.y);
    if offset != Point::ORIGIN {
        board_edit(
            stage,
            doc,
            &BoardEdit::Move {
                ids: vec![fp.id],
                offset,
            },
        )?;
    }
    Ok(())
}

fn place_footprint(
    project: &Project,
    params: &PlaceFootprintParams,
) -> Result<Outcome, EngineError> {
    let (doc, _) = board_doc(project, params.board.as_deref())?;
    let mut stage = Stage::new(project);
    let rotation = params.rotation.map(angle).transpose()?;
    put(
        &mut stage,
        doc,
        &params.designator,
        point(params.at),
        rotation,
        params.side,
    )?;
    let fp = footprint_by_designator(&stage.project, doc, &params.designator)?;
    let reply = json!({
        "designator": fp.designator,
        "at": [mm(fp.at.x), mm(fp.at.y)],
        "rotation": degrees(fp.rotation),
        "side": if fp.side == Side::Bottom { "bottom" } else { "top" },
    });
    Ok(stage.commit(labels::PLACE_FOOTPRINTS, reply))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArrangeParams {
    #[serde(default)]
    board: Option<String>,
    /// Only these parts (default: every footprint that is not on the board).
    #[serde(default)]
    designators: Option<Vec<String>>,
    /// Gap between footprints in millimetres.
    #[serde(default = "default_gap")]
    gap: f64,
}

const fn default_gap() -> f64 {
    2.0
}

/// Puts footprints in rows inside the board outline, in designator order.
fn arrange(project: &Project, params: &ArrangeParams) -> Result<Outcome, EngineError> {
    let (doc, pcb) = board_doc(project, params.board.as_deref())?;
    let area = outline_box(pcb).ok_or_else(|| {
        EngineError::invalid_params("the board has no outline: draw it first (board setup)")
    })?;
    let inside = |fp: &FootprintInstance| {
        let (x, y) = (fl(fp.at.x), fl(fp.at.y));
        x >= area.min.x && x <= area.max.x && y >= area.min.y && y <= area.max.y
    };
    let mut chosen: Vec<FootprintInstance> = match &params.designators {
        Some(list) => list
            .iter()
            .map(|d| {
                pcb.footprints
                    .iter()
                    .find(|fp| &fp.designator == d)
                    .cloned()
                    .ok_or_else(|| EngineError::not_found(format!("no footprint {d} on the board")))
            })
            .collect::<Result<_, _>>()?,
        None => pcb
            .footprints
            .iter()
            .filter(|fp| !inside(fp) && !fp.locked)
            .cloned()
            .collect(),
    };
    chosen.sort_by(|a, b| ew_schematic::connectivity::natural_cmp(&a.designator, &b.designator));
    let gap = fl(nm(params.gap.max(0.0)));
    let margin = 1_000_000.0_f64.max(gap);
    let mut stage = Stage::new(project);
    let mut x = area.min.x + margin;
    let mut y = area.max.y - margin;
    let mut row_height = 0.0_f64;
    let mut placed = Vec::new();
    let mut overflow = Vec::new();
    for fp in &chosen {
        let bbox = ew_pcb::edit::footprint_box(fp);
        let (w, h) = (bbox.max.x - bbox.min.x, bbox.max.y - bbox.min.y);
        if x + w > area.max.x - margin && x > area.min.x + margin {
            x = area.min.x + margin;
            y -= row_height + gap;
            row_height = 0.0;
        }
        if y - h < area.min.y + margin {
            overflow.push(fp.designator.clone());
            continue;
        }
        // The footprint's origin relative to its box.
        let origin = V::new(fl(fp.at.x) - bbox.min.x, fl(fp.at.y) - bbox.max.y);
        let at = Point::new(rnd(x + origin.x), rnd(y + origin.y));
        put(&mut stage, doc, &fp.designator, at, None, None)?;
        placed.push(fp.designator.clone());
        x += w + gap;
        row_height = row_height.max(h);
    }
    Ok(stage.commit(
        labels::PLACE_FOOTPRINTS,
        json!({ "placed": placed, "noRoom": overflow }),
    ))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteParams {
    #[serde(default)]
    board: Option<String>,
    /// Only these nets (default: all).
    #[serde(default)]
    nets: Option<Vec<String>>,
    /// Copper layers to use, in order of preference (default: top, bottom).
    #[serde(default)]
    layers: Option<Vec<CopperLayer>>,
    /// Track width in millimetres (default: the design rules).
    #[serde(default)]
    width: Option<f64>,
}

/// Routes the unrouted connections one by one with the walkaround router,
/// on one copper layer each (no vias), with 45° bends.
#[allow(clippy::too_many_lines, reason = "one loop over the connections")]
fn route(project: &Project, params: &RouteParams) -> Result<Outcome, EngineError> {
    let (doc, pcb) = board_doc(project, params.board.as_deref())?;
    let wanted: Option<BTreeSet<Id>> = match &params.nets {
        None => None,
        Some(names) => {
            let mut ids = BTreeSet::new();
            for name in names {
                // `LED_A` also finds the local net `/LED_A`.
                let suffix = format!("/{name}");
                let mut matches = pcb.nets.iter().filter(|net| net.name.ends_with(&suffix));
                let net = pcb
                    .nets
                    .iter()
                    .find(|net| &net.name == name)
                    .or_else(|| match (matches.next(), matches.next()) {
                        (Some(net), None) => Some(net),
                        _ => None,
                    })
                    .ok_or_else(|| EngineError::not_found(format!("no net {name} on the board")))?;
                ids.insert(net.id);
            }
            Some(ids)
        }
    };
    let layers: Vec<CopperLayer> = params.layers.clone().unwrap_or_else(|| {
        pcb.stackup
            .copper_layers()
            .filter(|l| matches!(l, CopperLayer::Top | CopperLayer::Bottom))
            .collect()
    });
    for layer in &layers {
        if !pcb
            .stackup
            .has_layer(ew_model::layer::Layer::Copper(*layer))
        {
            return Err(EngineError::invalid_params(format!(
                "the board has no layer {layer}"
            )));
        }
    }
    if let Some(width) = params.width
        && !(width > 0.0 && width < 20.0)
    {
        return Err(EngineError::invalid_params(
            "the width must be between 0 and 20 mm",
        ));
    }
    let mut stage = Stage::new(project);
    let mut routed: BTreeMap<String, usize> = BTreeMap::new();
    let mut failed: Vec<Value> = Vec::new();
    let mut tried: BTreeSet<(i64, i64, i64, i64)> = BTreeSet::new();
    let key = |a: [f64; 2], b: [f64; 2]| (rnd(a[0]), rnd(a[1]), rnd(b[0]), rnd(b[1]));
    // Each pass routes one connection; the airwires change after every track.
    for _ in 0..10_000 {
        let Some(Document::Pcb(current)) = stage.project.document(doc) else {
            break;
        };
        let board = ew_pcb::items::Board::new(current);
        let connectivity = ew_pcb::connectivity::analyze(&board);
        let next = connectivity.airwires.iter().find(|wire| {
            wanted.as_ref().is_none_or(|ids| ids.contains(&wire.net))
                && !tried.contains(&key(wire.from, wire.to))
                && !tried.contains(&key(wire.to, wire.from))
        });
        let Some(wire) = next.cloned() else {
            break;
        };
        tried.insert(key(wire.from, wire.to));
        let net_name = current
            .nets
            .get(wire.net)
            .map(|net| net.name.clone())
            .unwrap_or_default();
        let walker = Router::new(current, rules(&stage.project));
        let mut done = None;
        for layer in &layers {
            let start = walker.start(V::new(wire.from[0], wire.from[1]), *layer);
            let end = walker.start(V::new(wire.to[0], wire.to[1]), *layer);
            if start.net != Some(wire.net) || end.net != Some(wire.net) {
                continue;
            }
            let width = params.width.map_or(start.width, |w| fl(nm(w)));
            let request = ew_pcb::route::Request {
                layer: *layer,
                net: Some(wire.net),
                width,
                start: V::new(start.at[0], start.at[1]),
                target: V::new(end.at[0], end.at[1]),
                diagonal_first: false,
                corners: Corners::Deg45,
                previous: None,
            };
            let result = walker.route(&request);
            if result.reached && result.points.len() >= 2 {
                done = Some((*layer, width, result.points));
                break;
            }
        }
        let Some((layer, width, points)) = done else {
            failed.push(json!({
                "net": net_name,
                "from": [mm_f(wire.from[0]), mm_f(wire.from[1])],
                "to": [mm_f(wire.to[0]), mm_f(wire.to[1])],
            }));
            continue;
        };
        let mut requests = Vec::new();
        for pair in points.windows(2) {
            let (a, b) = (
                Point::new(rnd(pair[0][0]), rnd(pair[0][1])),
                Point::new(rnd(pair[1][0]), rnd(pair[1][1])),
            );
            if a == b {
                continue;
            }
            let track = Track {
                id: Id::random(),
                layer,
                net: Some(wire.net),
                width: rnd(width),
                start: a,
                end: b,
                mid: None,
                locked: false,
            };
            requests.push(Request::Insert {
                doc,
                object: Object::Track(track),
            });
        }
        stage.push(requests)?;
        *routed.entry(net_name).or_default() += 1;
    }
    let remaining = match stage.project.document(doc) {
        Some(Document::Pcb(current)) => {
            let board = ew_pcb::items::Board::new(current);
            ew_pcb::connectivity::analyze(&board).airwires.len()
        }
        _ => 0,
    };
    let count: usize = routed.values().sum();
    Ok(stage.commit(
        labels::ROUTE,
        json!({
            "routed": count,
            "nets": routed,
            "failed": failed,
            "unrouted": remaining,
        }),
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn pin_names_without_overbars() {
        assert_eq!(super::plain_name("~{RST}"), "RST");
        assert_eq!(super::plain_name("~{CS}/~{SHDN}"), "CS/SHDN");
        assert_eq!(super::plain_name("VCC"), "VCC");
    }
}
