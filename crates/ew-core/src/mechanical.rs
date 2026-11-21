//! The board's mechanical bodies and the enclosure check (M17): the
//! `mechanical.*` engine methods.
//!
//! - `mechanical.list` — the bodies with their files and placements.
//! - `mechanical.add` — copies a STEP file into the project's `mechanical/`
//!   folder and places it around the board.
//! - `mechanical.update` / `mechanical.remove` — edits as undo steps.
//! - `mechanical.check` — the enclosure check: overlaps and clearances of
//!   the board and its parts against the bodies, as design rules findings.
//! - `mechanical.enclosure` — a simple box enclosure made for the board.
//!
//! Meshes: STEP files tessellated by the desktop app (the mesh cache,
//! `step-<hash>.glb`), else read here when all their faces are planar (the
//! models ElectronicWerk writes). Files with curved faces and no mesh yet
//! are named in `tessellate`, so the app can tessellate them and check
//! again; parts whose model has no mesh are checked as boxes meanwhile.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use ew_board3d::board::{self, BoardOptions};
use ew_board3d::collide::{Bounds, Solid};
use ew_board3d::enclosure::{self, BoxSpec};
use ew_board3d::mechanical::{self as mech, Hit, Other, PartShape};
use ew_board3d::mesh::Mesh;
use ew_board3d::place::Matrix;
use ew_board3d::stack::Levels;
use ew_board3d::step::ModelData;
use ew_board3d::step::tessellate::{TessellateError, planar_mesh};
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::pcb::{MechanicalBody, MechanicalKind, Pcb};
use ew_model::rules::{ErcSeverity, Rules};
use ew_model::units::{Angle, Point, Vec3};
use ew_pcb::drc::{self, Finding, Target};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::board3d::{ModelSource, model_data, sources, step_mesh};
use crate::change::Request;
use crate::error::{EngineError, ErrorCode};
use crate::library::LibraryState;
use crate::session::Session;

/// History label of `mechanical.add`.
pub const ADD: &str = "add-mechanical";
/// History label of `mechanical.update`.
pub const EDIT: &str = "edit-mechanical";
/// History label of `mechanical.remove`.
pub const REMOVE: &str = "remove-mechanical";
/// History label of `mechanical.enclosure`.
pub const ENCLOSURE: &str = "make-enclosure";

/// The folder of the bodies' files in a project.
pub const FOLDER: &str = "mechanical";

/// Height of parts without a model and without a `Height` property (mm).
pub const DEFAULT_HEIGHT: f64 = 3.0;

/// The result of a `mechanical.*` call.
#[derive(Debug)]
pub enum Outcome {
    /// A reply without changes.
    Read(Value),
    /// Files to write, then requests to commit as one undo step.
    Commit {
        /// History label.
        label: &'static str,
        /// Files (path, content).
        files: Vec<(PathBuf, Vec<u8>)>,
        /// Requests.
        requests: Vec<Request>,
        /// Extra reply fields.
        reply: Value,
    },
}

type Stamp = (u64, Option<SystemTime>);

fn stamp(file: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(file).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

/// Why a body has no mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoMesh {
    /// The file is missing or unreadable.
    File,
    /// It needs the app's tessellation.
    Curved,
}

/// Meshes and solids of the last checks, so a check after a small edit
/// only repeats the geometry that changed.
#[derive(Default)]
pub struct Cache {
    meshes: HashMap<PathBuf, (Stamp, Result<Arc<Mesh>, NoMesh>)>,
    models: HashMap<String, (Option<Stamp>, Option<Arc<Mesh>>)>,
    solids: HashMap<Id, (MechanicalBody, Stamp, Arc<Solid>)>,
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("meshes", &self.meshes.len())
            .field("models", &self.models.len())
            .field("solids", &self.solids.len())
            .finish()
    }
}

fn merge_parts(parts: &[ew_board3d::glb::Part]) -> Mesh {
    let mut mesh = Mesh::default();
    for part in parts {
        let base = u32::try_from(mesh.positions.len()).unwrap_or(u32::MAX);
        mesh.positions.extend_from_slice(&part.positions);
        mesh.normals
            .extend(std::iter::repeat_n([0.0; 3], part.positions.len()));
        mesh.indices.extend(part.indices.iter().map(|i| base + i));
    }
    mesh
}

impl Cache {
    /// Forgets everything (a project was opened or closed).
    pub fn clear(&mut self) {
        self.meshes.clear();
        self.models.clear();
        self.solids.clear();
    }

    /// The mesh of a body's STEP file.
    fn body_mesh(&mut self, file: &Path, mesh_dir: Option<&Path>) -> Result<Arc<Mesh>, NoMesh> {
        let Some(now) = stamp(file) else {
            return Err(NoMesh::File);
        };
        if let Some((seen, mesh)) = self.meshes.get(file)
            && *seen == now
            && *mesh != Err(NoMesh::Curved)
        {
            return mesh.clone();
        }
        let mesh = mesh_dir
            .and_then(|dir| step_mesh(dir, file))
            .and_then(|glb| std::fs::read(glb).ok())
            .and_then(|bytes| ew_board3d::glb::read(&bytes).ok())
            .map(|parts| Arc::new(merge_parts(&parts)))
            .map_or_else(
                || match std::fs::read_to_string(file) {
                    Ok(text) => match planar_mesh(&text) {
                        Ok(mesh) => Ok(Arc::new(mesh)),
                        Err(TessellateError::Curved(_)) => Err(NoMesh::Curved),
                        Err(_) => Err(NoMesh::File),
                    },
                    Err(_) => Err(NoMesh::File),
                },
                Ok,
            );
        self.meshes.insert(file.to_path_buf(), (now, mesh.clone()));
        mesh
    }

    /// The mesh of a part's model; `None` (and the STEP file to tessellate)
    /// without one.
    fn model_mesh(
        &mut self,
        source: &ModelSource,
        mesh_dir: Option<&Path>,
        tessellate: &mut BTreeSet<String>,
    ) -> Option<Arc<Mesh>> {
        let now = source.file.as_deref().and_then(|f| stamp(Path::new(f)));
        if let Some((seen, mesh)) = self.models.get(&source.path)
            && *seen == now
            && mesh.is_some()
        {
            return mesh.clone();
        }
        let mesh = match model_data(source, mesh_dir, false) {
            Some(ModelData::Mesh(parts)) => Some(Arc::new(merge_parts(&parts))),
            Some(ModelData::Step(text)) => {
                let planar = planar_mesh(&text).ok().map(Arc::new);
                if planar.is_none()
                    && let Some(file) = &source.file
                {
                    tessellate.insert(file.clone());
                }
                planar
            }
            None => None,
        };
        self.models.insert(source.path.clone(), (now, mesh.clone()));
        mesh
    }
}

fn parse<T: for<'de> Deserialize<'de>>(params: &Value) -> Result<T, EngineError> {
    serde_json::from_value(params.clone()).map_err(|e| EngineError::invalid_params(e.to_string()))
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(EngineError::internal)
}

fn board_of(session: &Session, doc: Id) -> Result<&Pcb, EngineError> {
    match session.project().document(doc) {
        Some(ew_model::document::Document::Pcb(pcb)) => Ok(pcb),
        Some(_) => Err(EngineError::invalid_params("the document is not a board")),
        None => Err(EngineError::not_found(format!("no document {doc}"))),
    }
}

fn rules_of(session: &Session) -> Option<&Rules> {
    let project = session.project();
    let entry = project
        .documents_of(ew_model::document::DocumentKind::Rules)
        .next()?;
    match project.document(entry.id) {
        Some(ew_model::document::Document::Rules(rules)) => Some(rules),
        _ => None,
    }
}

/// The minimum clearance of the rules (mm).
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "clearances are small")]
pub fn clearance_of(rules: Option<&Rules>) -> f64 {
    rules.map_or_else(
        || ew_model::rules::Constraints::default().min_mechanical_clearance,
        |r| r.constraints.min_mechanical_clearance,
    ) as f64
        / 1e6
}

/// The file of a body.
#[must_use]
pub fn file_of(root: &Path, body: &MechanicalBody) -> PathBuf {
    let path = Path::new(&body.path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocParams {
    doc: Id,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AddParams {
    doc: Id,
    /// The STEP file (absolute).
    file: PathBuf,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    kind: Option<MechanicalKind>,
    #[serde(default)]
    position: Option<Vec3>,
    #[serde(default)]
    rotation: Option<[Angle; 3]>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateParams {
    doc: Id,
    id: Id,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    kind: Option<MechanicalKind>,
    #[serde(default)]
    position: Option<Vec3>,
    #[serde(default)]
    rotation: Option<[Angle; 3]>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default)]
    opacity: Option<u8>,
    #[serde(default)]
    check: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RemoveParams {
    doc: Id,
    ids: Vec<Id>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckParams {
    doc: Id,
    /// Minimum clearance (mm) instead of the rules'.
    #[serde(default)]
    clearance: Option<f64>,
    /// Height of parts without a model (mm).
    #[serde(default)]
    default_height: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnclosureParams {
    doc: Id,
    /// The enclosure; made to fit the board if left out.
    #[serde(default)]
    spec: Option<BoxSpec>,
    /// Gap between the board edge and the walls (mm).
    #[serde(default)]
    margin: Option<f64>,
}

/// Guesses the kind of a body from its name.
#[must_use]
pub fn guess_kind(name: &str) -> MechanicalKind {
    let lower = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if has(&["lid", "cover", "deckel", "top"]) {
        MechanicalKind::Lid
    } else if has(&["panel", "front", "rear", "back", "blende"]) {
        MechanicalKind::Panel
    } else if has(&[
        "screw", "bolt", "standoff", "spacer", "nut", "schraube", "mutter",
    ]) {
        MechanicalKind::Screw
    } else if has(&["carrier", "mainboard", "motherboard", "träger", "board"]) {
        MechanicalKind::Carrier
    } else {
        MechanicalKind::Enclosure
    }
}

/// The centre of the board outline (or of the footprints), nm.
fn board_center(pcb: &Pcb) -> Point {
    pcb.outline_bounds().map_or(Point::ORIGIN, |r| {
        Point::new((r.min.x + r.max.x) / 2, (r.min.y + r.max.y) / 2)
    })
}

/// A free file name in the project's `mechanical/` folder; an identical
/// file already there is used again.
fn target_file(root: &Path, source: &Path, data: &[u8]) -> PathBuf {
    let dir = root.join(FOLDER);
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("body")
        .to_owned();
    let ext = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("step")
        .to_ascii_lowercase();
    for n in 1.. {
        let name = if n == 1 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem}-{n}.{ext}")
        };
        let path = dir.join(&name);
        match std::fs::read(&path) {
            Ok(existing) if existing == data => return path,
            Ok(_) => {}
            Err(_) => return path,
        }
    }
    dir.join(format!("{stem}.{ext}"))
}

fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .map_or_else(|_| file.to_path_buf(), Path::to_path_buf)
        .to_string_lossy()
        .replace('\\', "/")
}

/// One body for `mechanical.list`.
fn body_json(root: &Path, body: &MechanicalBody) -> Result<Value, EngineError> {
    let file = file_of(root, body);
    let mut value = to_value(body)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "file".into(),
            Value::String(file.to_string_lossy().into_owned()),
        );
        object.insert("exists".into(), Value::Bool(file.is_file()));
        object.insert("matrix".into(), to_value(&mech::body_matrix(body))?);
        // Fill in the defaults the file leaves out.
        object.insert("opacity".into(), Value::from(body.opacity));
        object.insert("check".into(), Value::Bool(body.check));
        object.insert("hidden".into(), Value::Bool(body.hidden));
        object.insert("kind".into(), to_value(&body.kind)?);
        object.insert("position".into(), to_value(&body.position)?);
        object.insert("rotation".into(), to_value(&body.rotation)?);
    }
    Ok(value)
}

/// The result of an enclosure check.
#[derive(Debug, Default)]
pub struct Checked {
    /// Findings in the form of the design rules check.
    pub findings: Vec<Finding>,
    /// The raw results.
    pub hits: Vec<Hit>,
    /// STEP files that need the app's tessellation.
    pub tessellate: Vec<String>,
    /// Bodies that could not be checked (file missing or not tessellated).
    pub unchecked: Vec<Id>,
    /// Parts checked as boxes.
    pub stand_ins: Vec<String>,
    /// The highest point of the parts (mm), for enclosure heights.
    pub top: f64,
}

fn mm_text(value: f64) -> String {
    drc::mm(value * 1e6)
}

#[allow(clippy::cast_possible_truncation, reason = "board coordinates")]
fn point_nm(p: [f64; 3]) -> Point {
    Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64)
}

/// The highest point of the parts above the board's bottom face (mm).
fn tallest(parts: &[PartShape], levels: &Levels) -> f64 {
    parts
        .iter()
        .flat_map(|p| p.shapes.iter())
        .map(|s| match s {
            mech::Shape::Mesh(mesh, matrix) => Bounds::of_mesh(mesh, matrix).max[2],
            mech::Shape::Box(tris) => tris
                .iter()
                .flat_map(|t| t.iter())
                .map(|p| p[2])
                .fold(f64::NEG_INFINITY, f64::max),
        })
        .fold(levels.body_top, f64::max)
}

/// Runs the enclosure check of a board.
///
/// # Errors
///
/// Fails if the document is not a board.
#[allow(clippy::too_many_lines, reason = "one pass over bodies and parts")]
pub fn check(
    session: &Session,
    library: &mut LibraryState,
    cache: &mut Cache,
    doc: Id,
    clearance: Option<f64>,
    default_height: Option<f64>,
) -> Result<Checked, EngineError> {
    let pcb = board_of(session, doc)?;
    let rules = rules_of(session);
    let mut out = Checked::default();
    let bodies: Vec<&MechanicalBody> = pcb.mechanical.iter().filter(|b| b.check).collect();
    if bodies.is_empty() {
        return Ok(out);
    }
    let root = session.root();
    let mesh_dir = library.mesh_dir();
    let mut tessellate = BTreeSet::new();
    // The bodies' solids.
    let mut solids = Vec::new();
    for body in &bodies {
        let file = file_of(root, body);
        match cache.body_mesh(&file, mesh_dir.as_deref()) {
            Ok(mesh) => {
                let now = stamp(&file).unwrap_or((0, None));
                let solid = match cache.solids.get(&body.id) {
                    Some((seen, seen_stamp, solid)) if seen == *body && *seen_stamp == now => {
                        Arc::clone(solid)
                    }
                    _ => {
                        let solid = Arc::new(Solid::of_mesh(&mesh, &mech::body_matrix(body)));
                        cache
                            .solids
                            .insert(body.id, ((*body).clone(), now, Arc::clone(&solid)));
                        solid
                    }
                };
                solids.push((body.id, solid));
            }
            Err(reason) => {
                if reason == NoMesh::Curved {
                    tessellate.insert(file.to_string_lossy().into_owned());
                }
                out.unchecked.push(body.id);
            }
        }
    }
    // The parts' meshes.
    let found = sources(pcb, root, library);
    let mut meshes: HashMap<String, Arc<Mesh>> = HashMap::new();
    for source in &found {
        if let Some(mesh) = cache.model_mesh(source, mesh_dir.as_deref(), &mut tessellate) {
            meshes.insert(source.path.clone(), mesh);
        }
    }
    let levels = Levels::of(&pcb.stackup);
    let height = default_height.unwrap_or(DEFAULT_HEIGHT);
    let parts: Vec<PartShape> =
        mech::part_shapes(pcb, &levels, &|path| meshes.get(path).cloned(), height);
    out.stand_ins = parts
        .iter()
        .filter(|p| p.stand_in)
        .map(|p| p.designator.clone())
        .collect();
    out.top = tallest(&parts, &levels);
    // The board body.
    let resolve = |text: &str| session.resolve_text(text, Some(doc));
    let options = BoardOptions {
        body_only: true,
        tolerance: 20_000.0,
        ..BoardOptions::default()
    };
    let built = board::build(pcb, &resolve, &options);
    let clearance = clearance.unwrap_or_else(|| clearance_of(rules));
    out.hits = mech::check(&mech::Input {
        board: (!built.body.is_empty()).then_some(&built.body),
        bodies: solids,
        parts: &parts,
        clearance,
    });
    // As findings.
    let defaults = ew_model::rules::DrcSettings::default();
    let settings = rules.map_or(&defaults, |r| &r.drc);
    for hit in &out.hits {
        let code = if hit.overlap {
            "mechanical-collision"
        } else {
            "mechanical-clearance"
        };
        let severity = settings.severity(code);
        if severity == ErcSeverity::Ignore {
            continue;
        }
        let body = pcb.mechanical.get(hit.body);
        let body_name = body.map_or_else(String::new, |b| b.name.clone());
        let (what, target, key_part) = match hit.other {
            Other::Board => ("the board".to_owned(), None, "board".to_owned()),
            Other::Part { id } => {
                let designator = pcb
                    .footprints
                    .get(id)
                    .map_or_else(String::new, |fp| fp.designator.clone());
                (designator, Some(id), id.to_string())
            }
        };
        let message = if hit.overlap {
            format!("{} collides with {body_name}", capitalized(&what))
        } else {
            format!(
                "{} is {} from {body_name} (minimum {})",
                capitalized(&what),
                mm_text(hit.distance),
                mm_text(clearance)
            )
        };
        let mut args = std::collections::BTreeMap::new();
        args.insert(
            "part".to_owned(),
            if target.is_some() {
                what.clone()
            } else {
                String::new()
            },
        );
        args.insert("body".to_owned(), body_name.clone());
        args.insert("distance".to_owned(), mm_text(hit.distance));
        args.insert("minimum".to_owned(), mm_text(clearance));
        let mut targets = vec![Target::object(hit.body)];
        if let Some(id) = target {
            targets.push(Target::object(id));
        }
        let key = format!("{code}:{}/{key_part}", hit.body);
        out.findings.push(Finding {
            code: code.to_owned(),
            severity,
            message,
            args,
            at: point_nm(hit.point),
            layer: None,
            point: Some(hit.point.map(|v| (v * 1e4).round() / 1e4)),
            targets,
            excluded: pcb.drc_exclusions.contains(&key),
            key,
        });
    }
    out.tessellate = tessellate.into_iter().collect();
    Ok(out)
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}

fn checked_json(checked: &Checked, milliseconds: u128) -> Value {
    let errors = checked
        .findings
        .iter()
        .filter(|f| !f.excluded && f.severity == ErcSeverity::Error)
        .count();
    let warnings = checked
        .findings
        .iter()
        .filter(|f| !f.excluded && f.severity == ErcSeverity::Warning)
        .count();
    json!({
        "findings": checked.findings,
        "hits": checked.hits,
        "errors": errors,
        "warnings": warnings,
        "tessellate": checked.tessellate,
        "unchecked": checked.unchecked,
        "standIns": checked.stand_ins,
        "milliseconds": u64::try_from(milliseconds).unwrap_or(u64::MAX),
    })
}

/// A box enclosure that fits a board: the outline plus a margin, bosses
/// under its mounting holes, high enough for its tallest part.
#[must_use]
pub fn fit_enclosure(pcb: &Pcb, levels: &Levels, top: f64, margin: f64) -> (BoxSpec, Point) {
    let center = board_center(pcb);
    let mut spec = BoxSpec::default();
    #[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
    if let Some(r) = pcb.outline_bounds() {
        spec.width = (r.max.x - r.min.x) as f64 / 1e6 + 2.0 * margin;
        spec.depth = (r.max.y - r.min.y) as f64 / 1e6 + 2.0 * margin;
    }
    #[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
    let (cx, cy) = (center.x as f64 / 1e6, center.y as f64 / 1e6);
    let mounting: Vec<ew_output::drill::Hole> = ew_output::drill::holes(pcb)
        .into_iter()
        .filter(|h| {
            h.end.is_none()
                && h.diameter >= 1.9e6
                && h.kind == ew_output::drill::HoleKind::Mechanical
        })
        .collect();
    spec.bosses = mounting
        .iter()
        .map(|h| [h.start.x / 1e6 - cx, h.start.y / 1e6 - cy])
        .collect();
    if let Some(hole) = mounting.iter().map(|h| h.diameter / 1e6).reduce(f64::max) {
        // Holes for thread-forming screws, bosses twice the hole.
        spec.hole_diameter = (hole - 0.6).max(1.0);
        spec.boss_diameter = (hole * 2.0).max(spec.hole_diameter + 2.0);
    }
    spec.opening = None;
    // Above the tallest part with a gap of 2 mm.
    spec.height = (spec.standoff + top.max(levels.body_top) + 2.0).ceil();
    (spec, center)
}

/// Runs a `mechanical.*` method.
///
/// # Errors
///
/// Bad parameters, unknown documents or bodies, unreadable files.
#[allow(clippy::too_many_lines, reason = "one match over the methods")]
pub fn call(
    session: Option<&Session>,
    library: &mut LibraryState,
    cache: &mut Cache,
    method: &str,
    params: &Value,
) -> Result<Outcome, EngineError> {
    let session = session.ok_or_else(EngineError::no_project)?;
    let root = session.root();
    match method {
        "mechanical.list" => {
            let params: DocParams = parse(params)?;
            let pcb = board_of(session, params.doc)?;
            let mut sorted: Vec<&MechanicalBody> = pcb.mechanical.iter().collect();
            sorted
                .sort_by(|a, b| ew_board3d::place::natural(&a.name, &b.name).then(a.id.cmp(&b.id)));
            let bodies = sorted
                .into_iter()
                .map(|b| body_json(root, b))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Outcome::Read(json!({
                "bodies": bodies,
                "clearance": clearance_of(rules_of(session)),
            })))
        }
        "mechanical.add" => {
            let params: AddParams = parse(params)?;
            let pcb = board_of(session, params.doc)?;
            if !params.file.is_absolute() {
                return Err(EngineError::invalid_params(
                    "the file must be an absolute path",
                ));
            }
            let ext = params
                .file
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            if !matches!(ext.as_deref(), Some("step" | "stp")) {
                return Err(EngineError::invalid_params("the file must be a STEP file"));
            }
            let data = std::fs::read(&params.file)
                .map_err(|e| EngineError::not_found(format!("{}: {e}", params.file.display())))?;
            if !data.starts_with(b"ISO-10303-21") {
                return Err(EngineError::invalid_params(
                    "not a STEP file (ISO 10303-21)",
                ));
            }
            let mut files = Vec::new();
            let path = if params.file.starts_with(root) {
                relative(root, &params.file)
            } else {
                let target = target_file(root, &params.file, &data);
                if !target.is_file() {
                    files.push((target.clone(), data));
                }
                relative(root, &target)
            };
            let name = params.name.unwrap_or_else(|| {
                params
                    .file
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Body")
                    .to_owned()
            });
            let center = board_center(pcb);
            let body = MechanicalBody {
                id: Id::random(),
                kind: params.kind.unwrap_or_else(|| guess_kind(&name)),
                name,
                path: path.clone(),
                position: params.position.unwrap_or(Vec3 {
                    x: center.x,
                    y: center.y,
                    z: 0,
                }),
                rotation: params.rotation.unwrap_or_default(),
                hidden: false,
                opacity: 100,
                check: true,
            };
            let id = body.id;
            Ok(Outcome::Commit {
                label: ADD,
                files,
                requests: vec![Request::Insert {
                    doc: params.doc,
                    object: Object::Mechanical(body),
                }],
                reply: json!({"id": id, "path": path}),
            })
        }
        "mechanical.update" => {
            let params: UpdateParams = parse(params)?;
            let pcb = board_of(session, params.doc)?;
            if pcb.mechanical.get(params.id).is_none() {
                return Err(EngineError::not_found(format!("no body {}", params.id)));
            }
            let mut fields = Map::new();
            if let Some(name) = params.name {
                if name.trim().is_empty() {
                    return Err(EngineError::invalid_params("the name must not be empty"));
                }
                fields.insert("name".into(), Value::String(name));
            }
            if let Some(kind) = params.kind {
                fields.insert("kind".into(), to_value(&kind)?);
            }
            if let Some(position) = params.position {
                fields.insert("position".into(), to_value(&position)?);
            }
            if let Some(rotation) = params.rotation {
                fields.insert("rotation".into(), to_value(&rotation)?);
            }
            if let Some(hidden) = params.hidden {
                fields.insert("hidden".into(), Value::Bool(hidden));
            }
            if let Some(opacity) = params.opacity {
                if !(5..=100).contains(&opacity) {
                    return Err(EngineError::invalid_params("opacity must be 5 to 100"));
                }
                fields.insert("opacity".into(), Value::from(opacity));
            }
            if let Some(check) = params.check {
                fields.insert("check".into(), Value::Bool(check));
            }
            if fields.is_empty() {
                return Err(EngineError::invalid_params("nothing to change"));
            }
            Ok(Outcome::Commit {
                label: EDIT,
                files: Vec::new(),
                requests: vec![Request::Patch {
                    doc: params.doc,
                    id: params.id,
                    fields,
                }],
                reply: json!({"id": params.id}),
            })
        }
        "mechanical.remove" => {
            let params: RemoveParams = parse(params)?;
            let pcb = board_of(session, params.doc)?;
            if params.ids.is_empty() {
                return Err(EngineError::invalid_params("no bodies given"));
            }
            for id in &params.ids {
                if pcb.mechanical.get(*id).is_none() {
                    return Err(EngineError::not_found(format!("no body {id}")));
                }
            }
            Ok(Outcome::Commit {
                label: REMOVE,
                files: Vec::new(),
                requests: params
                    .ids
                    .iter()
                    .map(|id| Request::Delete {
                        doc: params.doc,
                        id: *id,
                    })
                    .collect(),
                reply: json!({"removed": params.ids.len()}),
            })
        }
        "mechanical.check" => {
            let params: CheckParams = parse(params)?;
            if params
                .clearance
                .is_some_and(|c| !(0.0..=100.0).contains(&c))
            {
                return Err(EngineError::invalid_params("clearance must be 0 to 100 mm"));
            }
            let started = Instant::now();
            let checked = check(
                session,
                library,
                cache,
                params.doc,
                params.clearance,
                params.default_height,
            )?;
            Ok(Outcome::Read(checked_json(
                &checked,
                started.elapsed().as_millis(),
            )))
        }
        "mechanical.enclosure" => {
            let params: EnclosureParams = parse(params)?;
            let pcb = board_of(session, params.doc)?;
            let levels = Levels::of(&pcb.stackup);
            let margin = params.margin.unwrap_or(0.5).clamp(0.0, 50.0);
            let (spec, center) = if let Some(spec) = params.spec {
                (spec, board_center(pcb))
            } else {
                // The tallest part decides the height.
                let found = sources(pcb, root, library);
                let mesh_dir = library.mesh_dir();
                let mut ignored = BTreeSet::new();
                let mut meshes = HashMap::new();
                for source in &found {
                    if let Some(mesh) = cache.model_mesh(source, mesh_dir.as_deref(), &mut ignored)
                    {
                        meshes.insert(source.path.clone(), mesh);
                    }
                }
                let parts = mech::part_shapes(
                    pcb,
                    &levels,
                    &|path| meshes.get(path).cloned(),
                    DEFAULT_HEIGHT,
                );
                fit_enclosure(pcb, &levels, tallest(&parts, &levels), margin)
            };
            if !(spec.width > 0.0 && spec.depth > 0.0 && spec.height > spec.standoff) {
                return Err(EngineError::invalid_params("the enclosure has no room"));
            }
            let (bottom, lid) = enclosure::step_files(&spec, crate::VERSION);
            let mut files = Vec::new();
            let mut requests = Vec::new();
            let mut ids = Vec::new();
            for (name, stem, text, kind) in [
                (
                    "Enclosure bottom",
                    "enclosure-bottom",
                    bottom,
                    MechanicalKind::Enclosure,
                ),
                ("Enclosure lid", "enclosure-lid", lid, MechanicalKind::Lid),
            ] {
                let data = text.into_bytes();
                let target = target_file(root, Path::new(&format!("{stem}.step")), &data);
                if !target.is_file() {
                    files.push((target.clone(), data));
                }
                #[allow(clippy::cast_possible_truncation, reason = "millimetres")]
                let body = MechanicalBody {
                    id: Id::random(),
                    name: name.to_owned(),
                    path: relative(root, &target),
                    kind,
                    position: Vec3 {
                        x: center.x,
                        y: center.y,
                        z: -((spec.standoff * 1e6).round() as i64),
                    },
                    rotation: [Angle::ZERO; 3],
                    hidden: false,
                    opacity: if kind == MechanicalKind::Lid { 40 } else { 100 },
                    check: true,
                };
                ids.push(body.id);
                requests.push(Request::Insert {
                    doc: params.doc,
                    object: Object::Mechanical(body),
                });
            }
            Ok(Outcome::Commit {
                label: ENCLOSURE,
                files,
                requests,
                reply: json!({"ids": ids, "spec": spec}),
            })
        }
        other => Err(EngineError::new(
            ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

/// The findings of the enclosure check for the design rules check, or
/// none if the board has no checked bodies.
pub fn drc_findings(
    session: &Session,
    library: &mut LibraryState,
    cache: &mut Cache,
    doc: Id,
) -> Vec<Finding> {
    let has_bodies = matches!(
        session.project().document(doc),
        Some(ew_model::document::Document::Pcb(pcb)) if pcb.mechanical.iter().any(|b| b.check)
    );
    if !has_bodies {
        return Vec::new();
    }
    check(session, library, cache, doc, None, None).map_or_else(|_| Vec::new(), |c| c.findings)
}

/// The placement of a body (for the 3D view and the STEP export).
#[must_use]
pub fn matrix(body: &MechanicalBody) -> Matrix {
    mech::body_matrix(body)
}
