//! Interactive routing in walkaround mode.
//!
//! A route runs from a start point (a pad centre, a track end or a via) to
//! the cursor with segments at 0°, 45° and 90°. If the direct two-segment
//! path is free, it is used; otherwise an A* search on a grid around the
//! obstacles finds a way, which is then straightened. Obstacles are the
//! copper of other nets (with the clearance from the rules), non-plated
//! holes, keep-out areas that forbid tracks and the board edge. When the
//! cursor cannot be reached, the route ends at the nearest reachable
//! point.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use ew_model::id::Id;
use ew_model::layer::CopperLayer;
use ew_model::pcb::Pcb;
use ew_model::rules::Rules;
use serde::{Deserialize, Serialize};

use crate::drc::{EPSILON, NetInfo, item_props, net_table};
use crate::geom::{BBox, Shape, V};
use crate::items::{Board, ItemRef};
use crate::rules::{NoZones, Props, RuleSet};

/// Which corners a route may have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum Corners {
    /// Only 45° bends (the default: no right angles, for signal integrity
    /// and EMC).
    #[default]
    #[serde(rename = "45")]
    Deg45,
    /// Right-angled bends allowed too; the direct path is an L.
    #[serde(rename = "90")]
    Deg90,
}

impl Corners {
    /// The largest bend between two segments, in 45° steps.
    const fn max_turn(self) -> u8 {
        match self {
            Self::Deg45 => 1,
            Self::Deg90 => 2,
        }
    }
}

/// What to route.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Copper layer.
    pub layer: CopperLayer,
    /// Net of the new track.
    pub net: Option<Id>,
    /// Track width.
    pub width: f64,
    /// Start point.
    pub start: V,
    /// Where the cursor is.
    pub target: V,
    /// Start with the diagonal (otherwise with the straight segment).
    pub diagonal_first: bool,
    /// Which bends are allowed.
    pub corners: Corners,
    /// Direction of the segment that ends at the start (a route that
    /// continues): the first bend counts too.
    pub previous: Option<V>,
}

/// A routed path.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    /// Points from the start to the end.
    pub points: Vec<[f64; 2]>,
    /// `true` if the route ends at the target.
    pub reached: bool,
}

/// Where a route starts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    /// The snapped start point.
    pub at: [f64; 2],
    /// Its net.
    pub net: Option<Id>,
    /// Name of the net.
    pub net_name: String,
    /// Track width from the rules.
    pub width: f64,
    /// Via diameter from the rules.
    pub via_diameter: f64,
    /// Via drill from the rules.
    pub via_drill: f64,
    /// Clearance of the net class.
    pub clearance: f64,
    /// Direction of the track whose end the route starts at (towards that
    /// end), so the first bend can be limited too.
    pub previous: Option<[f64; 2]>,
}

/// A router for one board.
pub struct Router<'a> {
    board: Board<'a>,
    rules: RuleSet,
    nets: HashMap<Id, NetInfo>,
    no_net: NetInfo,
}

const MAX_EXPANSIONS: usize = 200_000;

/// The eight grid directions, counter-clockwise from +x: neighbours in
/// this list are 45° apart.
const DIRS: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

/// No direction yet (the start of a new route).
const NO_DIR: u8 = 8;

/// A search state: a grid cell and the direction it was entered from.
type State = ((i32, i32), u8);

/// The index in [`DIRS`] of an octilinear vector, if it is one.
#[allow(clippy::cast_possible_truncation, reason = "index below 8")]
fn dir_index(d: V) -> Option<u8> {
    let tolerance = 1.0_f64.max(1e-6 * d.len());
    if d.len() < 1.0 {
        return None;
    }
    let sx = if d.x.abs() < tolerance {
        0
    } else {
        d.x.signum() as i32
    };
    let sy = if d.y.abs() < tolerance {
        0
    } else {
        d.y.signum() as i32
    };
    if sx != 0 && sy != 0 && (d.x.abs() - d.y.abs()).abs() > tolerance {
        return None;
    }
    DIRS.iter()
        .position(|&(x, y)| x == sx && y == sy)
        .map(|i| i as u8)
}

/// The bend between two directions in 45° steps (0 to 4).
const fn turn(a: u8, b: u8) -> u8 {
    let d = (a + 8 - b) % 8;
    if d > 4 { 8 - d } else { d }
}

/// `true` if every segment of the path is octilinear and no bend (also the
/// one from `previous`) is sharper than the corners allow.
fn bends_ok(points: &[V], previous: Option<u8>, corners: Corners) -> bool {
    let mut last = previous;
    for pair in points.windows(2) {
        let d = pair[1].sub(pair[0]);
        if d.len() < 1.0 {
            continue;
        }
        let Some(dir) = dir_index(d) else {
            return false;
        };
        if let Some(l) = last
            && turn(l, dir) > corners.max_turn()
        {
            return false;
        }
        last = Some(dir);
    }
    true
}

/// The corner of a two-segment path from `a` to `b`: which segment comes
/// first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape2 {
    /// Diagonal, then straight.
    Diagonal,
    /// Straight, then diagonal.
    Straight,
    /// Horizontal, then vertical (90° mode).
    Horizontal,
    /// Vertical, then horizontal (90° mode).
    Vertical,
}

fn corner(a: V, b: V, shape: Shape2) -> V {
    let d = b.sub(a);
    let m = d.x.abs().min(d.y.abs());
    let diag = V::new(m.copysign(d.x), m.copysign(d.y));
    match shape {
        Shape2::Diagonal => a.add(diag),
        Shape2::Straight => b.sub(diag),
        Shape2::Horizontal => V::new(b.x, a.y),
        Shape2::Vertical => V::new(a.x, b.y),
    }
}

/// The two-segment shapes to try, the preferred one first.
fn shapes(request: &Request) -> Vec<Shape2> {
    let diagonal = if request.diagonal_first {
        [Shape2::Diagonal, Shape2::Straight]
    } else {
        [Shape2::Straight, Shape2::Diagonal]
    };
    match request.corners {
        Corners::Deg45 => diagonal.to_vec(),
        Corners::Deg90 => {
            let square = if request.diagonal_first {
                [Shape2::Vertical, Shape2::Horizontal]
            } else {
                [Shape2::Horizontal, Shape2::Vertical]
            };
            square.into_iter().chain(diagonal).collect()
        }
    }
}

impl<'a> Router<'a> {
    /// A router for a board.
    #[must_use]
    pub fn new(pcb: &'a Pcb, rules: Option<&Rules>) -> Self {
        let mut set = RuleSet::new(rules);
        set.cache_classes(pcb.nets.iter().map(|n| (n.name.clone(), n.class.clone())));
        let nets = net_table(pcb, &set);
        Self {
            board: Board::new(pcb),
            rules: set,
            nets,
            no_net: NetInfo {
                name: String::new(),
                class: ew_model::rules::DEFAULT_NET_CLASS.to_owned(),
            },
        }
    }

    fn net(&self, net: Option<Id>) -> &NetInfo {
        net.and_then(|id| self.nets.get(&id))
            .unwrap_or(&self.no_net)
    }

    /// Where a route that begins at `at` on `layer` starts: the centre of a
    /// pad or via there, the nearer end of a track, or the point itself.
    #[must_use]
    pub fn start(&self, at: V, layer: CopperLayer) -> Start {
        let bit = self.board.layer_bit(layer).unwrap_or(0);
        let probe = Shape::disc(at, 0.0);
        let mut found: Option<(V, Option<Id>, u8, Option<V>)> = None;
        for index in self.board.copper_near(BBox::of(at).grow(1.0)) {
            let item = &self.board.copper[index];
            if item.layers & bit == 0 || item.shape.gap(&probe, 1.0).is_none() {
                continue;
            }
            let (point, rank, direction) = match item.item {
                ItemRef::Pad { .. } | ItemRef::Via { .. } => (item.anchors[0], 0, None),
                ItemRef::Track { id } => {
                    let Some(track) = self.board.pcb.tracks.get(id) else {
                        continue;
                    };
                    let (s, e) = (V::of(track.start), V::of(track.end));
                    let straight = track.mid.is_none();
                    if s.dist(at) <= e.dist(at) {
                        (s, 1, straight.then(|| s.sub(e)))
                    } else {
                        (e, 1, straight.then(|| e.sub(s)))
                    }
                }
                ItemRef::Zone { .. } => (at, 2, None),
            };
            if found.as_ref().is_none_or(|(_, _, best, _)| rank < *best) {
                found = Some((point, item.net, rank, direction));
            }
        }
        let (point, net, direction) = found.map_or((at, None, None), |(p, n, _, d)| (p, n, d));
        let info = self.net(net);
        let props = Props {
            kind: "track",
            net: &info.name,
            netclass: &info.class,
            layer: Some(layer),
            at: point,
            ..Props::default()
        };
        let width = self.rules.track_width(&props, &NoZones);
        let via = self.rules.via(&props, &NoZones);
        let class = self.rules.class(&info.class);
        Start {
            at: point.pair(),
            net,
            net_name: info.name.clone(),
            width: width.preferred,
            via_diameter: via.preferred_diameter,
            via_drill: via.preferred_drill,
            clearance: class.clearance as f64,
            previous: direction.map(V::pair),
        }
    }

    /// `true` if a track from `a` to `b` keeps every rule.
    #[must_use]
    pub fn clear(&self, request: &Request, a: V, b: V) -> bool {
        let Some(bit) = self.board.layer_bit(request.layer) else {
            return false;
        };
        let shape = Shape::stadium(a, b, request.width / 2.0);
        let mine = self.net(request.net);
        let me = Props {
            kind: "track",
            net: &mine.name,
            netclass: &mine.class,
            layer: Some(request.layer),
            width: Some(request.width),
            at: a,
            ..Props::default()
        };
        let reach = self.rules.max_clearance();
        for index in self.board.copper_near(shape.bbox().grow(reach)) {
            let item = &self.board.copper[index];
            if item.layers & bit == 0 {
                continue;
            }
            if request.net.is_some() && item.net == request.net {
                continue;
            }
            let other = item_props(&self.board, self.net(item.net), item, Some(request.layer));
            let limit = self.rules.clearance(&me, &other, &NoZones).value;
            if shape.gap(&item.shape, limit - EPSILON).is_some() {
                return false;
            }
        }
        for index in self.board.holes_near(shape.bbox().grow(reach)) {
            let hole = &self.board.holes[index];
            if !hole.plated && shape.gap(&hole.shape, reach).is_some() {
                return false;
            }
        }
        for keepout in &self.board.keepouts {
            if keepout.rules.tracks
                && keepout.layers & bit != 0
                && shape.gap(&keepout.shape, 0.0).is_some()
            {
                return false;
            }
        }
        if self.board.outline.area.is_some() {
            let edge = self.rules.edge_clearance(&me, &NoZones).value;
            if self
                .board
                .outline
                .edge_gap(&shape, edge - EPSILON)
                .is_some()
                || !self.board.outline.contains(a)
            {
                return false;
            }
        }
        true
    }

    /// A clear two-segment path from `a` to `b` whose bends (also from
    /// `previous`) are allowed.
    fn two_segments(
        &self,
        request: &Request,
        a: V,
        b: V,
        shape: Shape2,
        previous: Option<u8>,
    ) -> Option<Vec<V>> {
        let c = corner(a, b, shape);
        let mut points = vec![a];
        if c.dist(a) >= 1.0 && c.dist(b) >= 1.0 {
            points.push(c);
        }
        if b.dist(a) >= 1.0 {
            points.push(b);
        }
        if !bends_ok(&points, previous, request.corners) {
            return None;
        }
        let clear = points
            .windows(2)
            .all(|pair| self.clear(request, pair[0], pair[1]));
        clear.then_some(points)
    }

    /// Routes from the start towards the target.
    #[must_use]
    pub fn route(&self, request: &Request) -> Route {
        let (start, target) = (request.start, request.target);
        let previous = request.previous.and_then(dir_index);
        for shape in shapes(request) {
            if let Some(points) = self.two_segments(request, start, target, shape, previous) {
                return Route {
                    points: points.into_iter().map(V::pair).collect(),
                    reached: true,
                };
            }
        }
        let (points, reached) = self.search(request, previous);
        Route {
            points: points.into_iter().map(V::pair).collect(),
            reached,
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::too_many_lines,
        reason = "grid indices are small; the search reads best in one piece"
    )]
    fn search(&self, request: &Request, previous: Option<u8>) -> (Vec<V>, bool) {
        let (start, target) = (request.start, request.target);
        let step = (request.width / 2.0).clamp(25_000.0, 200_000.0);
        let cell = |p: V| -> (i32, i32) {
            (
                ((p.x - start.x) / step).round() as i32,
                ((p.y - start.y) / step).round() as i32,
            )
        };
        let at = |c: (i32, i32)| {
            V::new(
                start.x + f64::from(c.0) * step,
                start.y + f64::from(c.1) * step,
            )
        };
        let goal = cell(target);
        let margin = (start.dist(target) / 2.0).max(5e6);
        let window = BBox::of(start).with(target).grow(margin);
        let mut free: HashMap<(i32, i32), bool> = HashMap::new();
        let mut is_free = |c: (i32, i32)| -> bool {
            *free.entry(c).or_insert_with(|| {
                let p = at(c);
                window.intersects(BBox::of(p)) && self.clear(request, p, p)
            })
        };
        let octile = |c: (i32, i32)| -> u64 {
            let (dx, dy) = ((c.0 - goal.0).unsigned_abs(), (c.1 - goal.1).unsigned_abs());
            let (lo, hi) = (dx.min(dy), dx.max(dy));
            u64::from(lo) * 1414 + u64::from(hi - lo) * 1000
        };
        let max_turn = request.corners.max_turn();
        let origin = (0, 0);
        let first = previous.unwrap_or(NO_DIR);
        let mut best_cost: HashMap<State, u64> = HashMap::new();
        let mut parent: HashMap<State, State> = HashMap::new();
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((octile(origin), 0_u64, origin, first)));
        best_cost.insert((origin, first), 0);
        let mut closest = ((origin, first), octile(origin));
        let mut found = None;
        let mut expansions = 0;
        while let Some(Reverse((_, cost, c, dir))) = heap.pop() {
            if best_cost.get(&(c, dir)).is_some_and(|b| *b < cost) {
                continue;
            }
            let h = octile(c);
            if h < closest.1 {
                closest = ((c, dir), h);
            }
            if c == goal {
                found = Some((c, dir));
                break;
            }
            expansions += 1;
            if expansions > MAX_EXPANSIONS {
                break;
            }
            for (index, (dx, dy)) in DIRS.iter().enumerate() {
                let index = index as u8;
                // Only bends the corner mode allows; 45° bends cost a
                // little, so straight runs win.
                let bend = if dir == NO_DIR { 0 } else { turn(dir, index) };
                if bend > max_turn {
                    continue;
                }
                let next = (c.0 + dx, c.1 + dy);
                if !is_free(next) {
                    continue;
                }
                let step_cost =
                    if *dx != 0 && *dy != 0 { 1414 } else { 1000 } + 400 * u64::from(bend);
                let total = cost + step_cost;
                let key = (next, index);
                if best_cost.get(&key).is_none_or(|b| total < *b) {
                    best_cost.insert(key, total);
                    parent.insert(key, (c, dir));
                    heap.push(Reverse((total + octile(next), total, next, index)));
                }
            }
        }
        let reached = found.is_some();
        let mut state = found.unwrap_or(closest.0);
        let mut cells = vec![state.0];
        while let Some(prev) = parent.get(&state) {
            cells.push(prev.0);
            state = *prev;
        }
        cells.reverse();
        let mut path: Vec<V> = cells.into_iter().map(at).collect();
        path[0] = start;
        let mut points = self.straighten(request, &path, previous);
        if reached {
            // End exactly at the target: replace the tail with a clear
            // two-segment path whose bends are allowed.
            let n = points.len();
            let mut joined = false;
            'tail: for k in (n.saturating_sub(4)..n).rev() {
                let before = if k == 0 {
                    previous
                } else {
                    dir_index(points[k].sub(points[k - 1]))
                };
                for shape in shapes(request) {
                    if let Some(tail) = self.two_segments(request, points[k], target, shape, before)
                    {
                        points.truncate(k);
                        points.extend(tail);
                        joined = true;
                        break 'tail;
                    }
                }
            }
            if !joined && points.last().is_some_and(|p| p.dist(target) > 1.0) {
                return (points, false);
            }
        }
        (points, reached)
    }

    /// Joins grid steps into long octilinear segments. Every bend stays
    /// within the corner mode: towards the segment before, and towards the
    /// grid step after the new segment's end (so the rest still fits).
    fn straighten(&self, request: &Request, path: &[V], previous: Option<u8>) -> Vec<V> {
        let max_turn = request.corners.max_turn();
        let mut out = vec![path[0]];
        let mut last = previous;
        let mut i = 0;
        while i + 1 < path.len() {
            let far = (i + 400).min(path.len() - 1);
            let mut chosen = None;
            for k in (i + 1..=far).rev() {
                let Some(dir) = dir_index(path[k].sub(path[i])) else {
                    continue;
                };
                if last.is_some_and(|l| turn(l, dir) > max_turn) {
                    continue;
                }
                if k + 1 < path.len()
                    && let Some(next) = dir_index(path[k + 1].sub(path[k]))
                    && turn(dir, next) > max_turn
                {
                    continue;
                }
                if !self.clear(request, path[i], path[k]) {
                    continue;
                }
                chosen = Some((k, dir));
                break;
            }
            let Some((k, dir)) = chosen else {
                break;
            };
            out.push(path[k]);
            last = Some(dir);
            i = k;
        }
        out
    }

    /// `true` if a via fits at `at`.
    #[must_use]
    pub fn via_fits(&self, net: Option<Id>, at: V, diameter: f64) -> bool {
        let shape = Shape::disc(at, diameter / 2.0);
        let mine = self.net(net);
        let me = Props {
            kind: "via",
            net: &mine.name,
            netclass: &mine.class,
            diameter: Some(diameter),
            at,
            ..Props::default()
        };
        for index in self
            .board
            .copper_near(shape.bbox().grow(self.rules.max_clearance()))
        {
            let item = &self.board.copper[index];
            if net.is_some() && item.net == net {
                continue;
            }
            let layer = self.board.layers_of(item.layers).next();
            let other = item_props(&self.board, self.net(item.net), item, layer);
            let limit = self.rules.clearance(&me, &other, &NoZones).value;
            if shape.gap(&item.shape, limit - EPSILON).is_some() {
                return false;
            }
        }
        self.board.outline.contains(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectivity::tests::{board_with, pad};
    use ew_model::layer::Layer;
    use ew_model::pcb::{BoardShape, Net};
    use ew_model::shape::Geometry;
    use ew_model::units::{Point, um};

    fn setup() -> (Pcb, Id) {
        let (gnd, vcc) = (Id::random(), Id::random());
        let mut pcb = board_with(Vec::new());
        for (id, name) in [(gnd, "GND"), (vcc, "VCC")] {
            pcb.nets.insert(Net {
                id,
                name: name.into(),
                class: String::new(),
            });
        }
        let fp = pcb.footprints.iter().next().unwrap().id;
        let mut fp_obj = pcb.footprints.get(fp).unwrap().clone();
        fp_obj.pads = vec![
            pad("1", Point::mm(0, 0), Some(gnd)),
            pad("2", Point::mm(20, 0), Some(gnd)),
        ];
        // A wall of VCC pads between them, from y = -5 mm to 5 mm.
        for y in -5..=5 {
            let mut wall = pad("W", Point::mm(10, y), Some(vcc));
            wall.size = ew_model::units::Size::new(um(1_000), um(1_000));
            fp_obj.pads.push(wall);
        }
        pcb.footprints.insert(fp_obj);
        pcb.shapes.insert(BoardShape {
            id: Id::random(),
            layer: Layer::Outline,
            geometry: Geometry::Rect {
                start: Point::mm(-10, -20),
                end: Point::mm(30, 20),
            },
            width: um(100),
            filled: false,
        });
        (pcb, gnd)
    }

    #[test]
    fn routes_go_around_obstacles_and_keep_the_rules() {
        let (mut pcb, gnd) = setup();
        let router = Router::new(&pcb, None);
        let start = router.start(V::new(100_000.0, 0.0), CopperLayer::Top);
        assert_eq!(start.net, Some(gnd));
        assert_eq!(start.at, [0.0, 0.0], "snapped to the pad centre");
        assert_eq!(start.width, 250_000.0);
        let request = Request {
            layer: CopperLayer::Top,
            net: start.net,
            width: start.width,
            start: V::new(0.0, 0.0),
            target: V::new(20e6, 0.0),
            diagonal_first: false,
            corners: Corners::Deg45,
            previous: None,
        };
        let route = router.route(&request);
        assert!(route.reached, "{route:?}");
        let points: Vec<V> = route.points.iter().map(|p| V::new(p[0], p[1])).collect();
        assert_eq!(points[0], request.start);
        assert_eq!(*points.last().unwrap(), request.target);
        for pair in points.windows(2) {
            let d = pair[1].sub(pair[0]);
            let octilinear =
                d.x.abs() < 1.0 || d.y.abs() < 1.0 || (d.x.abs() - d.y.abs()).abs() < 2.0;
            assert!(octilinear, "{d:?}");
        }
        assert!(points.len() <= 8, "straightened: {}", points.len());
        assert!(
            bends_ok(&points, None, Corners::Deg45),
            "only 45° bends: {points:?}"
        );
        // The tracks pass the DRC without clearance problems.
        for pair in points.windows(2) {
            let mut t = crate::connectivity::tests::track(
                gnd,
                pair[0].point(),
                pair[1].point(),
                CopperLayer::Top,
            );
            t.width = 250_000;
            pcb.tracks.insert(t);
        }
        let report = crate::drc::check(&pcb, None, &crate::drc::Options::default());
        let bad: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.code.as_str())
            .filter(|c| ["clearance", "short", "unrouted"].contains(c))
            .collect();
        assert_eq!(bad, Vec::<&str>::new(), "{:#?}", report.findings);
    }

    #[test]
    fn blocked_targets_end_nearby() {
        let (pcb, gnd) = setup();
        let router = Router::new(&pcb, None);
        // The target lies inside a VCC pad.
        let request = Request {
            layer: CopperLayer::Top,
            net: Some(gnd),
            width: 250_000.0,
            start: V::new(0.0, 0.0),
            target: V::new(10e6, 0.0),
            diagonal_first: true,
            corners: Corners::Deg45,
            previous: None,
        };
        let route = router.route(&request);
        assert!(!route.reached);
        let end = route.points.last().unwrap();
        assert!(
            V::new(end[0], end[1]).dist(request.target) < 1.5e6,
            "{end:?}"
        );
        assert!(router.via_fits(Some(gnd), V::new(5e6, 0.0), 600_000.0));
        assert!(!router.via_fits(Some(gnd), V::new(10e6, 0.6e6), 600_000.0));
    }

    #[test]
    fn corner_modes_and_the_previous_segment() {
        let (pcb, gnd) = setup();
        let router = Router::new(&pcb, None);
        let base = Request {
            layer: CopperLayer::Top,
            net: Some(gnd),
            width: 250_000.0,
            start: V::new(0.0, -8e6),
            target: V::new(6e6, -14e6),
            diagonal_first: false,
            corners: Corners::Deg45,
            previous: None,
        };
        let path =
            |route: &Route| -> Vec<V> { route.points.iter().map(|p| V::new(p[0], p[1])).collect() };
        // 45°: a pure diagonal; 90°: an L with a right angle.
        let r45 = router.route(&base);
        assert_eq!(path(&r45).len(), 2);
        let r90 = router.route(&Request {
            corners: Corners::Deg90,
            ..base.clone()
        });
        let p90 = path(&r90);
        assert_eq!(p90.len(), 3, "{p90:?}");
        assert!(!bends_ok(&p90, None, Corners::Deg45));
        assert!(bends_ok(&p90, None, Corners::Deg90));
        // Continuing a segment that ran in +x: going straight down (−y)
        // would be a 90° bend, so the route turns through 45° first.
        let continued = router.route(&Request {
            target: V::new(0.0, -14e6),
            previous: Some(V::new(1e6, 0.0)),
            ..base.clone()
        });
        let points = path(&continued);
        assert!(continued.reached, "{points:?}");
        assert!(bends_ok(&points, Some(0), Corners::Deg45), "{points:?}");
        assert!(points.len() >= 3, "{points:?}");
        // Around the wall in 45° mode, also when starting towards it.
        let around = router.route(&Request {
            start: V::new(0.0, 0.0),
            target: V::new(20e6, 0.0),
            previous: Some(V::new(1e6, 0.0)),
            ..base
        });
        let points = path(&around);
        assert!(around.reached);
        assert!(bends_ok(&points, Some(0), Corners::Deg45), "{points:?}");
    }

    #[test]
    fn directions_and_bends() {
        assert_eq!(dir_index(V::new(5.0, 0.0)), Some(0));
        assert_eq!(dir_index(V::new(3.0, 3.0)), Some(1));
        assert_eq!(dir_index(V::new(0.0, -7.0)), Some(6));
        assert_eq!(dir_index(V::new(3.0, 1.0)), None);
        assert_eq!(turn(0, 1), 1);
        assert_eq!(turn(0, 7), 1);
        assert_eq!(turn(0, 2), 2);
        assert_eq!(turn(1, 5), 4);
        let square = [V::new(0.0, 0.0), V::new(10.0, 0.0), V::new(10.0, 10.0)];
        assert!(!bends_ok(&square, None, Corners::Deg45));
        assert!(bends_ok(&square, None, Corners::Deg90));
        let soft = [V::new(0.0, 0.0), V::new(10.0, 0.0), V::new(20.0, 10.0)];
        assert!(bends_ok(&soft, None, Corners::Deg45));
        assert!(!bends_ok(&soft, Some(2), Corners::Deg45));
    }
}
