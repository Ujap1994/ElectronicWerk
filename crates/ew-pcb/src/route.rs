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
use serde::Serialize;

use crate::drc::{EPSILON, NetInfo, item_props, net_table};
use crate::geom::{BBox, Shape, V};
use crate::items::{Board, ItemRef};
use crate::rules::{NoZones, Props, RuleSet};

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
}

/// A router for one board.
pub struct Router<'a> {
    board: Board<'a>,
    rules: RuleSet,
    nets: HashMap<Id, NetInfo>,
    no_net: NetInfo,
}

const MAX_EXPANSIONS: usize = 200_000;

/// The eight grid directions.
const DIRS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// A search state: a grid cell and the direction it was entered from
/// (8 = none).
type State = ((i32, i32), u8);

/// The two-segment path from `a` to `b` at 45° and 90°: the corner point.
fn corner(a: V, b: V, diagonal_first: bool) -> V {
    let d = b.sub(a);
    let m = d.x.abs().min(d.y.abs());
    let diag = V::new(m.copysign(d.x), m.copysign(d.y));
    if diagonal_first {
        a.add(diag)
    } else {
        b.sub(diag)
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
        let mut found: Option<(V, Option<Id>, u8)> = None;
        for index in self.board.copper_near(BBox::of(at).grow(1.0)) {
            let item = &self.board.copper[index];
            if item.layers & bit == 0 || item.shape.gap(&probe, 1.0).is_none() {
                continue;
            }
            let (point, rank) = match item.item {
                ItemRef::Pad { .. } | ItemRef::Via { .. } => (item.anchors[0], 0),
                ItemRef::Track { id } => {
                    let Some(track) = self.board.pcb.tracks.get(id) else {
                        continue;
                    };
                    let (s, e) = (V::of(track.start), V::of(track.end));
                    (if s.dist(at) <= e.dist(at) { s } else { e }, 1)
                }
                ItemRef::Zone { .. } => (at, 2),
            };
            if found.as_ref().is_none_or(|(_, _, best)| rank < *best) {
                found = Some((point, item.net, rank));
            }
        }
        let (point, net) = found.map_or((at, None), |(p, n, _)| (p, n));
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

    fn two_segments(&self, request: &Request, a: V, b: V, diagonal_first: bool) -> Option<Vec<V>> {
        let c = corner(a, b, diagonal_first);
        let ok = if c.dist(a) < 1.0 || c.dist(b) < 1.0 {
            self.clear(request, a, b)
        } else {
            self.clear(request, a, c) && self.clear(request, c, b)
        };
        ok.then(|| {
            let mut points = vec![a];
            if c.dist(a) >= 1.0 && c.dist(b) >= 1.0 {
                points.push(c);
            }
            if b.dist(a) >= 1.0 {
                points.push(b);
            }
            points
        })
    }

    /// Routes from the start towards the target.
    #[must_use]
    pub fn route(&self, request: &Request) -> Route {
        let (start, target) = (request.start, request.target);
        for diagonal_first in [request.diagonal_first, !request.diagonal_first] {
            if let Some(points) = self.two_segments(request, start, target, diagonal_first) {
                return Route {
                    points: points.into_iter().map(V::pair).collect(),
                    reached: true,
                };
            }
        }
        let (points, reached) = self.search(request);
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
    fn search(&self, request: &Request) -> (Vec<V>, bool) {
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
        // State: cell and the direction it was entered from (8 = none).
        let origin = (0, 0);
        let mut best_cost: HashMap<State, u64> = HashMap::new();
        let mut parent: HashMap<State, State> = HashMap::new();
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((octile(origin), 0_u64, origin, 8_u8)));
        best_cost.insert((origin, 8), 0);
        let mut closest = ((origin, 8_u8), octile(origin));
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
                let next = (c.0 + dx, c.1 + dy);
                if !is_free(next) {
                    continue;
                }
                let mut step_cost = if *dx != 0 && *dy != 0 { 1414 } else { 1000 };
                let index = index as u8;
                if dir != 8 && dir != index {
                    step_cost += 400;
                }
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
        let mut points = self.straighten(request, &path);
        if reached {
            // End exactly at the target: replace the tail with a clear
            // two-segment path.
            let n = points.len();
            let mut joined = false;
            for k in (n.saturating_sub(4)..n).rev() {
                for diagonal_first in [true, false] {
                    if let Some(tail) =
                        self.two_segments(request, points[k], target, diagonal_first)
                    {
                        points.truncate(k);
                        points.extend(tail);
                        joined = true;
                        break;
                    }
                }
                if joined {
                    break;
                }
            }
            if !joined && points.last().is_some_and(|p| p.dist(target) > 1.0) {
                return (points, false);
            }
        }
        (points, reached)
    }

    /// Joins grid steps into long segments at 0°, 45° and 90°.
    fn straighten(&self, request: &Request, path: &[V]) -> Vec<V> {
        let octilinear = |a: V, b: V| {
            let d = b.sub(a);
            d.x.abs() < 1.0 || d.y.abs() < 1.0 || (d.x.abs() - d.y.abs()).abs() < 1.0
        };
        let mut out = vec![path[0]];
        let mut i = 0;
        while i + 1 < path.len() {
            let mut j = i + 1;
            let mut k = path.len() - 1;
            // The farthest point reachable in one straight segment.
            while k > i + 1 {
                if octilinear(path[i], path[k]) && self.clear(request, path[i], path[k]) {
                    j = k;
                    break;
                }
                k -= 1;
            }
            if j == i + 1 && !self.clear(request, path[i], path[j]) {
                break;
            }
            out.push(path[j]);
            i = j;
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
}
