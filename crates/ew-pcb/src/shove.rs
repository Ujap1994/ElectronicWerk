//! Push and shove: the copper of a new route pushes the tracks of other
//! nets aside instead of walking around them.
//!
//! The straight, unlocked tracks of one net on one layer are chained into
//! **lines** between joints (pads, vias, branches, locked or curved
//! tracks). A line in the way **walks around the hull** of the pushing
//! copper: an octagon around every point of it at the clearance distance,
//! so the hull's edges run at multiples of 45°. Where the line meets the
//! hull at a right angle the corner is chamfered to two 45° bends. A pushed
//! line pushes the lines it now meets in turn.
//!
//! Pads, vias, locked and curved tracks, keep-out areas and the board edge
//! never move: a push that would hit them fails, and so does one that goes
//! on for too long. The router then walks around instead.

use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::consts::PI;

use ew_model::id::Id;
use ew_model::layer::CopperLayer;
use ew_model::pcb::Track;
use ew_model::units::Point;

use crate::geom::{BBox, V, point_segment, segment_segment};
use crate::items::ItemRef;
use crate::poly;
use crate::route::{Corners, Request, Router};
use crate::rules::{NoZones, Props};

/// Distance kept beyond the clearance, for the rounding of new corners.
const MARGIN: f64 = 2_000.0;
/// The most pushes of one route before giving up.
const MAX_PUSHES: usize = 64;
/// How far around the route lines are looked for.
const REACH: f64 = 20_000_000.0;
/// Bends up to this angle (radians) count as 45° (rounding).
const BEND_45: f64 = PI / 4.0 + 1e-3;
/// Bends up to this angle count as 90°.
const BEND_90: f64 = PI / 2.0 + 1e-3;

/// Copper of a route that pushes: a track polyline or a via.
#[derive(Debug, Clone, PartialEq)]
pub struct Head {
    /// The layers it is on: one for a track, the span for a via.
    pub layers: Vec<CopperLayer>,
    /// Its net.
    pub net: Option<Id>,
    /// Track width or via diameter.
    pub width: f64,
    /// The track's points, or the via's centre.
    pub points: Vec<V>,
}

impl Head {
    fn is_via(&self) -> bool {
        self.points.len() == 1
    }
}

/// What a push changes: the tracks it removes and the ones it adds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shoved {
    /// Tracks of the pushed lines, removed.
    pub removed: Vec<Id>,
    /// The pushed lines, as new tracks.
    pub added: Vec<Track>,
}

/// A route found in push-and-shove mode: its points and what it pushes.
#[derive(Debug, Clone, PartialEq)]
pub struct ShoveRoute {
    /// Points from the start to the end.
    pub points: Vec<[f64; 2]>,
    /// `true` if the route ends at the target.
    pub reached: bool,
    /// The tracks it pushes aside (with the copper placed before), or
    /// `None` if they cannot make room.
    pub shoved: Option<Shoved>,
}

/// A line that may move.
#[derive(Debug, Clone)]
struct Line {
    ids: Vec<Id>,
    net: Option<Id>,
    width: f64,
    points: Vec<V>,
    changed: bool,
}

/// Something that pushes: a head, or a line pushed before.
#[derive(Debug, Clone)]
struct Pusher {
    net: Option<Id>,
    width: f64,
    points: Vec<V>,
    via: bool,
    /// The line it is (lines do not push themselves).
    line: Option<usize>,
}

/// The eight corners of an octagon whose edges are `r` from `c` and run at
/// multiples of 45°.
fn octagon(c: V, r: f64) -> Vec<V> {
    let big = r / (PI / 8.0).cos();
    (0..8)
        .map(|k| {
            let angle = PI / 8.0 + f64::from(k) * PI / 4.0;
            V::new(c.x + big * angle.cos(), c.y + big * angle.sin())
        })
        .collect()
}

/// The convex hull of points, counter-clockwise.
fn convex_hull(mut points: Vec<V>) -> Vec<V> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| a.dist(*b) < 1e-6);
    if points.len() < 3 {
        return points;
    }
    let cross = |o: V, a: V, b: V| a.sub(o).cross(b.sub(o));
    let mut lower: Vec<V> = Vec::new();
    for &p in &points {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<V> = Vec::new();
    for &p in points.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// The outline at distance `r` around a polyline (or a point): octagons at
/// its points joined along its segments.
fn hull(points: &[V], r: f64) -> Option<Vec<V>> {
    let pieces: Vec<Vec<Vec<V>>> = if points.len() == 1 {
        vec![vec![octagon(points[0], r)]]
    } else {
        points
            .windows(2)
            .map(|pair| {
                let mut corners = octagon(pair[0], r);
                corners.extend(octagon(pair[1], r));
                vec![convex_hull(corners)]
            })
            .collect()
    };
    poly::union(&pieces)
        .into_iter()
        .filter_map(|polygon| polygon.into_iter().next())
        .max_by(|a, b| {
            crate::geom::signed_area2(a)
                .abs()
                .total_cmp(&crate::geom::signed_area2(b).abs())
        })
}

/// `true` if `p` lies strictly inside the ring.
#[allow(clippy::many_single_char_names, reason = "geometry")]
fn inside(ring: &[V], p: V) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if p.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Where segment `a`–`b` crosses segment `c`–`d`: the fractions along both.
#[allow(clippy::many_single_char_names, reason = "geometry")]
fn crossing(a: V, b: V, c: V, d: V) -> Option<(f64, f64)> {
    let r = b.sub(a);
    let s = d.sub(c);
    let denom = r.cross(s);
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = c.sub(a).cross(s) / denom;
    let u = c.sub(a).cross(r) / denom;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

/// The smallest distance between two polylines (a single point counts).
fn distance(a: &[V], b: &[V]) -> f64 {
    let segments = |points: &[V]| -> Vec<(V, V)> {
        if points.len() == 1 {
            vec![(points[0], points[0])]
        } else {
            points.windows(2).map(|w| (w[0], w[1])).collect()
        }
    };
    let (sa, sb) = (segments(a), segments(b));
    let mut best = f64::INFINITY;
    for &(p, q) in &sa {
        for &(r, s) in &sb {
            let d = if p.dist(q) < 1e-9 {
                point_segment(p, r, s)
            } else if r.dist(s) < 1e-9 {
                point_segment(r, p, q)
            } else {
                segment_segment(p, q, r, s)
            };
            best = best.min(d);
        }
    }
    best
}

/// The turn between two directions, in radians.
fn turn(d1: V, d2: V) -> f64 {
    d1.unit().dot(d2.unit()).clamp(-1.0, 1.0).acos()
}

/// Without repeated and in-line points.
fn simplify(points: &[V]) -> Vec<V> {
    let mut out: Vec<V> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last().is_some_and(|last| last.dist(p) < 1.0) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            if b.sub(a).unit().cross(p.sub(b).unit()).abs() < 1e-9 && b.sub(a).dot(p.sub(b)) > 0.0 {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// Chamfers right-angled bends to two 45° bends, keeping the cut outside
/// `ring`. `None` if a bend is sharper or a cut would enter the ring.
#[allow(clippy::many_single_char_names, reason = "geometry")]
fn chamfer(points: &[V], ring: &[V], r: f64, corners: Corners) -> Option<Vec<V>> {
    let points = simplify(points);
    let mut out = vec![points[0]];
    for i in 1..points.len().saturating_sub(1) {
        let (a, v, b) = (points[i - 1], points[i], points[i + 1]);
        let (d1, d2) = (v.sub(a), b.sub(v));
        let angle = turn(d1, d2);
        if angle <= BEND_45 || (corners == Corners::Deg90 && angle <= BEND_90) {
            out.push(v);
            continue;
        }
        if angle > BEND_90 {
            return None;
        }
        let cut = r.min(d1.len() / 2.0).min(d2.len() / 2.0);
        let (p, q) = (v.sub(d1.unit().scale(cut)), v.add(d2.unit().scale(cut)));
        if inside(ring, p.add(q).scale(0.5)) {
            return None;
        }
        out.push(p);
        out.push(q);
    }
    out.push(points[points.len() - 1]);
    Some(simplify(&out))
}

/// The two ways of a line around a ring it crosses: forwards and
/// backwards along the ring, the shorter one first. Empty if its ends lie
/// inside the ring.
fn walk(line: &[V], ring: &[V]) -> Vec<Vec<V>> {
    let first = line[0];
    let last = line[line.len() - 1];
    if inside(ring, first) || inside(ring, last) {
        return Vec::new();
    }
    let n = ring.len();
    let mut hits: Vec<(f64, f64, V)> = Vec::new();
    for (i, pair) in line.windows(2).enumerate() {
        for j in 0..n {
            let (c, d) = (ring[j], ring[(j + 1) % n]);
            if let Some((t, u)) = crossing(pair[0], pair[1], c, d) {
                hits.push((
                    i as f64 + t,
                    j as f64 + u,
                    pair[0].add(pair[1].sub(pair[0]).scale(t)),
                ));
            }
        }
    }
    if hits.len() < 2 {
        return Vec::new();
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (s_in, u_in, p_in) = hits[0];
    let (s_out, u_out, p_out) = hits[hits.len() - 1];
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "indices of small polylines"
    )]
    let (i_in, i_out, j_in, j_out) = (
        s_in.floor() as usize,
        s_out.floor() as usize,
        u_in.floor() as usize,
        u_out.floor() as usize,
    );
    let prefix: Vec<V> = line[..=i_in].iter().copied().chain([p_in]).collect();
    let suffix: Vec<V> = [p_out]
        .into_iter()
        .chain(line[(i_out + 1).min(line.len())..].iter().copied())
        .collect();
    // Forwards: the ring corners after the entry edge up to the exit edge.
    let mut forwards = Vec::new();
    if !(j_in == j_out && u_out >= u_in) {
        let mut j = (j_in + 1) % n;
        loop {
            forwards.push(ring[j]);
            if j == j_out {
                break;
            }
            j = (j + 1) % n;
        }
    }
    // Backwards: the ring corners from the entry edge down to after the
    // exit edge.
    let mut backwards = Vec::new();
    if !(j_in == j_out && u_out <= u_in) {
        let mut j = j_in;
        loop {
            backwards.push(ring[j]);
            let next = (j + n - 1) % n;
            if j == (j_out + 1) % n {
                break;
            }
            j = next;
        }
    }
    let assemble = |path: Vec<V>| -> Vec<V> {
        prefix
            .iter()
            .copied()
            .chain(path)
            .chain(suffix.iter().copied())
            .collect()
    };
    let length = |points: &[V]| points.windows(2).map(|w| w[0].dist(w[1])).sum::<f64>();
    let mut ways = vec![assemble(forwards), assemble(backwards)];
    ways.sort_by(|a, b| length(a).total_cmp(&length(b)));
    ways
}

impl Router<'_> {
    /// The clearance between two copper items as the rules see them.
    fn gap_between(&self, a: &Pusher, b: &Line, layer: CopperLayer) -> f64 {
        let (na, nb) = (self.net(a.net), self.net(b.net));
        let pa = Props {
            kind: if a.via { "via" } else { "track" },
            net: &na.name,
            netclass: &na.class,
            layer: Some(layer),
            width: (!a.via).then_some(a.width),
            diameter: a.via.then_some(a.width),
            ..Props::default()
        };
        let pb = Props {
            kind: "track",
            net: &nb.name,
            netclass: &nb.class,
            layer: Some(layer),
            width: Some(b.width),
            ..Props::default()
        };
        self.rules.clearance(&pa, &pb, &NoZones).value
    }

    /// `true` if a copper point of `layer` is on a pad or via.
    fn joint_copper(&self, layer: CopperLayer, p: V, vias: &HashSet<Point>) -> bool {
        if vias.contains(&p.point()) {
            return true;
        }
        let Some(bit) = self.board.layer_bit(layer) else {
            return true;
        };
        self.board.copper_near(BBox::of(p).grow(1.0)).any(|index| {
            let item = &self.board.copper[index];
            matches!(item.item, ItemRef::Pad { .. })
                && item.layers & bit != 0
                && item.shape.contains(p)
        })
    }

    /// The movable lines of a layer near `window`.
    fn lines(&self, layer: CopperLayer, window: BBox) -> Vec<Line> {
        let pcb = self.board.pcb;
        let movable =
            |t: &Track| t.layer == layer && t.mid.is_none() && !t.locked && t.net.is_some();
        let mut ends: HashMap<Point, Vec<Id>> = HashMap::new();
        for track in pcb.tracks.iter().filter(|t| t.layer == layer) {
            ends.entry(track.start).or_default().push(track.id);
            ends.entry(track.end).or_default().push(track.id);
        }
        let vias: HashSet<Point> = pcb.vias.iter().map(|v| v.at).collect();
        let mut seen: HashSet<Id> = HashSet::new();
        let mut out = Vec::new();
        // The next track of a chain beyond `p`, coming from `from`.
        let next = |p: Point, from: Id, net: Option<Id>, seen: &HashSet<Id>| -> Option<&Track> {
            let at = ends.get(&p)?;
            if at.len() != 2 || self.joint_copper(layer, V::of(p), &vias) {
                return None;
            }
            let other = *at.iter().find(|id| **id != from)?;
            let track = pcb.tracks.get(other)?;
            (movable(track) && track.net == net && !seen.contains(&other)).then_some(track)
        };
        for track in pcb.tracks.iter().filter(|t| movable(t)) {
            let bbox = BBox::of(V::of(track.start)).with(V::of(track.end));
            if seen.contains(&track.id) || !window.intersects(bbox) {
                continue;
            }
            seen.insert(track.id);
            // Backwards from the start, then forwards from the end.
            let mut ids = vec![track.id];
            let mut points = vec![V::of(track.start), V::of(track.end)];
            let (mut p, mut from) = (track.start, track.id);
            while let Some(t) = next(p, from, track.net, &seen) {
                seen.insert(t.id);
                p = if t.start == p { t.end } else { t.start };
                from = t.id;
                ids.insert(0, t.id);
                points.insert(0, V::of(p));
            }
            let (mut p, mut from) = (track.end, track.id);
            while let Some(t) = next(p, from, track.net, &seen) {
                seen.insert(t.id);
                p = if t.start == p { t.end } else { t.start };
                from = t.id;
                ids.push(t.id);
                points.push(V::of(p));
            }
            out.push(Line {
                ids,
                net: track.net,
                width: track.width as f64,
                points: simplify(&points),
                changed: false,
            });
        }
        out
    }

    /// `true` if a moved line keeps clear of everything that does not move:
    /// pads, vias, other tracks than the lines, keep-outs, the board edge,
    /// and the heads of the route.
    fn line_fits(
        &self,
        layer: CopperLayer,
        line: &Line,
        points: &[V],
        moving: &HashSet<Id>,
        heads: &[Pusher],
    ) -> bool {
        let request = Request {
            layer,
            net: line.net,
            partner: None,
            width: line.width,
            start: points[0],
            target: points[points.len() - 1],
            diagonal_first: false,
            corners: Corners::Deg45,
            previous: None,
        };
        let skip = |item: ItemRef| matches!(item, ItemRef::Track { id } if moving.contains(&id));
        // Only the new parts: what the line had before stays as it was
        // (also where it broke a rule already).
        let old: HashSet<(Point, Point)> = line
            .points
            .windows(2)
            .map(|w| (w[0].point(), w[1].point()))
            .collect();
        if !points
            .windows(2)
            .filter(|w| !old.contains(&(w[0].point(), w[1].point())))
            .all(|w| self.clear_skipping(&request, w[0], w[1], &skip))
        {
            return false;
        }
        heads.iter().all(|head| {
            head.net == line.net && head.net.is_some()
                || distance(&head.points, points)
                    >= self.gap_between(head, line, layer) + f64::midpoint(head.width, line.width)
        })
    }

    /// Pushes the lines of one layer out of the way of the heads on it.
    fn shove_layer(
        &self,
        layer: CopperLayer,
        heads: &[Pusher],
        corners: Corners,
    ) -> Option<Vec<Line>> {
        let window = heads
            .iter()
            .flat_map(|h| h.points.iter())
            .fold(BBox::EMPTY, |b, &p| b.union(BBox::of(p)));
        let mut lines = self.lines(layer, window.grow(REACH));
        let moving: HashSet<Id> = lines.iter().flat_map(|l| l.ids.iter().copied()).collect();
        let mut queue: VecDeque<Pusher> = heads.iter().cloned().collect();
        let mut pushes = 0;
        while let Some(pusher) = queue.pop_front() {
            #[allow(
                clippy::needless_range_loop,
                reason = "a pushed line is changed in place"
            )]
            for index in 0..lines.len() {
                let line = &lines[index];
                if pusher.line == Some(index) || (pusher.net.is_some() && pusher.net == line.net) {
                    continue;
                }
                let r = self.gap_between(&pusher, line, layer)
                    + f64::midpoint(pusher.width, line.width)
                    + MARGIN;
                if distance(&pusher.points, &line.points) >= r - MARGIN {
                    continue;
                }
                pushes += 1;
                if pushes > MAX_PUSHES {
                    return None;
                }
                let ring = hull(&pusher.points, r)?;
                let moved = walk(&line.points, &ring)
                    .into_iter()
                    .filter_map(|way| chamfer(&way, &ring, r, corners))
                    .find(|way| self.line_fits(layer, line, way, &moving, heads))?;
                let line = &mut lines[index];
                line.points = moved;
                line.changed = true;
                queue.push_back(Pusher {
                    net: line.net,
                    width: line.width,
                    points: line.points.clone(),
                    via: false,
                    line: Some(index),
                });
            }
        }
        // Every line clear of every other one.
        for (i, a) in lines.iter().enumerate() {
            for b in lines.iter().skip(i + 1) {
                if (a.changed || b.changed) && a.net != b.net {
                    let first = Pusher {
                        net: a.net,
                        width: a.width,
                        points: a.points.clone(),
                        via: false,
                        line: None,
                    };
                    let gap = self.gap_between(&first, b, layer) + f64::midpoint(a.width, b.width);
                    if distance(&a.points, &b.points) < gap {
                        return None;
                    }
                }
            }
        }
        Some(lines.into_iter().filter(|l| l.changed).collect())
    }

    /// `true` if a path keeps clear of everything that cannot be pushed:
    /// pads, vias, locked and curved tracks, keep-outs and the board edge.
    fn hard_clear(&self, request: &Request, points: &[V]) -> bool {
        let pcb = self.board.pcb;
        let skip = |item: ItemRef| match item {
            ItemRef::Track { id } => pcb
                .tracks
                .get(id)
                .is_some_and(|t| t.mid.is_none() && !t.locked && t.net.is_some()),
            _ => false,
        };
        points
            .windows(2)
            .all(|w| self.clear_skipping(request, w[0], w[1], &skip))
    }

    /// Routes from the start towards the target, pushing the tracks of
    /// other nets aside: the direct path when they can make room for it
    /// (with the copper `placed` before on this route), otherwise the path
    /// around them.
    #[must_use]
    pub fn route_shoving(&self, request: &Request, placed: &[Head]) -> ShoveRoute {
        let head = |points: &[V]| Head {
            layers: vec![request.layer],
            net: request.net,
            width: request.width,
            points: points.to_vec(),
        };
        let with = |points: &[V]| -> Vec<Head> {
            let mut heads = placed.to_vec();
            if points.len() >= 2 {
                heads.push(head(points));
            }
            heads
        };
        for path in Self::direct_paths(request) {
            if path.len() < 2 || !self.hard_clear(request, &path) {
                continue;
            }
            if let Some(shoved) = self.shove(&with(&path), request.corners) {
                return ShoveRoute {
                    points: path.into_iter().map(V::pair).collect(),
                    reached: true,
                    shoved: Some(shoved),
                };
            }
        }
        let around = self.route(request);
        let points: Vec<V> = around.points.iter().map(|&[x, y]| V::new(x, y)).collect();
        ShoveRoute {
            shoved: self.shove(&with(&points), request.corners),
            points: around.points,
            reached: around.reached,
        }
    }

    /// Pushes the tracks of other nets out of the way of a route's copper
    /// (its tracks and vias). `None` if they cannot make room.
    #[must_use]
    pub fn shove(&self, heads: &[Head], corners: Corners) -> Option<Shoved> {
        let mut layers: Vec<CopperLayer> = heads.iter().flat_map(|h| h.layers.clone()).collect();
        layers.sort_by_key(|l| self.board.layer_bit(*l));
        layers.dedup();
        let mut shoved = Shoved::default();
        for layer in layers {
            let pushers: Vec<Pusher> = heads
                .iter()
                .filter(|h| h.layers.contains(&layer))
                .map(|h| Pusher {
                    net: h.net,
                    width: h.width,
                    points: h.points.clone(),
                    via: h.is_via(),
                    line: None,
                })
                .collect();
            for line in self.shove_layer(layer, &pushers, corners)? {
                let template = self.board.pcb.tracks.get(line.ids[0])?;
                shoved.removed.extend(line.ids.iter().copied());
                for pair in line.points.windows(2) {
                    shoved.added.push(Track {
                        id: Id::random(),
                        layer,
                        net: line.net,
                        width: template.width,
                        start: pair[0].point(),
                        end: pair[1].point(),
                        mid: None,
                        locked: false,
                    });
                }
            }
        }
        Some(shoved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectivity::tests::{board_with, track};
    use crate::drc::{Options, check};
    use ew_model::layer::Layer;
    use ew_model::pcb::{BoardShape, Net, Pcb};
    use ew_model::shape::Geometry;
    use ew_model::units::um;

    /// Net A pads at (0, 0) and (0, 20) mm, net B pads at (-10, 10) and
    /// (10, 10) joined by a straight track.
    fn board() -> (Pcb, Id, Id) {
        let (a, b) = (Id::random(), Id::random());
        let mut pcb = board_with(vec![
            (Point::mm(0, 0), Some(a)),
            (Point::mm(0, 20), Some(a)),
            (Point::mm(-10, 10), Some(b)),
            (Point::mm(10, 10), Some(b)),
        ]);
        for (id, name) in [(a, "A"), (b, "B")] {
            pcb.nets.insert(Net {
                id,
                name: name.into(),
                class: String::new(),
            });
        }
        pcb.tracks.insert(track(
            b,
            Point::mm(-10, 10),
            Point::mm(10, 10),
            CopperLayer::Top,
        ));
        pcb.shapes.insert(BoardShape {
            id: Id::random(),
            layer: Layer::Outline,
            geometry: Geometry::Rect {
                start: Point::mm(-20, -10),
                end: Point::mm(20, 30),
            },
            width: um(100),
            filled: false,
        });
        (pcb, a, b)
    }

    fn request(net: Id, target: V) -> Request {
        Request {
            layer: CopperLayer::Top,
            net: Some(net),
            partner: None,
            width: 250_000.0,
            start: V::new(0.0, 0.0),
            target,
            diagonal_first: false,
            corners: Corners::Deg45,
            previous: None,
        }
    }

    #[test]
    fn a_route_pushes_a_track_of_another_net() {
        let (mut pcb, a, b) = board();
        let router = Router::new(&pcb, None);
        // Straight up to just below the B track.
        let route = router.route_shoving(&request(a, V::new(0.0, 9.8e6)), &[]);
        assert!(route.reached);
        assert_eq!(route.points, vec![[0.0, 0.0], [0.0, 9.8e6]]);
        let shoved = route.shoved.unwrap();
        assert_eq!(shoved.removed.len(), 1);
        assert!(shoved.added.len() >= 3, "{shoved:?}");
        // The pushed line keeps its ends and bends at 45° only.
        let mut points: Vec<V> = vec![V::of(shoved.added[0].start)];
        points.extend(shoved.added.iter().map(|t| V::of(t.end)));
        assert_eq!(points[0], V::new(-10.0e6, 10.0e6));
        assert_eq!(points[points.len() - 1], V::new(10.0e6, 10.0e6));
        for w in points.windows(3) {
            assert!(turn(w[1].sub(w[0]), w[2].sub(w[1])) <= BEND_45);
        }
        // On the board, nothing is too close.
        drop(router);
        for id in &shoved.removed {
            pcb.tracks.remove(*id);
        }
        for t in shoved.added {
            pcb.tracks.insert(t);
        }
        pcb.tracks.insert(track(
            a,
            Point::mm(0, 0),
            Point::new(0, 9_800_000),
            CopperLayer::Top,
        ));
        let report = check(&pcb, None, &Options::default());
        let bad: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.code.as_str())
            .filter(|code| matches!(*code, "clearance" | "short"))
            .collect();
        assert!(bad.is_empty(), "{bad:?}");
        let _ = b;
    }

    #[test]
    fn a_pushed_track_pushes_the_next_one() {
        let (mut pcb, a, _) = board();
        let c = Id::random();
        pcb.nets.insert(Net {
            id: c,
            name: "C".into(),
            class: String::new(),
        });
        // 0.5 mm above B: clear now, too close once B bends up.
        pcb.tracks.insert(track(
            c,
            Point::new(-8_000_000, 10_500_000),
            Point::new(8_000_000, 10_500_000),
            CopperLayer::Top,
        ));
        let router = Router::new(&pcb, None);
        let route = router.route_shoving(&request(a, V::new(0.0, 9.8e6)), &[]);
        assert_eq!(route.points, vec![[0.0, 0.0], [0.0, 9.8e6]]);
        let shoved = route.shoved.unwrap();
        // Both lines moved.
        assert_eq!(shoved.removed.len(), 2, "{shoved:?}");
        drop(router);
        for id in &shoved.removed {
            pcb.tracks.remove(*id);
        }
        for t in shoved.added {
            pcb.tracks.insert(t);
        }
        pcb.tracks.insert(track(
            a,
            Point::mm(0, 0),
            Point::new(0, 9_800_000),
            CopperLayer::Top,
        ));
        let report = check(&pcb, None, &Options::default());
        let bad: Vec<String> = report
            .findings
            .iter()
            .filter(|f| matches!(f.code.as_str(), "clearance" | "short"))
            .map(|f| f.message.clone())
            .collect();
        assert!(bad.is_empty(), "{bad:?}");
    }

    #[test]
    fn a_track_held_by_pads_on_both_sides_is_walked_around() {
        let (pcb, a, _) = board();
        let router = Router::new(&pcb, None);
        // Right across the B track to the other A pad: B cannot go round
        // the A pads, so the route walks around (and pushes nothing).
        let route = router.route_shoving(&request(a, V::new(0.0, 20.0e6)), &[]);
        assert_ne!(route.points, vec![[0.0, 0.0], [0.0, 20.0e6]]);
    }

    #[test]
    fn octagon_hulls_have_45_degree_edges() {
        let ring = hull(&[V::new(0.0, 0.0), V::new(10.0e6, 0.0)], 1.0e6).unwrap();
        for i in 0..ring.len() {
            let d = ring[(i + 1) % ring.len()].sub(ring[i]);
            let angle = d.y.atan2(d.x).to_degrees().rem_euclid(45.0);
            assert!(angle < 0.01 || angle > 44.99, "edge at {angle}°");
        }
        assert!(inside(&ring, V::new(5.0e6, 0.5e6)));
        assert!(!inside(&ring, V::new(5.0e6, 1.5e6)));
    }

    #[test]
    fn a_crossing_line_walks_around_and_gets_chamfered() {
        // A head going up through a horizontal line.
        let head = [V::new(0.0, -5.0e6), V::new(0.0, 0.0)];
        let ring = hull(&head, 1.0e6).unwrap();
        let line = [V::new(-10.0e6, -2.0e6), V::new(10.0e6, -2.0e6)];
        let ways = walk(&line, &ring);
        assert_eq!(ways.len(), 2);
        let way = chamfer(&ways[0], &ring, 1.0e6, Corners::Deg45).unwrap();
        // Around the top of the head: every bend 45°, ends unchanged.
        assert_eq!(way[0], line[0]);
        assert_eq!(way[way.len() - 1], line[1]);
        for w in way.windows(3) {
            assert!(turn(w[1].sub(w[0]), w[2].sub(w[1])) <= BEND_45);
        }
        assert!(distance(&head, &way) >= 1.0e6 - 1.0);
    }
}
