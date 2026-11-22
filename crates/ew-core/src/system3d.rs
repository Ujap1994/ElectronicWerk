//! The 3D assembly of a system (M18): every board at its place in one
//! scene, a board stacked onto its mating connector, collisions and
//! clearances between the boards (and their enclosures), and the assembly
//! as one STEP file.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use ew_board3d::board::{self, Board3d, BoardOptions, LayerMesh};
use ew_board3d::collide::{self, Bounds, Meeting, Solid, Tri};
use ew_board3d::mechanical::Shape;
use ew_board3d::mesh::Mesh;
use ew_board3d::place::{self, Matrix, Part, PlacedModel};
use ew_board3d::stack::Levels;
use ew_model::id::Id;
use ew_model::layer::Side;
use ew_model::pcb::Pcb;
use ew_model::system::{Pairing, System, SystemBoard};
use ew_model::units::{Angle, Vec3};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::board3d::{ModelSource, model_data, sources};
use crate::error::EngineError;
use crate::library::LibraryState;
use crate::session::Session;
use crate::system::{Loaded, SystemFinding, board_matrix, connector_of};

/// Default distance between two stacked boards when the lower connector
/// has no model: a 2.54 mm socket strip.
pub const STACK_GAP: f64 = 8.5;

#[allow(clippy::cast_possible_truncation, reason = "mesh coordinates are f32")]
fn transform_mesh(mesh: &Mesh, m: &Matrix) -> Mesh {
    let point = |p: &[f32; 3]| {
        let q = m.apply([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]);
        [q[0] as f32, q[1] as f32, q[2] as f32]
    };
    let normal = |n: &[f32; 3]| {
        let q = m.apply_direction([f64::from(n[0]), f64::from(n[1]), f64::from(n[2])]);
        let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt().max(1e-12);
        [(q[0] / l) as f32, (q[1] / l) as f32, (q[2] / l) as f32]
    };
    Mesh {
        positions: mesh.positions.iter().map(point).collect(),
        normals: mesh.normals.iter().map(normal).collect(),
        indices: mesh.indices.clone(),
    }
}

fn union_bounds(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].min(b[2]),
        a[3].max(b[3]),
        a[4].max(b[4]),
        a[5].max(b[5]),
    ]
}

fn transform_bounds(b: [f64; 6], m: &Matrix) -> [f64; 6] {
    let mut out = [
        f64::INFINITY,
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for i in 0..8 {
        let p = m.apply([
            if i & 1 == 0 { b[0] } else { b[3] },
            if i & 2 == 0 { b[1] } else { b[4] },
            if i & 4 == 0 { b[2] } else { b[5] },
        ]);
        out = union_bounds(out, [p[0], p[1], p[2], p[0], p[1], p[2]]);
    }
    out
}

fn identity(text: &str) -> String {
    text.to_owned()
}

/// Builds a board (its meshes) with the right text resolver.
fn build(session: &Session, l: &Loaded, pcb: &Pcb, options: &BoardOptions) -> Board3d {
    if l.local {
        let doc = pcb.id;
        let resolve = |text: &str| session.resolve_text(text, Some(doc));
        board::build(pcb, &resolve, options)
    } else {
        board::build(pcb, &identity, options)
    }
}

/// The scene of the system's 3D assembly: the meshes of every shown board
/// at its place (one buffer, like `board3d.scene`), the parts with their
/// models, and the boards' enclosure bodies as parts.
///
/// # Errors
///
/// Fails if no board can be shown.
#[allow(clippy::too_many_lines, reason = "boards, parts, bodies")]
pub fn scene(
    session: &Session,
    library: &mut LibraryState,
    system: &System,
    loaded: &[Loaded],
) -> Result<(Value, Vec<u8>), EngineError> {
    let mut merged: Option<Board3d> = None;
    let mut layers: BTreeMap<String, LayerMesh> = BTreeMap::new();
    let mut parts: Vec<Part> = Vec::new();
    let mut models: Vec<ModelSource> = Vec::new();
    let mut seen_models: BTreeSet<String> = BTreeSet::new();
    let mut boards = Vec::new();
    for l in loaded {
        let Some(pcb) = l.pcb.as_ref() else { continue };
        if l.entry.hidden {
            continue;
        }
        let m = board_matrix(&l.entry);
        let built = build(session, l, pcb, &BoardOptions::default());
        let bounds = transform_bounds(built.bounds, &m);
        boards.push(json!({"id": l.entry.id, "name": l.entry.name, "bounds": bounds}));
        // Models: paths of other projects become their files, so two
        // projects' `models/part.step` stay apart.
        let found = sources(pcb, &l.root, library);
        let key_of = |path: &str| -> String {
            if l.local {
                return path.to_owned();
            }
            found
                .iter()
                .find(|s| s.path == path)
                .and_then(|s| s.file.clone())
                .unwrap_or_else(|| path.to_owned())
        };
        for source in &found {
            let key = key_of(&source.path);
            if seen_models.insert(key.clone()) {
                let mut s = source.clone();
                s.path = key;
                models.push(s);
            }
        }
        for mut part in place::parts(pcb, &built.levels) {
            part.matrix = m.mul(&part.matrix);
            part.models = part
                .models
                .into_iter()
                .map(|model| PlacedModel {
                    path: key_of(&model.path),
                    matrix: m.mul(&model.matrix),
                })
                .collect();
            parts.push(part);
        }
        // The board's enclosure.
        for body in pcb.mechanical.iter().filter(|b| !b.hidden) {
            let file = crate::mechanical::file_of(&l.root, body);
            let path = file.to_string_lossy().into_owned();
            if seen_models.insert(path.clone()) {
                models.push(ModelSource {
                    path: path.clone(),
                    file: Some(path.clone()),
                    format: Some("step"),
                    entry: None,
                    mesh: None,
                });
            }
            let matrix = m.mul(&crate::mechanical::matrix(body));
            parts.push(Part {
                id: body.id,
                designator: format!("{}: {}", l.entry.name, body.name),
                value: String::new(),
                footprint: String::new(),
                side: Side::Top,
                dnp: false,
                matrix,
                pads: [0.0; 4],
                models: vec![PlacedModel { path, matrix }],
            });
        }
        let Board3d {
            levels,
            colors,
            area,
            holes,
            body,
            layers: own_layers,
            barrels,
            bounds: _,
            warnings,
        } = built;
        let body = transform_mesh(&body, &m);
        let barrels = transform_mesh(&barrels, &m);
        for layer in own_layers {
            let key = layer.key();
            let mesh = transform_mesh(&layer.mesh, &m);
            layers
                .entry(key)
                .and_modify(|existing| existing.mesh.append(&mesh))
                .or_insert(LayerMesh { mesh, ..layer });
        }
        match &mut merged {
            None => {
                merged = Some(Board3d {
                    levels,
                    colors,
                    area,
                    holes,
                    body,
                    layers: Vec::new(),
                    barrels,
                    bounds,
                    warnings: warnings
                        .into_iter()
                        .map(|w| format!("{}: {w}", l.entry.name))
                        .collect(),
                });
            }
            Some(all) => {
                all.body.append(&body);
                all.barrels.append(&barrels);
                all.holes.extend(holes);
                all.bounds = union_bounds(all.bounds, bounds);
                all.warnings.extend(
                    warnings
                        .into_iter()
                        .map(|w| format!("{}: {w}", l.entry.name)),
                );
            }
        }
    }
    let mut all =
        merged.ok_or_else(|| EngineError::invalid_params("no board of the system can be shown"))?;
    all.layers = layers.into_values().collect();
    let (meta, data) = ew_board3d::scene::encode(&system.name, &all, parts);
    let mut value = serde_json::to_value(&meta).map_err(EngineError::internal)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "models".into(),
            serde_json::to_value(&models).map_err(EngineError::internal)?,
        );
        object.insert("boards".into(), Value::Array(boards));
    }
    Ok((value, data))
}

// ---------------------------------------------------------------------------
// Stacking.

/// Board heights: thickness and the surface where parts sit.
fn levels_of(pcb: &Pcb) -> Levels {
    Levels::of(&pcb.stackup)
}

/// How far a connector reaches from its board (mm): its model's height, or
/// [`STACK_GAP`] without one.
fn connector_height(
    library: &mut LibraryState,
    mechanical: &mut crate::mechanical::Cache,
    l: &Loaded,
    pcb: &Pcb,
    designator: &str,
) -> f64 {
    let mut tessellate = BTreeSet::new();
    let shapes =
        crate::mechanical::part_shapes_in(&l.root, library, mechanical, pcb, 0.0, &mut tessellate);
    let levels = levels_of(pcb);
    let Some(fp) = pcb.footprints.iter().find(|f| f.designator == designator) else {
        return STACK_GAP;
    };
    let Some(part) = shapes.iter().find(|p| p.id == fp.id && !p.stand_in) else {
        return STACK_GAP;
    };
    let bounds = shape_bounds(&part.shapes);
    if bounds.is_empty() {
        return STACK_GAP;
    }
    let height = match fp.side {
        Side::Top => bounds.max[2] - levels.mount(Side::Top),
        Side::Bottom => levels.mount(Side::Bottom) - bounds.min[2],
    };
    if height > 0.5 { height } else { STACK_GAP }
}

fn shape_bounds(shapes: &[Shape]) -> Bounds {
    shapes.iter().fold(Bounds::EMPTY, |b, s| {
        let sb = match s {
            Shape::Mesh(mesh, matrix) => Bounds::of_mesh(mesh, matrix),
            Shape::Box(tris) => {
                let mut bb = Bounds::EMPTY;
                for t in tris {
                    for p in t {
                        bb.add(*p);
                    }
                }
                bb
            }
        };
        b.union(&sb)
    })
}

/// Where board `b` goes so that its connector mates the other board's
/// connector: turned about z (and over, when both connectors face the same
/// way) to put the pins on each other, `gap` mm away along the lower
/// connector's direction.
#[allow(clippy::too_many_arguments, reason = "both connectors and the gap")]
fn stacked(
    a: &Loaded,
    pa: &Pcb,
    da: &str,
    b: &Loaded,
    pb: &Pcb,
    db: &str,
    pairing: Pairing,
    gap: f64,
    flip: bool,
) -> Result<(Vec3, [Angle; 3]), EngineError> {
    let fa = pa
        .footprints
        .iter()
        .find(|f| f.designator == da)
        .ok_or_else(|| EngineError::not_found(format!("{} has no {da}", a.entry.name)))?;
    let fb = pb
        .footprints
        .iter()
        .find(|f| f.designator == db)
        .ok_or_else(|| EngineError::not_found(format!("{} has no {db}", b.entry.name)))?;
    let ca = connector_of(pa, fa);
    let cb = connector_of(pb, fb);
    let (la, lb) = (levels_of(pa), levels_of(pb));
    let ma = board_matrix(&a.entry);
    // A's connector face: its surface and the direction it faces.
    let up_a = ma.apply_direction([0.0, 0.0, if fa.side == Side::Top { 1.0 } else { -1.0 }]);
    // Connectors sit on the outer copper.
    let surface_a = la.mount(fa.side);
    let surface_b = lb.mount(fb.side);
    // The pin pairs: by number, or with mirrored rows (a header on the
    // bottom side meets a socket on the top side that way); for pairing
    // by position both are tried and the better fit is taken.
    let pairs_by = |mirrored: bool| -> Vec<([f64; 2], [f64; 2])> {
        let mut pairs = Vec::new();
        for pin in &ca.pins {
            let partner = if mirrored {
                pin.number.parse::<u64>().ok().and_then(|n| {
                    let other = if n % 2 == 1 { n + 1 } else { n - 1 };
                    cb.pins
                        .iter()
                        .find(|q| q.number.parse::<u64>().ok() == Some(other))
                })
            } else {
                cb.pins.iter().find(|q| q.number == pin.number)
            };
            if let Some(q) = partner {
                let t = ma.apply([pin.at[0], pin.at[1], surface_a]);
                let s = if flip { [q.at[0], -q.at[1]] } else { q.at };
                pairs.push(([t[0], t[1]], s));
            }
        }
        pairs
    };
    let candidates: Vec<Vec<([f64; 2], [f64; 2])>> = match pairing {
        Pairing::Number => vec![pairs_by(false)],
        Pairing::Mirrored => vec![pairs_by(true)],
        Pairing::Position => vec![pairs_by(false), pairs_by(true)],
    };
    let fits: Vec<(f64, f64, [f64; 2])> = candidates
        .iter()
        .filter(|pairs| !pairs.is_empty())
        .map(|pairs| fit(pairs))
        .collect();
    let Some(&(_, theta, shift)) = fits.iter().min_by(|a, b| a.0.total_cmp(&b.0)) else {
        return Err(EngineError::invalid_params(
            "the connectors have no pins in common",
        ));
    };
    // Height: B's connector surface `gap` above (below) A's.
    let target_z = ma.apply([0.0, 0.0, surface_a])[2] + gap * up_a[2].signum();
    let local_z = if flip { -surface_b } else { surface_b };
    let z = target_z - local_z;
    let degrees = theta.to_degrees();
    let degrees = if degrees < 0.0 {
        degrees + 360.0
    } else {
        degrees
    };
    #[allow(clippy::cast_possible_truncation, reason = "board coordinates")]
    Ok((
        Vec3 {
            x: (shift[0] * 1e6).round() as i64,
            y: (shift[1] * 1e6).round() as i64,
            z: (z * 1e6).round() as i64,
        },
        [
            Angle::from_degrees(if flip { 180 } else { 0 }),
            Angle::ZERO,
            Angle::from_degrees_f64(degrees).unwrap_or(Angle::ZERO),
        ],
    ))
}

/// The turn (radians) and shift (mm) that put the source points of the
/// pairs on their targets (2D Kabsch), with the remaining error (RMS, mm).
fn fit(pairs: &[([f64; 2], [f64; 2])]) -> (f64, f64, [f64; 2]) {
    #[allow(clippy::cast_precision_loss, reason = "pin counts")]
    let n = pairs.len() as f64;
    let ct = pairs
        .iter()
        .fold([0.0, 0.0], |c, (t, _)| [c[0] + t[0] / n, c[1] + t[1] / n]);
    let cs = pairs
        .iter()
        .fold([0.0, 0.0], |c, (_, s)| [c[0] + s[0] / n, c[1] + s[1] / n]);
    let (mut sxx, mut sxy) = (0.0, 0.0);
    for (t, s) in pairs {
        let (px, py) = (s[0] - cs[0], s[1] - cs[1]);
        let (qx, qy) = (t[0] - ct[0], t[1] - ct[1]);
        sxx += px * qx + py * qy;
        sxy += px * qy - py * qx;
    }
    let mut theta = if pairs.len() > 1 { sxy.atan2(sxx) } else { 0.0 };
    // Exact quarter turns stay exact.
    let quarter = std::f64::consts::FRAC_PI_2;
    let snapped = (theta / quarter).round() * quarter;
    if (theta - snapped).abs() < 1e-6 {
        theta = snapped;
    }
    let (sin, cos) = theta.sin_cos();
    let rotated = [cos * cs[0] - sin * cs[1], sin * cs[0] + cos * cs[1]];
    let shift = [ct[0] - rotated[0], ct[1] - rotated[1]];
    let error = (pairs
        .iter()
        .map(|(t, s)| {
            let x = cos * s[0] - sin * s[1] + shift[0] - t[0];
            let y = sin * s[0] + cos * s[1] + shift[1] - t[1];
            x * x + y * y
        })
        .sum::<f64>()
        / n)
        .sqrt();
    (error, theta, shift)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StackParams {
    doc: Id,
    link: Id,
    /// Distance between the connectors' boards (mm); default: the lower
    /// connector's height.
    #[serde(default)]
    gap: Option<f64>,
    /// Turn the board over (a mezzanine board facing down).
    #[serde(default)]
    flip: bool,
}

/// `system.stack`: the new place of the link's second board.
///
/// # Errors
///
/// Unknown link, unreadable boards, connectors without common pins.
pub fn stack(
    library: &mut LibraryState,
    mechanical: &mut crate::mechanical::Cache,
    system: &System,
    loaded: &[Loaded],
    params: &Value,
) -> Result<(Vec<SystemBoard>, Value), EngineError> {
    let p: StackParams = serde_json::from_value(params.clone())
        .map_err(|e| EngineError::invalid_params(e.to_string()))?;
    let _ = p.doc;
    let link = system
        .links
        .iter()
        .find(|l| l.id == p.link)
        .ok_or_else(|| EngineError::not_found(format!("no connection {}", p.link)))?;
    let find = |id: Id| {
        loaded
            .iter()
            .find(|l| l.entry.id == id)
            .ok_or_else(|| EngineError::not_found(format!("no system board {id}")))
    };
    let (a, b) = (find(link.a.board)?, find(link.b.board)?);
    let (Some(pa), Some(pb)) = (a.pcb.clone(), b.pcb.clone()) else {
        return Err(EngineError::invalid_params(
            "a board of the connection cannot be read",
        ));
    };
    let gap = match p.gap {
        Some(g) if (0.0..=500.0).contains(&g) => g,
        Some(_) => return Err(EngineError::invalid_params("gap must be 0 to 500 mm")),
        None => connector_height(library, mechanical, a, &pa, &link.a.designator),
    };
    let (position, rotation) = stacked(
        a,
        &pa,
        &link.a.designator,
        b,
        &pb,
        &link.b.designator,
        link.pairing,
        gap,
        p.flip,
    )?;
    let mut boards = system.boards.clone();
    if let Some(entry) = boards.iter_mut().find(|e| e.id == b.entry.id) {
        entry.position = position;
        entry.rotation = rotation;
    }
    Ok((
        boards,
        json!({"board": b.entry.id, "position": position, "rotation": rotation, "gap": gap}),
    ))
}

// ---------------------------------------------------------------------------
// Collisions between the boards.

/// Something of one board, in system coordinates.
struct Item {
    board: usize,
    label: String,
    /// The footprint id of a part.
    part: Option<Id>,
    /// `true` for the board body.
    is_board: bool,
    /// `true` for an enclosure body.
    is_body: bool,
    tris: Vec<Tri>,
    bounds: Bounds,
}

fn item(board: usize, label: String, tris: Vec<Tri>) -> Item {
    let mut bounds = Bounds::EMPTY;
    for t in &tris {
        for p in t {
            bounds.add(*p);
        }
    }
    Item {
        board,
        label,
        part: None,
        is_board: false,
        is_body: false,
        tris,
        bounds,
    }
}

fn transformed(tris: impl Iterator<Item = Tri>, m: &Matrix) -> Vec<Tri> {
    tris.map(|t| t.map(|p| m.apply(p))).collect()
}

/// The collision and clearance findings between the boards of a system
/// (each board's own parts are the board's business), with the STEP files
/// that still need a mesh.
#[allow(clippy::too_many_lines, reason = "items, then pairs")]
pub fn collisions(
    session: &Session,
    library: &mut LibraryState,
    mechanical: &mut crate::mechanical::Cache,
    system: &System,
    loaded: &[Loaded],
    clearance: f64,
) -> (Vec<SystemFinding>, Vec<String>) {
    let mut tessellate = BTreeSet::new();
    let mut items: Vec<Item> = Vec::new();
    let shown: Vec<(usize, &Loaded, Arc<Pcb>)> = loaded
        .iter()
        .filter(|l| !l.entry.hidden)
        .filter_map(|l| l.pcb.clone().map(|p| (l, p)))
        .enumerate()
        .map(|(i, (l, p))| (i, l, p))
        .collect();
    if shown.len() < 2 {
        return (Vec::new(), Vec::new());
    }
    for (index, l, pcb) in &shown {
        let m = board_matrix(&l.entry);
        let options = BoardOptions {
            body_only: true,
            tolerance: 20_000.0,
            ..BoardOptions::default()
        };
        let built = build(session, l, pcb, &options);
        if !built.body.is_empty() {
            let mut it = item(
                *index,
                format!("{} (board)", l.entry.name),
                transformed(collide::triangles(&built.body, &Matrix::IDENTITY), &m),
            );
            it.is_board = true;
            items.push(it);
        }
        for part in crate::mechanical::part_shapes_in(
            &l.root,
            library,
            mechanical,
            pcb,
            crate::mechanical::DEFAULT_HEIGHT,
            &mut tessellate,
        ) {
            let mut tris = Vec::new();
            for shape in &part.shapes {
                match shape {
                    Shape::Mesh(mesh, matrix) => {
                        tris.extend(transformed(collide::triangles(mesh, matrix), &m));
                    }
                    Shape::Box(b) => tris.extend(transformed(b.iter().copied(), &m)),
                }
            }
            let mut it = item(
                *index,
                format!("{} {}", l.entry.name, part.designator),
                tris,
            );
            it.part = Some(part.id);
            items.push(it);
        }
        let mut unchecked = Vec::new();
        let mesh_dir = library.mesh_dir();
        for (id, solid) in crate::mechanical::body_solids(
            &l.root,
            mesh_dir.as_deref(),
            mechanical,
            pcb,
            &mut unchecked,
            &mut tessellate,
        ) {
            let name = pcb
                .mechanical
                .get(id)
                .map_or_else(String::new, |b| b.name.clone());
            let mut it = item(
                *index,
                format!("{}: {name}", l.entry.name),
                transformed(solid.triangles.iter().copied(), &m),
            );
            it.is_body = true;
            items.push(it);
        }
    }
    // Mated connectors may touch each other and the other board.
    let index_of = |id: Id| shown.iter().position(|(_, l, _)| l.entry.id == id);
    let part_id = |board: usize, designator: &str| {
        shown
            .get(board)
            .and_then(|(_, _, pcb)| pcb.footprints.iter().find(|f| f.designator == designator))
            .map(|f| f.id)
    };
    let mut mated: BTreeSet<(Id, Id)> = BTreeSet::new();
    let mut against_board: BTreeSet<(Id, usize)> = BTreeSet::new();
    for link in &system.links {
        if let (Some(ia), Some(ib)) = (index_of(link.a.board), index_of(link.b.board))
            && let (Some(pa), Some(pb)) = (
                part_id(ia, &link.a.designator),
                part_id(ib, &link.b.designator),
            )
        {
            mated.insert((pa, pb));
            mated.insert((pb, pa));
            against_board.insert((pa, ib));
            against_board.insert((pb, ia));
        }
    }
    let mut solids: HashMap<usize, Solid> = HashMap::new();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for i in 0..items.len() {
        for j in 0..items.len() {
            let (a, b) = (&items[i], &items[j]);
            if a.board >= b.board || (a.is_body && b.is_body) {
                continue;
            }
            if let (Some(pa), Some(pb)) = (a.part, b.part)
                && mated.contains(&(pa, pb))
            {
                continue;
            }
            if (a
                .part
                .is_some_and(|p| against_board.contains(&(p, b.board)))
                && b.is_board)
                || (b
                    .part
                    .is_some_and(|p| against_board.contains(&(p, a.board)))
                    && a.is_board)
            {
                continue;
            }
            if !a.bounds.grow(clearance).intersects(&b.bounds) {
                continue;
            }
            let solid = solids
                .entry(j)
                .or_insert_with(|| Solid::new(items[j].tris.clone()));
            let Some(meeting) = collide::meet(a.tris.iter().copied(), &a.bounds, solid, clearance)
            else {
                continue;
            };
            let (overlap, distance, point) = match meeting {
                Meeting::Overlap { at } => (true, 0.0, at),
                Meeting::Gap { distance, b, .. } => {
                    if distance >= clearance - 1e-6 {
                        continue;
                    }
                    (
                        false,
                        if distance < collide::TOUCH {
                            0.0
                        } else {
                            distance
                        },
                        b,
                    )
                }
            };
            let code = if overlap {
                "system-collision"
            } else {
                "system-clearance"
            };
            let key = format!("{code}:{}/{}", a.label, b.label);
            if !seen.insert(key.clone()) {
                continue;
            }
            let mm = |v: f64| ew_pcb::drc::mm(v * 1e6);
            let message = if overlap {
                format!("{} collides with {}", a.label, b.label)
            } else {
                format!(
                    "{} is {} from {} (minimum {})",
                    a.label,
                    mm(distance),
                    b.label,
                    mm(clearance)
                )
            };
            let mut args = BTreeMap::new();
            args.insert("a".to_owned(), a.label.clone());
            args.insert("b".to_owned(), b.label.clone());
            args.insert("distance".to_owned(), mm(distance));
            args.insert("minimum".to_owned(), mm(clearance));
            args.insert(
                "point".to_owned(),
                format!("{:.3},{:.3},{:.3}", point[0], point[1], point[2]),
            );
            out.push(SystemFinding {
                code: code.to_owned(),
                severity: if overlap { "error" } else { "warning" }.to_owned(),
                message,
                args,
                connection: None,
                pins: Vec::new(),
                excluded: system.exclusions.contains(&key),
                key,
            });
        }
    }
    (out, tessellate.into_iter().collect())
}

// ---------------------------------------------------------------------------
// STEP.

/// The system as one STEP file: every shown board's assembly (with its
/// enclosure) at its place. Returns the file and notes.
pub fn step(
    session: &Session,
    library: &mut LibraryState,
    system: &System,
    loaded: &[Loaded],
    version: &str,
    created: &str,
) -> (String, Vec<String>) {
    use ew_board3d::export::{ExportOptions, Format};
    use ew_board3d::step::{Child, ModelData, Origin};
    let mut children = Vec::new();
    let mut notes = Vec::new();
    let mesh_dir = library.mesh_dir();
    for l in loaded {
        let Some(pcb) = l.pcb.as_ref() else {
            notes.push(format!(
                "{}: {}",
                l.entry.name,
                l.error.clone().unwrap_or_default()
            ));
            continue;
        };
        if l.entry.hidden {
            continue;
        }
        let found = sources(pcb, &l.root, library);
        let root = l.root.clone();
        let mut models = |path: &str| -> Option<ModelData> {
            if let Some(body) = pcb.mechanical.iter().find(|b| b.path == path) {
                return crate::board3d::body_data(
                    &crate::mechanical::file_of(&root, body),
                    mesh_dir.as_deref(),
                    true,
                );
            }
            let source = found.iter().find(|s| s.path == path)?;
            model_data(source, mesh_dir.as_deref(), true)
        };
        let options = ExportOptions {
            format: Format::Step,
            origin: Origin::Absolute,
            include_dnp: false,
            units: ew_board3d::vrml::Units::default(),
            name: format!("{}.step", l.entry.name),
            created: created.to_owned(),
            version: version.to_owned(),
            mechanical: true,
        };
        let (data, report) = if l.local {
            let doc = pcb.id;
            let resolve = |text: &str| session.resolve_text(text, Some(doc));
            ew_board3d::export::export(pcb, &resolve, &mut models, &options)
        } else {
            ew_board3d::export::export(pcb, &identity, &mut models, &options)
        };
        if !report.missing.is_empty() {
            notes.push(format!(
                "{}: {} models not found",
                l.entry.name,
                report.missing.len()
            ));
        }
        children.push(Child {
            name: l.entry.name.clone(),
            matrix: board_matrix(&l.entry),
            text: String::from_utf8(data).unwrap_or_default(),
        });
    }
    let (text, failed) = ew_board3d::step::assembly(&system.name, &children, created, version);
    notes.extend(
        failed
            .into_iter()
            .map(|n| format!("{n}: could not be merged")),
    );
    (text, notes)
}
