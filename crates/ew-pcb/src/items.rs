//! The objects of a board as shapes: copper (pads, tracks, vias, zone
//! fills), holes, the board outline, keep-out areas, courtyards and
//! silkscreen, with an R-tree over the copper.

use std::collections::HashMap;

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::pcb::{FootprintInstance, Keepout, Pad, PadShape, Pcb};
use ew_model::shape::Geometry;
use rstar::{AABB, RTree, RTreeObject};
use serde::{Deserialize, Serialize};

use crate::geom::{self, BBox, Shape, V, Xform};

/// Chord error of curves converted to segments (1 µm).
pub const TOLERANCE: f64 = 1_000.0;

/// What a copper item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ItemRef {
    /// A pad of a footprint.
    Pad {
        /// The footprint.
        footprint: Id,
        /// Index of the pad.
        index: usize,
    },
    /// A track.
    Track {
        /// The track.
        id: Id,
    },
    /// A via.
    Via {
        /// The via.
        id: Id,
    },
    /// A filled area of a zone.
    Zone {
        /// The zone.
        id: Id,
        /// Index of the filled area.
        area: usize,
    },
}

impl ItemRef {
    /// The board object: the footprint of a pad, else the item itself.
    #[must_use]
    pub const fn object(self) -> Id {
        match self {
            Self::Pad { footprint, .. } => footprint,
            Self::Track { id } | Self::Via { id } | Self::Zone { id, .. } => id,
        }
    }

    /// `pad`, `track`, `via` or `zone`.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Pad { .. } => "pad",
            Self::Track { .. } => "track",
            Self::Via { .. } => "via",
            Self::Zone { .. } => "zone",
        }
    }
}

/// A piece of copper.
#[derive(Debug, Clone)]
pub struct CopperItem {
    /// What it is.
    pub item: ItemRef,
    /// Its net.
    pub net: Option<Id>,
    /// Copper layers as bits (index in the stack-up, top = 0).
    pub layers: u64,
    /// The shape.
    pub shape: Shape,
    /// Bounding box of the shape.
    pub bbox: BBox,
    /// Connection points for the ratsnest (pad and via centres, track
    /// ends).
    pub anchors: Vec<V>,
}

/// A drilled hole.
#[derive(Debug, Clone)]
pub struct Hole {
    /// The pad or via.
    pub item: ItemRef,
    /// The hole (a disc or a slot).
    pub shape: Shape,
    /// Plated.
    pub plated: bool,
    /// Diameter (slot width for slots).
    pub diameter: f64,
}

/// A keep-out area.
#[derive(Debug, Clone)]
pub struct KeepoutArea {
    /// The zone.
    pub zone: Id,
    /// Name of the zone.
    pub name: String,
    /// Copper layers as bits.
    pub layers: u64,
    /// The area.
    pub shape: Shape,
    /// What it forbids.
    pub rules: Keepout,
}

/// The courtyard of a footprint.
#[derive(Debug, Clone)]
pub struct Courtyard {
    /// The footprint.
    pub footprint: Id,
    /// Board side.
    pub side: Side,
    /// The closed courtyard outlines.
    pub shapes: Vec<Shape>,
}

/// A silkscreen stroke.
#[derive(Debug, Clone)]
pub struct Silk {
    /// The board object (a board shape or a footprint).
    pub object: Id,
    /// Board side.
    pub side: Side,
    /// The stroke.
    pub shape: Shape,
    /// Line width.
    pub width: f64,
}

/// The board outline, built from the shapes on the outline layer.
#[derive(Debug, Clone, Default)]
pub struct Outline {
    /// Closed rings (the board and its cut-outs).
    pub rings: Vec<Vec<V>>,
    /// Ends of outline lines that do not close.
    pub open_ends: Vec<V>,
    /// The rings as one polygon (even-odd: cut-outs are holes).
    pub area: Option<Shape>,
}

impl Outline {
    /// `true` if the point lies on the board (always true without an
    /// outline).
    #[must_use]
    pub fn contains(&self, p: V) -> bool {
        self.area.as_ref().is_none_or(|area| area.contains(p))
    }

    /// The distance from a shape to the nearest outline edge, if below
    /// `limit` (negative when the shape crosses the edge).
    #[must_use]
    pub fn edge_gap(&self, shape: &Shape, limit: f64) -> Option<f64> {
        let geom::Core::Poly(poly) = &self.area.as_ref()?.core else {
            return None;
        };
        let area = shape.bbox().grow(limit);
        let mut best: Option<f64> = None;
        poly.edges_near(area, &mut |a, b| {
            if let Some(gap) = Shape::stadium(a, b, 0.0).gap(shape, limit) {
                best = Some(best.map_or(gap, |own: f64| own.min(gap)));
            }
        });
        best
    }
}

struct Entry {
    index: usize,
    envelope: AABB<[f64; 2]>,
}

impl RTreeObject for Entry {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// A board prepared for checks: every object as shapes.
pub struct Board<'a> {
    /// The board.
    pub pcb: &'a Pcb,
    /// Copper layers from top to bottom.
    pub stack: Vec<CopperLayer>,
    /// Copper.
    pub copper: Vec<CopperItem>,
    /// Holes.
    pub holes: Vec<Hole>,
    /// Keep-out areas.
    pub keepouts: Vec<KeepoutArea>,
    /// Courtyards.
    pub courtyards: Vec<Courtyard>,
    /// Silkscreen strokes.
    pub silk: Vec<Silk>,
    /// Exposed pad copper per side (pads with a solder mask opening): copper
    /// item indices.
    pub exposed: Vec<(Side, usize)>,
    /// The board outline.
    pub outline: Outline,
    tree: RTree<Entry>,
    hole_tree: RTree<Entry>,
    footprints: HashMap<Id, usize>,
}

impl<'a> Board<'a> {
    /// Prepares a board.
    #[must_use]
    pub fn new(pcb: &'a Pcb) -> Self {
        let stack: Vec<CopperLayer> = pcb.stackup.copper_layers().collect();
        let mut board = Self {
            pcb,
            stack,
            copper: Vec::new(),
            holes: Vec::new(),
            keepouts: Vec::new(),
            courtyards: Vec::new(),
            silk: Vec::new(),
            exposed: Vec::new(),
            outline: outline(pcb),
            tree: RTree::new(),
            hole_tree: RTree::new(),
            footprints: HashMap::new(),
        };
        for (index, fp) in pcb.footprints.iter().enumerate() {
            board.footprints.insert(fp.id, index);
            board.add_footprint(fp);
        }
        board.add_tracks_and_vias();
        board.add_zones();
        board.add_board_silk();
        board.tree = RTree::bulk_load(
            board
                .copper
                .iter()
                .enumerate()
                .map(|(index, item)| Entry {
                    index,
                    envelope: item.bbox.aabb(),
                })
                .collect(),
        );
        board.hole_tree = RTree::bulk_load(
            board
                .holes
                .iter()
                .enumerate()
                .map(|(index, hole)| Entry {
                    index,
                    envelope: hole.shape.bbox().aabb(),
                })
                .collect(),
        );
        board
    }

    /// The bit of a copper layer, if the stack-up has it.
    #[must_use]
    pub fn layer_bit(&self, layer: CopperLayer) -> Option<u64> {
        self.stack
            .iter()
            .position(|own| *own == layer)
            .map(|index| 1_u64 << index.min(63))
    }

    /// All copper layer bits.
    #[must_use]
    pub fn all_layers(&self) -> u64 {
        match self.stack.len() {
            0 => 0,
            n if n >= 64 => u64::MAX,
            n => (1_u64 << n) - 1,
        }
    }

    /// The copper layers in a bit set, from the top.
    pub fn layers_of(&self, bits: u64) -> impl Iterator<Item = CopperLayer> + '_ {
        self.stack
            .iter()
            .enumerate()
            .filter(move |(index, _)| bits & (1_u64 << (*index).min(63)) != 0)
            .map(|(_, layer)| *layer)
    }

    /// The footprint with this id.
    #[must_use]
    pub fn footprint(&self, id: Id) -> Option<&'a FootprintInstance> {
        self.footprints
            .get(&id)
            .and_then(|index| self.pcb.footprints.iter().nth(*index))
    }

    /// Copper items whose box meets `area`.
    pub fn copper_near(&self, area: BBox) -> impl Iterator<Item = usize> + '_ {
        self.tree
            .locate_in_envelope_intersecting(area.aabb())
            .map(|entry| entry.index)
    }

    /// Holes whose box meets `area`.
    pub fn holes_near(&self, area: BBox) -> impl Iterator<Item = usize> + '_ {
        self.hole_tree
            .locate_in_envelope_intersecting(area.aabb())
            .map(|entry| entry.index)
    }

    fn copper_bits(&self, spec: LayerSpec, flip: &dyn Fn(Layer) -> Layer) -> u64 {
        match spec {
            LayerSpec::AllCopper => self.all_layers(),
            LayerSpec::Layer(layer) => match flip(layer) {
                Layer::Copper(copper) => self.layer_bit(copper).unwrap_or(0),
                _ => 0,
            },
            LayerSpec::AllMask | LayerSpec::AllPaste => 0,
        }
    }

    fn add_footprint(&mut self, fp: &FootprintInstance) {
        let bottom = fp.side == Side::Bottom;
        let copper_count = self.pcb.stackup.copper_count();
        let flip = move |layer: Layer| {
            if bottom {
                layer.flipped(copper_count)
            } else {
                layer
            }
        };
        let xf = footprint_xform(fp);
        for (index, pad) in fp.pads.iter().enumerate() {
            let item = ItemRef::Pad {
                footprint: fp.id,
                index,
            };
            let layers = pad
                .layers
                .iter()
                .fold(0, |bits, spec| bits | self.copper_bits(*spec, &flip));
            let shape = pad_shape(pad, &xf, 0.0);
            let center = xf.apply(V::of(pad.at));
            if layers != 0 {
                let mask_sides: Vec<Side> = [Side::Top, Side::Bottom]
                    .into_iter()
                    .filter(|side| {
                        pad.layers.iter().any(|spec| match spec {
                            LayerSpec::AllMask => true,
                            LayerSpec::Layer(layer) => flip(*layer) == Layer::SolderMask(*side),
                            _ => false,
                        })
                    })
                    .collect();
                for side in mask_sides {
                    self.exposed.push((side, self.copper.len()));
                }
                self.copper.push(CopperItem {
                    item,
                    net: pad.net,
                    layers,
                    bbox: shape.bbox(),
                    shape,
                    anchors: vec![center],
                });
            }
            if let Some(hole) = pad_hole(pad, &xf) {
                self.holes.push(Hole {
                    item,
                    plated: pad.kind != ew_model::pcb::PadKind::NpThroughHole,
                    diameter: pad.drill.map_or(0.0, |d| d.diameter as f64),
                    shape: hole,
                });
            }
        }
        let side = fp.side;
        let mut courtyard = Vec::new();
        let mut courtyard_lines = Vec::new();
        for graphic in &fp.graphics {
            let layer = flip(graphic.layer);
            match layer {
                Layer::Courtyard(_) => match closed_ring(&graphic.geometry, &xf) {
                    Some(ring) => courtyard.push(Shape::polygon(ring, Vec::new(), 0.0)),
                    None => courtyard_lines.extend(open_path(&graphic.geometry, &xf)),
                },
                Layer::Silkscreen(silk_side) => {
                    for shape in
                        strokes(&graphic.geometry, &xf, graphic.width as f64, graphic.filled)
                    {
                        self.silk.push(Silk {
                            object: fp.id,
                            side: silk_side,
                            shape,
                            width: graphic.width as f64,
                        });
                    }
                }
                _ => {}
            }
        }
        let (rings, _) = chain(courtyard_lines);
        courtyard.extend(
            rings
                .into_iter()
                .map(|ring| Shape::polygon(ring, Vec::new(), 0.0)),
        );
        if !courtyard.is_empty() {
            self.courtyards.push(Courtyard {
                footprint: fp.id,
                side,
                shapes: courtyard,
            });
        }
    }

    fn add_tracks_and_vias(&mut self) {
        let pcb = self.pcb;
        for track in &pcb.tracks {
            let Some(bit) = self.layer_bit(track.layer) else {
                continue;
            };
            let item = ItemRef::Track { id: track.id };
            let (start, end) = (V::of(track.start), V::of(track.end));
            let radius = track.width as f64 / 2.0;
            let points = match track.mid {
                Some(mid) => geom::arc_points(start, V::of(mid), end, TOLERANCE),
                None => vec![start, end],
            };
            let last = points.len() - 1;
            for (index, pair) in points.windows(2).enumerate() {
                let shape = Shape::stadium(pair[0], pair[1], radius);
                let mut anchors = Vec::new();
                if index == 0 {
                    anchors.push(start);
                }
                if index + 1 == last {
                    anchors.push(end);
                }
                self.copper.push(CopperItem {
                    item,
                    net: track.net,
                    layers: bit,
                    bbox: shape.bbox(),
                    shape,
                    anchors,
                });
            }
        }
        for via in &pcb.vias {
            let item = ItemRef::Via { id: via.id };
            let from = self.stack.iter().position(|l| *l == via.from);
            let to = self.stack.iter().position(|l| *l == via.to);
            let (Some(from), Some(to)) = (from, to) else {
                continue;
            };
            let (low, high) = (from.min(to), from.max(to));
            let layers = (low..=high).fold(0_u64, |bits, index| bits | (1 << index.min(63)));
            let at = V::of(via.at);
            let shape = Shape::disc(at, via.diameter as f64 / 2.0);
            self.copper.push(CopperItem {
                item,
                net: via.net,
                layers,
                bbox: shape.bbox(),
                shape,
                anchors: vec![at],
            });
            self.holes.push(Hole {
                item,
                shape: Shape::disc(at, via.drill as f64 / 2.0),
                plated: true,
                diameter: via.drill as f64,
            });
        }
    }

    fn add_zones(&mut self) {
        let pcb = self.pcb;
        for zone in &pcb.zones {
            let layers = zone
                .layers
                .iter()
                .fold(0, |bits, layer| bits | self.layer_bit(*layer).unwrap_or(0));
            if let Some(rules) = zone.keepout {
                if zone.outline.len() >= 3 {
                    self.keepouts.push(KeepoutArea {
                        zone: zone.id,
                        name: zone.name.clone(),
                        layers,
                        shape: Shape::polygon(
                            zone.outline.iter().map(|p| V::of(*p)).collect(),
                            zone.holes
                                .iter()
                                .map(|hole| hole.iter().map(|p| V::of(*p)).collect())
                                .collect(),
                            0.0,
                        ),
                        rules,
                    });
                }
                continue;
            }
            for (area_index, area) in zone.filled.iter().enumerate() {
                let Some(bit) = self.layer_bit(area.layer) else {
                    continue;
                };
                if area.points.len() < 3 {
                    continue;
                }
                let shape = Shape::polygon(
                    area.points.iter().map(|p| V::of(*p)).collect(),
                    area.holes
                        .iter()
                        .map(|hole| hole.iter().map(|p| V::of(*p)).collect())
                        .collect(),
                    0.0,
                );
                self.copper.push(CopperItem {
                    item: ItemRef::Zone {
                        id: zone.id,
                        area: area_index,
                    },
                    net: zone.net,
                    layers: bit,
                    bbox: shape.bbox(),
                    shape,
                    anchors: Vec::new(),
                });
            }
        }
    }

    fn add_board_silk(&mut self) {
        for shape in &self.pcb.shapes {
            if let Layer::Silkscreen(side) = shape.layer {
                for stroke in strokes(
                    &shape.geometry,
                    &Xform::IDENTITY,
                    shape.width as f64,
                    shape.filled,
                ) {
                    self.silk.push(Silk {
                        object: shape.id,
                        side,
                        shape: stroke,
                        width: shape.width as f64,
                    });
                }
            }
        }
    }
}

/// The placement of a footprint.
#[must_use]
pub fn footprint_xform(fp: &FootprintInstance) -> Xform {
    Xform {
        offset: V::of(fp.at),
        angle: geom::radians(fp.rotation),
        mirror: fp.side == Side::Bottom,
    }
}

/// The copper of a pad on the board, grown by `margin`.
#[must_use]
pub fn pad_shape(pad: &Pad, xf: &Xform, margin: f64) -> Shape {
    let rotation = geom::radians(pad.rotation);
    let at = V::of(pad.at);
    let place = |q: V| xf.apply(at.add(q.rotate(rotation)));
    let w = pad.size.w as f64 / 2.0;
    let h = pad.size.h as f64 / 2.0;
    let rect = |hw: f64, hh: f64| {
        vec![
            place(V::new(-hw, -hh)),
            place(V::new(hw, -hh)),
            place(V::new(hw, hh)),
            place(V::new(-hw, hh)),
        ]
    };
    // A rectangle of half sizes (hw, hh) grown by r, degenerate ones as
    // stadiums or discs.
    let rounded = |hw: f64, hh: f64, r: f64| {
        if hw <= 0.0 && hh <= 0.0 {
            Shape::disc(place(V::default()), r)
        } else if hw <= 0.0 {
            Shape::stadium(place(V::new(0.0, -hh)), place(V::new(0.0, hh)), r)
        } else if hh <= 0.0 {
            Shape::stadium(place(V::new(-hw, 0.0)), place(V::new(hw, 0.0)), r)
        } else {
            Shape::polygon(rect(hw, hh), Vec::new(), r)
        }
    };
    let grow = margin.max(-w.min(h));
    match &pad.shape {
        PadShape::Circle => Shape::disc(place(V::default()), (w + grow).max(0.0)),
        PadShape::Oval => {
            let r = w.min(h);
            rounded(w - r, h - r, r + grow)
        }
        PadShape::Rect => {
            if grow > 0.0 {
                rounded(w, h, grow)
            } else {
                Shape::polygon(rect(w + grow, h + grow), Vec::new(), 0.0)
            }
        }
        PadShape::RoundRect { ratio } => {
            let corner = (w.min(h) * 2.0 * f64::from(*ratio) / 1000.0).min(w.min(h));
            rounded(w - corner, h - corner, corner + grow)
        }
        PadShape::Trapezoid { delta } => {
            let (dw, dh) = (delta.w as f64 / 2.0, delta.h as f64 / 2.0);
            let corners = vec![
                place(V::new(-(w + dw), -(h + dh))),
                place(V::new(w + dw, -(h - dh))),
                place(V::new(w - dw, h - dh)),
                place(V::new(-(w - dw), h + dh)),
            ];
            Shape::polygon(corners, Vec::new(), grow.max(0.0))
        }
        PadShape::Custom { points } => {
            if points.len() < 3 {
                return Shape::disc(place(V::default()), (w.min(h) + grow).max(0.0));
            }
            let ring = points.iter().map(|p| place(V::of(*p))).collect();
            Shape::polygon(ring, Vec::new(), grow.max(0.0))
        }
    }
}

/// The hole of a pad on the board.
#[must_use]
pub fn pad_hole(pad: &Pad, xf: &Xform) -> Option<Shape> {
    let drill = pad.drill?;
    let rotation = geom::radians(pad.rotation);
    let at = V::of(pad.at);
    let place = |q: V| xf.apply(at.add(q.rotate(rotation)));
    let offset = V::of(drill.offset);
    let radius = drill.diameter as f64 / 2.0;
    Some(match drill.slot_length {
        Some(length) if length > drill.diameter => {
            let reach = (length - drill.diameter) as f64 / 2.0;
            Shape::stadium(
                place(offset.add(V::new(-reach, 0.0))),
                place(offset.add(V::new(reach, 0.0))),
                radius,
            )
        }
        _ => Shape::disc(place(offset), radius),
    })
}

/// A closed geometry as a ring on the board.
fn closed_ring(geometry: &Geometry, xf: &Xform) -> Option<Vec<V>> {
    match geometry {
        Geometry::Rect { start, end } => {
            let (a, b) = (V::of(*start), V::of(*end));
            Some(
                [
                    V::new(a.x, a.y),
                    V::new(b.x, a.y),
                    V::new(b.x, b.y),
                    V::new(a.x, b.y),
                ]
                .into_iter()
                .map(|p| xf.apply(p))
                .collect(),
            )
        }
        Geometry::Circle { center, radius } => Some(
            geom::circle_polygon(V::of(*center), *radius as f64, TOLERANCE)
                .into_iter()
                .map(|p| xf.apply(p))
                .collect(),
        ),
        Geometry::Polygon { points } if points.len() >= 3 => {
            Some(points.iter().map(|p| xf.apply(V::of(*p))).collect())
        }
        _ => None,
    }
}

/// An open geometry as a list of points on the board.
fn open_path(geometry: &Geometry, xf: &Xform) -> Option<Vec<V>> {
    let points: Vec<V> = match geometry {
        Geometry::Line { start, end } => vec![V::of(*start), V::of(*end)],
        Geometry::Arc { start, mid, end } => {
            geom::arc_points(V::of(*start), V::of(*mid), V::of(*end), TOLERANCE)
        }
        Geometry::Polyline { points } if points.len() >= 2 => {
            points.iter().map(|p| V::of(*p)).collect()
        }
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => bezier(
            V::of(*start),
            V::of(*control1),
            V::of(*control2),
            V::of(*end),
        ),
        _ => return None,
    };
    Some(points.into_iter().map(|p| xf.apply(p)).collect())
}

#[allow(clippy::cast_precision_loss, reason = "small step counts")]
fn bezier(p0: V, p1: V, p2: V, p3: V) -> Vec<V> {
    let steps = 32;
    (0..=steps)
        .map(|i| {
            let t = f64::from(i) / f64::from(steps);
            let u = 1.0 - t;
            p0.scale(u * u * u)
                .add(p1.scale(3.0 * u * u * t))
                .add(p2.scale(3.0 * u * t * t))
                .add(p3.scale(t * t * t))
        })
        .collect()
}

/// The strokes of a drawn geometry (filled closed shapes as one polygon).
fn strokes(geometry: &Geometry, xf: &Xform, width: f64, filled: bool) -> Vec<Shape> {
    let r = width / 2.0;
    if let Some(ring) = closed_ring(geometry, xf) {
        if filled {
            return vec![Shape::polygon(ring, Vec::new(), r)];
        }
        let n = ring.len();
        return (0..n)
            .map(|i| Shape::stadium(ring[i], ring[(i + 1) % n], r))
            .collect();
    }
    open_path(geometry, xf).map_or_else(Vec::new, |points| {
        points
            .windows(2)
            .map(|pair| Shape::stadium(pair[0], pair[1], r))
            .collect()
    })
}

/// Joins open paths into closed rings where their ends meet (within
/// 10 µm). Returns the rings and the ends that stay open.
#[must_use]
pub fn chain(paths: Vec<Vec<V>>) -> (Vec<Vec<V>>, Vec<V>) {
    const SNAP: f64 = 10_000.0;
    let mut open: Vec<Vec<V>> = paths.into_iter().filter(|p| p.len() >= 2).collect();
    let mut rings = Vec::new();
    let mut ends = Vec::new();
    while let Some(mut current) = open.pop() {
        loop {
            let first = current[0];
            let last = current[current.len() - 1];
            if current.len() > 2 && first.dist(last) <= SNAP {
                current.pop();
                rings.push(current);
                break;
            }
            let next = open.iter().position(|path| {
                path[0].dist(last) <= SNAP || path[path.len() - 1].dist(last) <= SNAP
            });
            if let Some(index) = next {
                let mut path = open.swap_remove(index);
                if path[0].dist(last) > SNAP {
                    path.reverse();
                }
                current.extend(path.into_iter().skip(1));
            } else {
                // Try to grow at the other end before giving up.
                let prev = open.iter().position(|path| {
                    path[0].dist(first) <= SNAP || path[path.len() - 1].dist(first) <= SNAP
                });
                if let Some(index) = prev {
                    let mut path = open.swap_remove(index);
                    if path[path.len() - 1].dist(first) > SNAP {
                        path.reverse();
                    }
                    path.pop();
                    path.extend(current);
                    current = path;
                    continue;
                }
                ends.push(first);
                ends.push(last);
                break;
            }
        }
    }
    (rings, ends)
}

/// The board outline from the shapes on the outline layer.
#[must_use]
pub fn outline(pcb: &Pcb) -> Outline {
    let mut rings = Vec::new();
    let mut paths = Vec::new();
    for shape in pcb.shapes.iter().filter(|s| s.layer == Layer::Outline) {
        match closed_ring(&shape.geometry, &Xform::IDENTITY) {
            Some(ring) => rings.push(ring),
            None => paths.extend(open_path(&shape.geometry, &Xform::IDENTITY)),
        }
    }
    let (chained, open_ends) = chain(paths);
    rings.extend(chained);
    if rings.is_empty() {
        return Outline {
            rings,
            open_ends,
            area: None,
        };
    }
    // The largest ring is the board; the others are cut-outs (even-odd).
    rings.sort_by(|a, b| {
        geom::signed_area2(b)
            .abs()
            .total_cmp(&geom::signed_area2(a).abs())
    });
    let area = Shape::polygon(rings[0].clone(), rings[1..].to_vec(), 0.0);
    Outline {
        rings,
        open_ends,
        area: Some(area),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::pcb::{BoardShape, Drill, PadKind, STANDARD_THICKNESS, Stackup};
    use ew_model::units::{Angle, Point, Size, mm, um};

    fn pad(shape: PadShape, w: i64, h: i64) -> Pad {
        Pad {
            id: Id::random(),
            number: "1".into(),
            net: None,
            kind: PadKind::Smd,
            shape,
            at: Point::ORIGIN,
            rotation: Angle::ZERO,
            size: Size::new(w, h),
            drill: None,
            layers: vec![LayerSpec::Layer(Layer::TOP_COPPER)],
            clearance: None,
            mask_margin: None,
            paste_margin: None,
        }
    }

    #[test]
    fn pad_shapes_have_their_size() {
        let xf = Xform::IDENTITY;
        for shape in [
            PadShape::Rect,
            PadShape::Oval,
            PadShape::RoundRect { ratio: 250 },
            PadShape::Circle,
        ] {
            let pad = pad(shape.clone(), mm(2), mm(1));
            let s = pad_shape(&pad, &xf, 0.0);
            let bbox = s.bbox();
            let w = bbox.max.x - bbox.min.x;
            assert!((w - 2e6).abs() < 1.0, "{shape:?}: {w}");
            // A point just outside the right edge is 0.1 mm away.
            let probe = Shape::disc(V::new(bbox.max.x + 100_000.0, 0.0), 0.0);
            let gap = s.gap(&probe, 1e6).unwrap();
            assert!((gap - 100_000.0).abs() < 1.0, "{shape:?}: {gap}");
        }
        let mut th = pad(PadShape::Circle, mm(2), mm(2));
        th.drill = Some(Drill {
            diameter: mm(1),
            slot_length: Some(mm(2)),
            offset: Point::ORIGIN,
        });
        let hole = pad_hole(&th, &xf).unwrap();
        assert!((hole.bbox().max.x - hole.bbox().min.x - 2e6).abs() < 1.0);
    }

    #[test]
    fn outline_lines_are_chained_into_a_board() {
        let mut pcb = Pcb::new(Id::random(), "B", Stackup::standard(2, STANDARD_THICKNESS));
        let corners = [
            Point::mm(0, 0),
            Point::mm(50, 0),
            Point::mm(50, 30),
            Point::mm(0, 30),
        ];
        for i in 0..4 {
            // Lines in mixed directions.
            let (a, b) = if i % 2 == 0 {
                (corners[i], corners[(i + 1) % 4])
            } else {
                (corners[(i + 1) % 4], corners[i])
            };
            pcb.shapes.insert(BoardShape {
                id: Id::random(),
                layer: Layer::Outline,
                geometry: Geometry::Line { start: a, end: b },
                width: um(100),
                filled: false,
            });
        }
        let outline = outline(&pcb);
        assert_eq!(outline.rings.len(), 1);
        assert_eq!(outline.open_ends.len(), 0);
        assert!(outline.contains(V::new(25e6, 15e6)));
        assert!(!outline.contains(V::new(55e6, 15e6)));
        let via = Shape::disc(V::new(1e6, 15e6), 0.3e6);
        let gap = outline.edge_gap(&via, 2e6).unwrap();
        assert!((gap - 0.7e6).abs() < 1.0, "{gap}");
    }
}
