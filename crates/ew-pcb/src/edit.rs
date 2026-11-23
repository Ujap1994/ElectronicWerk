//! Editing operations of the layout editor.
//!
//! Every operation takes the board and returns an [`Edit`]: the changed
//! objects and what to select afterwards. Moving, turning and flipping
//! work on every kind of board object; aligning and distributing on
//! footprints; dragging a straight track segment moves it sideways while
//! its neighbours keep their directions (so 45° routing stays 45°).

use std::collections::{BTreeMap, HashMap, HashSet};

use ew_model::dimension::{Dimension, DimensionKind};
use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::object::Object;
use ew_model::pcb::{BoardShape, BoardText, FootprintInstance, Pcb, Room, Track, Via, Zone};
use ew_model::shape::Geometry;
use ew_model::units::{Angle, Nm, Point};
use ew_schematic::edit::{Change, Edit};
use serde::Deserialize;

use crate::geom::{BBox, V};
use crate::items::{footprint_xform, pad_shape};

/// How footprints line up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    /// Left edges.
    Left,
    /// Right edges.
    Right,
    /// Top edges (smallest y).
    Top,
    /// Bottom edges (largest y).
    Bottom,
    /// Centres on one vertical line.
    CenterX,
    /// Centres on one horizontal line.
    CenterY,
}

/// The axis to distribute along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
    /// Horizontally.
    X,
    /// Vertically.
    Y,
}

/// An editing operation.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BoardEdit {
    /// Move objects.
    Move {
        /// The objects.
        ids: Vec<Id>,
        /// The offset.
        offset: Point,
    },
    /// Turn objects around a point.
    Rotate {
        /// The objects.
        ids: Vec<Id>,
        /// Centre of the turn (the centre of the objects if missing).
        #[serde(default)]
        center: Option<Point>,
        /// Angle (counter-clockwise in the scene).
        angle: Angle,
    },
    /// Put objects on the other side of the board, mirrored around a
    /// vertical line.
    Flip {
        /// The objects.
        ids: Vec<Id>,
        /// x of the mirror line (the centre of the objects if missing).
        #[serde(default)]
        center_x: Option<Nm>,
    },
    /// Delete objects (locked ones stay).
    Delete {
        /// The objects.
        ids: Vec<Id>,
    },
    /// Lock or unlock objects.
    Lock {
        /// The objects.
        ids: Vec<Id>,
        /// Locked.
        locked: bool,
    },
    /// Line up footprints.
    Align {
        /// The footprints.
        ids: Vec<Id>,
        /// How.
        mode: Align,
    },
    /// Spread footprints with equal gaps between them.
    Distribute {
        /// The footprints.
        ids: Vec<Id>,
        /// Along which axis.
        axis: Axis,
    },
    /// Move tracks to another copper layer.
    SetLayer {
        /// The tracks.
        ids: Vec<Id>,
        /// The layer.
        layer: CopperLayer,
    },
    /// Change the width of tracks.
    SetWidth {
        /// The tracks.
        ids: Vec<Id>,
        /// Width.
        width: Nm,
    },
    /// Change the net of tracks, vias and zones.
    SetNet {
        /// The objects.
        ids: Vec<Id>,
        /// The net (none: unconnected).
        #[serde(default)]
        net: Option<Id>,
    },
    /// Move a track segment; tracks and vias at its ends follow.
    DragTrack {
        /// The track.
        id: Id,
        /// The offset.
        offset: Point,
    },
    /// The objects become one group (leaving any group they were in).
    Group {
        /// The objects.
        ids: Vec<Id>,
        /// Name of the group (may be empty).
        #[serde(default)]
        name: String,
    },
    /// The groups of the objects (or the groups themselves) are dissolved.
    Ungroup {
        /// Objects or groups.
        ids: Vec<Id>,
    },
}

/// Why an edit is not possible.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// An id names no board object.
    #[error("no board object {0}")]
    Unknown(Id),
    /// The parameters do not fit.
    #[error("{0}")]
    Invalid(String),
}

/// A point transformation.
trait Map {
    fn point(&self, p: Point) -> Point;
}

struct Shift(Point);

impl Map for Shift {
    fn point(&self, p: Point) -> Point {
        p + self.0
    }
}

struct Turn {
    center: Point,
    angle: Angle,
}

impl Map for Turn {
    fn point(&self, p: Point) -> Point {
        rotate_point(p, self.center, self.angle)
    }
}

struct Mirror(Nm);

impl Map for Mirror {
    fn point(&self, p: Point) -> Point {
        Point::new(2 * self.0 - p.x, p.y)
    }
}

/// `p` turned around `c` (exact for multiples of 90°).
#[must_use]
pub fn rotate_point(p: Point, c: Point, angle: Angle) -> Point {
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    let quarter = angle.millidegrees().rem_euclid(360_000);
    let (x, y) = match quarter {
        0 => (dx, dy),
        90_000 => (-dy, dx),
        180_000 => (-dx, -dy),
        270_000 => (dy, -dx),
        _ => {
            let v = V::new(dx as f64, dy as f64).rotate(crate::geom::radians(angle));
            (v.x.round() as Nm, v.y.round() as Nm)
        }
    };
    Point::new(c.x + x, c.y + y)
}

fn add_angle(a: Angle, b: Angle) -> Angle {
    Angle::from_millidegrees(i64::from(a.millidegrees() + b.millidegrees()).rem_euclid(360_000))
}

fn negate(a: Angle) -> Angle {
    Angle::from_millidegrees(i64::from(-a.millidegrees()).rem_euclid(360_000))
}

fn map_geometry(geometry: &Geometry, map: &dyn Map, keep_rect: bool) -> Geometry {
    match geometry {
        Geometry::Line { start, end } => Geometry::Line {
            start: map.point(*start),
            end: map.point(*end),
        },
        Geometry::Rect { start, end } => {
            if keep_rect {
                Geometry::Rect {
                    start: map.point(*start),
                    end: map.point(*end),
                }
            } else {
                let corners = [
                    *start,
                    Point::new(end.x, start.y),
                    *end,
                    Point::new(start.x, end.y),
                ];
                Geometry::Polygon {
                    points: corners.iter().map(|p| map.point(*p)).collect(),
                }
            }
        }
        Geometry::Circle { center, radius } => Geometry::Circle {
            center: map.point(*center),
            radius: *radius,
        },
        Geometry::Arc { start, mid, end } => Geometry::Arc {
            start: map.point(*start),
            mid: map.point(*mid),
            end: map.point(*end),
        },
        Geometry::Polyline { points } => Geometry::Polyline {
            points: points.iter().map(|p| map.point(*p)).collect(),
        },
        Geometry::Polygon { points } => Geometry::Polygon {
            points: points.iter().map(|p| map.point(*p)).collect(),
        },
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => Geometry::Bezier {
            start: map.point(*start),
            control1: map.point(*control1),
            control2: map.point(*control2),
            end: map.point(*end),
        },
    }
}

/// A geometry moved by `offset`.
#[must_use]
pub fn shifted(geometry: &Geometry, offset: Point) -> Geometry {
    map_geometry(geometry, &Shift(offset), true)
}

/// One board object, owned, for editing.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    Footprint(FootprintInstance),
    Track(Track),
    Via(Via),
    Zone(Zone),
    Text(BoardText),
    Shape(BoardShape),
    Dimension(Dimension),
    Room(Room),
}

impl Item {
    fn find(pcb: &Pcb, id: Id) -> Option<Self> {
        pcb.footprints
            .get(id)
            .cloned()
            .map(Self::Footprint)
            .or_else(|| pcb.tracks.get(id).cloned().map(Self::Track))
            .or_else(|| pcb.vias.get(id).cloned().map(Self::Via))
            .or_else(|| pcb.zones.get(id).cloned().map(Self::Zone))
            .or_else(|| pcb.texts.get(id).cloned().map(Self::Text))
            .or_else(|| pcb.shapes.get(id).cloned().map(Self::Shape))
            .or_else(|| pcb.dimensions.get(id).cloned().map(Self::Dimension))
            .or_else(|| pcb.rooms.get(id).cloned().map(Self::Room))
    }

    fn object(self) -> Object {
        match self {
            Self::Footprint(o) => Object::Footprint(o),
            Self::Track(o) => Object::Track(o),
            Self::Via(o) => Object::Via(o),
            Self::Zone(o) => Object::Zone(o),
            Self::Text(o) => Object::BoardText(o),
            Self::Shape(o) => Object::BoardShape(o),
            Self::Dimension(o) => Object::Dimension(o),
            Self::Room(o) => Object::Room(o),
        }
    }

    fn locked(&self) -> bool {
        match self {
            Self::Footprint(o) => o.locked,
            Self::Track(o) => o.locked,
            Self::Via(o) => o.locked,
            Self::Dimension(o) => o.locked,
            Self::Room(o) => o.locked,
            _ => false,
        }
    }

    fn set_locked(&mut self, locked: bool) {
        match self {
            Self::Footprint(o) => o.locked = locked,
            Self::Track(o) => o.locked = locked,
            Self::Via(o) => o.locked = locked,
            Self::Dimension(o) => o.locked = locked,
            Self::Room(o) => o.locked = locked,
            _ => {}
        }
    }

    /// The points that decide where the object is (for the centre of a
    /// selection).
    fn anchor_points(&self) -> Vec<Point> {
        match self {
            Self::Footprint(o) => vec![o.at],
            Self::Track(o) => vec![o.start, o.end],
            Self::Via(o) => vec![o.at],
            Self::Zone(o) => o.outline.clone(),
            Self::Text(o) => vec![o.at],
            Self::Shape(o) => o.geometry.points(),
            Self::Dimension(o) => vec![o.start, o.end],
            Self::Room(o) => o.outline.clone(),
        }
    }

    fn map(&mut self, map: &dyn Map, keep_rect: bool) {
        match self {
            Self::Footprint(o) => o.at = map.point(o.at),
            Self::Track(o) => {
                o.start = map.point(o.start);
                o.end = map.point(o.end);
                o.mid = o.mid.map(|m| map.point(m));
            }
            Self::Via(o) => o.at = map.point(o.at),
            Self::Zone(o) => {
                for p in &mut o.outline {
                    *p = map.point(*p);
                }
                for hole in &mut o.holes {
                    for p in hole {
                        *p = map.point(*p);
                    }
                }
                for area in &mut o.filled {
                    for p in &mut area.points {
                        *p = map.point(*p);
                    }
                    for hole in &mut area.holes {
                        for p in hole {
                            *p = map.point(*p);
                        }
                    }
                }
            }
            Self::Text(o) => o.at = map.point(o.at),
            Self::Shape(o) => o.geometry = map_geometry(&o.geometry, map, keep_rect),
            Self::Dimension(o) => map_dimension(o, map),
            Self::Room(o) => {
                for p in &mut o.outline {
                    *p = map.point(*p);
                }
            }
        }
    }
}

fn center_of(items: &[Item]) -> Point {
    let points: Vec<Point> = items.iter().flat_map(Item::anchor_points).collect();
    ew_model::units::Rect::from_points(points).map_or(Point::ORIGIN, |r| {
        Point::new(r.min.x / 2 + r.max.x / 2, r.min.y / 2 + r.max.y / 2)
    })
}

fn collect(pcb: &Pcb, ids: &[Id]) -> Result<Vec<Item>, EditError> {
    let mut seen = HashSet::new();
    ids.iter()
        .filter(|id| seen.insert(**id))
        .map(|id| Item::find(pcb, *id).ok_or(EditError::Unknown(*id)))
        .collect()
}

fn updates(doc: Id, pcb: &Pcb, items: Vec<Item>, select: Vec<Id>) -> Edit {
    let mut edit = Edit {
        changes: Vec::new(),
        select,
    };
    for item in items {
        let before = Item::find(pcb, object_id(&item));
        if before.as_ref() == Some(&item) {
            continue;
        }
        edit.changes.push(Change::Update {
            doc,
            object: item.object(),
        });
    }
    edit
}

fn object_id(item: &Item) -> Id {
    match item {
        Item::Footprint(o) => o.id,
        Item::Track(o) => o.id,
        Item::Via(o) => o.id,
        Item::Zone(o) => o.id,
        Item::Text(o) => o.id,
        Item::Shape(o) => o.id,
        Item::Dimension(o) => o.id,
        Item::Room(o) => o.id,
    }
}

/// Where footprints go when they are packed into an area: in rows from the
/// top left, in the order given, `gap` apart and `margin` from the edge.
/// Footprints that do not fit come back in the second list. With an
/// `outline` (a polygon area), every footprint box must lie inside it.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "rounded board coordinates fit i64"
)]
pub fn pack(
    footprints: &[&FootprintInstance],
    area: (V, V),
    outline: Option<&[Point]>,
    gap: f64,
    margin: f64,
) -> (Vec<(Id, Point)>, Vec<Id>) {
    let (min, max) = area;
    let fits = |x0: f64, y0: f64, x1: f64, y1: f64| {
        outline.is_none_or(|poly| {
            [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
                .iter()
                .all(|(x, y)| inside_polygon(poly, Point::new(x.round() as i64, y.round() as i64)))
        })
    };
    let mut placed = Vec::new();
    let mut overflow = Vec::new();
    let mut x = min.x + margin;
    let mut y = max.y - margin;
    let mut row_height = 0.0_f64;
    for fp in footprints {
        let bbox = footprint_box(fp);
        let (w, h) = (bbox.max.x - bbox.min.x, bbox.max.y - bbox.min.y);
        let mut spot = None;
        // Along the row, then the next rows, until it fits.
        while y - h >= min.y + margin {
            if x + w > max.x - margin {
                if x <= min.x + margin {
                    break;
                }
                x = min.x + margin;
                // At least a little lower, so that the search ends.
                y -= (row_height + gap).max(100_000.0);
                row_height = 0.0;
                continue;
            }
            if fits(x, y - h, x + w, y) {
                spot = Some((x, y));
                break;
            }
            x += gap.max(w / 4.0).max(100_000.0);
        }
        let Some((left, top)) = spot else {
            overflow.push(fp.id);
            continue;
        };
        // The footprint's origin relative to its box.
        let at = Point::new(
            (left + (V::of(fp.at).x - bbox.min.x)).round() as i64,
            (top + (V::of(fp.at).y - bbox.max.y)).round() as i64,
        );
        placed.push((fp.id, at));
        x = left + w + gap;
        row_height = row_height.max(h);
    }
    (placed, overflow)
}

/// `true` if `p` lies inside the polygon (even-odd rule).
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
pub fn inside_polygon(points: &[Point], point: Point) -> bool {
    let (px, py) = (point.x as f64, point.y as f64);
    let mut odd = false;
    let count = points.len();
    for index in 0..count {
        let (cur, prev) = (points[index], points[(index + count - 1) % count]);
        let (ax, ay, bx, by) = (cur.x as f64, cur.y as f64, prev.x as f64, prev.y as f64);
        if (ay > py) != (by > py) && px < (bx - ax) * (py - ay) / (by - ay) + ax {
            odd = !odd;
        }
    }
    odd
}

/// The footprints a moved room takes along: unlocked ones whose origin
/// lies inside it and that are not moved themselves.
fn riders(pcb: &Pcb, items: &[Item], ids: &[Id]) -> Vec<Item> {
    let mut out = Vec::new();
    for item in items {
        let Item::Room(room) = item else {
            continue;
        };
        for fp in &pcb.footprints {
            if !fp.locked
                && !ids.contains(&fp.id)
                && inside_polygon(&room.outline, fp.at)
                && !out.iter().any(|o: &Item| object_id(o) == fp.id)
            {
                out.push(Item::Footprint(fp.clone()));
            }
        }
    }
    out
}

/// Moves, turns or mirrors a dimension. The dimension line stays on the
/// same side of the measured points; a horizontal dimension turned by 90°
/// becomes a vertical one, by other angles an aligned one.
fn map_dimension(dim: &mut Dimension, map: &dyn Map) {
    let (start, end) = (map.point(dim.start), map.point(dim.end));
    // Where the dimension line lies, as a vector from the first point.
    let lift = match dim.kind {
        DimensionKind::Horizontal => Point::new(0, dim.offset),
        DimensionKind::Vertical => Point::new(dim.offset, 0),
        DimensionKind::Aligned => {
            let d = V::of(dim.end).sub(V::of(dim.start));
            let n = V::new(-d.y, d.x).unit().scale(dim.offset as f64);
            n.point()
        }
        _ => Point::ORIGIN,
    };
    let moved = map.point(dim.start + lift) - start;
    match dim.kind {
        DimensionKind::Horizontal | DimensionKind::Vertical => {
            let (dx, dy) = (moved.x.abs(), moved.y.abs());
            if dx == 0 || dy == 0 {
                if dx > dy {
                    dim.kind = DimensionKind::Vertical;
                    dim.offset = moved.x;
                } else {
                    dim.kind = DimensionKind::Horizontal;
                    dim.offset = moved.y;
                }
            } else {
                dim.kind = DimensionKind::Aligned;
                dim.offset = aligned_offset(start, end, moved);
            }
        }
        DimensionKind::Aligned => dim.offset = aligned_offset(start, end, moved),
        _ => {}
    }
    dim.start = start;
    dim.end = end;
}

/// The offset of an aligned dimension whose line is moved by `lift`: its
/// length along the left normal of `start` → `end`.
#[allow(
    clippy::cast_possible_truncation,
    reason = "rounded board lengths fit i64"
)]
fn aligned_offset(start: Point, end: Point, lift: Point) -> Nm {
    let d = V::of(end).sub(V::of(start));
    let n = V::new(-d.y, d.x).unit();
    (n.x * lift.x as f64 + n.y * lift.y as f64).round() as Nm
}

/// The box of a footprint on the board (its pads and graphics).
#[must_use]
pub fn footprint_box(fp: &FootprintInstance) -> BBox {
    let xf = footprint_xform(fp);
    let mut bbox = BBox::of(V::of(fp.at));
    for pad in &fp.pads {
        bbox = bbox.union(pad_shape(pad, &xf, 0.0).bbox());
    }
    for graphic in &fp.graphics {
        if matches!(graphic.layer, Layer::Courtyard(_) | Layer::Fabrication(_))
            && let Some(bounds) = graphic.geometry.bounds()
        {
            for p in [
                bounds.min,
                bounds.max,
                Point::new(bounds.min.x, bounds.max.y),
                Point::new(bounds.max.x, bounds.min.y),
            ] {
                bbox = bbox.with(xf.apply(V::of(p)));
            }
        }
    }
    bbox
}

/// A copy of the board with everything moved by `offset` (same ids).
#[must_use]
pub fn translated(pcb: &Pcb, offset: Point) -> Pcb {
    let shift = Shift(offset);
    let mut out = pcb.clone();
    let map = |item: Item| -> Item {
        let mut item = item;
        item.map(&shift, true);
        item
    };
    for fp in &mut out.footprints {
        if let Item::Footprint(moved) = map(Item::Footprint(fp.clone())) {
            *fp = moved;
        }
    }
    for track in &mut out.tracks {
        if let Item::Track(moved) = map(Item::Track(track.clone())) {
            *track = moved;
        }
    }
    for via in &mut out.vias {
        if let Item::Via(moved) = map(Item::Via(via.clone())) {
            *via = moved;
        }
    }
    for zone in &mut out.zones {
        if let Item::Zone(moved) = map(Item::Zone(zone.clone())) {
            *zone = moved;
        }
    }
    for text in &mut out.texts {
        if let Item::Text(moved) = map(Item::Text(text.clone())) {
            *text = moved;
        }
    }
    for shape in &mut out.shapes {
        if let Item::Shape(moved) = map(Item::Shape(shape.clone())) {
            *shape = moved;
        }
    }
    out
}

/// Runs an edit on the board `doc`.
///
/// # Errors
///
/// Unknown objects or impossible parameters.
#[allow(clippy::too_many_lines, reason = "one arm per operation")]
pub fn apply(doc: Id, pcb: &Pcb, edit: &BoardEdit) -> Result<Edit, EditError> {
    let copper_count = pcb.stackup.copper_count();
    match edit {
        BoardEdit::Move { ids, offset } => {
            let mut items = collect(pcb, ids)?;
            items.retain(|item| !item.locked());
            // A room takes the parts inside it along.
            let along = riders(pcb, &items, ids);
            items.extend(along);
            for item in &mut items {
                item.map(&Shift(*offset), true);
            }
            Ok(updates(doc, pcb, items, ids.clone()))
        }
        BoardEdit::Rotate { ids, center, angle } => {
            let mut items = collect(pcb, ids)?;
            items.retain(|item| !item.locked());
            let center = center.unwrap_or_else(|| center_of(&items));
            let turn = Turn {
                center,
                angle: *angle,
            };
            let keep_rect = angle.millidegrees() % 90_000 == 0;
            for item in &mut items {
                item.map(&turn, false);
                if keep_rect
                    && let Item::Shape(shape) = item
                    && let Geometry::Polygon { points } = &shape.geometry
                    && points.len() == 4
                    && is_axis_rect(points)
                {
                    shape.geometry = Geometry::Rect {
                        start: points[0],
                        end: points[2],
                    };
                }
                match item {
                    Item::Footprint(fp) => fp.rotation = add_angle(fp.rotation, *angle),
                    Item::Text(text) => text.rotation = add_angle(text.rotation, *angle),
                    _ => {}
                }
            }
            Ok(updates(doc, pcb, items, ids.clone()))
        }
        BoardEdit::Flip { ids, center_x } => {
            let mut items = collect(pcb, ids)?;
            items.retain(|item| !item.locked());
            let x = center_x.unwrap_or_else(|| center_of(&items).x);
            for item in &mut items {
                item.map(&Mirror(x), true);
                match item {
                    Item::Footprint(fp) => {
                        fp.side = fp.side.flipped();
                        fp.rotation = negate(fp.rotation);
                    }
                    Item::Track(track) => track.layer = track.layer.flipped(copper_count),
                    Item::Via(via) => {
                        let (from, to) =
                            (via.from.flipped(copper_count), via.to.flipped(copper_count));
                        let order: Vec<CopperLayer> = pcb.stackup.copper_layers().collect();
                        let rank = |l: CopperLayer| order.iter().position(|o| *o == l);
                        if rank(from) <= rank(to) {
                            (via.from, via.to) = (from, to);
                        } else {
                            (via.from, via.to) = (to, from);
                        }
                    }
                    Item::Zone(zone) => {
                        for layer in &mut zone.layers {
                            *layer = layer.flipped(copper_count);
                        }
                        for area in &mut zone.filled {
                            area.layer = area.layer.flipped(copper_count);
                        }
                    }
                    Item::Text(text) => {
                        text.layer = text.layer.flipped(copper_count);
                        text.mirrored = !text.mirrored;
                        text.rotation = negate(text.rotation);
                    }
                    Item::Shape(shape) => shape.layer = shape.layer.flipped(copper_count),
                    Item::Dimension(dim) => dim.layer = dim.layer.flipped(copper_count),
                    Item::Room(_) => {}
                }
            }
            Ok(updates(doc, pcb, items, ids.clone()))
        }
        BoardEdit::Delete { ids } => {
            let items = collect(pcb, ids)?;
            let gone: Vec<Id> = items
                .iter()
                .filter(|item| !item.locked())
                .map(object_id)
                .collect();
            let mut changes: Vec<Change> = gone
                .iter()
                .map(|id| Change::Delete { doc, id: *id })
                .collect();
            // Groups lose the deleted members; one member is no group.
            for group in &pcb.groups {
                if !group.members.iter().any(|m| gone.contains(m)) {
                    continue;
                }
                let rest: Vec<Id> = group
                    .members
                    .iter()
                    .copied()
                    .filter(|m| !gone.contains(m))
                    .collect();
                changes.push(if rest.len() < 2 {
                    Change::Delete { doc, id: group.id }
                } else {
                    Change::Update {
                        doc,
                        object: Object::Group(ew_model::pcb::Group {
                            members: rest,
                            ..group.clone()
                        }),
                    }
                });
            }
            Ok(Edit {
                changes,
                select: Vec::new(),
            })
        }
        BoardEdit::Group { ids, name } => Ok(group(doc, pcb, ids, name)),
        BoardEdit::Ungroup { ids } => {
            let changes = pcb
                .groups
                .iter()
                .filter(|g| ids.contains(&g.id) || g.members.iter().any(|m| ids.contains(m)))
                .map(|g| Change::Delete { doc, id: g.id })
                .collect();
            Ok(Edit {
                changes,
                select: ids.clone(),
            })
        }
        BoardEdit::Lock { ids, locked } => {
            let mut items = collect(pcb, ids)?;
            for item in &mut items {
                item.set_locked(*locked);
            }
            Ok(updates(doc, pcb, items, ids.clone()))
        }
        BoardEdit::Align { ids, mode } => {
            let footprints = footprints(pcb, ids)?;
            if footprints.len() < 2 {
                return Ok(Edit::default());
            }
            let boxes: Vec<BBox> = footprints.iter().map(footprint_box).collect();
            let all = boxes.iter().fold(BBox::EMPTY, |a, b| a.union(*b));
            let target = match mode {
                Align::Left => all.min.x,
                Align::Right => all.max.x,
                Align::Top => all.min.y,
                Align::Bottom => all.max.y,
                Align::CenterX => all.center().x,
                Align::CenterY => all.center().y,
            };
            let mut items = Vec::new();
            for (mut fp, bbox) in footprints.into_iter().zip(boxes) {
                if fp.locked {
                    continue;
                }
                let offset = match mode {
                    Align::Left => V::new(target - bbox.min.x, 0.0),
                    Align::Right => V::new(target - bbox.max.x, 0.0),
                    Align::Top => V::new(0.0, target - bbox.min.y),
                    Align::Bottom => V::new(0.0, target - bbox.max.y),
                    Align::CenterX => V::new(target - bbox.center().x, 0.0),
                    Align::CenterY => V::new(0.0, target - bbox.center().y),
                };
                fp.at = fp.at + offset.point();
                items.push(Item::Footprint(fp));
            }
            Ok(updates(doc, pcb, items, ids.clone()))
        }
        BoardEdit::Distribute { ids, axis } => {
            let mut footprints: Vec<(FootprintInstance, BBox)> = footprints(pcb, ids)?
                .into_iter()
                .map(|fp| {
                    let bbox = footprint_box(&fp);
                    (fp, bbox)
                })
                .collect();
            if footprints.len() < 3 {
                return Ok(Edit::default());
            }
            let lo = |b: &BBox| if *axis == Axis::X { b.min.x } else { b.min.y };
            let hi = |b: &BBox| if *axis == Axis::X { b.max.x } else { b.max.y };
            footprints.sort_by(|a, b| lo(&a.1).total_cmp(&lo(&b.1)));
            let start = lo(&footprints[0].1);
            let end = footprints
                .iter()
                .map(|(_, b)| hi(b))
                .fold(f64::MIN, f64::max);
            let sizes: f64 = footprints.iter().map(|(_, b)| hi(b) - lo(b)).sum();
            let gap = (end - start - sizes) / (footprints.len() - 1) as f64;
            let mut cursor = start;
            let mut items = Vec::new();
            for (mut fp, bbox) in footprints {
                let shift = cursor - lo(&bbox);
                cursor += hi(&bbox) - lo(&bbox) + gap;
                if fp.locked {
                    continue;
                }
                let offset = if *axis == Axis::X {
                    V::new(shift, 0.0)
                } else {
                    V::new(0.0, shift)
                };
                fp.at = fp.at + offset.point();
                items.push(Item::Footprint(fp));
            }
            Ok(updates(doc, pcb, items, ids.clone()))
        }
        BoardEdit::SetLayer { ids, layer } => {
            if !pcb.stackup.has_layer(Layer::Copper(*layer)) {
                return Err(EditError::Invalid(format!(
                    "the board has no layer {layer}"
                )));
            }
            let mut items = collect(pcb, ids)?;
            for item in &mut items {
                if let Item::Track(track) = item {
                    track.layer = *layer;
                }
            }
            Ok(updates(doc, pcb, items, ids.clone()))
        }
        BoardEdit::SetWidth { ids, width } => {
            if *width <= 0 {
                return Err(EditError::Invalid("the width must be positive".into()));
            }
            let mut items = collect(pcb, ids)?;
            for item in &mut items {
                if let Item::Track(track) = item {
                    track.width = *width;
                }
            }
            Ok(updates(doc, pcb, items, ids.clone()))
        }
        BoardEdit::SetNet { ids, net } => {
            if let Some(net) = net
                && pcb.nets.get(*net).is_none()
            {
                return Err(EditError::Unknown(*net));
            }
            let mut items = collect(pcb, ids)?;
            for item in &mut items {
                match item {
                    Item::Track(o) => o.net = *net,
                    Item::Via(o) => o.net = *net,
                    Item::Zone(o) => o.net = *net,
                    _ => {}
                }
            }
            Ok(updates(doc, pcb, items, ids.clone()))
        }
        BoardEdit::DragTrack { id, offset } => drag_track(doc, pcb, *id, *offset),
    }
}

fn is_axis_rect(points: &[Point]) -> bool {
    let xs: HashSet<Nm> = points.iter().map(|p| p.x).collect();
    let ys: HashSet<Nm> = points.iter().map(|p| p.y).collect();
    xs.len() == 2 && ys.len() == 2
}

fn footprints(pcb: &Pcb, ids: &[Id]) -> Result<Vec<FootprintInstance>, EditError> {
    ids.iter()
        .filter_map(|id| match Item::find(pcb, *id) {
            Some(Item::Footprint(fp)) => Some(Ok(fp)),
            Some(_) => None,
            None => Some(Err(EditError::Unknown(*id))),
        })
        .collect()
}

/// The shortest track left by a drag (nm).
const MIN_TRACK: f64 = 1_000.0;

/// What happens at an end of a dragged segment.
enum Joint {
    /// A single straight neighbour whose end slides along its own line.
    Slide { neighbour: Id, far: V },
    /// The end stays (pad, via, branch, locked or curved neighbour): a 45°
    /// track joins it to the moved segment.
    Fixed,
    /// Nothing is attached: the end moves with the segment.
    Free,
}

/// The joint at the end `end` of a track.
fn joint(pcb: &Pcb, track: &Track, end: Point) -> Joint {
    if pcb.vias.iter().any(|via| via.at == end) {
        return Joint::Fixed;
    }
    let at = V::of(end);
    let on_pad = pcb.footprints.iter().any(|fp| {
        let xf = footprint_xform(fp);
        // Bottom-side footprints have their outer copper flipped.
        let placed = |layer: CopperLayer| match (fp.side, layer) {
            (Side::Bottom, CopperLayer::Top) => CopperLayer::Bottom,
            (Side::Bottom, CopperLayer::Bottom) => CopperLayer::Top,
            _ => layer,
        };
        fp.pads.iter().any(|pad| {
            pad.layers.iter().any(|spec| match spec {
                LayerSpec::AllCopper => true,
                LayerSpec::Layer(Layer::Copper(layer)) => placed(*layer) == track.layer,
                _ => false,
            }) && pad_shape(pad, &xf, 0.0).contains(at)
        })
    });
    if on_pad {
        return Joint::Fixed;
    }
    let mut neighbours = pcb.tracks.iter().filter(|other| {
        other.id != track.id
            && other.layer == track.layer
            && other.net == track.net
            && (other.start == end || other.end == end)
    });
    match (neighbours.next(), neighbours.next()) {
        (None, _) => Joint::Free,
        (Some(other), None) if !other.locked && other.mid.is_none() => Joint::Slide {
            neighbour: other.id,
            far: V::of(if other.start == end {
                other.end
            } else {
                other.start
            }),
        },
        _ => Joint::Fixed,
    }
}

/// Where the line through `far` towards `end` meets the line through `on`
/// with direction `dir`, if it lies beyond `far` (the neighbour keeps its
/// direction and some length).
fn slide_to(far: V, end: V, on: V, dir: V) -> Option<V> {
    let along = end.sub(far);
    let denom = along.cross(dir);
    if denom.abs() < 1e-9 * along.len().max(1.0) {
        return None;
    }
    let t = on.sub(far).cross(dir) / denom;
    let point = far.add(along.scale(t));
    (t > 0.0 && point.dist(far) >= MIN_TRACK).then_some(point)
}

/// Moves a straight track segment sideways (the part of `offset` across
/// it). Its neighbours keep their directions: their ends slide along their
/// own lines to the moved segment. Where an end cannot slide, a 45° track
/// joins it to the moved segment. Curved tracks are moved with the ends of
/// the tracks and vias attached to them.
fn drag_track(doc: Id, pcb: &Pcb, id: Id, offset: Point) -> Result<Edit, EditError> {
    let track = pcb.tracks.get(id).ok_or(EditError::Unknown(id))?;
    if track.locked {
        return Ok(Edit::default());
    }
    if track.mid.is_some() {
        return Ok(stretch_track(doc, pcb, track, offset));
    }
    let (a, b) = (V::of(track.start), V::of(track.end));
    let dir = b.sub(a).unit();
    let normal = V::new(-dir.y, dir.x);
    let shift = normal.scale(V::of(offset).dot(normal));
    if shift.len() < 1.0 || a.dist(b) < MIN_TRACK {
        return Ok(Edit::default());
    }
    let mut updated: BTreeMap<Id, Track> = BTreeMap::new();
    let mut joiners: Vec<Track> = Vec::new();
    let mut ends = [a.add(shift), b.add(shift)];
    for (index, end) in [track.start, track.end].into_iter().enumerate() {
        let moved = ends[index];
        let slid = match joint(pcb, track, end) {
            Joint::Free => continue,
            Joint::Slide { neighbour, far } => {
                slide_to(far, V::of(end), moved, dir).map(|point| (neighbour, point))
            }
            Joint::Fixed => None,
        };
        if let Some((neighbour, point)) = slid {
            if let Some(other) = pcb.tracks.get(neighbour) {
                let mut other = other.clone();
                if other.start == end {
                    other.start = point.point();
                } else {
                    other.end = point.point();
                }
                updated.insert(other.id, other);
            }
            ends[index] = point;
        } else {
            // A 45° joiner from the old end into the moved segment.
            let inward = if index == 0 { dir } else { dir.scale(-1.0) };
            let point = moved.add(inward.scale(shift.len()));
            joiners.push(Track {
                id: Id::random(),
                start: end,
                end: point.point(),
                mid: None,
                locked: false,
                ..track.clone()
            });
            ends[index] = point;
        }
    }
    if ends[1].sub(ends[0]).dot(dir) < MIN_TRACK {
        // Too far: the segment would turn round.
        return Ok(Edit::default());
    }
    let mut moved = track.clone();
    moved.start = ends[0].point();
    moved.end = ends[1].point();
    let mut edit = Edit {
        changes: vec![Change::Update {
            doc,
            object: Object::Track(moved),
        }],
        select: vec![id],
    };
    for other in updated.into_values() {
        edit.changes.push(Change::Update {
            doc,
            object: Object::Track(other),
        });
    }
    for joiner in joiners {
        edit.changes.push(Change::Insert {
            doc,
            object: Object::Track(joiner),
        });
    }
    Ok(edit)
}

/// One group of the given objects (those that exist and are no nets or
/// groups); they leave the groups they were in, and groups left with fewer
/// than two members go.
fn group(doc: Id, pcb: &Pcb, ids: &[Id], name: &str) -> Edit {
    let groupable = |id: &Id| {
        pcb.footprints.contains(*id)
            || pcb.tracks.contains(*id)
            || pcb.vias.contains(*id)
            || pcb.zones.contains(*id)
            || pcb.texts.contains(*id)
            || pcb.shapes.contains(*id)
            || pcb.dimensions.contains(*id)
    };
    let mut members: Vec<Id> = Vec::new();
    for id in ids {
        if groupable(id) && !members.contains(id) {
            members.push(*id);
        }
    }
    if members.len() < 2 {
        return Edit::default();
    }
    let mut edit = Edit::default();
    for other in &pcb.groups {
        if !other.members.iter().any(|m| members.contains(m)) {
            continue;
        }
        let rest: Vec<Id> = other
            .members
            .iter()
            .copied()
            .filter(|m| !members.contains(m))
            .collect();
        if rest.len() < 2 {
            edit.changes.push(Change::Delete { doc, id: other.id });
        } else {
            edit.changes.push(Change::Update {
                doc,
                object: Object::Group(ew_model::pcb::Group {
                    members: rest,
                    ..other.clone()
                }),
            });
        }
    }
    edit.changes.push(Change::Insert {
        doc,
        object: Object::Group(ew_model::pcb::Group {
            id: Id::random(),
            name: name.to_owned(),
            members: members.clone(),
        }),
    });
    edit.select = members;
    edit
}

/// Moves a track; the ends of tracks and vias that touch its ends move with
/// it.
fn stretch_track(doc: Id, pcb: &Pcb, track: &Track, offset: Point) -> Edit {
    let id = track.id;
    let ends = [track.start, track.end];
    let moved = |p: Point| ends.contains(&p);
    let mut tracks: BTreeMap<Id, Track> = BTreeMap::new();
    let mut vias: HashMap<Id, Via> = HashMap::new();
    // Vias at the ends move, and with them the ends of tracks on other
    // layers.
    for via in &pcb.vias {
        if moved(via.at) && via.net == track.net && !via.locked {
            let mut via = via.clone();
            via.at = via.at + offset;
            vias.insert(via.id, via);
        }
    }
    let via_points: HashSet<Point> = vias.values().map(|v| v.at - offset).collect();
    for other in &pcb.tracks {
        if other.net != track.net || other.locked {
            continue;
        }
        let same_layer = other.layer == track.layer || other.id == id;
        let follows = |p: Point| (same_layer && moved(p)) || via_points.contains(&p);
        let (start, end) = (follows(other.start), follows(other.end));
        if !(start || end) {
            continue;
        }
        let mut copy = other.clone();
        if start {
            copy.start = copy.start + offset;
        }
        if end {
            copy.end = copy.end + offset;
        }
        if start && end {
            copy.mid = copy.mid.map(|m| m + offset);
        } else {
            copy.mid = None;
        }
        tracks.insert(copy.id, copy);
    }
    let mut edit = Edit {
        changes: Vec::new(),
        select: vec![id],
    };
    for track in tracks.into_values() {
        if track.start == track.end {
            edit.changes.push(Change::Delete { doc, id: track.id });
        } else {
            edit.changes.push(Change::Update {
                doc,
                object: Object::Track(track),
            });
        }
    }
    let mut vias: Vec<Via> = vias.into_values().collect();
    vias.sort_by_key(|v| v.id);
    for via in vias {
        edit.changes.push(Change::Update {
            doc,
            object: Object::Via(via),
        });
    }
    edit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectivity::tests::{board_with, track};
    use ew_model::layer::Side;
    use ew_model::units::mm;

    fn updated(edit: &Edit) -> Vec<Object> {
        edit.changes
            .iter()
            .filter_map(|c| match c {
                Change::Update { object, .. } => Some(object.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn turning_and_flipping_footprints() {
        let pcb = board_with(vec![(Point::mm(1, 0), None)]);
        let doc = pcb.id;
        let fp = pcb.footprints.iter().next().unwrap().id;
        let edit = apply(
            doc,
            &pcb,
            &BoardEdit::Rotate {
                ids: vec![fp],
                center: Some(Point::mm(10, 0)),
                angle: Angle::DEG_90,
            },
        )
        .unwrap();
        let Object::Footprint(turned) = &updated(&edit)[0] else {
            panic!("a footprint");
        };
        assert_eq!(turned.at, Point::mm(10, -10));
        assert_eq!(turned.rotation, Angle::DEG_90);
        let edit = apply(
            doc,
            &pcb,
            &BoardEdit::Flip {
                ids: vec![fp],
                center_x: Some(mm(5)),
            },
        )
        .unwrap();
        let Object::Footprint(flipped) = &updated(&edit)[0] else {
            panic!("a footprint");
        };
        assert_eq!(flipped.side, Side::Bottom);
        assert_eq!(flipped.at, Point::mm(10, 0));
        // The pad at local x = 1 mm ends up mirrored: x = 10 - 1 mm.
        let xf = footprint_xform(flipped);
        let pad = xf.apply(V::of(flipped.pads[0].at));
        assert!((pad.x - 9e6).abs() < 1.0, "{pad:?}");
    }

    #[test]
    fn aligning_and_distributing() {
        let mut pcb = board_with(Vec::new());
        let doc = pcb.id;
        let template = pcb.footprints.iter().next().unwrap().clone();
        let mut ids = Vec::new();
        for (x, y) in [(0, 0), (3, 5), (20, -4)] {
            let mut fp = template.clone();
            fp.id = Id::random();
            fp.at = Point::mm(x, y);
            fp.pads = vec![crate::connectivity::tests::pad("1", Point::ORIGIN, None)];
            ids.push(fp.id);
            pcb.footprints.insert(fp);
        }
        let edit = apply(
            doc,
            &pcb,
            &BoardEdit::Align {
                ids: ids.clone(),
                mode: Align::Top,
            },
        )
        .unwrap();
        for object in updated(&edit) {
            let Object::Footprint(fp) = object else {
                panic!("footprints");
            };
            assert_eq!(fp.at.y, mm(-4));
        }
        let edit = apply(doc, &pcb, &BoardEdit::Distribute { ids, axis: Axis::X }).unwrap();
        let moved = updated(&edit);
        assert_eq!(moved.len(), 1, "only the middle one moves");
        let Object::Footprint(fp) = &moved[0] else {
            panic!("a footprint");
        };
        assert_eq!(fp.at.x, mm(10));
    }

    #[test]
    fn dragging_a_track_stretches_its_neighbours() {
        let mut pcb = board_with(Vec::new());
        let doc = pcb.id;
        let net = Id::random();
        let a = track(net, Point::mm(0, 0), Point::mm(10, 0), CopperLayer::Top);
        let b = track(net, Point::mm(10, 0), Point::mm(10, 10), CopperLayer::Top);
        let c = track(net, Point::mm(10, 10), Point::mm(20, 10), CopperLayer::Top);
        let (bid, aid, cid) = (b.id, a.id, c.id);
        pcb.tracks.insert(a);
        pcb.tracks.insert(b);
        pcb.tracks.insert(c);
        let edit = apply(
            doc,
            &pcb,
            &BoardEdit::DragTrack {
                id: bid,
                offset: Point::mm(2, 0),
            },
        )
        .unwrap();
        let tracks: HashMap<Id, Track> = updated(&edit)
            .into_iter()
            .filter_map(|o| match o {
                Object::Track(t) => Some((t.id, t)),
                _ => None,
            })
            .collect();
        assert_eq!(tracks[&bid].start, Point::mm(12, 0));
        assert_eq!(tracks[&aid].end, Point::mm(12, 0));
        assert_eq!(tracks[&aid].start, Point::mm(0, 0));
        assert_eq!(tracks[&cid].start, Point::mm(12, 10));
    }

    fn drag(pcb: &Pcb, id: Id, offset: Point) -> (HashMap<Id, Track>, Vec<Track>) {
        let edit = apply(pcb.id, pcb, &BoardEdit::DragTrack { id, offset }).unwrap();
        let mut updated_tracks = HashMap::new();
        let mut inserted = Vec::new();
        for change in edit.changes {
            match change {
                Change::Update {
                    object: Object::Track(t),
                    ..
                } => {
                    updated_tracks.insert(t.id, t);
                }
                Change::Insert {
                    object: Object::Track(t),
                    ..
                } => inserted.push(t),
                _ => {}
            }
        }
        (updated_tracks, inserted)
    }

    #[test]
    fn dragging_keeps_45_degree_neighbours() {
        let mut pcb = board_with(Vec::new());
        let net = Id::random();
        let a = track(net, Point::mm(0, 0), Point::mm(5, 5), CopperLayer::Top);
        let b = track(net, Point::mm(5, 5), Point::mm(15, 5), CopperLayer::Top);
        let c = track(net, Point::mm(15, 5), Point::mm(20, 0), CopperLayer::Top);
        let (aid, bid, cid) = (a.id, b.id, c.id);
        for t in [a, b, c] {
            pcb.tracks.insert(t);
        }
        // Up by 2 mm (the part along the segment does not count).
        let (tracks, inserted) = drag(&pcb, bid, Point::mm(1, 2));
        assert_eq!(inserted.len(), 0);
        assert_eq!(
            (tracks[&bid].start, tracks[&bid].end),
            (Point::mm(7, 7), Point::mm(13, 7))
        );
        assert_eq!(
            (tracks[&aid].start, tracks[&aid].end),
            (Point::mm(0, 0), Point::mm(7, 7))
        );
        assert_eq!(
            (tracks[&cid].start, tracks[&cid].end),
            (Point::mm(13, 7), Point::mm(20, 0))
        );
        // So far that the segment would turn round: nothing happens.
        let (tracks, inserted) = drag(&pcb, bid, Point::mm(0, 6));
        assert_eq!((tracks.len(), inserted.len()), (0, 0));
    }

    #[test]
    fn a_pad_end_gets_a_45_degree_joiner() {
        let net = Id::random();
        let mut pcb = board_with(vec![(Point::mm(0, 0), Some(net))]);
        let t = track(net, Point::mm(0, 0), Point::mm(10, 0), CopperLayer::Top);
        let id = t.id;
        pcb.tracks.insert(t);
        let (tracks, inserted) = drag(&pcb, id, Point::mm(0, 2));
        // The free end moves; the pad end stays and a joiner goes up at 45°.
        assert_eq!(
            (tracks[&id].start, tracks[&id].end),
            (Point::mm(2, 2), Point::mm(10, 2))
        );
        assert_eq!(inserted.len(), 1);
        assert_eq!(
            (inserted[0].start, inserted[0].end),
            (Point::mm(0, 0), Point::mm(2, 2))
        );
    }

    #[test]
    fn groups_form_change_and_dissolve() {
        let net = Id::random();
        let mut pcb = board_with(vec![]);
        let doc = pcb.id;
        let ids: Vec<Id> = (0..3)
            .map(|i| {
                let t = track(net, Point::mm(i, 0), Point::mm(i, 5), CopperLayer::Top);
                let id = t.id;
                pcb.tracks.insert(t);
                id
            })
            .collect();
        // One object (or unknown ids) makes no group.
        let edit = apply(
            doc,
            &pcb,
            &BoardEdit::Group {
                ids: vec![ids[0], Id::random()],
                name: String::new(),
            },
        )
        .unwrap();
        assert_eq!(edit.changes, Vec::new());
        // Group all three.
        let edit = apply(
            doc,
            &pcb,
            &BoardEdit::Group {
                ids: ids.clone(),
                name: "Filter".into(),
            },
        )
        .unwrap();
        let [
            Change::Insert {
                object: Object::Group(group),
                ..
            },
        ] = edit.changes.as_slice()
        else {
            panic!("one new group: {:?}", edit.changes);
        };
        assert_eq!((group.name.as_str(), &group.members), ("Filter", &ids));
        pcb.groups.insert(group.clone());
        // Two of them in a new group: the old one keeps one member and goes.
        let edit = apply(
            doc,
            &pcb,
            &BoardEdit::Group {
                ids: ids[1..].to_vec(),
                name: String::new(),
            },
        )
        .unwrap();
        assert!(matches!(edit.changes[0], Change::Delete { id, .. } if id == group.id));
        assert!(matches!(edit.changes[1], Change::Insert { .. }));
        // Deleting a member shrinks the group.
        let edit = apply(doc, &pcb, &BoardEdit::Delete { ids: vec![ids[0]] }).unwrap();
        let Some(Change::Update {
            object: Object::Group(shrunk),
            ..
        }) = edit.changes.last()
        else {
            panic!("the group is updated: {:?}", edit.changes);
        };
        assert_eq!(shrunk.members, ids[1..].to_vec());
        // Ungrouping by a member removes the group.
        let edit = apply(doc, &pcb, &BoardEdit::Ungroup { ids: vec![ids[2]] }).unwrap();
        assert!(matches!(edit.changes.as_slice(), [Change::Delete { id, .. }] if *id == group.id));
    }
}
