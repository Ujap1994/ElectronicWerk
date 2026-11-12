//! Copper geometry in floating-point nanometres.
//!
//! Every piece of copper is a [`Shape`]: a core (a point, a segment or a
//! polygon) grown by a radius. A via is a point with a radius, a track a
//! segment with half its width, a rounded rectangle pad a smaller rectangle
//! with its corner radius. The distance between two shapes is the distance
//! between their cores minus both radii, which is exact for all of them.

use std::f64::consts::{PI, TAU};
use std::sync::Arc;

use ew_model::units::{Angle, Point};
use rstar::{AABB, RTree, RTreeObject};

/// A point or vector in nanometres.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct V {
    /// x.
    pub x: f64,
    /// y.
    pub y: f64,
}

#[allow(
    clippy::should_implement_trait,
    reason = "plain methods read better in geometry code than operators"
)]
impl V {
    /// A new point.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// A model point.
    #[must_use]
    pub fn of(point: Point) -> Self {
        Self::new(point.x as f64, point.y as f64)
    }

    /// The nearest model point.
    #[must_use]
    pub fn point(self) -> Point {
        Point::new(self.x.round() as i64, self.y.round() as i64)
    }

    /// Sum.
    #[must_use]
    pub fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }

    /// Difference.
    #[must_use]
    pub fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    /// Scaled by `factor`.
    #[must_use]
    pub fn scale(self, factor: f64) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }

    /// Dot product.
    #[must_use]
    pub fn dot(self, other: Self) -> f64 {
        self.x.mul_add(other.x, self.y * other.y)
    }

    /// z of the cross product.
    #[must_use]
    pub fn cross(self, other: Self) -> f64 {
        self.x.mul_add(other.y, -(self.y * other.x))
    }

    /// Length.
    #[must_use]
    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }

    /// Distance to another point.
    #[must_use]
    pub fn dist(self, other: Self) -> f64 {
        self.sub(other).len()
    }

    /// Rotated counter-clockwise by `angle` radians (the scene's convention).
    #[must_use]
    pub fn rotate(self, angle: f64) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self::new(
            self.x.mul_add(cos, -(self.y * sin)),
            self.x.mul_add(sin, self.y * cos),
        )
    }

    /// The unit vector in this direction (zero stays zero).
    #[must_use]
    pub fn unit(self) -> Self {
        let len = self.len();
        if len > 0.0 {
            self.scale(1.0 / len)
        } else {
            self
        }
    }

    /// The vector turned by 90°.
    #[must_use]
    pub const fn perp(self) -> Self {
        Self::new(-self.y, self.x)
    }

    /// The point as an `[x, y]` pair (for the polygon library).
    #[must_use]
    pub const fn pair(self) -> [f64; 2] {
        [self.x, self.y]
    }
}

/// Radians of a model angle.
#[must_use]
pub fn radians(angle: Angle) -> f64 {
    angle.degrees().to_radians()
}

/// Placement of local coordinates (footprints): mirror x first, then rotate,
/// then translate, exactly like the board scene.
#[derive(Debug, Clone, Copy)]
pub struct Xform {
    /// Translation.
    pub offset: V,
    /// Rotation in radians.
    pub angle: f64,
    /// Mirror x before rotating.
    pub mirror: bool,
}

impl Xform {
    /// No change.
    pub const IDENTITY: Self = Self {
        offset: V::new(0.0, 0.0),
        angle: 0.0,
        mirror: false,
    };

    /// Maps a local point.
    #[must_use]
    pub fn apply(&self, p: V) -> V {
        let local = if self.mirror { V::new(-p.x, p.y) } else { p };
        local.rotate(self.angle).add(self.offset)
    }

    /// Maps a local direction angle.
    #[must_use]
    pub fn angle_of(&self, local: f64) -> f64 {
        let local = if self.mirror { PI - local } else { local };
        local + self.angle
    }
}

/// An axis-aligned box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    /// Lower corner.
    pub min: V,
    /// Upper corner.
    pub max: V,
}

impl BBox {
    /// The box that contains nothing.
    pub const EMPTY: Self = Self {
        min: V::new(f64::INFINITY, f64::INFINITY),
        max: V::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
    };

    /// The box of one point.
    #[must_use]
    pub const fn of(p: V) -> Self {
        Self { min: p, max: p }
    }

    /// The box of several points.
    #[must_use]
    pub fn of_points(points: &[V]) -> Self {
        points.iter().fold(Self::EMPTY, |b, p| b.with(*p))
    }

    /// `true` if the box contains nothing.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y
    }

    /// The box grown to include `p`.
    #[must_use]
    pub const fn with(self, p: V) -> Self {
        Self {
            min: V::new(self.min.x.min(p.x), self.min.y.min(p.y)),
            max: V::new(self.max.x.max(p.x), self.max.y.max(p.y)),
        }
    }

    /// The union of two boxes.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            min: V::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: V::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    /// The box grown by `d` on every side.
    #[must_use]
    pub fn grow(self, d: f64) -> Self {
        if self.is_empty() {
            return self;
        }
        Self {
            min: V::new(self.min.x - d, self.min.y - d),
            max: V::new(self.max.x + d, self.max.y + d),
        }
    }

    /// `true` if the boxes overlap or touch.
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
    }

    /// `true` if `other` lies completely inside.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.min.x <= other.min.x
            && self.min.y <= other.min.y
            && other.max.x <= self.max.x
            && other.max.y <= self.max.y
    }

    /// The centre.
    #[must_use]
    pub fn center(self) -> V {
        V::new(
            f64::midpoint(self.min.x, self.max.x),
            f64::midpoint(self.min.y, self.max.y),
        )
    }

    /// The box as an R-tree envelope.
    #[must_use]
    pub fn aabb(self) -> AABB<[f64; 2]> {
        AABB::from_corners(self.min.pair(), self.max.pair())
    }
}

/// Distance from `p` to the segment `a`–`b`.
#[must_use]
pub fn point_segment(p: V, a: V, b: V) -> f64 {
    p.dist(closest_on_segment(p, a, b))
}

/// The point of the segment `a`–`b` nearest to `p`.
#[must_use]
pub fn closest_on_segment(p: V, a: V, b: V) -> V {
    let ab = b.sub(a);
    let len2 = ab.dot(ab);
    if len2 <= 0.0 {
        return a;
    }
    let t = (p.sub(a).dot(ab) / len2).clamp(0.0, 1.0);
    a.add(ab.scale(t))
}

fn orient(a: V, b: V, c: V) -> f64 {
    b.sub(a).cross(c.sub(a))
}

fn on_segment(a: V, b: V, p: V) -> bool {
    p.x >= a.x.min(b.x) && p.x <= a.x.max(b.x) && p.y >= a.y.min(b.y) && p.y <= a.y.max(b.y)
}

/// `true` if the segments `a`–`b` and `c`–`d` cross or touch.
#[must_use]
pub fn segments_intersect(a: V, b: V, c: V, d: V) -> bool {
    let d1 = orient(c, d, a);
    let d2 = orient(c, d, b);
    let d3 = orient(a, b, c);
    let d4 = orient(a, b, d);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    (d1 == 0.0 && on_segment(c, d, a))
        || (d2 == 0.0 && on_segment(c, d, b))
        || (d3 == 0.0 && on_segment(a, b, c))
        || (d4 == 0.0 && on_segment(a, b, d))
}

/// Distance between the segments `a`–`b` and `c`–`d` (0 if they cross).
#[must_use]
pub fn segment_segment(a: V, b: V, c: V, d: V) -> f64 {
    if segments_intersect(a, b, c, d) {
        return 0.0;
    }
    point_segment(a, c, d)
        .min(point_segment(b, c, d))
        .min(point_segment(c, a, b))
        .min(point_segment(d, a, b))
}

/// Twice the signed area of a ring (positive: counter-clockwise in a y-up
/// frame).
#[must_use]
pub fn signed_area2(ring: &[V]) -> f64 {
    let mut sum = 0.0;
    for (i, p) in ring.iter().enumerate() {
        let q = ring[(i + 1) % ring.len()];
        sum += p.cross(q);
    }
    sum
}

/// Number of segments for an arc of `sweep` radians and radius `r`, so
/// that the chord error stays below `tolerance`.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the count is clamped to 4..=256"
)]
pub fn arc_segments(r: f64, sweep: f64, tolerance: f64) -> usize {
    if r <= tolerance {
        return 4;
    }
    let step = 2.0 * (1.0 - tolerance / r).clamp(-1.0, 1.0).acos();
    let n = (sweep.abs() / step.max(1e-3)).ceil();
    (n as usize).clamp(4, 256)
}

/// A circle as a polygon that contains the true circle (the vertices lie
/// slightly outside, so knock-outs are never too small).
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "small segment counts")]
pub fn circle_polygon(center: V, r: f64, tolerance: f64) -> Vec<V> {
    let n = arc_segments(r, TAU, tolerance).max(8);
    let step = TAU / n as f64;
    let outer = r / (step / 2.0).cos();
    (0..n)
        .map(|i| {
            let a = step * i as f64;
            center.add(V::new(a.cos(), a.sin()).scale(outer))
        })
        .collect()
}

/// The centre, radius, start angle and sweep of the arc through three
/// points, or `None` if they are on one line.
#[must_use]
pub fn arc_through(start: V, mid: V, end: V) -> Option<(V, f64, f64, f64)> {
    let d =
        2.0 * (start.x * (mid.y - end.y) + mid.x * (end.y - start.y) + end.x * (start.y - mid.y));
    let scale = start.dist(end).max(start.dist(mid)).max(1.0);
    if d.abs() < 1e-9 * scale * scale {
        return None;
    }
    let a2 = start.dot(start);
    let b2 = mid.dot(mid);
    let c2 = end.dot(end);
    let center = V::new(
        (a2 * (mid.y - end.y) + b2 * (end.y - start.y) + c2 * (start.y - mid.y)) / d,
        (a2 * (end.x - mid.x) + b2 * (start.x - end.x) + c2 * (mid.x - start.x)) / d,
    );
    let radius = center.dist(start);
    let angle = |p: V| (p.y - center.y).atan2(p.x - center.x);
    let from = angle(start);
    let ccw = |to: f64| (to - from).rem_euclid(TAU);
    let to_mid = ccw(angle(mid));
    let to_end = ccw(angle(end));
    let sweep = if to_mid <= to_end {
        to_end
    } else {
        to_end - TAU
    };
    Some((center, radius, from, sweep))
}

/// Points along the arc through three points (straight if they are on one
/// line), including both ends.
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "small segment counts")]
pub fn arc_points(start: V, mid: V, end: V, tolerance: f64) -> Vec<V> {
    let Some((center, radius, from, sweep)) = arc_through(start, mid, end) else {
        return vec![start, end];
    };
    let n = arc_segments(radius, sweep, tolerance);
    let mut points: Vec<V> = (0..=n)
        .map(|i| {
            let a = from + sweep * i as f64 / n as f64;
            center.add(V::new(a.cos(), a.sin()).scale(radius))
        })
        .collect();
    points[0] = start;
    points[n] = end;
    points
}

/// One edge of a polygon, stored in the polygon's R-tree.
#[derive(Debug, Clone, Copy)]
pub struct Edge {
    /// Start.
    pub a: V,
    /// End.
    pub b: V,
}

impl RTreeObject for Edge {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        BBox::of(self.a).with(self.b).aabb()
    }
}

/// Polygons with more edges than this get an R-tree of their edges.
const TREE_EDGES: usize = 48;

/// A polygon with holes (rings in any orientation; even-odd filling).
#[derive(Debug)]
pub struct Poly {
    rings: Vec<Vec<V>>,
    bbox: BBox,
    edges: usize,
    tree: Option<RTree<Edge>>,
}

impl Poly {
    /// A polygon from its outline and holes.
    #[must_use]
    pub fn new(outline: Vec<V>, holes: Vec<Vec<V>>) -> Self {
        let mut rings = vec![outline];
        rings.extend(holes.into_iter().filter(|hole| hole.len() >= 3));
        let bbox = rings
            .iter()
            .fold(BBox::EMPTY, |b, ring| b.union(BBox::of_points(ring)));
        let edges = rings.iter().map(Vec::len).sum();
        let tree = (edges > TREE_EDGES).then(|| {
            RTree::bulk_load(
                rings
                    .iter()
                    .flat_map(|ring| ring_edges(ring))
                    .collect::<Vec<_>>(),
            )
        });
        Self {
            rings,
            bbox,
            edges,
            tree,
        }
    }

    /// The outline.
    #[must_use]
    pub fn outline(&self) -> &[V] {
        &self.rings[0]
    }

    /// The holes.
    #[must_use]
    pub fn holes(&self) -> &[Vec<V>] {
        &self.rings[1..]
    }

    /// All rings: the outline, then the holes.
    #[must_use]
    pub fn rings(&self) -> &[Vec<V>] {
        &self.rings
    }

    /// The bounding box.
    #[must_use]
    pub const fn bbox(&self) -> BBox {
        self.bbox
    }

    /// Number of edges.
    #[must_use]
    pub const fn edge_count(&self) -> usize {
        self.edges
    }

    /// Calls `visit` for every edge whose box meets `area` (all edges of
    /// small polygons).
    pub fn edges_near(&self, area: BBox, visit: &mut dyn FnMut(V, V)) {
        match &self.tree {
            Some(tree) => {
                for edge in tree.locate_in_envelope_intersecting(area.aabb()) {
                    visit(edge.a, edge.b);
                }
            }
            None => {
                for ring in &self.rings {
                    for edge in ring_edges(ring) {
                        visit(edge.a, edge.b);
                    }
                }
            }
        }
    }

    /// `true` if `p` lies inside (even-odd over all rings).
    #[must_use]
    pub fn contains(&self, p: V) -> bool {
        if p.x < self.bbox.min.x
            || p.x > self.bbox.max.x
            || p.y < self.bbox.min.y
            || p.y > self.bbox.max.y
        {
            return false;
        }
        let ray = BBox {
            min: p,
            max: V::new(self.bbox.max.x + 1.0, p.y),
        };
        let mut inside = false;
        self.edges_near(ray, &mut |a, b| {
            if (a.y > p.y) != (b.y > p.y) {
                let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
                if x > p.x {
                    inside = !inside;
                }
            }
        });
        inside
    }

    /// Area (outline minus holes).
    #[must_use]
    pub fn area(&self) -> f64 {
        let outline = signed_area2(self.outline()).abs() / 2.0;
        let holes: f64 = self
            .holes()
            .iter()
            .map(|hole| signed_area2(hole).abs() / 2.0)
            .sum();
        outline - holes
    }
}

fn ring_edges(ring: &[V]) -> impl Iterator<Item = Edge> + '_ {
    let n = ring.len();
    (0..n).map(move |i| Edge {
        a: ring[i],
        b: ring[(i + 1) % n],
    })
}

/// The core of a shape.
#[derive(Debug, Clone)]
pub enum Core {
    /// A point (discs).
    Point(V),
    /// A segment (stadiums: tracks, ovals, slots).
    Segment(V, V),
    /// A polygon (rectangles, custom pads, zone fills).
    Poly(Arc<Poly>),
}

/// A copper shape: a core grown by a radius.
#[derive(Debug, Clone)]
pub struct Shape {
    /// The core.
    pub core: Core,
    /// The radius around the core.
    pub radius: f64,
}

impl Shape {
    /// A disc.
    #[must_use]
    pub const fn disc(center: V, radius: f64) -> Self {
        Self {
            core: Core::Point(center),
            radius,
        }
    }

    /// A stadium (a segment with round ends).
    #[must_use]
    pub const fn stadium(a: V, b: V, radius: f64) -> Self {
        Self {
            core: Core::Segment(a, b),
            radius,
        }
    }

    /// A polygon grown by `radius`.
    #[must_use]
    pub fn polygon(outline: Vec<V>, holes: Vec<Vec<V>>, radius: f64) -> Self {
        Self {
            core: Core::Poly(Arc::new(Poly::new(outline, holes))),
            radius,
        }
    }

    /// The bounding box.
    #[must_use]
    pub fn bbox(&self) -> BBox {
        let core = match &self.core {
            Core::Point(p) => BBox::of(*p),
            Core::Segment(a, b) => BBox::of(*a).with(*b),
            Core::Poly(poly) => poly.bbox(),
        };
        core.grow(self.radius)
    }

    /// A point inside the shape.
    #[must_use]
    pub fn anchor(&self) -> V {
        match &self.core {
            Core::Point(p) | Core::Segment(p, _) => *p,
            Core::Poly(poly) => poly.outline()[0],
        }
    }

    /// The distance between the edges of two shapes if it is below
    /// `limit` (negative when they overlap), else `None`.
    #[must_use]
    pub fn gap(&self, other: &Self, limit: f64) -> Option<f64> {
        let radii = self.radius + other.radius;
        let bound = limit + radii;
        if !self.bbox().grow(limit).intersects(other.bbox()) {
            return None;
        }
        core_distance(&self.core, &other.core, bound)
            .map(|d| d - radii)
            .filter(|gap| *gap < limit)
    }

    /// `true` if the shapes touch or overlap.
    #[must_use]
    pub fn touches(&self, other: &Self) -> bool {
        self.gap(other, 1.0).is_some()
    }

    /// `true` if the point lies on the shape.
    #[must_use]
    pub fn contains(&self, p: V) -> bool {
        self.gap(&Self::disc(p, 0.0), 0.5).is_some()
    }

    /// The shape grown by `extra` as polygon outlines (each with its
    /// holes), for the zone filler. Curves are approximated from outside.
    #[must_use]
    pub fn outlines(&self, extra: f64, tolerance: f64) -> Vec<Vec<Vec<V>>> {
        let r = self.radius + extra;
        match &self.core {
            Core::Point(p) => {
                if r <= 0.0 {
                    return Vec::new();
                }
                vec![vec![circle_polygon(*p, r, tolerance)]]
            }
            Core::Segment(a, b) => {
                if r <= 0.0 {
                    return Vec::new();
                }
                vec![vec![stadium_polygon(*a, *b, r, tolerance)]]
            }
            Core::Poly(poly) => {
                let rings: Vec<Vec<V>> = poly.rings().to_vec();
                if r.abs() < 0.5 {
                    return vec![rings];
                }
                if poly.holes().is_empty() && is_convex(poly.outline()) && r > 0.0 {
                    return vec![vec![grow_convex(poly.outline(), r, tolerance)]];
                }
                crate::poly::offset(&[rings], r, tolerance)
            }
        }
    }
}

#[allow(clippy::many_single_char_names, reason = "geometry")]
fn core_distance(a: &Core, b: &Core, bound: f64) -> Option<f64> {
    let d = match (a, b) {
        (Core::Point(p), Core::Point(q)) => p.dist(*q),
        (Core::Point(p), Core::Segment(s, e)) | (Core::Segment(s, e), Core::Point(p)) => {
            point_segment(*p, *s, *e)
        }
        (Core::Segment(a1, b1), Core::Segment(a2, b2)) => segment_segment(*a1, *b1, *a2, *b2),
        (Core::Poly(poly), Core::Point(p)) | (Core::Point(p), Core::Poly(poly)) => {
            if poly.contains(*p) {
                0.0
            } else {
                let mut best = f64::INFINITY;
                poly.edges_near(BBox::of(*p).grow(bound), &mut |a, b| {
                    best = best.min(point_segment(*p, a, b));
                });
                best
            }
        }
        (Core::Poly(poly), Core::Segment(s, e)) | (Core::Segment(s, e), Core::Poly(poly)) => {
            if poly.contains(*s) {
                0.0
            } else {
                let mut best = f64::INFINITY;
                poly.edges_near(BBox::of(*s).with(*e).grow(bound), &mut |a, b| {
                    best = best.min(segment_segment(*s, *e, a, b));
                });
                best
            }
        }
        (Core::Poly(p), Core::Poly(q)) => {
            if p.contains(q.outline()[0]) || q.contains(p.outline()[0]) {
                0.0
            } else {
                let (small, large) = if p.edge_count() <= q.edge_count() {
                    (p, q)
                } else {
                    (q, p)
                };
                let mut best = f64::INFINITY;
                small.edges_near(large.bbox().grow(bound), &mut |a, b| {
                    large.edges_near(BBox::of(a).with(b).grow(bound), &mut |c, d| {
                        best = best.min(segment_segment(a, b, c, d));
                    });
                });
                best
            }
        }
    };
    (d < bound).then_some(d)
}

/// The stadium around `a`–`b` with radius `r` as a polygon.
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "small segment counts")]
pub fn stadium_polygon(a: V, b: V, r: f64, tolerance: f64) -> Vec<V> {
    if a.dist(b) < 0.5 {
        return circle_polygon(a, r, tolerance);
    }
    let dir = b.sub(a);
    let base = dir.y.atan2(dir.x);
    let n = arc_segments(r, PI, tolerance).max(4);
    let step = PI / n as f64;
    let outer = r / (step / 2.0).cos();
    let mut points = Vec::with_capacity(2 * n + 4);
    // Half circle around b from -90° to +90°, then around a from +90° to 270°.
    for (center, start) in [(b, base - PI / 2.0), (a, base + PI / 2.0)] {
        points.push(center.add(V::new(start.cos(), start.sin()).scale(r)));
        for i in 0..n {
            let angle = start + step * (i as f64 + 0.5);
            points.push(center.add(V::new(angle.cos(), angle.sin()).scale(outer)));
        }
        let end = start + PI;
        points.push(center.add(V::new(end.cos(), end.sin()).scale(r)));
    }
    points
}

/// `true` if the ring is convex.
#[must_use]
pub fn is_convex(ring: &[V]) -> bool {
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut sign = 0.0_f64;
    for i in 0..n {
        let turn = orient(ring[i], ring[(i + 1) % n], ring[(i + 2) % n]);
        if turn.abs() < 1e-9 {
            continue;
        }
        if sign.abs() < 0.5 {
            sign = turn.signum();
        } else if (turn.signum() - sign).abs() > 0.5 {
            return false;
        }
    }
    true
}

/// A convex ring grown by `r` (round corners, approximated from outside).
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "small segment counts")]
pub fn grow_convex(ring: &[V], r: f64, tolerance: f64) -> Vec<V> {
    let ccw = signed_area2(ring) > 0.0;
    let ordered: Vec<V> = if ccw {
        ring.to_vec()
    } else {
        ring.iter().rev().copied().collect()
    };
    let n = ordered.len();
    let normal = |a: V, b: V| {
        let d = b.sub(a).unit();
        V::new(d.y, -d.x)
    };
    let mut out = Vec::new();
    for i in 0..n {
        let prev = ordered[(i + n - 1) % n];
        let here = ordered[i];
        let next = ordered[(i + 1) % n];
        if here.dist(prev) < 1e-6 {
            continue;
        }
        let n1 = normal(prev, here);
        let n2 = normal(
            here,
            if next.dist(here) < 1e-6 {
                ordered[(i + 2) % n]
            } else {
                next
            },
        );
        let a1 = n1.y.atan2(n1.x);
        let mut sweep = n2.y.atan2(n2.x) - a1;
        while sweep < 0.0 {
            sweep += TAU;
        }
        if sweep > PI {
            sweep = 0.0;
        }
        let steps = if sweep < 1e-6 {
            0
        } else {
            arc_segments(r, sweep, tolerance)
        };
        out.push(here.add(n1.scale(r)));
        if steps > 0 {
            let step = sweep / steps as f64;
            let outer = r / (step / 2.0).cos();
            for k in 0..steps {
                let angle = a1 + step * (k as f64 + 0.5);
                out.push(here.add(V::new(angle.cos(), angle.sin()).scale(outer)));
            }
            out.push(here.add(n2.scale(r)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn distances_between_shapes() {
        let a = Shape::disc(V::new(0.0, 0.0), 1.0);
        let b = Shape::disc(V::new(5.0, 0.0), 1.0);
        assert!(close(a.gap(&b, 10.0).unwrap(), 3.0));
        assert!(a.gap(&b, 3.0).is_none());
        let track = Shape::stadium(V::new(0.0, 4.0), V::new(10.0, 4.0), 0.5);
        assert!(close(a.gap(&track, 10.0).unwrap(), 2.5));
        let rect = Shape::polygon(
            vec![
                V::new(-1.0, -1.0),
                V::new(1.0, -1.0),
                V::new(1.0, 1.0),
                V::new(-1.0, 1.0),
            ],
            Vec::new(),
            0.0,
        );
        assert!(close(rect.gap(&b, 10.0).unwrap(), 3.0));
        // A disc inside the rectangle overlaps it.
        assert!(rect.gap(&Shape::disc(V::new(0.0, 0.0), 0.1), 1.0).unwrap() < 0.0);
        assert!(rect.touches(&Shape::stadium(V::new(-5.0, 0.0), V::new(5.0, 0.0), 0.1)));
    }

    #[test]
    fn polygons_with_holes_and_many_edges() {
        let outer = circle_polygon(V::new(0.0, 0.0), 100.0, 0.01);
        assert!(outer.len() > TREE_EDGES);
        let hole = vec![
            V::new(-10.0, -10.0),
            V::new(10.0, -10.0),
            V::new(10.0, 10.0),
            V::new(-10.0, 10.0),
        ];
        let ring = Shape::polygon(outer, vec![hole], 0.0);
        assert!(ring.contains(V::new(50.0, 0.0)));
        assert!(!ring.contains(V::new(0.0, 0.0)), "inside the hole");
        // A via in the hole is 9 away from the hole's edge.
        let via = Shape::disc(V::new(0.0, 0.0), 1.0);
        assert!(close(ring.gap(&via, 20.0).unwrap(), 9.0));
    }

    #[test]
    fn polygons_cover_their_curves() {
        let circle = circle_polygon(V::new(0.0, 0.0), 1000.0, 1.0);
        for p in &circle {
            assert!(p.len() >= 1000.0 - 1e-6 && p.len() <= 1001.5);
        }
        let stadium = stadium_polygon(V::new(0.0, 0.0), V::new(5000.0, 0.0), 500.0, 1.0);
        let shape = Shape::stadium(V::new(0.0, 0.0), V::new(5000.0, 0.0), 500.0);
        for p in &stadium {
            let d = point_segment(*p, V::new(0.0, 0.0), V::new(5000.0, 0.0));
            assert!((500.0 - 1e-6..=501.5).contains(&d), "{d}");
        }
        assert!(shape.contains(V::new(2500.0, 499.0)));
        let grown = grow_convex(
            &[
                V::new(0.0, 0.0),
                V::new(0.0, 10.0),
                V::new(10.0, 10.0),
                V::new(10.0, 0.0),
            ],
            2.0,
            0.01,
        );
        let bbox = BBox::of_points(&grown);
        assert!(close(bbox.min.x, -2.0) || bbox.min.x < -2.0);
        assert!(bbox.max.x <= 12.1 && bbox.max.x >= 12.0);
    }

    #[test]
    fn arcs_through_three_points() {
        let (center, radius, _, sweep) =
            arc_through(V::new(1.0, 0.0), V::new(0.0, 1.0), V::new(-1.0, 0.0)).unwrap();
        assert!(close(center.x, 0.0) && close(center.y, 0.0) && close(radius, 1.0));
        assert!(close(sweep, PI));
        let points = arc_points(
            V::new(1.0, 0.0),
            V::new(0.0, -1.0),
            V::new(-1.0, 0.0),
            0.001,
        );
        assert!(
            points.iter().all(|p| p.y <= 1e-9),
            "the arc runs through -y"
        );
        let xf = Xform {
            offset: V::new(10.0, 0.0),
            angle: PI / 2.0,
            mirror: true,
        };
        let p = xf.apply(V::new(1.0, 0.0));
        assert!(close(p.x, 10.0) && close(p.y, -1.0));
    }
}
