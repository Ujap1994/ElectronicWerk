//! The board as meshes: the body (outline minus holes, extruded through the
//! dielectric stack), the copper, mask, silkscreen and paste of each side,
//! optionally the inner copper layers, and the plated barrels.
//!
//! Every layer is built from the same features as the Gerber files
//! ([`ew_output::art::BoardArt`]), so the 3D view shows exactly what is
//! manufactured: copper is the union of its features minus the holes, the
//! mask covers the board except its openings, the silkscreen is cut at the
//! mask openings and the board edge.

use std::f64::consts::{PI, TAU};

use ew_model::layer::{CopperLayer, Layer, Side};
use ew_model::pcb::Pcb;
use ew_output::art::{ArtOptions, BoardArt, Feature, Resolver};
use ew_output::drill::{self, Hole};
use ew_pcb::geom::{self, BBox, Poly, V, arc_segments, circle_polygon, stadium_polygon};
use ew_pcb::items;
use ew_pcb::poly;
use rayon::prelude::*;
use rstar::{AABB, RTree, RTreeObject};
use serde::Serialize;

use crate::mesh::{Mesh, Polygons, circle_segments};
use crate::mm;
use crate::stack::{Colors, Levels, colors};

/// Options of the board model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardOptions {
    /// Largest chord error of curves (nanometres).
    pub tolerance: f64,
    /// Also build the inner copper layers (for section views).
    pub inner: bool,
    /// What the layers show (as for the Gerber files).
    pub art: ArtOptions,
}

impl Default for BoardOptions {
    fn default() -> Self {
        Self {
            tolerance: 5_000.0,
            inner: false,
            art: ArtOptions::default(),
        }
    }
}

/// What a layer mesh shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LayerKind {
    /// Copper (outer layers carry the surface finish).
    Copper,
    /// Solder mask.
    Mask,
    /// Silkscreen.
    Silk,
    /// Solder paste.
    Paste,
}

/// The mesh of one board layer.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerMesh {
    /// What it shows.
    pub kind: LayerKind,
    /// The side of outer layers; `None` for inner copper.
    pub side: Option<Side>,
    /// The copper layer of copper meshes.
    pub copper: Option<CopperLayer>,
    /// Triangles.
    pub mesh: Mesh,
}

impl LayerMesh {
    /// A stable key such as `copper:top`, `mask:bottom` or `copper:in1`.
    #[must_use]
    pub fn key(&self) -> String {
        let kind = match self.kind {
            LayerKind::Copper => "copper",
            LayerKind::Mask => "mask",
            LayerKind::Silk => "silk",
            LayerKind::Paste => "paste",
        };
        match (self.side, self.copper) {
            (_, Some(CopperLayer::Inner(n))) => format!("{kind}:in{n}"),
            (Some(side), _) => format!("{kind}:{}", side.as_str()),
            (None, _) => kind.to_owned(),
        }
    }
}

/// The 3D board.
#[derive(Debug, Clone)]
pub struct Board3d {
    /// Heights.
    pub levels: Levels,
    /// Material colours.
    pub colors: Colors,
    /// The board area (nm): outline with its cut-outs, before drilling.
    pub area: Polygons,
    /// Every drilled hole.
    pub holes: Vec<Hole>,
    /// The body.
    pub body: Mesh,
    /// Copper, mask, silkscreen and paste.
    pub layers: Vec<LayerMesh>,
    /// Plated barrels of vias and through-hole pads.
    pub barrels: Mesh,
    /// Bounds of the board in millimetres `[x0, y0, z0, x1, y1, z1]`.
    pub bounds: [f64; 6],
    /// Problems found while building (no outline, open outline, …).
    pub warnings: Vec<String>,
}

/// Polygons of one plotted feature.
pub fn feature_polygons(feature: &Feature, tolerance: f64, out: &mut Polygons) {
    match feature {
        Feature::Flash { shape, .. } => out.extend(shape.outlines(0.0, tolerance)),
        Feature::Line { a, b, width, .. } => {
            if *width > 0.0 {
                out.push(vec![stadium_polygon(*a, *b, width / 2.0, tolerance)]);
            }
        }
        Feature::Arc {
            start,
            end,
            center,
            ccw,
            width,
            ..
        } => {
            if *width > 0.0 {
                arc_polygons(*start, *end, *center, *ccw, *width / 2.0, tolerance, out);
            }
        }
        Feature::Region { outline, holes, .. } => {
            if outline.len() >= 3 {
                let mut polygon = vec![outline.clone()];
                polygon.extend(holes.iter().filter(|h| h.len() >= 3).cloned());
                out.push(polygon);
            }
        }
    }
}

fn direction(angle: f64) -> V {
    V::new(angle.cos(), angle.sin())
}

/// A thick arc as one ring with round ends (a ring with a hole for full
/// circles).
fn arc_polygons(
    start: V,
    end: V,
    center: V,
    ccw: bool,
    hw: f64,
    tolerance: f64,
    out: &mut Polygons,
) {
    let r = center.dist(start);
    let a0 = (start.y - center.y).atan2(start.x - center.x);
    let a1 = (end.y - center.y).atan2(end.x - center.x);
    let full = start.dist(end) < 1.0;
    if full {
        let mut polygon = vec![circle_polygon(center, r + hw, tolerance)];
        if r - hw > tolerance {
            polygon.push(circle_polygon(center, r - hw, tolerance));
        }
        out.push(polygon);
        return;
    }
    let sweep = if ccw {
        (a1 - a0).rem_euclid(TAU)
    } else {
        -(a0 - a1).rem_euclid(TAU)
    };
    if r - hw <= tolerance {
        // The pen is wider than the radius: segments with round ends.
        let n = arc_segments(r.max(tolerance), sweep, tolerance);
        let points: Vec<V> = (0..=n)
            .map(|i| center.add(direction(a0 + sweep * i as f64 / n as f64).scale(r)))
            .collect();
        for pair in points.windows(2) {
            out.push(vec![stadium_polygon(pair[0], pair[1], hw, tolerance)]);
        }
        return;
    }
    let s = sweep.signum();
    let n = arc_segments(r + hw, sweep, tolerance);
    let caps = arc_segments(hw, PI, tolerance);
    let mut ring = Vec::with_capacity(2 * n + 2 * caps + 4);
    for i in 0..=n {
        ring.push(center.add(direction(a0 + sweep * i as f64 / n as f64).scale(r + hw)));
    }
    let a_end = a0 + sweep;
    for i in 1..caps {
        ring.push(end.add(direction(a_end + PI * s * i as f64 / caps as f64).scale(hw)));
    }
    for i in (0..=n).rev() {
        ring.push(center.add(direction(a0 + sweep * i as f64 / n as f64).scale(r - hw)));
    }
    for i in 1..caps {
        ring.push(start.add(direction(a0 + PI * s + PI * s * i as f64 / caps as f64).scale(hw)));
    }
    out.push(vec![ring]);
}

/// Polygons of every feature of a layer.
fn layer_polygons(art: &BoardArt<'_>, layer: Layer, tolerance: f64) -> Polygons {
    let mut out = Vec::new();
    for feature in art.features(layer) {
        feature_polygons(&feature, tolerance, &mut out);
    }
    out
}

/// The polygon of a hole.
#[must_use]
pub fn hole_polygon(hole: &Hole, tolerance: f64) -> Vec<V> {
    let r = hole.diameter / 2.0;
    match hole.end {
        Some(end) => stadium_polygon(hole.start, end, r, tolerance),
        None => circle_polygon(hole.start, r, tolerance),
    }
}

/// The board area: the outline with its cut-outs, or a rectangle around
/// the content if the board has no closed outline.
fn board_area(pcb: &Pcb, art: &BoardArt<'_>, warnings: &mut Vec<String>) -> Polygons {
    let outline = items::outline(pcb);
    if !outline.open_ends.is_empty() {
        warnings.push(format!(
            "the board outline has {} open ends; only closed outlines shape the board",
            outline.open_ends.len()
        ));
    }
    if let Some((first, rest)) = outline.rings.split_first() {
        // Rings inside cut-outs would be islands; they are cut-outs too.
        let mut polygon = vec![first.clone()];
        polygon.extend(rest.iter().cloned());
        return poly::union(&[polygon]);
    }
    warnings.push("the board has no outline; the 3D board is a rectangle around the parts".into());
    let mut bbox = BBox::EMPTY;
    let mut polygons = Vec::new();
    for layer in [
        Layer::TOP_COPPER,
        Layer::BOTTOM_COPPER,
        Layer::Silkscreen(Side::Top),
        Layer::Silkscreen(Side::Bottom),
    ] {
        polygons.extend(layer_polygons(art, layer, 50_000.0));
    }
    for polygon in &polygons {
        if let Some(ring) = polygon.first() {
            bbox = bbox.union(BBox::of_points(ring));
        }
    }
    if bbox.is_empty() {
        bbox = BBox::of(V::new(0.0, 0.0)).with(V::new(50e6, 50e6));
    }
    let b = bbox.grow(2e6);
    vec![vec![vec![
        V::new(b.min.x, b.min.y),
        V::new(b.max.x, b.min.y),
        V::new(b.max.x, b.max.y),
        V::new(b.min.x, b.max.y),
    ]]]
}

/// Polygons that cut other layers (holes, mask openings), found by their
/// boxes.
struct Clip {
    polygons: Polygons,
    tree: RTree<Boxed>,
}

struct Boxed {
    index: usize,
    bbox: BBox,
}

impl RTreeObject for Boxed {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.bbox.aabb()
    }
}

fn polygon_bbox(polygon: &[Vec<V>]) -> BBox {
    polygon
        .first()
        .map_or(BBox::EMPTY, |ring| BBox::of_points(ring))
}

impl Clip {
    fn new(polygons: Polygons) -> Self {
        let boxes = polygons
            .iter()
            .enumerate()
            .map(|(index, polygon)| Boxed {
                index,
                bbox: polygon_bbox(polygon),
            })
            .collect();
        Self {
            polygons,
            tree: RTree::bulk_load(boxes),
        }
    }

    fn near(&self, bbox: BBox) -> Polygons {
        self.tree
            .locate_in_envelope_intersecting(bbox.aabb())
            .filter(|boxed| boxed.bbox.intersects(bbox))
            .map(|boxed| self.polygons[boxed.index].clone())
            .collect()
    }
}

/// The board area for "is this completely on the board" tests.
struct Area {
    polygons: Polygons,
    parts: Vec<Poly>,
}

impl Area {
    fn new(polygons: &Polygons) -> Self {
        Self {
            polygons: polygons.clone(),
            parts: polygons
                .iter()
                .filter(|p| p.first().is_some_and(|o| o.len() >= 3))
                .map(|p| Poly::new(p[0].clone(), p[1..].to_vec()))
                .collect(),
        }
    }

    /// `true` if the box lies inside one polygon of the area without
    /// touching its edges.
    fn holds(&self, bbox: BBox) -> bool {
        let corners = [
            bbox.min,
            V::new(bbox.max.x, bbox.min.y),
            bbox.max,
            V::new(bbox.min.x, bbox.max.y),
        ];
        self.parts.iter().any(|part| {
            if !corners.iter().all(|c| part.contains(*c)) {
                return false;
            }
            let mut crossed = false;
            part.edges_near(bbox, &mut |a, b| {
                if BBox::of(a).with(b).intersects(bbox) {
                    crossed = true;
                }
            });
            !crossed
        })
    }
}

/// An opaque layer: every polygon as it is (overlaps do not show on
/// opaque material), except those near a cut or the board edge, which are
/// cut one by one. Much faster than one union of the whole layer.
fn opaque(polygons: Polygons, clip: &Clip, area: Option<&Area>) -> Polygons {
    polygons
        .into_par_iter()
        .flat_map_iter(|polygon| {
            let bbox = polygon_bbox(&polygon);
            let near = clip.near(bbox);
            let inside = area.is_none_or(|a| a.holds(bbox));
            if near.is_empty() && inside {
                return vec![polygon];
            }
            let mut cut = if near.is_empty() {
                vec![polygon]
            } else {
                poly::difference(&[polygon], &near)
            };
            if let Some(area) = area.filter(|_| !inside) {
                cut = poly::intersection(&cut, &area.polygons);
            }
            cut
        })
        .collect()
}

/// Polygons as a mesh, built in parallel chunks.
fn caps(polygons: &Polygons, z: f64, up: bool, walls_from: Option<f64>) -> Mesh {
    let parts: Vec<Mesh> = polygons
        .par_chunks(512)
        .map(|chunk| {
            let mut mesh = Mesh::default();
            mesh.cap(chunk, z, up);
            if let Some(base) = walls_from {
                mesh.walls(chunk, z.min(base), z.max(base));
            }
            mesh
        })
        .collect();
    let mut out = Mesh::default();
    for part in &parts {
        out.append(part);
    }
    out
}

/// One layer to build.
#[derive(Debug, Clone, Copy)]
enum Job {
    Body,
    Copper(Side),
    Mask(Side),
    Silk(Side),
    Paste(Side),
    Inner(usize, CopperLayer),
}

/// Everything the layer jobs need, prepared once.
struct Inputs {
    levels: Levels,
    area: Polygons,
    board: Area,
    holes: Polygons,
    hole_clip: Clip,
    polygons: Vec<(Layer, Polygons)>,
    openings: Vec<(Side, Clip)>,
}

impl Inputs {
    fn of_layer(&self, layer: Layer) -> Polygons {
        self.polygons
            .iter()
            .find(|(own, _)| *own == layer)
            .map_or_else(Vec::new, |(_, p)| p.clone())
    }

    fn opening(&self, side: Side) -> &Clip {
        self.openings
            .iter()
            .find(|(own, _)| *own == side)
            .map_or(&self.hole_clip, |(_, clip)| clip)
    }

    /// The mesh of one job.
    fn mesh(&self, job: Job) -> Mesh {
        let levels = &self.levels;
        match job {
            Job::Body => {
                let drilled = poly::difference(&self.area, &self.holes);
                let mut mesh = Mesh::default();
                mesh.cap(&drilled, 0.0, false);
                mesh.cap(&drilled, levels.body_top, true);
                mesh.walls(&drilled, 0.0, levels.body_top);
                mesh
            }
            Job::Copper(side) => {
                let copper = opaque(
                    self.of_layer(Layer::Copper(copper_of(side))),
                    &self.hole_clip,
                    None,
                );
                let base = match side {
                    Side::Top => levels.body_top,
                    Side::Bottom => 0.0,
                };
                caps(&copper, levels.copper(side), side == Side::Top, Some(base))
            }
            Job::Mask(side) => {
                let mask = poly::difference(&self.area, &self.opening(side).polygons);
                caps(&mask, levels.mask(side), side == Side::Top, None)
            }
            Job::Silk(side) => {
                let silk = opaque(
                    self.of_layer(Layer::Silkscreen(side)),
                    self.opening(side),
                    Some(&self.board),
                );
                caps(&silk, levels.silk(side), side == Side::Top, None)
            }
            Job::Paste(side) => caps(
                &self.of_layer(Layer::Paste(side)),
                levels.paste(side),
                side == Side::Top,
                None,
            ),
            Job::Inner(index, layer) => {
                let copper = opaque(self.of_layer(Layer::Copper(layer)), &self.hole_clip, None);
                let z = levels.inner(index);
                let mut mesh = caps(&copper, z, true, None);
                mesh.append(&caps(&copper, z, false, None));
                mesh
            }
        }
    }
}

/// The jobs of a board and the layers whose features they need (with their
/// curve tolerance).
fn plan(
    levels: &Levels,
    stack: &[CopperLayer],
    options: &BoardOptions,
) -> (Vec<Job>, Vec<(Layer, f64)>) {
    let tolerance = options.tolerance.max(100.0);
    // Printing is coarser than etching: silkscreen and paste curves need
    // less detail.
    let coarse = tolerance * 4.0;
    let mut sides = vec![Side::Top];
    if levels.copper_layers >= 2 {
        sides.push(Side::Bottom);
    }
    let mut jobs = vec![Job::Body];
    let mut layers = Vec::new();
    for &side in &sides {
        let mask = match side {
            Side::Top => levels.top_mask > 0.0,
            Side::Bottom => levels.bottom_mask > 0.0,
        };
        jobs.push(Job::Copper(side));
        if mask {
            jobs.push(Job::Mask(side));
        }
        jobs.extend([Job::Silk(side), Job::Paste(side)]);
        layers.extend([
            (Layer::Copper(copper_of(side)), tolerance),
            (Layer::SolderMask(side), tolerance),
            (Layer::Silkscreen(side), coarse),
            (Layer::Paste(side), coarse),
        ]);
    }
    if options.inner {
        for (index, layer) in stack.iter().enumerate() {
            if matches!(layer, CopperLayer::Inner(_)) {
                jobs.push(Job::Inner(index, *layer));
                layers.push((Layer::Copper(*layer), tolerance));
            }
        }
    }
    (jobs, layers)
}

/// Builds the 3D board.
#[must_use]
pub fn build(pcb: &Pcb, resolve: Resolver<'_>, options: &BoardOptions) -> Board3d {
    let tolerance = options.tolerance.max(100.0);
    let art = BoardArt::new(pcb, options.art, resolve);
    let levels = Levels::of(&pcb.stackup);
    let mut warnings = Vec::new();
    let area = board_area(pcb, &art, &mut warnings);
    let holes = drill::holes(pcb);
    let hole_polygons: Polygons = holes
        .iter()
        .map(|hole| vec![hole_polygon(hole, tolerance)])
        .collect();
    let (jobs, wanted) = plan(&levels, art.stack(), options);
    // The features of every layer (the text resolver is not thread-safe,
    // so they are collected first), then their polygons in parallel.
    let features: Vec<(Layer, f64, Vec<Feature>)> = wanted
        .into_iter()
        .map(|(layer, tol)| (layer, tol, art.features(layer)))
        .collect();
    drop(art);
    let polygons: Vec<(Layer, Polygons)> = features
        .par_iter()
        .map(|(layer, tol, features)| {
            let mut out = Vec::new();
            for feature in features {
                feature_polygons(feature, *tol, &mut out);
            }
            (*layer, out)
        })
        .collect();
    // Mask openings plus holes cut the mask and the silkscreen.
    let openings: Vec<(Side, Clip)> = [Side::Top, Side::Bottom]
        .par_iter()
        .map(|&side| {
            let mut cut = polygons
                .iter()
                .find(|(layer, _)| *layer == Layer::SolderMask(side))
                .map_or_else(Vec::new, |(_, p)| p.clone());
            cut.extend(hole_polygons.iter().cloned());
            (side, Clip::new(cut))
        })
        .collect();
    let inputs = Inputs {
        levels,
        board: Area::new(&area),
        area,
        hole_clip: Clip::new(hole_polygons.clone()),
        holes: hole_polygons,
        polygons,
        openings,
    };
    let built: Vec<(Job, Mesh)> = jobs
        .par_iter()
        .map(|&job| (job, inputs.mesh(job)))
        .collect();

    let mut body = Mesh::default();
    let mut layers = Vec::new();
    for (job, mesh) in built {
        let (kind, side, copper) = match job {
            Job::Body => {
                body = mesh;
                continue;
            }
            Job::Copper(side) => (LayerKind::Copper, Some(side), Some(copper_of(side))),
            Job::Mask(side) => (LayerKind::Mask, Some(side), None),
            Job::Silk(side) => (LayerKind::Silk, Some(side), None),
            Job::Paste(side) => (LayerKind::Paste, Some(side), None),
            Job::Inner(_, layer) => (LayerKind::Copper, None, Some(layer)),
        };
        if mesh.is_empty() && kind != LayerKind::Copper {
            continue;
        }
        layers.push(LayerMesh {
            kind,
            side,
            copper,
            mesh,
        });
    }

    let barrels = barrels(&holes, &levels, tolerance);
    let mut bounds = body.bounds().unwrap_or([0.0; 6]);
    bounds[2] = levels.mask(Side::Bottom);
    bounds[5] = levels.mask(Side::Top);
    Board3d {
        levels,
        colors: colors(&pcb.stackup),
        area: inputs.area,
        holes,
        body,
        layers,
        barrels,
        bounds,
        warnings,
    }
}

const fn copper_of(side: Side) -> CopperLayer {
    match side {
        Side::Top => CopperLayer::Top,
        Side::Bottom => CopperLayer::Bottom,
    }
}

/// Plated hole walls from the copper of the first to the copper of the last
/// layer the hole connects.
fn barrels(holes: &[Hole], levels: &Levels, tolerance: f64) -> Mesh {
    let mut mesh = Mesh::default();
    let last = usize::from(levels.copper_layers.saturating_sub(1));
    let height = |index: usize| -> f64 {
        if index == 0 {
            levels.copper(Side::Top)
        } else if index >= last {
            levels.copper(Side::Bottom)
        } else {
            levels.inner(index)
        }
    };
    for hole in holes.iter().filter(|h| h.plated) {
        // Slightly inside the drilled wall, so the laminate never shows
        // through.
        let r = mm(hole.diameter / 2.0) - 0.001;
        if r <= 0.0 {
            continue;
        }
        let a = [mm(hole.start.x), mm(hole.start.y)];
        let b = hole.end.map_or(a, |end| [mm(end.x), mm(end.y)]);
        let (z0, z1) = (height(hole.span.1), height(hole.span.0));
        let segments = circle_segments(r, mm(tolerance));
        mesh.tube(a, b, r, z0.min(z1), z0.max(z1), segments);
    }
    mesh
}

/// The bounding box of the board area in nanometres.
#[must_use]
pub fn area_bbox(area: &Polygons) -> BBox {
    area.iter()
        .filter_map(|polygon| polygon.first())
        .fold(BBox::EMPTY, |b, ring| b.union(BBox::of_points(ring)))
}

/// Area of polygons in mm².
#[must_use]
pub fn area_mm2(polygons: &Polygons) -> f64 {
    poly::area(polygons) / 1e12
}

/// `true` if `p` (nm) lies on the board area.
#[must_use]
pub fn on_board(area: &Polygons, p: V) -> bool {
    area.iter().any(|polygon| {
        let inside = |ring: &Vec<V>| point_in_ring(ring, p);
        polygon.first().is_some_and(inside) && !polygon.iter().skip(1).any(inside)
    })
}

fn point_in_ring(ring: &[V], p: V) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// Signed area of a ring (positive counter-clockwise), in nm².
#[must_use]
pub fn ring_area(ring: &[V]) -> f64 {
    geom::signed_area2(ring) / 2.0
}
