//! The `pcb.*` methods of the layout editor: design rules check, zone
//! filling, ratsnest and the design rules. The logic lives in `ew-pcb`;
//! this module finds the documents and turns results into change requests.

use std::collections::{BTreeMap, HashMap};

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::pcb::Pcb;
use ew_model::project::Project;
use ew_model::rules::{DRC_CHECKS, DrcSettings, ErcSeverity, Rules, RulesProps};
use ew_pcb::drc::{self, Report};
use ew_pcb::geom::{BBox, V};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::change::Request;
use crate::error::{EngineError, ErrorCode};

/// History labels of the board methods.
pub mod labels {
    /// `pcb.fillZones`.
    pub const FILL: &str = "fill-zones";
    /// `pcb.unfillZones`.
    pub const UNFILL: &str = "unfill-zones";
    /// `pcb.setRules`.
    pub const RULES: &str = "design-rules";
    /// `pcb.exclude`.
    pub const EXCLUDE: &str = "drc-exclusions";
    /// `pcb.syncApply`.
    pub const UPDATE_BOARD: &str = "update-board";
    /// `pcb.backApply`.
    pub const UPDATE_SCHEMATIC: &str = "update-schematic";
    /// `pcb.place`.
    pub const PLACE: &str = "place-footprint";
    /// `pcb.addTracks`.
    pub const ROUTE: &str = "route";
    /// `pcb.applyPreset`.
    pub const PRESET: &str = "board-preset";
    /// `pcb.importDxf`.
    pub const DXF: &str = "import-dxf";
    /// `pcb.addImage`.
    pub const IMAGE: &str = "add-image";
    /// `pcb.addQrCode`.
    pub const QR_CODE: &str = "add-qr-code";
    /// `pcb.importSession`.
    pub const SESSION: &str = "import-session";
    /// `pcb.importGerbers`.
    pub const GERBER: &str = "import-gerber";
    /// `pcb.tuneLength`.
    pub const TUNE: &str = "tune-length";
    /// `pcb.matchLengths`.
    pub const MATCH: &str = "match-lengths";
    /// `pcb.teardrops`.
    pub const TEARDROPS: &str = "teardrops";
    /// `pcb.teardrops` with `remove`.
    pub const REMOVE_TEARDROPS: &str = "remove-teardrops";
    /// `pcb.stitch`.
    pub const STITCH: &str = "stitch-vias";
    /// `pcb.stitch` with `remove`.
    pub const REMOVE_STITCH: &str = "remove-stitching";
    /// `pcb.copyChannel`.
    pub const COPY_CHANNEL: &str = "copy-channel";

    /// The label of a `pcb.edit` operation.
    #[must_use]
    pub const fn edit(op: &ew_pcb::edit::BoardEdit) -> &'static str {
        use ew_pcb::edit::BoardEdit;
        match op {
            BoardEdit::Move { .. } => "move",
            BoardEdit::Rotate { .. } => "rotate",
            BoardEdit::Flip { .. } => "flip",
            BoardEdit::Delete { .. } => "delete",
            BoardEdit::Lock { .. } => "lock",
            BoardEdit::Align { .. } => "align",
            BoardEdit::Distribute { .. } => "distribute",
            BoardEdit::SetLayer { .. } => "set-layer",
            BoardEdit::SetWidth { .. } => "set-width",
            BoardEdit::SetNet { .. } => "set-net",
            BoardEdit::DragTrack { .. } => "drag-track",
            BoardEdit::Group { .. } => "group",
            BoardEdit::Ungroup { .. } => "ungroup",
        }
    }
}

/// What a method does.
pub enum Outcome {
    /// A reply without changes.
    Read(Value),
    /// Requests to commit as one undo step, and the reply to add to the
    /// commit result.
    Commit {
        /// History label.
        label: &'static str,
        /// Requests.
        requests: Vec<Request>,
        /// Extra reply fields.
        reply: Value,
    },
}

/// The last design rules check of every board, for online checks of a
/// region.
#[derive(Debug, Default)]
pub struct DrcCache {
    reports: HashMap<Id, Report>,
    /// Findings of the enclosure check for the next `pcb.drc` (computed by
    /// the engine, which holds the meshes).
    pub mechanical: Vec<drc::Finding>,
}

impl DrcCache {
    /// Forgets every report (a project was opened or closed).
    pub fn clear(&mut self) {
        self.reports.clear();
        self.mechanical.clear();
    }
}

fn parse<T: DeserializeOwned>(params: &Value) -> Result<T, EngineError> {
    let value = if params.is_null() {
        Value::Object(Map::new())
    } else {
        params.clone()
    };
    serde_json::from_value(value).map_err(|error| EngineError::invalid_params(error.to_string()))
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(EngineError::internal)
}

/// The board with this id.
fn board(project: &Project, doc: Id) -> Result<&Pcb, EngineError> {
    match project.document(doc) {
        Some(Document::Pcb(pcb)) => Ok(pcb),
        Some(_) => Err(EngineError::invalid_params(format!("{doc} is not a board"))),
        None => Err(EngineError::not_found(format!("no document {doc}"))),
    }
}

/// The first rules document of the project.
fn rules(project: &Project) -> Option<(Id, &Rules)> {
    let entry = project.documents_of(DocumentKind::Rules).next()?;
    match project.document(entry.id) {
        Some(Document::Rules(rules)) => Some((entry.id, rules)),
        _ => None,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DrcParams {
    doc: Id,
    /// Only check this area (`[x0, y0, x1, y1]` in nm), merged with the
    /// previous report of the board.
    #[serde(default)]
    region: Option<[i64; 4]>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ZonesParams {
    doc: Id,
    #[serde(default)]
    zones: Option<Vec<Id>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocParams {
    #[serde(default)]
    doc: Option<Id>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetRulesParams {
    props: RulesProps,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImpedanceParams {
    doc: Id,
    /// Rules that are still being edited (instead of the saved ones).
    #[serde(default)]
    rules: Option<RulesProps>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValidateParams {
    condition: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SyncParams {
    doc: Id,
    /// Indices of the changes to apply (all if missing).
    #[serde(default)]
    changes: Option<Vec<usize>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProbeParams {
    doc: Id,
    #[serde(default)]
    symbols: Vec<Id>,
    #[serde(default)]
    footprints: Vec<Id>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditParams {
    doc: Id,
    #[serde(flatten)]
    edit: ew_pcb::edit::BoardEdit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlaceParams {
    doc: Id,
    footprint: String,
    at: ew_model::units::Point,
    #[serde(default)]
    rotation: ew_model::units::Angle,
    #[serde(default)]
    side: ew_model::layer::Side,
    #[serde(default)]
    designator: Option<String>,
    #[serde(default)]
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteStartParams {
    doc: Id,
    at: [f64; 2],
    layer: ew_model::layer::CopperLayer,
    /// Also find the other net of a differential pair and where it begins.
    #[serde(default)]
    pair: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RoutePairParams {
    doc: Id,
    layer: ew_model::layer::CopperLayer,
    net: Id,
    partner: Id,
    width: f64,
    gap: f64,
    start: [f64; 2],
    partner_start: [f64; 2],
    /// The centre line fixed so far.
    #[serde(default)]
    centre: Vec<[f64; 2]>,
    target: [f64; 2],
    #[serde(default)]
    diagonal_first: bool,
    #[serde(default)]
    corners: ew_pcb::route::Corners,
}

/// The tracks of the other net of a differential pair, added together with
/// the route.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PairTracks {
    net: Id,
    segments: Vec<Segment>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteParams {
    doc: Id,
    layer: ew_model::layer::CopperLayer,
    #[serde(default)]
    net: Option<Id>,
    width: f64,
    start: [f64; 2],
    target: [f64; 2],
    #[serde(default)]
    diagonal_first: bool,
    /// `"45"` (default: only 45° bends) or `"90"` (right angles allowed).
    #[serde(default)]
    corners: ew_pcb::route::Corners,
    /// Direction of the segment that ends at the start.
    #[serde(default)]
    previous: Option<[f64; 2]>,
    /// Check whether a via of this diameter fits at the end.
    #[serde(default)]
    via: Option<f64>,
    /// The layers of that via (default: through the board).
    #[serde(default)]
    via_layers: Option<[ew_model::layer::CopperLayer; 2]>,
    /// `walkaround` (default) or `shove`: push the tracks of other nets
    /// aside.
    #[serde(default)]
    mode: RouteMode,
    /// Segments already fixed on this route (they push too).
    #[serde(default)]
    fixed: Vec<Segment>,
    /// Vias already placed on this route.
    #[serde(default)]
    vias: Vec<NewVia>,
}

/// How a route deals with the tracks of other nets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RouteMode {
    /// Around them.
    #[default]
    Walkaround,
    /// Push them aside.
    Shove,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImpedanceCalcParams {
    doc: Id,
    layer: ew_model::layer::CopperLayer,
    #[serde(default)]
    differential: bool,
    width: f64,
    #[serde(default)]
    gap: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TuneParams {
    doc: Id,
    track: Id,
    /// How much longer the net gets (nm) …
    #[serde(default)]
    extra: Option<f64>,
    /// … or the length the net should get.
    #[serde(default)]
    length: Option<f64>,
    #[serde(default)]
    spacing: Option<f64>,
    #[serde(default)]
    amplitude: Option<f64>,
    #[serde(default)]
    side: ew_pcb::length::MeanderSide,
    /// Change the board (otherwise only show the result).
    #[serde(default)]
    apply: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MatchParams {
    doc: Id,
    #[serde(default)]
    spacing: Option<f64>,
    #[serde(default)]
    amplitude: Option<f64>,
    #[serde(default)]
    side: ew_pcb::length::MeanderSide,
    #[serde(default)]
    apply: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CopyChannelParams {
    doc: Id,
    /// The repeated sheet.
    sheet: Id,
    /// The instance whose layout is copied.
    from: String,
    /// The instances that get it (all others by default).
    #[serde(default)]
    to: Option<Vec<String>>,
    /// Copy the tracks and vias too.
    #[serde(default = "yes")]
    routing: bool,
    #[serde(default)]
    apply: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StitchParams {
    doc: Id,
    /// Stitch this zone …
    #[serde(default)]
    zone: Option<Id>,
    /// … or fence these tracks.
    #[serde(default)]
    tracks: Vec<Id>,
    /// The net of the vias (a zone's own net by default).
    #[serde(default)]
    net: Option<Id>,
    #[serde(default)]
    pitch: Option<f64>,
    #[serde(default)]
    diameter: Option<f64>,
    #[serde(default)]
    drill: Option<f64>,
    /// Fence: from the track's edge to the via's edge.
    #[serde(default)]
    gap: Option<f64>,
    #[serde(default)]
    staggered: bool,
    /// Remove the free vias of the net there instead.
    #[serde(default)]
    remove: bool,
    #[serde(default)]
    apply: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TeardropParams {
    doc: Id,
    /// Remove the teardrops instead of making them.
    #[serde(default)]
    remove: bool,
    /// Settings to use and keep in the design rules.
    #[serde(default)]
    settings: Option<ew_model::rules::TeardropSettings>,
}

/// The shape of meanders; missing values come from the track and its class.
struct MeanderParams {
    spacing: Option<f64>,
    amplitude: Option<f64>,
    side: ew_pcb::length::MeanderSide,
}

impl MeanderParams {
    fn options(&self, base: ew_pcb::length::MeanderOptions) -> ew_pcb::length::MeanderOptions {
        ew_pcb::length::MeanderOptions {
            spacing: self.spacing.filter(|s| *s > 0.0).unwrap_or(base.spacing),
            max_amplitude: self
                .amplitude
                .filter(|a| *a > 0.0)
                .unwrap_or(base.max_amplitude),
            side: self.side,
        }
    }

    const fn is_set(&self) -> bool {
        self.spacing.is_some()
            || self.amplitude.is_some()
            || !matches!(self.side, ew_pcb::length::MeanderSide::Auto)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ViaFitsParams {
    doc: Id,
    #[serde(default)]
    net: Option<Id>,
    at: [f64; 2],
    diameter: f64,
    /// First and last layer (default: through the board).
    #[serde(default)]
    layers: Option<[ew_model::layer::CopperLayer; 2]>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Segment {
    layer: ew_model::layer::CopperLayer,
    points: Vec<ew_model::units::Point>,
    /// Its own track width (an impedance profile sets one per layer);
    /// otherwise the width of the request.
    #[serde(default)]
    width: Option<ew_model::units::Nm>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewVia {
    at: ew_model::units::Point,
    diameter: ew_model::units::Nm,
    drill: ew_model::units::Nm,
    #[serde(default)]
    from: Option<ew_model::layer::CopperLayer>,
    #[serde(default)]
    to: Option<ew_model::layer::CopperLayer>,
    /// A laser-drilled microvia.
    #[serde(default)]
    micro: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AddTracksParams {
    doc: Id,
    #[serde(default)]
    net: Option<Id>,
    width: ew_model::units::Nm,
    #[serde(default)]
    segments: Vec<Segment>,
    #[serde(default)]
    vias: Vec<NewVia>,
    /// `shove` pushes the tracks of other nets aside, as the preview did.
    #[serde(default)]
    mode: RouteMode,
    /// The bends the pushed tracks may have (default `"45"`).
    #[serde(default)]
    corners: ew_pcb::route::Corners,
    /// The other net of a differential pair with its own segments.
    #[serde(default)]
    pair: Option<PairTracks>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PresetParams {
    doc: Id,
    preset: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DxfPreviewParams {
    path: String,
    #[serde(default)]
    scale: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageParams {
    doc: Id,
    /// Board layer, for example `silk.top` or `cu.top`.
    layer: ew_model::layer::Layer,
    /// The picture from the top: `#` or `1` set, `.` or `0` clear.
    rows: Vec<String>,
    /// Size of one pixel.
    pixel: ew_model::units::Nm,
    /// Middle of the picture.
    center: ew_model::units::Point,
    /// Draw the clear pixels instead.
    #[serde(default)]
    invert: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QrParams {
    doc: Id,
    layer: ew_model::layer::Layer,
    /// What the code says (a web address, a serial number …).
    text: String,
    /// Width of the whole code, quiet zone included.
    size: ew_model::units::Nm,
    center: ew_model::units::Point,
    #[serde(default)]
    ecc: ew_pcb::bitmap::QrEcc,
    /// Draw the light modules (and the quiet zone): a dark code on a light
    /// print, as on silkscreen.
    #[serde(default)]
    invert: bool,
    /// Modules of quiet zone around the code.
    #[serde(default)]
    quiet: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QrPreviewParams {
    text: String,
    #[serde(default)]
    ecc: ew_pcb::bitmap::QrEcc,
    #[serde(default)]
    invert: bool,
    #[serde(default)]
    quiet: usize,
}

/// `pcb.qrCode`: the modules of a QR code as rows, for a preview (no
/// project needed).
pub(crate) fn qr_preview(params: &Value) -> Result<Value, EngineError> {
    let params: QrPreviewParams = parse(params)?;
    if params.text.is_empty() {
        return Err(EngineError::invalid_params("the text is empty"));
    }
    let bitmap = ew_pcb::bitmap::Bitmap::qr(&params.text, params.ecc, params.quiet.min(10))
        .map_err(|e| EngineError::invalid_params(e.to_string()))?;
    let bitmap = if params.invert {
        bitmap.inverted()
    } else {
        bitmap
    };
    let rows: Vec<String> = (0..bitmap.height())
        .map(|y| {
            (0..bitmap.width())
                .map(|x| {
                    let (x, y) = (i64::try_from(x).unwrap_or(0), i64::try_from(y).unwrap_or(0));
                    if bitmap.get(x, y) { '#' } else { '.' }
                })
                .collect()
        })
        .collect();
    Ok(json!({"rows": rows, "modules": bitmap.width()}))
}

/// Filled polygons of a picture, one undo step, grouped so that they move
/// together.
fn picture_outcome(
    label: &'static str,
    doc: Id,
    layer: ew_model::layer::Layer,
    bitmap: &ew_pcb::bitmap::Bitmap,
    pixel: ew_model::units::Nm,
    center: ew_model::units::Point,
) -> Result<Outcome, EngineError> {
    if pixel < 1_000 {
        return Err(EngineError::invalid_params("`pixel` must be at least 1 µm"));
    }
    let polygons = ew_pcb::bitmap::polygons(bitmap, pixel, center);
    if polygons.is_empty() {
        return Err(EngineError::invalid_params("the picture has no set pixels"));
    }
    let mut requests = Vec::new();
    let mut select = Vec::new();
    for points in polygons {
        let shape = ew_model::pcb::BoardShape {
            id: Id::random(),
            layer,
            geometry: ew_model::shape::Geometry::Polygon { points },
            width: 0,
            filled: true,
        };
        select.push(shape.id);
        requests.push(Request::Insert {
            doc,
            object: Object::BoardShape(shape),
        });
    }
    let group = (select.len() > 1).then(|| {
        let group = ew_model::pcb::Group {
            id: Id::random(),
            name: String::new(),
            members: select.clone(),
        };
        let id = group.id;
        requests.push(Request::Insert {
            doc,
            object: Object::Group(group),
        });
        id
    });
    let pixels = |n: usize| i64::try_from(n).unwrap_or(0) * pixel;
    Ok(Outcome::Commit {
        label,
        requests,
        reply: json!({
            "select": select,
            "shapes": select.len(),
            "group": group,
            "width": pixels(bitmap.width()),
            "height": pixels(bitmap.height()),
        }),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DxfParams {
    doc: Id,
    path: String,
    /// Board layer, for example `outline` or `mech.1`.
    layer: ew_model::layer::Layer,
    /// Only these DXF layers (all if missing).
    #[serde(default)]
    layers: Option<Vec<String>>,
    /// Remove the shapes on the target layer first.
    #[serde(default)]
    replace: bool,
    /// nm per drawing unit (the file's units if missing).
    #[serde(default)]
    scale: Option<f64>,
    /// Move the drawing so that its lower left corner lies here.
    #[serde(default)]
    origin: Option<ew_model::units::Point>,
    /// Line width.
    #[serde(default)]
    width: Option<ew_model::units::Nm>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GerberPreviewParams {
    doc: Id,
    /// Gerber and Excellon files.
    paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GerberFileParams {
    path: String,
    /// Board layer for the file's drawing (`user.3`, `mech.2`, …).
    layer: ew_model::layer::Layer,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GerberImportParams {
    doc: Id,
    files: Vec<GerberFileParams>,
    /// Move the drawings so that the lower left corner of all of them lies
    /// here (as in the files if missing).
    #[serde(default)]
    origin: Option<ew_model::units::Point>,
    /// Remove the shapes already on the target layers.
    #[serde(default)]
    replace: bool,
}

/// A Gerber or Excellon file: its name, kind and image.
fn read_gerber(path: &str) -> Result<(String, &'static str, ew_output::read::Image), EngineError> {
    let bytes = std::fs::read(path)
        .map_err(|error| EngineError::invalid_params(format!("cannot read {path}: {error}")))?;
    let text = String::from_utf8_lossy(&bytes);
    let name = std::path::Path::new(path)
        .file_name()
        .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned());
    let kind = match ew_output::read::detect(&text) {
        Some(ew_output::read::FileKind::Gerber) => "gerber",
        Some(ew_output::read::FileKind::Excellon) => "drill",
        None => {
            return Err(EngineError::invalid_params(format!(
                "{name} is not a Gerber or Excellon file"
            )));
        }
    };
    let image = ew_output::read::read(&text).ok_or_else(|| {
        EngineError::invalid_params(format!("{name} is not a Gerber or Excellon file"))
    })?;
    Ok((name, kind, image))
}

/// User layers without shapes, for reference drawings.
fn free_user_layers(pcb: &Pcb) -> Vec<ew_model::layer::Layer> {
    use ew_model::layer::Layer;
    let used: std::collections::BTreeSet<Layer> = pcb
        .shapes
        .iter()
        .map(|s| s.layer)
        .chain(pcb.texts.iter().map(|t| t.layer))
        .collect();
    (1..=32)
        .map(Layer::User)
        .filter(|layer| !used.contains(layer))
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionParams {
    doc: Id,
    /// The Specctra session file (SES) of an autorouter.
    path: String,
}

fn edit_failed(error: &ew_pcb::edit::EditError) -> EngineError {
    match error {
        ew_pcb::edit::EditError::Unknown(_) => EngineError::not_found(error.to_string()),
        ew_pcb::edit::EditError::Invalid(_) => EngineError::invalid_params(error.to_string()),
    }
}

fn read_dxf(path: &str, scale: Option<f64>) -> Result<ew_pcb::dxf::Drawing, EngineError> {
    let bytes = std::fs::read(path).map_err(|error| EngineError::io(path, &error))?;
    let text = String::from_utf8_lossy(&bytes);
    ew_pcb::dxf::read(&text, scale).map_err(|error| EngineError::invalid_params(error.0))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExcludeParams {
    doc: Id,
    keys: Vec<String>,
    #[serde(default = "yes")]
    excluded: bool,
}

const fn yes() -> bool {
    true
}

/// Runs a `pcb.*` method.
///
/// # Errors
///
/// Unknown documents, invalid parameters, or an unknown method.
#[allow(clippy::too_many_lines, reason = "one match over the methods")]
pub fn call(
    project: &Project,
    method: &str,
    params: &Value,
    cache: &mut DrcCache,
) -> Result<Outcome, EngineError> {
    match method {
        "pcb.drc" => {
            let params: DrcParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let rules = rules(project).map(|(_, r)| r);
            #[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
            let region = params.region.map(|[x0, y0, x1, y1]| {
                BBox::of(V::new(x0 as f64, y0 as f64)).with(V::new(x1 as f64, y1 as f64))
            });
            let mut extra = parity(project, pcb, rules);
            extra.append(&mut cache.mechanical);
            let report = match (region, cache.reports.get(&params.doc)) {
                (Some(region), Some(previous)) => {
                    let fresh = drc::check(
                        pcb,
                        rules,
                        &drc::Options {
                            region: Some(region),
                            extra,
                        },
                    );
                    let reach = ew_pcb::rules::RuleSet::new(rules).max_clearance();
                    drc::merge(previous, fresh, region, reach)
                }
                _ => drc::check(
                    pcb,
                    rules,
                    &drc::Options {
                        region: None,
                        extra,
                    },
                ),
            };
            let value = to_value(&report)?;
            cache.reports.insert(params.doc, report);
            Ok(Outcome::Read(value))
        }
        "pcb.nets" => {
            // The net inspector: every net with its class, routed length,
            // tracks, vias, pads and unrouted connections.
            let params: DocParams = parse(params)?;
            let doc = params
                .doc
                .ok_or_else(|| EngineError::invalid_params("missing doc"))?;
            let pcb = board(project, doc)?;
            let mut set = ew_pcb::rules::RuleSet::new(rules(project).map(|(_, r)| r));
            set.cache_classes(pcb.nets.iter().map(|n| (n.name.clone(), n.class.clone())));
            let lengths: HashMap<Id, f64> = ew_pcb::length::report(pcb, &set)
                .nets
                .into_iter()
                .map(|n| (n.net, n.length))
                .collect();
            let mut tracks: HashMap<Id, usize> = HashMap::new();
            for track in &pcb.tracks {
                if let Some(net) = track.net {
                    *tracks.entry(net).or_default() += 1;
                }
            }
            let mut vias: HashMap<Id, usize> = HashMap::new();
            for via in &pcb.vias {
                if let Some(net) = via.net {
                    *vias.entry(net).or_default() += 1;
                }
            }
            let mut pads: HashMap<Id, usize> = HashMap::new();
            for fp in &pcb.footprints {
                for pad in &fp.pads {
                    if let Some(net) = pad.net {
                        *pads.entry(net).or_default() += 1;
                    }
                }
            }
            let items = ew_pcb::items::Board::new(pcb);
            let mut unrouted: HashMap<Id, usize> = HashMap::new();
            for airwire in ew_pcb::connectivity::analyze(&items).airwires {
                *unrouted.entry(airwire.net).or_default() += 1;
            }
            let nets: Vec<Value> = pcb
                .nets
                .iter()
                .map(|net| {
                    json!({
                        "id": net.id,
                        "name": net.name,
                        "class": set.class_name(&net.name, &net.class),
                        "length": lengths.get(&net.id).copied().unwrap_or(0.0),
                        "tracks": tracks.get(&net.id).copied().unwrap_or(0),
                        "vias": vias.get(&net.id).copied().unwrap_or(0),
                        "pads": pads.get(&net.id).copied().unwrap_or(0),
                        "unrouted": unrouted.get(&net.id).copied().unwrap_or(0),
                    })
                })
                .collect();
            Ok(Outcome::Read(json!({ "nets": nets })))
        }
        "pcb.ratsnest" => {
            let params: DocParams = parse(params)?;
            let doc = params
                .doc
                .ok_or_else(|| EngineError::invalid_params("missing doc"))?;
            let pcb = board(project, doc)?;
            let board = ew_pcb::items::Board::new(pcb);
            let connectivity = ew_pcb::connectivity::analyze(&board);
            Ok(Outcome::Read(json!({
                "airwires": connectivity.airwires,
                "unrouted": connectivity.airwires.len(),
            })))
        }
        "pcb.fillZones" | "pcb.unfillZones" => {
            let params: ZonesParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let fill = method == "pcb.fillZones";
            let fills: Vec<(Id, Vec<ew_model::pcb::FilledArea>)> = if fill {
                ew_pcb::fill::fill(pcb, rules(project).map(|(_, r)| r), params.zones.as_deref())
                    .into_iter()
                    .map(|filled| (filled.zone, filled.areas))
                    .collect()
            } else {
                pcb.zones
                    .iter()
                    .filter(|z| params.zones.as_ref().is_none_or(|ids| ids.contains(&z.id)))
                    .map(|z| (z.id, Vec::new()))
                    .collect()
            };
            let mut requests = Vec::new();
            let mut areas = 0;
            for (id, filled) in fills {
                let Some(zone) = pcb.zones.get(id) else {
                    continue;
                };
                if zone.filled == filled {
                    continue;
                }
                areas += filled.len();
                let mut zone = zone.clone();
                zone.filled = filled;
                requests.push(Request::Update {
                    doc: params.doc,
                    object: Object::Zone(zone),
                });
            }
            let zones = requests.len();
            if requests.is_empty() {
                return Ok(Outcome::Read(json!({"seq": null, "zones": 0, "areas": 0})));
            }
            Ok(Outcome::Commit {
                label: if fill { labels::FILL } else { labels::UNFILL },
                requests,
                reply: json!({"zones": zones, "areas": areas}),
            })
        }
        "pcb.syncPreview" => {
            let params: DocParams = parse(params)?;
            let doc = params
                .doc
                .ok_or_else(|| EngineError::invalid_params("missing doc"))?;
            let pcb = board(project, doc)?;
            let connectivity = ew_schematic::analyze(project);
            let diff = ew_pcb::sync::diff(project, &connectivity, pcb);
            let changes: Vec<Value> = diff
                .changes
                .iter()
                .enumerate()
                .map(|(index, change)| {
                    let mut value = serde_json::to_value(change).unwrap_or(Value::Null);
                    if let Some(map) = value.as_object_mut() {
                        map.insert("index".into(), json!(index));
                        map.insert("description".into(), json!(change.describe()));
                    }
                    value
                })
                .collect();
            Ok(Outcome::Read(
                json!({"changes": changes, "problems": diff.problems}),
            ))
        }
        "pcb.syncApply" => {
            let params: SyncParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let connectivity = ew_schematic::analyze(project);
            let diff = ew_pcb::sync::diff(project, &connectivity, pcb);
            let selected: Vec<usize> = params
                .changes
                .unwrap_or_else(|| (0..diff.changes.len()).collect());
            let edit =
                ew_pcb::sync::apply(project, &connectivity, params.doc, pcb, &diff, &selected);
            if edit.changes.is_empty() {
                return Ok(Outcome::Read(
                    json!({"seq": null, "select": [], "changes": 0}),
                ));
            }
            let count = edit.changes.len();
            Ok(Outcome::Commit {
                label: labels::UPDATE_BOARD,
                requests: crate::schematic::requests(&edit),
                reply: json!({"select": edit.select, "changes": count}),
            })
        }
        "pcb.backPreview" => {
            let params: DocParams = parse(params)?;
            let doc = params
                .doc
                .ok_or_else(|| EngineError::invalid_params("missing doc"))?;
            let pcb = board(project, doc)?;
            let connectivity = ew_schematic::analyze(project);
            let changes: Vec<Value> = ew_pcb::sync::back_diff(project, &connectivity, pcb)
                .iter()
                .enumerate()
                .map(|(index, change)| {
                    let mut value = serde_json::to_value(change).unwrap_or(Value::Null);
                    if let Some(map) = value.as_object_mut() {
                        map.insert("index".into(), json!(index));
                        map.insert("description".into(), json!(change.describe()));
                    }
                    value
                })
                .collect();
            Ok(Outcome::Read(json!({"changes": changes})))
        }
        "pcb.backApply" => {
            let params: SyncParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let connectivity = ew_schematic::analyze(project);
            let changes = ew_pcb::sync::back_diff(project, &connectivity, pcb);
            let selected: Vec<usize> = params
                .changes
                .unwrap_or_else(|| (0..changes.len()).collect());
            let edit = ew_pcb::sync::back_apply(project, &connectivity, &changes, &selected);
            if edit.changes.is_empty() {
                return Ok(Outcome::Read(json!({"seq": null, "changes": 0})));
            }
            let count = edit.changes.len();
            Ok(Outcome::Commit {
                label: labels::UPDATE_SCHEMATIC,
                requests: crate::schematic::requests(&edit),
                reply: json!({"changes": count}),
            })
        }
        "pcb.crossProbe" => {
            let params: ProbeParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let connectivity = ew_schematic::analyze(project);
            let footprints =
                ew_pcb::sync::footprints_of(project, &connectivity, pcb, &params.symbols);
            let symbols: Vec<Value> =
                ew_pcb::sync::symbols_of(project, &connectivity, pcb, &params.footprints)
                    .into_iter()
                    .map(|(sheet, id)| json!({"doc": sheet, "id": id}))
                    .collect();
            Ok(Outcome::Read(
                json!({"footprints": footprints, "symbols": symbols}),
            ))
        }
        "pcb.rules" => {
            let params: DocParams = parse(params)?;
            Ok(Outcome::Read(rules_info(project, params.doc)?))
        }
        "pcb.setRules" => {
            let params: SetRulesParams = parse(params)?;
            let Value::Object(props) = to_value(&params.props)? else {
                return Err(EngineError::internal("rules are not an object"));
            };
            let requests = if let Some((doc, _)) = rules(project) {
                vec![Request::SetProps { doc, props }]
            } else {
                let doc = Id::random();
                vec![
                    Request::AddDocument {
                        kind: DocumentKind::Rules,
                        name: params.props.name.clone(),
                        path: None,
                        id: Some(doc),
                    },
                    Request::SetProps { doc, props },
                ]
            };
            Ok(Outcome::Commit {
                label: labels::RULES,
                requests,
                reply: json!({}),
            })
        }
        "pcb.edit" => {
            let params: EditParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let edit = ew_pcb::edit::apply(params.doc, pcb, &params.edit)
                .map_err(|error| edit_failed(&error))?;
            if edit.changes.is_empty() {
                return Ok(Outcome::Read(
                    json!({"seq": null, "select": edit.select, "changes": 0}),
                ));
            }
            let count = edit.changes.len();
            Ok(Outcome::Commit {
                label: labels::edit(&params.edit),
                requests: crate::schematic::requests(&edit),
                reply: json!({"select": edit.select, "changes": count}),
            })
        }
        "pcb.editPreview" => {
            let params: EditParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let edit = ew_pcb::edit::apply(params.doc, pcb, &params.edit)
                .map_err(|error| edit_failed(&error))?;
            Ok(Outcome::Read(track_preview(&edit.changes)))
        }
        "pcb.place" => {
            let params: PlaceParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let def = ew_pcb::sync::footprint_def(project, &params.footprint).ok_or_else(|| {
                EngineError::not_found(format!("no footprint {}", params.footprint))
            })?;
            let designator = params.designator.unwrap_or_else(|| {
                let taken: std::collections::HashSet<&str> = pcb
                    .footprints
                    .iter()
                    .map(|f| f.designator.as_str())
                    .collect();
                (1..=taken.len() + 1)
                    .map(|n| format!("X{n}"))
                    .find(|d| !taken.contains(d.as_str()))
                    .unwrap_or_default()
            });
            let mut fp = ew_pcb::sync::instance(def, &designator, &params.value, params.at);
            fp.rotation = params.rotation;
            fp.side = params.side;
            let id = fp.id;
            Ok(Outcome::Commit {
                label: labels::PLACE,
                requests: vec![Request::Insert {
                    doc: params.doc,
                    object: Object::Footprint(fp),
                }],
                reply: json!({"select": [id], "designator": designator}),
            })
        }
        "pcb.routeStart" => {
            let params: RouteStartParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let router = ew_pcb::route::Router::new(pcb, rules(project).map(|(_, r)| r));
            let start = router.start(V::new(params.at[0], params.at[1]), params.layer);
            let mut value = to_value(&start)?;
            if params.pair {
                let partner = start
                    .net
                    .and_then(|net| ew_pcb::pairs::partner(pcb, net))
                    .and_then(|partner| {
                        let at = ew_pcb::pairs::partner_start(
                            &router,
                            partner,
                            V::new(start.at[0], start.at[1]),
                            params.layer,
                        )?;
                        let name = pcb.nets.get(partner)?.name.clone();
                        Some(json!({"net": partner, "netName": name, "at": [at.x, at.y]}))
                    });
                if let Some(map) = value.as_object_mut() {
                    map.insert("partner".into(), partner.unwrap_or(Value::Null));
                }
            }
            Ok(Outcome::Read(value))
        }
        "pcb.routePair" => {
            let params: RoutePairParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            if params.width <= 0.0 || params.gap <= 0.0 {
                return Err(EngineError::invalid_params(
                    "the width and the gap must be positive",
                ));
            }
            let router = ew_pcb::route::Router::new(pcb, rules(project).map(|(_, r)| r));
            let request = ew_pcb::pairs::PairRequest {
                layer: params.layer,
                net: params.net,
                partner: params.partner,
                width: params.width,
                gap: params.gap,
                start: V::new(params.start[0], params.start[1]),
                partner_start: V::new(params.partner_start[0], params.partner_start[1]),
                centre: params.centre.iter().map(|p| V::new(p[0], p[1])).collect(),
                target: V::new(params.target[0], params.target[1]),
                diagonal_first: params.diagonal_first,
                corners: params.corners,
            };
            Ok(Outcome::Read(to_value(&router.route_pair(&request))?))
        }
        "pcb.lengths" => {
            let params: DocParams = parse(params)?;
            let doc = params
                .doc
                .ok_or_else(|| EngineError::invalid_params("missing doc"))?;
            let pcb = board(project, doc)?;
            let mut set = ew_pcb::rules::RuleSet::new(rules(project).map(|(_, r)| r));
            set.cache_classes(pcb.nets.iter().map(|n| (n.name.clone(), n.class.clone())));
            Ok(Outcome::Read(to_value(&ew_pcb::length::report(pcb, &set))?))
        }
        "pcb.tuneLength" => {
            let params: TuneParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let track = pcb
                .tracks
                .get(params.track)
                .ok_or_else(|| EngineError::not_found(format!("no track {}", params.track)))?;
            let net = track
                .net
                .ok_or_else(|| EngineError::invalid_params("the track has no net"))?;
            let before = ew_pcb::length::net_length(pcb, net);
            let extra = match (params.extra, params.length) {
                (Some(extra), None) => extra,
                (None, Some(length)) => length - before,
                _ => {
                    return Err(EngineError::invalid_params("give either extra or length"));
                }
            };
            let router = ew_pcb::route::Router::new(pcb, rules(project).map(|(_, r)| r));
            let class = router.class_of(track.net);
            #[allow(clippy::cast_precision_loss, reason = "track sizes fit f64")]
            let base = ew_pcb::length::MeanderOptions::for_track(
                track.width as f64,
                class.clearance as f64,
            );
            let tuned = router
                .tune(
                    track,
                    extra,
                    &MeanderParams {
                        spacing: params.spacing,
                        amplitude: params.amplitude,
                        side: params.side,
                    }
                    .options(base),
                )
                .ok_or_else(|| {
                    EngineError::invalid_params(
                        "no meander fits on this track: it is too short, curved, locked or hemmed in",
                    )
                })?;
            let reply = json!({
                "extra": tuned.extra,
                "length": before + tuned.extra,
                "reached": tuned.extra + 0.5 >= extra,
            });
            if !params.apply {
                return Ok(Outcome::Read(tuned_preview(
                    reply,
                    &[tuned.track],
                    &tuned.tracks,
                )));
            }
            let mut requests = vec![Request::Delete {
                doc: params.doc,
                id: tuned.track,
            }];
            let select: Vec<Id> = tuned.tracks.iter().map(|t| t.id).collect();
            requests.extend(tuned.tracks.into_iter().map(|track| Request::Insert {
                doc: params.doc,
                object: Object::Track(track),
            }));
            let mut reply = reply;
            if let Some(map) = reply.as_object_mut() {
                map.insert("select".into(), json!(select));
            }
            Ok(Outcome::Commit {
                label: labels::TUNE,
                requests,
                reply,
            })
        }
        "pcb.matchLengths" => {
            let params: MatchParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let saved = rules(project).map(|(_, r)| r);
            let mut set = ew_pcb::rules::RuleSet::new(saved);
            set.cache_classes(pcb.nets.iter().map(|n| (n.name.clone(), n.class.clone())));
            let report = ew_pcb::length::report(pcb, &set);
            let wanted = ew_pcb::length::shortfalls(&report);
            let given = MeanderParams {
                spacing: params.spacing,
                amplitude: params.amplitude,
                side: params.side,
            };
            #[allow(clippy::cast_precision_loss, reason = "track sizes fit f64")]
            let shape = given.is_set().then(|| {
                let width = pcb.tracks.iter().map(|t| t.width).max().unwrap_or(0) as f64;
                let clearance = set.constraints.min_clearance as f64;
                given.options(ew_pcb::length::MeanderOptions::for_track(width, clearance))
            });
            let done = ew_pcb::length::lengthen(pcb, saved, &wanted, shape);
            let nets: Vec<Value> = done
                .nets
                .iter()
                .map(|(net, wanted, gained)| {
                    json!({
                        "net": net,
                        "name": pcb.nets.get(*net).map_or("", |n| n.name.as_str()),
                        "wanted": wanted,
                        "gained": gained,
                        "reached": gained + 0.5 >= *wanted,
                    })
                })
                .collect();
            let reply = json!({ "nets": nets });
            if !params.apply {
                return Ok(Outcome::Read(tuned_preview(
                    reply,
                    &done.removed,
                    &done.added,
                )));
            }
            if done.added.is_empty() {
                return Ok(Outcome::Read(reply));
            }
            let mut requests: Vec<Request> = done
                .removed
                .iter()
                .map(|id| Request::Delete {
                    doc: params.doc,
                    id: *id,
                })
                .collect();
            requests.extend(done.added.into_iter().map(|track| Request::Insert {
                doc: params.doc,
                object: Object::Track(track),
            }));
            Ok(Outcome::Commit {
                label: labels::MATCH,
                requests,
                reply,
            })
        }
        "pcb.channels" => {
            let params: DocParams = parse(params)?;
            let doc = params
                .doc
                .ok_or_else(|| EngineError::invalid_params("missing doc"))?;
            let pcb = board(project, doc)?;
            let connectivity = ew_schematic::analyze(project);
            Ok(Outcome::Read(to_value(&ew_pcb::channels::channels(
                project,
                &connectivity,
                pcb,
            ))?))
        }
        "pcb.copyChannel" => {
            let params: CopyChannelParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let connectivity = ew_schematic::analyze(project);
            let channels = ew_pcb::channels::channels(project, &connectivity, pcb);
            let channel = channels
                .iter()
                .find(|c| c.sheet == params.sheet)
                .ok_or_else(|| {
                    EngineError::not_found(format!("no repeated sheet {}", params.sheet))
                })?;
            if !channel.instances.iter().any(|i| i.key == params.from) {
                return Err(EngineError::not_found(format!(
                    "no instance {}",
                    params.from
                )));
            }
            let to: Vec<String> = params.to.clone().unwrap_or_else(|| {
                channel
                    .instances
                    .iter()
                    .map(|i| i.key.clone())
                    .filter(|k| *k != params.from)
                    .collect()
            });
            let copied =
                ew_pcb::channels::copy_layout(pcb, channel, &params.from, &to, params.routing);
            let reply = json!({
                "moved": copied.footprints.len(),
                "removed": copied.removed.len(),
                "tracks": copied.tracks.len(),
                "vias": copied.vias.len(),
                "skipped": copied.skipped,
            });
            if !params.apply {
                return Ok(Outcome::Read(reply));
            }
            let mut requests: Vec<Request> = copied
                .footprints
                .into_iter()
                .map(|fp| Request::Update {
                    doc: params.doc,
                    object: Object::Footprint(fp),
                })
                .collect();
            requests.extend(copied.removed.iter().map(|id| Request::Delete {
                doc: params.doc,
                id: *id,
            }));
            requests.extend(copied.tracks.into_iter().map(|track| Request::Insert {
                doc: params.doc,
                object: Object::Track(track),
            }));
            requests.extend(copied.vias.into_iter().map(|via| Request::Insert {
                doc: params.doc,
                object: Object::Via(via),
            }));
            if requests.is_empty() {
                return Ok(Outcome::Read(reply));
            }
            Ok(Outcome::Commit {
                label: labels::COPY_CHANNEL,
                requests,
                reply,
            })
        }
        "pcb.stitch" => {
            let params: StitchParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let saved = rules(project).map(|(_, r)| r);
            let zone = params
                .zone
                .map(|id| {
                    pcb.zones
                        .get(id)
                        .ok_or_else(|| EngineError::not_found(format!("no zone {id}")))
                })
                .transpose()?;
            let tracks: Vec<&ew_model::pcb::Track> = params
                .tracks
                .iter()
                .filter_map(|id| pcb.tracks.get(*id))
                .collect();
            if zone.is_none() && tracks.is_empty() {
                return Err(EngineError::invalid_params("give a zone or tracks"));
            }
            let net = params
                .net
                .or_else(|| zone.and_then(|z| z.net))
                .ok_or_else(|| EngineError::invalid_params("the vias need a net"))?;
            if pcb.nets.get(net).is_none() {
                return Err(EngineError::not_found(format!("no net {net}")));
            }
            let router = ew_pcb::route::Router::new(pcb, saved);
            let class = router.class_of(Some(net)).clone();
            #[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
            let size = ew_pcb::stitch::ViaSize {
                diameter: params.diameter.unwrap_or(class.via_diameter as f64),
                drill: params.drill.unwrap_or(class.via_drill as f64),
            };
            #[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
            let gap = params.gap.unwrap_or(class.clearance as f64);
            // Where: inside the zone, or near the tracks.
            let near = |at: V| -> bool {
                if let Some(zone) = zone {
                    let outline: Vec<V> = zone.outline.iter().map(|p| V::of(*p)).collect();
                    ew_pcb::geom::Poly::new(outline, Vec::new()).contains(at)
                } else {
                    #[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
                    tracks.iter().any(|t| {
                        ew_pcb::geom::point_segment(at, V::of(t.start), V::of(t.end))
                            < t.width as f64 / 2.0 + gap + size.diameter + 1.0
                    })
                }
            };
            let removed: Vec<Id> = ew_pcb::stitch::free_vias(pcb, net)
                .into_iter()
                .filter(|via| near(V::of(via.at)))
                .map(|via| via.id)
                .collect();
            let mut after = pcb.clone();
            let mut positions = Vec::new();
            if params.remove {
                for id in &removed {
                    after.vias.remove(*id);
                }
            } else {
                positions = if let Some(zone) = zone {
                    router.stitch_zone(
                        pcb,
                        zone,
                        size,
                        params.pitch.unwrap_or(3_000_000.0),
                        params.staggered,
                    )
                } else {
                    router.fence(&tracks, net, size, params.pitch.unwrap_or(2_000_000.0), gap)
                };
            }
            drop(router);
            let stack: Vec<ew_model::layer::CopperLayer> = pcb.stackup.copper_layers().collect();
            #[allow(clippy::cast_possible_truncation, reason = "via sizes")]
            let vias: Vec<ew_model::pcb::Via> = positions
                .iter()
                .map(|at| ew_model::pcb::Via {
                    id: Id::random(),
                    at: at.point(),
                    diameter: size.diameter.round() as i64,
                    drill: size.drill.round() as i64,
                    net: Some(net),
                    from: stack[0],
                    to: stack[stack.len() - 1],
                    micro: false,
                    locked: false,
                })
                .collect();
            let reply = json!({
                "added": vias.len(),
                "removed": if params.remove { removed.len() } else { 0 },
                "vias": positions.iter().map(|p| p.pair()).collect::<Vec<_>>(),
            });
            if !params.apply || (vias.is_empty() && !params.remove) {
                return Ok(Outcome::Read(reply));
            }
            let mut requests: Vec<Request> = Vec::new();
            if params.remove {
                requests.extend(removed.iter().map(|id| Request::Delete {
                    doc: params.doc,
                    id: *id,
                }));
            }
            for via in vias {
                after.vias.insert(via.clone());
                requests.push(Request::Insert {
                    doc: params.doc,
                    object: Object::Via(via),
                });
            }
            // The zones fill again around (and through) the new vias.
            requests.extend(refill(params.doc, &after, saved));
            if requests.is_empty() {
                return Ok(Outcome::Read(reply));
            }
            Ok(Outcome::Commit {
                label: if params.remove {
                    labels::REMOVE_STITCH
                } else {
                    labels::STITCH
                },
                requests,
                reply,
            })
        }
        "pcb.teardrops" => {
            let params: TeardropParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let saved = rules(project);
            let old: Vec<Id> = pcb
                .zones
                .iter()
                .filter(|z| z.teardrop)
                .map(|z| z.id)
                .collect();
            let mut requests: Vec<Request> = old
                .iter()
                .map(|id| Request::Delete {
                    doc: params.doc,
                    id: *id,
                })
                .collect();
            let mut added = 0;
            if !params.remove {
                let settings = params
                    .settings
                    .or_else(|| saved.map(|(_, r)| r.teardrops))
                    .unwrap_or_default();
                // Made and filled on the board without the old teardrops.
                let mut fresh = pcb.clone();
                for id in &old {
                    fresh.zones.remove(*id);
                }
                let zones = ew_pcb::teardrop::teardrops(&fresh, saved.map(|(_, r)| r), &settings);
                let ids: Vec<Id> = zones.iter().map(|z| z.id).collect();
                for zone in zones {
                    fresh.zones.insert(zone);
                }
                let fills = ew_pcb::fill::fill(&fresh, saved.map(|(_, r)| r), Some(&ids));
                for id in &ids {
                    let Some(mut zone) = fresh.zones.get(*id).cloned() else {
                        continue;
                    };
                    zone.filled = fills
                        .iter()
                        .find(|f| f.zone == *id)
                        .map(|f| f.areas.clone())
                        .unwrap_or_default();
                    added += 1;
                    requests.push(Request::Insert {
                        doc: params.doc,
                        object: Object::Zone(zone),
                    });
                }
                // The settings stay with the design rules.
                if let Some(settings) = params.settings
                    && saved.is_none_or(|(_, r)| r.teardrops != settings)
                {
                    let mut props = saved.map_or_else(
                        || Rules::new(Id::random(), "Design rules").props(),
                        |(_, r)| r.props(),
                    );
                    props.teardrops = settings;
                    let Value::Object(props) = to_value(&props)? else {
                        return Err(EngineError::internal("rules are not an object"));
                    };
                    if let Some((doc, _)) = saved {
                        requests.push(Request::SetProps { doc, props });
                    } else {
                        let doc = Id::random();
                        requests.push(Request::AddDocument {
                            kind: DocumentKind::Rules,
                            name: "Design rules".into(),
                            path: None,
                            id: Some(doc),
                        });
                        requests.push(Request::SetProps { doc, props });
                    }
                }
            }
            let reply = json!({"added": added, "removed": old.len()});
            if requests.is_empty() {
                return Ok(Outcome::Read(reply));
            }
            Ok(Outcome::Commit {
                label: if params.remove {
                    labels::REMOVE_TEARDROPS
                } else {
                    labels::TEARDROPS
                },
                requests,
                reply,
            })
        }
        "pcb.pairs" => {
            let params: DocParams = parse(params)?;
            let doc = params
                .doc
                .ok_or_else(|| EngineError::invalid_params("missing doc"))?;
            let pcb = board(project, doc)?;
            Ok(Outcome::Read(pair_table(pcb)))
        }
        "pcb.route" => {
            let params: RouteParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let router = ew_pcb::route::Router::new(pcb, rules(project).map(|(_, r)| r));
            let request = ew_pcb::route::Request {
                layer: params.layer,
                net: params.net,
                partner: None,
                width: params.width,
                start: V::new(params.start[0], params.start[1]),
                target: V::new(params.target[0], params.target[1]),
                diagonal_first: params.diagonal_first,
                corners: params.corners,
                previous: params.previous.map(|[x, y]| V::new(x, y)),
            };
            let (route, shoved) = if params.mode == RouteMode::Shove {
                let placed = heads(pcb, params.net, params.width, &params.fixed, &params.vias);
                let found = router.route_shoving(&request, &placed);
                (
                    ew_pcb::route::Route {
                        points: found.points,
                        reached: found.reached,
                    },
                    Some(found.shoved),
                )
            } else {
                (router.route(&request), None)
            };
            let mut value = to_value(&route)?;
            if let (Some(shoved), Some(map)) = (shoved, value.as_object_mut()) {
                map.insert(
                    "shove".into(),
                    shoved.as_ref().map_or(Value::Null, shove_preview),
                );
            }
            if let (Some(diameter), Some(end)) = (params.via, route.points.last()) {
                let fits = router.via_fits(
                    params.net,
                    V::new(end[0], end[1]),
                    diameter,
                    params.via_layers.map(|[a, b]| (a, b)),
                );
                if let Some(map) = value.as_object_mut() {
                    map.insert("viaFits".into(), json!(fits));
                }
            }
            Ok(Outcome::Read(value))
        }
        "pcb.impedance" => {
            let params: ImpedanceParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let edited = params.rules.map(|props| {
                let mut edited = Rules::new(Id::random(), &props.name);
                edited.set_props(props);
                edited
            });
            let rules = edited.as_ref().or_else(|| rules(project).map(|(_, r)| r));
            Ok(Outcome::Read(impedance_table(pcb, rules)))
        }
        "pcb.impedanceCalc" => {
            let params: ImpedanceCalcParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let structure =
                ew_pcb::impedance::structure(&pcb.stackup, params.layer).ok_or_else(|| {
                    EngineError::invalid_params(format!(
                        "the layer {} has no plane next to it",
                        params.layer
                    ))
                })?;
            let kind = if params.differential {
                ew_pcb::impedance::Kind::Differential
            } else {
                ew_pcb::impedance::Kind::Single
            };
            let value = ew_pcb::impedance::impedance(&structure, kind, params.width, params.gap);
            Ok(Outcome::Read(
                json!({"impedance": value, "structure": structure}),
            ))
        }
        "pcb.viaFits" => {
            let params: ViaFitsParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let router = ew_pcb::route::Router::new(pcb, rules(project).map(|(_, r)| r));
            let fits = router.via_fits(
                params.net,
                V::new(params.at[0], params.at[1]),
                params.diameter,
                params.layers.map(|[a, b]| (a, b)),
            );
            Ok(Outcome::Read(json!({ "fits": fits })))
        }
        "pcb.addTracks" => {
            let params: AddTracksParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            if params.width <= 0 {
                return Err(EngineError::invalid_params("the width must be positive"));
            }
            if let Some(net) = params.net
                && pcb.nets.get(net).is_none()
            {
                return Err(EngineError::not_found(format!("no net {net}")));
            }
            if let Some(pair) = &params.pair
                && pcb.nets.get(pair.net).is_none()
            {
                return Err(EngineError::not_found(format!("no net {}", pair.net)));
            }
            let mut requests = Vec::new();
            let mut select = Vec::new();
            let own = params.segments.iter().map(|s| (params.net, s));
            let partner = params
                .pair
                .iter()
                .flat_map(|pair| pair.segments.iter().map(|s| (Some(pair.net), s)));
            for (net, segment) in own.chain(partner) {
                if !pcb
                    .stackup
                    .has_layer(ew_model::layer::Layer::Copper(segment.layer))
                {
                    return Err(EngineError::invalid_params(format!(
                        "the board has no layer {}",
                        segment.layer
                    )));
                }
                for pair in segment.points.windows(2) {
                    if pair[0] == pair[1] {
                        continue;
                    }
                    let track = ew_model::pcb::Track {
                        id: Id::random(),
                        layer: segment.layer,
                        net,
                        width: segment.width.unwrap_or(params.width),
                        start: pair[0],
                        end: pair[1],
                        mid: None,
                        locked: false,
                    };
                    select.push(track.id);
                    requests.push(Request::Insert {
                        doc: params.doc,
                        object: Object::Track(track),
                    });
                }
            }
            if params.mode == RouteMode::Shove {
                #[allow(clippy::cast_precision_loss, reason = "track widths fit f64")]
                let width = params.width as f64;
                let router = ew_pcb::route::Router::new(pcb, rules(project).map(|(_, r)| r));
                let shoved = router
                    .shove(
                        &heads(pcb, params.net, width, &params.segments, &params.vias),
                        params.corners,
                    )
                    .ok_or_else(|| {
                        EngineError::invalid_params(
                            "the tracks of other nets cannot make room for this route",
                        )
                    })?;
                for id in shoved.removed {
                    requests.push(Request::Delete {
                        doc: params.doc,
                        id,
                    });
                }
                for track in shoved.added {
                    requests.push(Request::Insert {
                        doc: params.doc,
                        object: Object::Track(track),
                    });
                }
            }
            let stack: Vec<ew_model::layer::CopperLayer> = pcb.stackup.copper_layers().collect();
            for via in &params.vias {
                let via = ew_model::pcb::Via {
                    id: Id::random(),
                    at: via.at,
                    diameter: via.diameter,
                    drill: via.drill,
                    net: params.net,
                    from: via.from.unwrap_or(stack[0]),
                    to: via.to.unwrap_or(stack[stack.len() - 1]),
                    micro: via.micro,
                    locked: false,
                };
                select.push(via.id);
                requests.push(Request::Insert {
                    doc: params.doc,
                    object: Object::Via(via),
                });
            }
            if requests.is_empty() {
                return Ok(Outcome::Read(
                    json!({"seq": null, "select": [], "changes": 0}),
                ));
            }
            let count = requests.len();
            Ok(Outcome::Commit {
                label: labels::ROUTE,
                requests,
                reply: json!({"select": select, "changes": count}),
            })
        }
        "pcb.presets" => Ok(Outcome::Read(to_value(&ew_pcb::presets::presets())?)),
        "pcb.applyPreset" => {
            let params: PresetParams = parse(params)?;
            board(project, params.doc)?;
            let preset = ew_pcb::presets::preset(&params.preset)
                .ok_or_else(|| EngineError::not_found(format!("no preset {}", params.preset)))?;
            let defaults = Rules::new(Id::from_u128(0), "Design rules");
            let current = rules(project).map_or(&defaults, |(_, r)| r);
            let new_rules = preset.apply_to(current);
            let Value::Object(rules_props) = to_value(&new_rules.props())? else {
                return Err(EngineError::internal("rules are not an object"));
            };
            let mut requests = vec![Request::SetProps {
                doc: params.doc,
                props: Map::from_iter([("stackup".to_owned(), to_value(&preset.stackup())?)]),
            }];
            if let Some((doc, _)) = rules(project) {
                requests.push(Request::SetProps {
                    doc,
                    props: rules_props,
                });
            } else {
                let doc = Id::random();
                requests.push(Request::AddDocument {
                    kind: DocumentKind::Rules,
                    name: "Design rules".into(),
                    path: None,
                    id: Some(doc),
                });
                requests.push(Request::SetProps {
                    doc,
                    props: rules_props,
                });
            }
            Ok(Outcome::Commit {
                label: labels::PRESET,
                requests,
                reply: json!({}),
            })
        }
        "pcb.dxfPreview" => {
            let params: DxfPreviewParams = parse(params)?;
            Ok(Outcome::Read(to_value(&read_dxf(
                &params.path,
                params.scale,
            )?)?))
        }
        "pcb.addImage" => {
            let params: ImageParams = parse(params)?;
            board(project, params.doc)?;
            let bitmap = ew_pcb::bitmap::Bitmap::from_rows(&params.rows)
                .map_err(|e| EngineError::invalid_params(e.to_string()))?;
            let bitmap = if params.invert {
                bitmap.inverted()
            } else {
                bitmap
            };
            picture_outcome(
                labels::IMAGE,
                params.doc,
                params.layer,
                &bitmap,
                params.pixel,
                params.center,
            )
        }
        "pcb.addQrCode" => {
            let params: QrParams = parse(params)?;
            board(project, params.doc)?;
            if params.text.is_empty() {
                return Err(EngineError::invalid_params("the text is empty"));
            }
            let bitmap = ew_pcb::bitmap::Bitmap::qr(&params.text, params.ecc, params.quiet.min(10))
                .map_err(|e| EngineError::invalid_params(e.to_string()))?;
            let bitmap = if params.invert {
                bitmap.inverted()
            } else {
                bitmap
            };
            let modules = i64::try_from(bitmap.width()).unwrap_or(1).max(1);
            picture_outcome(
                labels::QR_CODE,
                params.doc,
                params.layer,
                &bitmap,
                params.size / modules,
                params.center,
            )
        }
        "pcb.importDxf" => {
            let params: DxfParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let drawing = read_dxf(&params.path, params.scale)?;
            let shapes: Vec<&ew_model::shape::Geometry> = drawing
                .shapes
                .iter()
                .filter(|(layer, _)| params.layers.as_ref().is_none_or(|l| l.contains(layer)))
                .map(|(_, g)| g)
                .collect();
            if shapes.is_empty() {
                return Err(EngineError::invalid_params(
                    "the drawing has no shapes to import",
                ));
            }
            let bounds = ew_model::units::Rect::from_points(shapes.iter().flat_map(|g| g.points()));
            // The board's y axis points up: the lower left corner is the minimum.
            let shift = match (params.origin, bounds) {
                (Some(origin), Some(b)) => {
                    ew_model::units::Point::new(origin.x - b.min.x, origin.y - b.min.y)
                }
                _ => ew_model::units::Point::ORIGIN,
            };
            let mut requests = Vec::new();
            if params.replace {
                for shape in pcb.shapes.iter().filter(|s| s.layer == params.layer) {
                    requests.push(Request::Delete {
                        doc: params.doc,
                        id: shape.id,
                    });
                }
            }
            let width = params.width.unwrap_or(100_000);
            let mut select = Vec::new();
            for geometry in shapes {
                let moved = ew_pcb::edit::shifted(geometry, shift);
                let shape = ew_model::pcb::BoardShape {
                    id: Id::random(),
                    layer: params.layer,
                    geometry: moved,
                    width,
                    filled: false,
                };
                select.push(shape.id);
                requests.push(Request::Insert {
                    doc: params.doc,
                    object: Object::BoardShape(shape),
                });
            }
            let count = select.len();
            Ok(Outcome::Commit {
                label: labels::DXF,
                requests,
                reply: json!({"select": select, "shapes": count}),
            })
        }
        "pcb.exportSpecctra" => {
            let params: DocParams = parse(params)?;
            let doc = params
                .doc
                .ok_or_else(|| EngineError::invalid_params("missing doc"))?;
            let pcb = board(project, doc)?;
            let mut set = ew_pcb::rules::RuleSet::new(rules(project).map(|(_, r)| r));
            set.cache_classes(pcb.nets.iter().map(|n| (n.name.clone(), n.class.clone())));
            let text = ew_pcb::specctra::write_dsn(&ew_pcb::specctra::DsnInput {
                pcb,
                rules: &set,
                host: concat!("ElectronicWerk ", env!("CARGO_PKG_VERSION")),
            });
            Ok(Outcome::Read(json!({
                "file": format!("{}.dsn", ew_output::job::sanitize(&pcb.name)),
                "text": text,
            })))
        }
        "pcb.importSession" => {
            let params: SessionParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let text = std::fs::read_to_string(&params.path).map_err(|error| {
                EngineError::invalid_params(format!("cannot read {}: {error}", params.path))
            })?;
            let session = ew_pcb::specctra::read_ses(&text)
                .map_err(|error| EngineError::invalid_params(error.0))?;
            let mut set = ew_pcb::rules::RuleSet::new(rules(project).map(|(_, r)| r));
            set.cache_classes(pcb.nets.iter().map(|n| (n.name.clone(), n.class.clone())));
            let plan = ew_pcb::specctra::plan_session(pcb, &set, &session);
            if plan.tracks.is_empty() && plan.vias.is_empty() {
                return Err(EngineError::invalid_params(
                    "the session has no routing for the nets of this board",
                ));
            }
            let mut requests = Vec::new();
            for id in plan.remove_tracks.iter().chain(&plan.remove_vias) {
                requests.push(Request::Delete {
                    doc: params.doc,
                    id: *id,
                });
            }
            let reply = json!({
                "tracks": plan.tracks.len(),
                "vias": plan.vias.len(),
                "removedTracks": plan.remove_tracks.len(),
                "removedVias": plan.remove_vias.len(),
                "unknownNets": plan.unknown_nets,
                "unknownLayers": plan.unknown_layers,
            });
            for track in plan.tracks {
                requests.push(Request::Insert {
                    doc: params.doc,
                    object: Object::Track(track),
                });
            }
            for via in plan.vias {
                requests.push(Request::Insert {
                    doc: params.doc,
                    object: Object::Via(via),
                });
            }
            Ok(Outcome::Commit {
                label: labels::SESSION,
                requests,
                reply,
            })
        }
        "pcb.gerberPreview" => {
            let params: GerberPreviewParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let mut free = free_user_layers(pcb).into_iter();
            let mut files = Vec::new();
            for path in &params.paths {
                let (name, kind, image) = read_gerber(path)?;
                let function = ew_output::viewer::function_of(&ew_output::viewer::ViewerFile {
                    name: name.clone(),
                    image: image.clone(),
                });
                // In board orientation (y down), nanometres.
                // Gerber and the board both have y pointing up.
                let bounds = image.bounds().map(|[x0, y0, x1, y1]| {
                    let (low, high) = (V::new(x0, y0).point(), V::new(x1, y1).point());
                    [low.x, low.y, high.x, high.y]
                });
                files.push(json!({
                    "path": path,
                    "name": name,
                    "kind": kind,
                    "function": function,
                    "objects": image.objects.len(),
                    "bounds": bounds,
                    "layer": free.next().map(|l| l.to_string()),
                }));
            }
            Ok(Outcome::Read(json!({ "files": files })))
        }
        "pcb.importGerbers" => {
            let params: GerberImportParams = parse(params)?;
            board(project, params.doc)?;
            if params.files.is_empty() {
                return Err(EngineError::invalid_params("no files to import"));
            }
            let mut drawings = Vec::new();
            for file in &params.files {
                let (name, _, image) = read_gerber(&file.path)?;
                // File coordinates and the board both have y pointing up.
                let areas: Vec<Vec<ew_model::units::Point>> = ew_output::reference::areas(&image)
                    .into_iter()
                    .map(|ring| ring.into_iter().map(V::point).collect())
                    .collect();
                drawings.push((name, file.layer, areas));
            }
            let bounds = ew_model::units::Rect::from_points(
                drawings
                    .iter()
                    .flat_map(|(_, _, areas)| areas.iter().flatten().copied()),
            );
            let shift = match (params.origin, bounds) {
                (Some(origin), Some(b)) => {
                    ew_model::units::Point::new(origin.x - b.min.x, origin.y - b.min.y)
                }
                _ => ew_model::units::Point::ORIGIN,
            };
            let pcb = board(project, params.doc)?;
            let mut requests = Vec::new();
            if params.replace {
                let targets: Vec<ew_model::layer::Layer> =
                    params.files.iter().map(|f| f.layer).collect();
                for shape in pcb.shapes.iter().filter(|s| targets.contains(&s.layer)) {
                    requests.push(Request::Delete {
                        doc: params.doc,
                        id: shape.id,
                    });
                }
            }
            let mut layers = Vec::new();
            let mut total = 0;
            for (name, layer, areas) in drawings {
                let count = areas.len();
                for ring in areas {
                    let points = ring
                        .into_iter()
                        .map(|p| ew_model::units::Point::new(p.x + shift.x, p.y + shift.y))
                        .collect();
                    requests.push(Request::Insert {
                        doc: params.doc,
                        object: Object::BoardShape(ew_model::pcb::BoardShape {
                            id: Id::random(),
                            layer,
                            geometry: ew_model::shape::Geometry::Polygon { points },
                            width: 0,
                            filled: true,
                        }),
                    });
                }
                total += count;
                layers.push(json!({"file": name, "layer": layer.to_string(), "shapes": count}));
            }
            if total == 0 {
                return Err(EngineError::invalid_params(
                    "the files have nothing to draw",
                ));
            }
            Ok(Outcome::Commit {
                label: labels::GERBER,
                requests,
                reply: json!({"shapes": total, "layers": layers}),
            })
        }
        "pcb.validateRule" => {
            let params: ValidateParams = parse(params)?;
            Ok(Outcome::Read(
                match ew_pcb::rules::validate(&params.condition) {
                    None => json!({"ok": true}),
                    Some((message, position)) => {
                        json!({"ok": false, "message": message, "position": position})
                    }
                },
            ))
        }
        "pcb.exclude" => {
            let params: ExcludeParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let mut list = pcb.drc_exclusions.clone();
            if params.excluded {
                for key in params.keys {
                    if !list.contains(&key) {
                        list.push(key);
                    }
                }
            } else {
                list.retain(|key| !params.keys.contains(key));
            }
            list.sort();
            if let Some(report) = cache.reports.get_mut(&params.doc) {
                for finding in &mut report.findings {
                    finding.excluded = list.contains(&finding.key);
                }
            }
            Ok(Outcome::Commit {
                label: labels::EXCLUDE,
                requests: vec![Request::SetProps {
                    doc: params.doc,
                    props: Map::from_iter([("drcExclusions".to_owned(), json!(list))]),
                }],
                reply: json!({}),
            })
        }
        other => Err(EngineError::new(
            ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

/// Findings for the differences between schematic and board.
#[allow(clippy::too_many_lines, reason = "one arm per kind of difference")]
fn parity(project: &Project, pcb: &Pcb, rules: Option<&Rules>) -> Vec<drc::Finding> {
    use ew_pcb::sync::Change;
    let connectivity = ew_schematic::analyze(project);
    let has_parts = !ew_pcb::sync::parts(project, &connectivity).0.is_empty();
    if !has_parts {
        return Vec::new();
    }
    let defaults = DrcSettings::default();
    let settings = rules.map_or(&defaults, |r| &r.drc);
    let diff = ew_pcb::sync::diff(project, &connectivity, pcb);
    let mut out = Vec::new();
    for change in &diff.changes {
        let (code, at, targets, args): (
            &str,
            ew_model::units::Point,
            Vec<drc::Target>,
            Vec<(&str, String)>,
        ) = match change {
            Change::AddFootprint { designator, .. } => (
                "missing-footprint",
                ew_model::units::Point::ORIGIN,
                Vec::new(),
                vec![("designator", designator.clone())],
            ),
            Change::RemoveFootprint { id, designator } => (
                "extra-footprint",
                pcb.footprints
                    .get(*id)
                    .map_or(ew_model::units::Point::ORIGIN, |fp| fp.at),
                vec![drc::Target { id: *id, pad: None }],
                vec![("designator", designator.clone())],
            ),
            Change::SetPadNet {
                id,
                designator,
                pad,
                from,
                to,
            } => {
                let fp = pcb.footprints.get(*id);
                let index = fp.and_then(|fp| fp.pads.iter().position(|p| p.number == *pad));
                let at = fp.map_or(ew_model::units::Point::ORIGIN, |fp| {
                    let xf = ew_pcb::items::footprint_xform(fp);
                    index
                        .and_then(|i| fp.pads.get(i))
                        .map_or(fp.at, |p| xf.apply(V::of(p.at)).point())
                });
                (
                    "net-mismatch",
                    at,
                    vec![drc::Target {
                        id: *id,
                        pad: index,
                    }],
                    vec![
                        ("pad", format!("{designator}-{pad}")),
                        ("board", from.clone().unwrap_or_default()),
                        ("schematic", to.clone().unwrap_or_default()),
                    ],
                )
            }
            _ => continue,
        };
        let severity = settings.severity(code);
        if severity == ErcSeverity::Ignore {
            continue;
        }
        let message = match code {
            "missing-footprint" => {
                format!("{} is in the schematic but not on the board", args[0].1)
            }
            "extra-footprint" => format!("{} is on the board but not in the schematic", args[0].1),
            _ => format!(
                "Pad {} is on net {} on the board but on {} in the schematic",
                args[0].1,
                if args[1].1.is_empty() {
                    "(none)"
                } else {
                    &args[1].1
                },
                if args[2].1.is_empty() {
                    "(none)"
                } else {
                    &args[2].1
                }
            ),
        };
        let key = format!(
            "{code}:{}",
            args.iter()
                .map(|(_, v)| v.as_str())
                .collect::<Vec<_>>()
                .join("/")
        );
        out.push(drc::Finding {
            code: code.to_owned(),
            severity,
            message,
            args: args.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
            at,
            layer: None,
            point: None,
            targets,
            key,
            excluded: false,
        });
    }
    out
}

/// The design rules with what the editors need around them: the checks
/// with their severities and, for a board, which nets fall into which
/// class.
fn rules_info(project: &Project, doc: Option<Id>) -> Result<Value, EngineError> {
    let found = rules(project);
    let defaults = Rules::new(Id::from_u128(0), "Design rules");
    let (rules_doc, rules) = found.map_or((None, &defaults), |(id, r)| (Some(id), r));
    let settings: &DrcSettings = &rules.drc;
    let checks: Vec<Value> = DRC_CHECKS
        .iter()
        .map(|(code, default)| {
            json!({
                "code": code,
                "default": default,
                "severity": settings.severity(code),
            })
        })
        .collect();
    let mut classes: BTreeMap<String, Vec<String>> = rules
        .net_classes
        .iter()
        .map(|class| (class.name.clone(), Vec::new()))
        .collect();
    if let Some(doc) = doc {
        let pcb = board(project, doc)?;
        let set = ew_pcb::rules::RuleSet::new(Some(rules));
        for net in &pcb.nets {
            let class = set.class_name(&net.name, &net.class);
            classes.entry(class).or_default().push(net.name.clone());
        }
        for nets in classes.values_mut() {
            nets.sort_by(|a, b| ew_schematic::connectivity::natural_cmp(a, b));
        }
    }
    let errors = ew_pcb::rules::RuleSet::new(Some(rules)).errors;
    Ok(json!({
        "doc": rules_doc,
        "rules": rules.props(),
        "checks": checks,
        "nets": classes,
        "errors": errors,
        "severities": [ErcSeverity::Error, ErcSeverity::Warning, ErcSeverity::Ignore],
    }))
}

/// The impedance view of a board: the interfaces of the beginner mode, the
/// structure of every copper layer and every profile on every layer.
#[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
fn impedance_table(pcb: &Pcb, rules: Option<&Rules>) -> Value {
    let set = ew_pcb::rules::RuleSet::new(rules);
    let layers: Vec<ew_model::layer::CopperLayer> = pcb.stackup.copper_layers().collect();
    let structures: Vec<Value> = layers
        .iter()
        .map(|&layer| {
            json!({
                "layer": layer,
                "structure": ew_pcb::impedance::structure(&pcb.stackup, layer),
            })
        })
        .collect();
    let min_width = set.constraints.min_track_width as f64;
    let profiles: Vec<Value> = set
        .profiles
        .iter()
        .map(|profile| {
            // The smallest gap: the largest clearance of the classes that
            // use the profile (at least the board minimum).
            let clearance = rules
                .and_then(|r| {
                    r.net_classes
                        .iter()
                        .filter(|c| c.impedance == profile.name)
                        .map(|c| c.clearance)
                        .max()
                })
                .unwrap_or(0)
                .max(set.constraints.min_clearance) as f64;
            let per_layer: Vec<Value> = layers
                .iter()
                .map(|&layer| {
                    json!({
                        "layer": layer,
                        "layout": ew_pcb::impedance::layout(profile, &pcb.stackup, layer, min_width, clearance),
                    })
                })
                .collect();
            json!({"profile": profile, "layers": per_layer})
        })
        .collect();
    json!({
        "presets": ew_pcb::impedance::PRESETS,
        "layers": structures,
        "profiles": profiles,
    })
}

/// Updates for the zones of a board whose fill changes when they are filled
/// again.
fn refill(doc: Id, pcb: &Pcb, rules: Option<&Rules>) -> Vec<Request> {
    ew_pcb::fill::fill(pcb, rules, None)
        .into_iter()
        .filter_map(|filled| {
            let zone = pcb.zones.get(filled.zone)?;
            (zone.filled != filled.areas).then(|| {
                let mut zone = zone.clone();
                zone.filled = filled.areas;
                Request::Update {
                    doc,
                    object: Object::Zone(zone),
                }
            })
        })
        .collect()
}

/// A preview of meanders: the new tracks and the tracks they replace, as
/// `pcb.editPreview` shows them.
fn tuned_preview(mut reply: Value, hidden: &[Id], tracks: &[ew_model::pcb::Track]) -> Value {
    if let Some(map) = reply.as_object_mut() {
        map.insert("hidden".into(), json!(hidden));
        map.insert(
            "tracks".into(),
            json!(
                tracks
                    .iter()
                    .map(|t| json!({"layer": t.layer, "width": t.width, "start": t.start, "end": t.end}))
                    .collect::<Vec<_>>()
            ),
        );
    }
    reply
}

/// The differential pairs of a board with the routed length of both nets
/// and their difference (skew), in nm.
#[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
fn pair_table(pcb: &Pcb) -> Value {
    let length = |net: Id| -> f64 {
        pcb.tracks
            .iter()
            .filter(|t| t.net == Some(net))
            .map(|t| V::of(t.start).dist(V::of(t.end)))
            .sum()
    };
    let name = |net: Id| pcb.nets.get(net).map_or(String::new(), |n| n.name.clone());
    let pairs: Vec<Value> = ew_pcb::pairs::pairs(pcb)
        .into_iter()
        .map(|pair| {
            let (p, n) = (length(pair.positive), length(pair.negative));
            json!({
                "name": pair.name,
                "positive": {"net": pair.positive, "name": name(pair.positive), "length": p},
                "negative": {"net": pair.negative, "name": name(pair.negative), "length": n},
                "skew": (p - n).abs(),
            })
        })
        .collect();
    json!({ "pairs": pairs })
}

/// The copper of a route that pushes in push-and-shove mode: its segments
/// and its vias (on every layer between their first and last one).
#[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
fn heads(
    pcb: &Pcb,
    net: Option<Id>,
    width: f64,
    segments: &[Segment],
    vias: &[NewVia],
) -> Vec<ew_pcb::shove::Head> {
    let stack: Vec<ew_model::layer::CopperLayer> = pcb.stackup.copper_layers().collect();
    let mut out: Vec<ew_pcb::shove::Head> = segments
        .iter()
        .filter(|s| s.points.len() >= 2)
        .map(|s| ew_pcb::shove::Head {
            layers: vec![s.layer],
            net,
            width: s.width.map_or(width, |w| w as f64),
            points: s.points.iter().map(|p| V::of(*p)).collect(),
        })
        .collect();
    for via in vias {
        let index = |layer: Option<ew_model::layer::CopperLayer>, fallback: usize| {
            layer
                .and_then(|l| stack.iter().position(|own| *own == l))
                .unwrap_or(fallback)
        };
        let (a, b) = (
            index(via.from, 0),
            index(via.to, stack.len().saturating_sub(1)),
        );
        out.push(ew_pcb::shove::Head {
            layers: stack[a.min(b)..=a.max(b)].to_vec(),
            net,
            width: via.diameter as f64,
            points: vec![V::of(via.at)],
        });
    }
    out
}

/// What a push changes, for the preview: the pushed tracks (`tracks`) and
/// the tracks they replace (`hidden`).
fn shove_preview(shoved: &ew_pcb::shove::Shoved) -> Value {
    let tracks: Vec<Value> = shoved
        .added
        .iter()
        .map(|t| json!({"layer": t.layer, "width": t.width, "start": t.start, "end": t.end}))
        .collect();
    json!({ "tracks": tracks, "hidden": shoved.removed })
}

/// The tracks an edit would leave, for a preview: the changed and new
/// tracks (`tracks`) and the objects they replace or remove (`hidden`).
fn track_preview(changes: &[ew_schematic::edit::Change]) -> Value {
    use ew_model::object::Object;
    use ew_schematic::edit::Change;
    let mut tracks = Vec::new();
    let mut hidden = Vec::new();
    for change in changes {
        match change {
            Change::Update {
                object: Object::Track(track),
                ..
            }
            | Change::Insert {
                object: Object::Track(track),
                ..
            } => {
                if matches!(change, Change::Update { .. }) {
                    hidden.push(track.id);
                }
                tracks.push(json!({
                    "layer": track.layer,
                    "width": track.width,
                    "start": track.start,
                    "end": track.end,
                }));
            }
            Change::Update { object, .. } => hidden.push(object.id()),
            Change::Delete { id, .. } => hidden.push(*id),
            Change::Insert { .. } => {}
        }
    }
    json!({ "tracks": tracks, "hidden": hidden })
}
