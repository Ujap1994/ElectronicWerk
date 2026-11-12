//! The `schematic.*` methods: electrical rules check, net highlighting,
//! editing operations, annotation, find and replace and exports. The logic
//! lives in `ew-schematic`; this module parses parameters and turns its
//! edits into change requests.

use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::project::Project;
use ew_model::units::{Angle, Point};
use ew_schematic::annotate::AnnotateOptions;
use ew_schematic::edit::{self, AddItem, Change, Edit, EditError, Orientation, PlaceSymbol};
use ew_schematic::find::FindOptions;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::change::Request;
use crate::error::EngineError;

/// History labels of the schematic edits.
pub mod labels {
    /// `schematic.place`.
    pub const PLACE: &str = "place-symbol";
    /// `schematic.wire`.
    pub const WIRE: &str = "draw-wire";
    /// `schematic.add` with a label.
    pub const LABEL: &str = "add-label";
    /// `schematic.add` with a power port.
    pub const POWER: &str = "add-power-port";
    /// `schematic.add` with a no-connect flag.
    pub const NO_CONNECT: &str = "add-no-connect";
    /// `schematic.add` with a junction.
    pub const JUNCTION: &str = "add-junction";
    /// `schematic.add` with a text.
    pub const TEXT: &str = "add-text";
    /// `schematic.move`.
    pub const MOVE: &str = "move";
    /// `schematic.move` with `drag`.
    pub const DRAG: &str = "drag";
    /// `schematic.orient` (turn).
    pub const ROTATE: &str = "rotate";
    /// `schematic.orient` (mirror).
    pub const MIRROR: &str = "mirror";
    /// `schematic.delete`.
    pub const DELETE: &str = "delete";
    /// `schematic.paste`.
    pub const PASTE: &str = "paste";
    /// `schematic.annotate`.
    pub const ANNOTATE: &str = "annotate";
    /// `schematic.replace`.
    pub const REPLACE: &str = "replace";
    /// `schematic.syncSheetPins`.
    pub const SYNC_SHEET_PINS: &str = "sync-sheet-pins";
    /// `schematic.snapToGrid`.
    pub const SNAP_TO_GRID: &str = "snap-to-grid";
    /// `schematic.setErcSettings`.
    pub const ERC_SETTINGS: &str = "erc-settings";
}

/// What a method does: answer, or change the project.
pub enum Outcome {
    /// A reply without changes.
    Read(Value),
    /// Raw requests to commit (changes that are not object edits).
    Requests {
        /// History label.
        label: &'static str,
        /// The requests.
        requests: Vec<Request>,
    },
    /// Changes to commit as one undo step.
    Edit {
        /// History label.
        label: &'static str,
        /// The changes and the objects to select.
        edit: Edit,
    },
}

fn parse<T: DeserializeOwned>(params: &Value) -> Result<T, EngineError> {
    let value = if params.is_null() {
        Value::Object(Map::new())
    } else {
        params.clone()
    };
    serde_json::from_value(value).map_err(|error| EngineError::invalid_params(error.to_string()))
}

fn failed(error: &EditError) -> EngineError {
    match error {
        EditError::UnknownObject(_) | EditError::NotASchematic(_) | EditError::UnknownSymbol(_) => {
            EngineError::not_found(error.to_string())
        }
        EditError::Invalid(_) => EngineError::invalid_params(error.to_string()),
    }
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(EngineError::internal)
}

/// The change requests of an edit.
#[must_use]
pub fn requests(edit: &Edit) -> Vec<Request> {
    edit.changes
        .iter()
        .map(|change| match change {
            Change::Insert { doc, object } => Request::Insert {
                doc: *doc,
                object: object.clone(),
            },
            Change::Update { doc, object } => Request::Update {
                doc: *doc,
                object: object.clone(),
            },
            Change::Delete { doc, id } => Request::Delete { doc: *doc, id: *id },
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocIds {
    doc: Id,
    #[serde(default)]
    ids: Vec<Id>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaceParams {
    doc: Id,
    #[serde(flatten)]
    place: PlaceSymbol,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireParams {
    doc: Id,
    points: Vec<Point>,
    #[serde(default)]
    bus: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SymbolParams {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddParams {
    doc: Id,
    item: AddItem,
    at: Point,
    #[serde(default)]
    rotation: Angle,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveParams {
    doc: Id,
    ids: Vec<Id>,
    delta: Point,
    #[serde(default)]
    drag: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrientParams {
    doc: Id,
    ids: Vec<Id>,
    orientation: Orientation,
    #[serde(default)]
    center: Option<Point>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PasteParams {
    doc: Id,
    objects: Vec<Object>,
    #[serde(default)]
    offset: Point,
    #[serde(default = "yes")]
    annotate: bool,
}

const fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplaceParams {
    #[serde(flatten)]
    find: FindOptions,
    with: String,
    #[serde(default)]
    only: Option<Vec<HitRef>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HitRef {
    doc: Id,
    id: Id,
    field: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncParams {
    doc: Id,
    id: Id,
    #[serde(default)]
    remove: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinRef {
    symbol: Id,
    index: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetParams {
    doc: Id,
    #[serde(default)]
    id: Option<Id>,
    #[serde(default)]
    pin: Option<PinRef>,
    #[serde(default)]
    at: Option<Point>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErcSettingsParams {
    #[serde(default)]
    severities: std::collections::BTreeMap<String, ew_model::rules::ErcSeverity>,
    matrix: [[ew_model::rules::ErcSeverity; 12]; 12],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetlistParams {
    #[serde(default)]
    format: NetlistFormat,
}

#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum NetlistFormat {
    #[default]
    Kicad,
    Csv,
}

/// Runs a `schematic.*` method.
///
/// # Errors
///
/// Fails for unknown methods, bad parameters and impossible edits.
#[allow(clippy::too_many_lines, reason = "one arm per method")]
pub fn call(project: &Project, method: &str, params: &Value) -> Result<Outcome, EngineError> {
    let edit =
        |label: &'static str, result: Result<Edit, EditError>| -> Result<Outcome, EngineError> {
            Ok(Outcome::Edit {
                label,
                edit: result.map_err(|e| failed(&e))?,
            })
        };
    match method {
        "schematic.erc" => {
            let connectivity = ew_schematic::analyze(project);
            let settings = ew_schematic::erc::settings(project);
            let report = ew_schematic::check(project, &connectivity, &settings);
            Ok(Outcome::Read(to_value(&report)?))
        }
        "schematic.net" => {
            let params: NetParams = parse(params)?;
            Ok(Outcome::Read(net(project, &params)))
        }
        "schematic.symbols" => Ok(Outcome::Read(symbols(project))),
        "schematic.components" => Ok(Outcome::Read(components(project))),
        "schematic.symbol" => {
            let params: SymbolParams = parse(params)?;
            let def = ew_schematic::connectivity::symbol_def(project, &params.name)
                .ok_or_else(|| EngineError::not_found(format!("no symbol `{}`", params.name)))?;
            Ok(Outcome::Read(to_value(def)?))
        }
        "schematic.dragInfo" => {
            let params: DocIds = parse(params)?;
            let wires =
                edit::drag_info(project, params.doc, &params.ids).map_err(|e| failed(&e))?;
            Ok(Outcome::Read(to_value(&wires)?))
        }
        "schematic.copy" => {
            let params: DocIds = parse(params)?;
            let document = project
                .document(params.doc)
                .ok_or_else(|| EngineError::not_found(format!("no document {}", params.doc)))?;
            let objects: Vec<Object> = params
                .ids
                .iter()
                .filter_map(|id| document.object(*id))
                .collect();
            Ok(Outcome::Read(json!({"objects": objects})))
        }
        "schematic.place" => {
            let params: PlaceParams = parse(params)?;
            edit(
                labels::PLACE,
                edit::place_symbol(project, params.doc, &params.place),
            )
        }
        "schematic.wire" => {
            let params: WireParams = parse(params)?;
            edit(
                labels::WIRE,
                edit::draw_wire(project, params.doc, &params.points, params.bus),
            )
        }
        "schematic.add" => {
            let params: AddParams = parse(params)?;
            let label = match params.item {
                AddItem::Label { .. } => labels::LABEL,
                AddItem::PowerPort { .. } => labels::POWER,
                AddItem::NoConnect => labels::NO_CONNECT,
                AddItem::Junction => labels::JUNCTION,
                AddItem::Text { .. } => labels::TEXT,
            };
            edit(
                label,
                edit::add_item(
                    project,
                    params.doc,
                    &params.item,
                    params.at,
                    params.rotation,
                ),
            )
        }
        "schematic.move" => {
            let params: MoveParams = parse(params)?;
            let label = if params.drag {
                labels::DRAG
            } else {
                labels::MOVE
            };
            edit(
                label,
                edit::move_items(project, params.doc, &params.ids, params.delta, params.drag),
            )
        }
        "schematic.orient" => {
            let params: OrientParams = parse(params)?;
            let label = match params.orientation {
                Orientation::Left | Orientation::Right => labels::ROTATE,
                Orientation::Horizontal | Orientation::Vertical => labels::MIRROR,
            };
            edit(
                label,
                edit::orient_items(
                    project,
                    params.doc,
                    &params.ids,
                    params.orientation,
                    params.center,
                ),
            )
        }
        "schematic.delete" => {
            let params: DocIds = parse(params)?;
            edit(
                labels::DELETE,
                edit::delete_items(project, params.doc, &params.ids),
            )
        }
        "schematic.paste" => {
            let params: PasteParams = parse(params)?;
            edit(
                labels::PASTE,
                edit::paste(
                    project,
                    params.doc,
                    &params.objects,
                    params.offset,
                    params.annotate,
                ),
            )
        }
        "schematic.annotate" => {
            let options: AnnotateOptions = parse(params)?;
            edit(
                labels::ANNOTATE,
                Ok(ew_schematic::annotate::annotate(project, &options)),
            )
        }
        "schematic.find" => {
            let options: FindOptions = parse(params)?;
            Ok(Outcome::Read(to_value(&ew_schematic::find::find(
                project, &options,
            ))?))
        }
        "schematic.replace" => {
            let params: ReplaceParams = parse(params)?;
            let only: Option<Vec<(Id, Id, String)>> = params
                .only
                .map(|list| list.into_iter().map(|h| (h.doc, h.id, h.field)).collect());
            edit(
                labels::REPLACE,
                Ok(ew_schematic::find::replace(
                    project,
                    &params.find,
                    &params.with,
                    only.as_deref(),
                )),
            )
        }
        "schematic.syncSheetPins" => {
            let params: SyncParams = parse(params)?;
            edit(
                labels::SYNC_SHEET_PINS,
                edit::sync_sheet_pins(project, params.doc, params.id, params.remove),
            )
        }
        "schematic.snapToGrid" => {
            let params: DocIds = parse(params)?;
            edit(
                labels::SNAP_TO_GRID,
                edit::snap_to_grid(project, params.doc, &params.ids),
            )
        }
        "schematic.ercSettings" => {
            let settings = ew_schematic::erc::settings(project);
            let checks: Vec<Value> = ew_model::rules::ERC_CHECKS
                .iter()
                .map(|(code, default)| json!({"code": code, "default": default, "value": settings.severity(code)}))
                .collect();
            let kinds: Vec<&str> = ew_model::library::PinKind::ALL
                .iter()
                .map(|k| k.name())
                .collect();
            Ok(Outcome::Read(json!({
                "checks": checks,
                "kinds": kinds,
                "defaultMatrix": ew_model::rules::ERC_PIN_MATRIX,
                "matrix": settings.matrix(),
                "rules": rules_document(project),
            })))
        }
        "schematic.setErcSettings" => {
            let params: ErcSettingsParams = parse(params)?;
            let doc = rules_document(project)
                .ok_or_else(|| EngineError::not_found("the project has no rules document"))?;
            let mut settings = ew_model::rules::ErcSettings::default();
            settings.set_from(&params.severities, &params.matrix);
            let mut props = Map::new();
            props.insert("erc".to_owned(), to_value(&settings)?);
            Ok(Outcome::Requests {
                label: labels::ERC_SETTINGS,
                requests: vec![Request::SetProps { doc, props }],
            })
        }
        "schematic.netlist" => {
            let params: NetlistParams = parse(params)?;
            let connectivity = ew_schematic::analyze(project);
            let text = match params.format {
                NetlistFormat::Kicad => ew_schematic::export::kicad_netlist(project, &connectivity),
                NetlistFormat::Csv => ew_schematic::export::csv_netlist(&connectivity),
            };
            Ok(Outcome::Read(json!({"text": text})))
        }
        "schematic.bom" => {
            let connectivity = ew_schematic::analyze(project);
            let text = ew_schematic::export::bom_csv(project, &connectivity);
            Ok(Outcome::Read(json!({"text": text})))
        }
        other => Err(EngineError::new(
            crate::error::ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

/// The components of the project libraries, sorted by name.
fn components(project: &Project) -> Value {
    let mut seen = std::collections::BTreeSet::new();
    let mut list: Vec<Value> = Vec::new();
    for entry in &project.manifest.documents {
        let Some(ew_model::document::Document::Library(library)) = project.document(entry.id)
        else {
            continue;
        };
        for def in &library.components {
            if seen.insert(def.name.clone()) {
                list.push(json!({
                    "name": def.name,
                    "description": def.description,
                    "keywords": def.keywords,
                    "symbol": def.symbol,
                    "footprint": def.footprint,
                    "value": def.value,
                    "manufacturer": def.manufacturer,
                    "mpn": def.mpn,
                    "library": library.name,
                }));
            }
        }
    }
    list.sort_by(|a, b| {
        ew_schematic::connectivity::natural_cmp(
            a["name"].as_str().unwrap_or_default(),
            b["name"].as_str().unwrap_or_default(),
        )
    });
    Value::Array(list)
}

/// The first rules document of the project.
fn rules_document(project: &Project) -> Option<Id> {
    project
        .documents_of(ew_model::document::DocumentKind::Rules)
        .next()
        .map(|entry| entry.id)
}

/// The symbols of the project libraries (first library wins for equal
/// names), sorted by name.
fn symbols(project: &Project) -> Value {
    let mut seen = std::collections::BTreeSet::new();
    let mut list: Vec<Value> = Vec::new();
    for entry in &project.manifest.documents {
        let Some(ew_model::document::Document::Library(library)) = project.document(entry.id)
        else {
            continue;
        };
        for def in &library.symbols {
            if !seen.insert(def.name.clone()) {
                continue;
            }
            list.push(json!({
                "name": def.name,
                "description": def.description,
                "keywords": def.keywords,
                "prefix": def.prefix,
                "value": def.value,
                "footprint": def.footprint,
                "power": def.power,
                "units": def.units.len(),
                "pins": def.units.iter().map(|u| u.pins.len()).sum::<usize>(),
                "library": library.name,
            }));
        }
    }
    list.sort_by(|a, b| {
        ew_schematic::connectivity::natural_cmp(
            a["name"].as_str().unwrap_or_default(),
            b["name"].as_str().unwrap_or_default(),
        )
    });
    Value::Array(list)
}

/// The net of an object, pin or point of a sheet and its members on every
/// sheet.
fn net(project: &Project, params: &NetParams) -> Value {
    let connectivity = ew_schematic::analyze(project);
    let Some(instance) = connectivity.instance_of(params.doc) else {
        return json!({"net": null, "sheets": {}});
    };
    let net = params
        .pin
        .as_ref()
        .and_then(|pin| connectivity.net_of_pin(instance, pin.symbol, pin.index))
        .or_else(|| params.id.and_then(|id| connectivity.net_of(instance, id)))
        .or_else(|| params.at.and_then(|at| connectivity.net_at(instance, at)));
    let Some(net) = net else {
        return json!({"net": null, "sheets": {}});
    };
    let sheets: Map<String, Value> = connectivity
        .members(net)
        .into_iter()
        .map(|(doc, members)| {
            (
                doc.to_string(),
                json!({"items": members.items, "pins": members.pins}),
            )
        })
        .collect();
    json!({
        "net": connectivity.nets[net].name,
        "sheets": sheets,
        "places": net_places(project, &connectivity, net),
    })
}

/// Order of the places of a net within one sheet: names first, pins last.
fn place_rank(kind: &str) -> u8 {
    match kind {
        "globalLabel" => 0,
        "hierarchicalLabel" => 1,
        "label" => 2,
        "powerPort" => 3,
        "powerSymbol" => 4,
        "sheetPin" => 5,
        _ => 6,
    }
}

/// Where a net shows up: labels, power ports and symbols, sheet pins and
/// component pins, per sheet instance (top sheet first), for the net
/// navigator. Wires and junctions are left out.
fn net_places(
    project: &Project,
    connectivity: &ew_schematic::connectivity::Connectivity,
    net: usize,
) -> Vec<Value> {
    use ew_model::schematic::LabelKind;
    use ew_schematic::connectivity::ItemKind;
    let Some(entry) = connectivity.nets.get(net) else {
        return Vec::new();
    };
    let mut out: Vec<(usize, u8, String, Value)> = Vec::new();
    for &piece in &entry.pieces {
        let piece = &connectivity.pieces[piece];
        let instance = &connectivity.instances[piece.instance];
        let Some(ew_model::document::Document::Schematic(sheet)) = project.document(instance.sheet)
        else {
            continue;
        };
        let base = |kind: &str, id: Id, at: Point, text: &str| {
            json!({
                "doc": instance.sheet,
                "sheet": sheet.name,
                "path": instance.name,
                "kind": kind,
                "id": id,
                "at": at,
                "text": text,
            })
        };
        for item in &piece.items {
            let place = match item.kind {
                ItemKind::Label | ItemKind::GlobalLabel | ItemKind::HierarchicalLabel => {
                    sheet.labels.get(item.id).map(|label| {
                        let kind = match label.kind {
                            LabelKind::Local => "label",
                            LabelKind::Global => "globalLabel",
                            LabelKind::Hierarchical => "hierarchicalLabel",
                        };
                        (kind, label.text.clone())
                    })
                }
                ItemKind::PowerPort => sheet
                    .power_ports
                    .get(item.id)
                    .map(|port| ("powerPort", port.net.clone())),
                ItemKind::SheetPin => sheet.sheets.get(item.id).and_then(|symbol| {
                    let pin = symbol.pins.get(item.index?)?;
                    Some(("sheetPin", format!("{}: {}", symbol.name, pin.name)))
                }),
                _ => None,
            };
            if let Some((kind, text)) = place {
                out.push((
                    piece.instance,
                    place_rank(kind),
                    text.clone(),
                    base(kind, item.id, item.at, &text),
                ));
            }
        }
        for &pin in &piece.pins {
            let pin = &connectivity.pins[pin];
            let (kind, text) = if pin.power_symbol {
                ("powerSymbol", pin.value.clone())
            } else {
                ("pin", format!("{}.{}", pin.designator, pin.number))
            };
            let mut value = base(kind, pin.symbol, pin.at, &text);
            if let Some(map) = value.as_object_mut() {
                map.insert("pin".to_owned(), json!(pin.number));
                map.insert("name".to_owned(), json!(pin.name));
                map.insert("designator".to_owned(), json!(pin.designator));
            }
            out.push((piece.instance, place_rank(kind), text, value));
        }
    }
    out.sort_by(|a, b| {
        (a.0, a.1)
            .cmp(&(b.0, b.1))
            .then_with(|| ew_schematic::connectivity::natural_cmp(&a.2, &b.2))
    });
    out.into_iter().map(|(_, _, _, value)| value).collect()
}
