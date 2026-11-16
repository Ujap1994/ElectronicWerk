//! Differential pairs: two nets that carry one signal with opposite
//! polarity. They are found by their names (`USB_DP` and `USB_DN`, `TX+`
//! and `TX-`, `CLK_P` and `CLK_N`, `D_P` and `D_M`, `CANH` and `CANL`),
//! routed together at a fixed gap and checked for that gap.
//!
//! A pair is routed as its centre line: the walkaround router finds a path
//! for a track as wide as both tracks and their gap, with the copper of
//! both nets as its own, and the two tracks are that path moved sideways by
//! half their pitch (width + gap), with mitred bends so the gap stays the
//! same around them. Short 45° tracks join the pads to the coupled part.

use std::collections::HashMap;

use ew_model::id::Id;
use ew_model::layer::CopperLayer;
use ew_model::pcb::Pcb;
use serde::Serialize;

use crate::geom::{BBox, V};
use crate::items::ItemRef;
use crate::route::{Corners, Request, Router};

/// The polarity of one net of a pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Polarity {
    /// `P`, `+`, `H`.
    Positive,
    /// `N`, `M`, `-`, `L`.
    Negative,
}

/// The common part of a net name and its polarity, if the name ends like
/// one net of a pair (`USB_DP` → `USB_D`, positive).
#[must_use]
pub fn polarity(name: &str) -> Option<(&str, Polarity)> {
    let last = name.chars().last()?;
    let base = &name[..name.len() - last.len_utf8()];
    if base.is_empty() {
        return None;
    }
    let polarity = match last {
        '+' | 'P' | 'p' | 'H' | 'h' => Polarity::Positive,
        '-' | 'N' | 'n' | 'M' | 'm' | 'L' | 'l' => Polarity::Negative,
        _ => return None,
    };
    Some((base, polarity))
}

/// A differential pair of a board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    /// The common part of both names (`USB_D`).
    pub name: String,
    /// The positive net.
    pub positive: Id,
    /// The negative net.
    pub negative: Id,
}

/// The pairs of a board: two nets whose names differ only in the polarity
/// at the end, with exactly one net of each polarity (`D+`, `D-`). Names
/// with several candidates of one polarity (`VN` and `VM` next to `VP`) are
/// left out.
#[must_use]
pub fn pairs(pcb: &Pcb) -> Vec<Pair> {
    let mut found: HashMap<String, (Vec<Id>, Vec<Id>)> = HashMap::new();
    for net in &pcb.nets {
        if let Some((base, polarity)) = polarity(&net.name) {
            let entry = found.entry(base.to_owned()).or_default();
            match polarity {
                Polarity::Positive => entry.0.push(net.id),
                Polarity::Negative => entry.1.push(net.id),
            }
        }
    }
    let mut pairs: Vec<Pair> = found
        .into_iter()
        .filter_map(
            |(name, (positive, negative))| match (&positive[..], &negative[..]) {
                ([p], [n]) => Some(Pair {
                    name,
                    positive: *p,
                    negative: *n,
                }),
                _ => None,
            },
        )
        .collect();
    pairs.sort_by(|a, b| a.name.cmp(&b.name));
    pairs
}

/// The other net of the pair a net belongs to.
#[must_use]
pub fn partner(pcb: &Pcb, net: Id) -> Option<Id> {
    pairs(pcb).into_iter().find_map(|pair| {
        if pair.positive == net {
            Some(pair.negative)
        } else if pair.negative == net {
            Some(pair.positive)
        } else {
            None
        }
    })
}

/// How far from the start of a pair route its partner may begin.
const PARTNER_REACH: f64 = 10_000_000.0;

/// Where the partner net of a pair route begins: the centre of its pad or
/// via, or the end of its track, nearest to `at` on the layer (within
/// 10 mm).
#[must_use]
pub fn partner_start(router: &Router<'_>, partner: Id, at: V, layer: CopperLayer) -> Option<V> {
    let bit = router.board.layer_bit(layer)?;
    let mut best: Option<(f64, V)> = None;
    for index in router.board.copper_near(BBox::of(at).grow(PARTNER_REACH)) {
        let item = &router.board.copper[index];
        if item.net != Some(partner)
            || item.layers & bit == 0
            || matches!(item.item, ItemRef::Zone { .. })
        {
            continue;
        }
        for anchor in &item.anchors {
            let d = anchor.dist(at);
            if d <= PARTNER_REACH && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, *anchor));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// A pair route from the cursor's point of view.
#[derive(Debug, Clone, PartialEq)]
pub struct PairRequest {
    /// Copper layer.
    pub layer: CopperLayer,
    /// The net the route started at.
    pub net: Id,
    /// The other net of the pair.
    pub partner: Id,
    /// Width of each track.
    pub width: f64,
    /// Gap between the tracks (edge to edge).
    pub gap: f64,
    /// Where the route's net begins (a pad centre, for example).
    pub start: V,
    /// Where the partner begins.
    pub partner_start: V,
    /// The centre line fixed so far (empty: none yet).
    pub centre: Vec<V>,
    /// Where the cursor is.
    pub target: V,
    /// Start with the diagonal.
    pub diagonal_first: bool,
    /// Which bends are allowed.
    pub corners: Corners,
}

/// A routed pair.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRoute {
    /// The centre line from the middle between the starts.
    pub centre: Vec<[f64; 2]>,
    /// The track of the route's net, from its start.
    pub points: Vec<[f64; 2]>,
    /// The track of the partner, from its start.
    pub partner: Vec<[f64; 2]>,
    /// `true` if the centre line ends at the target.
    pub reached: bool,
    /// `true` if both tracks end on the pads at the target (the pair is
    /// complete).
    pub landed: bool,
}

/// Points of a polyline without repeats (closer than a nanometre).
fn dedupe(points: Vec<V>) -> Vec<V> {
    let mut out: Vec<V> = Vec::with_capacity(points.len());
    for p in points {
        if out.last().is_none_or(|last| last.dist(p) >= 1.0) {
            out.push(p);
        }
    }
    out
}

/// A polyline without points in the middle of a straight run.
fn straight_runs(points: Vec<V>) -> Vec<V> {
    let mut out: Vec<V> = Vec::with_capacity(points.len());
    for p in points {
        if let [.., a, b] = out[..] {
            let (u, v) = (b.sub(a), p.sub(b));
            if u.cross(v).abs() <= 1e-6 * u.len() * v.len() && u.dot(v) > 0.0 {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// A polyline moved sideways by `d` (to the left of its direction for
/// positive `d`), with mitred bends: the moved segments stay parallel to the
/// originals at the same distance.
#[must_use]
pub fn offset(points: &[V], d: f64) -> Vec<V> {
    let points = dedupe(points.to_vec());
    if points.len() < 2 {
        return points;
    }
    let normals: Vec<V> = points
        .windows(2)
        .map(|w| {
            let dir = w[1].sub(w[0]).unit();
            V::new(-dir.y, dir.x)
        })
        .collect();
    let mut out = Vec::with_capacity(points.len());
    out.push(points[0].add(normals[0].scale(d)));
    for (i, pair) in normals.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let k = 1.0 + a.dot(b);
        // A U-turn has no mitre; it never comes out of the router.
        let shift = if k < 1e-6 {
            a.scale(d)
        } else {
            a.add(b).scale(d / k)
        };
        out.push(points[i + 1].add(shift));
    }
    if let (Some(last), Some(normal)) = (points.last(), normals.last()) {
        out.push(last.add(normal.scale(d)));
    }
    out
}

/// The nearest of the eight routing directions to `d`.
fn snap45(d: V) -> V {
    let step = std::f64::consts::FRAC_PI_4;
    let angle = (d.y.atan2(d.x) / step).round() * step;
    V::new(angle.cos(), angle.sin())
}

/// Where a track from `start` joins the moved first segment `a`–`b` of the
/// centre line: at 45° towards it, or straight on when it already lies on
/// it.
fn junction(start: V, a: V, b: V) -> V {
    let dir = b.sub(a).unit();
    let normal = V::new(-dir.y, dir.x);
    let along = start.sub(a).dot(dir);
    let across = start.sub(a).dot(normal).abs();
    let at = (along + across).clamp(0.0, b.dist(a));
    a.add(dir.scale(at))
}

/// The straight piece of centre line at an end of a pair, between the
/// points `a` and `b` where its tracks begin (or end): from their middle
/// across the line between them (snapped to a routing direction, to the
/// side `towards` lies on), long enough for both tracks to meet it at 45°.
/// Returns the middle and the far end of the piece.
fn straight_end(a: V, b: V, towards: V, pitch: f64) -> Option<[V; 2]> {
    let between = b.sub(a);
    if between.len() < 1.0 {
        return None;
    }
    let middle = a.add(b).scale(0.5);
    let across = V::new(-between.y, between.x);
    let dir = snap45(if across.dot(towards.sub(middle)) >= 0.0 {
        across
    } else {
        across.scale(-1.0)
    });
    let normal = V::new(-dir.y, dir.x);
    let reach = [a, b]
        .iter()
        .map(|q| {
            let rel = q.sub(middle);
            rel.dot(dir) + (rel.dot(normal).abs() - pitch / 2.0).abs()
        })
        .fold(0.0_f64, f64::max);
    let length = (reach + pitch / 2.0).max(pitch);
    Some([middle, middle.add(dir.scale(length))])
}

/// The first piece of a pair's centre line, from both starts.
fn launch(request: &PairRequest) -> Option<[V; 2]> {
    straight_end(
        request.start,
        request.partner_start,
        request.target,
        request.width + request.gap,
    )
}

/// `true` if every segment of a track keeps the rules (the partner's
/// copper counts as other copper).
fn track_clear(router: &Router<'_>, request: &PairRequest, net: Id, points: &[V]) -> bool {
    let single = Request {
        layer: request.layer,
        net: Some(net),
        partner: None,
        width: request.width,
        start: request.start,
        target: request.target,
        diagonal_first: request.diagonal_first,
        corners: Corners::Deg90,
        previous: None,
    };
    points
        .windows(2)
        .all(|w| w[0].dist(w[1]) < 1.0 || router.clear(&single, w[0], w[1]))
}

/// The two tracks of a centre line: each starts at its own start and joins
/// the line moved to its side at 45°; with `ends` (the route's net, the
/// partner) each leaves it at 45° for its own end too.
fn tracks(request: &PairRequest, centre: &[V], ends: Option<(V, V)>) -> (Vec<V>, Vec<V>) {
    let half = f64::midpoint(request.width, request.gap);
    let left = offset(centre, half);
    let right = offset(centre, -half);
    // The route's net takes the side nearer to its start.
    let (mine, theirs) = match (left.first(), right.first()) {
        (Some(l), Some(r)) if l.dist(request.start) > r.dist(request.start) => (right, left),
        _ => (left, right),
    };
    let join = |start: V, line: Vec<V>, end: Option<V>| -> Vec<V> {
        let (Some(a), Some(b)) = (line.first().copied(), line.get(1).copied()) else {
            return vec![start];
        };
        let mut out = vec![start, junction(start, a, b)];
        out.extend(line.iter().skip(1));
        if let (Some(end), [.., before, last]) = (end, &line[..]) {
            out.pop();
            out.push(junction(end, *last, *before));
            out.push(end);
        }
        straight_runs(dedupe(out))
    };
    (
        join(request.start, mine, ends.map(|e| e.0)),
        join(request.partner_start, theirs, ends.map(|e| e.1)),
    )
}

impl Router<'_> {
    /// The ends of a pair at the target, when the target is a pad or via
    /// centre (or a track end) of one of its nets: that point and the
    /// nearest one of the other net, as (the route's net, the partner).
    fn landing(&self, request: &PairRequest) -> Option<(V, V)> {
        let at = |net: Id| {
            partner_start(self, net, request.target, request.layer)
                .filter(|p| p.dist(request.target) < 1_000.0)
        };
        let ends = if let Some(mine) = at(request.net) {
            (
                mine,
                partner_start(self, request.partner, mine, request.layer)?,
            )
        } else {
            let theirs = at(request.partner)?;
            (
                partner_start(self, request.net, theirs, request.layer)?,
                theirs,
            )
        };
        let starts = [request.start, request.partner_start];
        (!starts
            .iter()
            .any(|s| s.dist(ends.0) < 1.0 || s.dist(ends.1) < 1.0))
        .then_some(ends)
    }

    /// Routes a differential pair from the fixed centre line (or the middle
    /// between both starts) towards the target. The new part is shortened
    /// until both tracks keep every rule. A target on a pad of one of the
    /// nets ends the pair on that pad and the nearest pad of the other net.
    #[must_use]
    pub fn route_pair(&self, request: &PairRequest) -> PairRoute {
        // The centre line so far: the fixed one, or the straight launch
        // from both starts.
        let first = request.centre.is_empty();
        let fixed: Vec<V> = if first {
            launch(request).map_or_else(Vec::new, Vec::from)
        } else {
            request.centre.clone()
        };
        let done =
            |centre: &[V], mine: &[V], theirs: &[V], reached: bool, landed: bool| PairRoute {
                centre: centre.iter().map(|p| p.pair()).collect(),
                points: mine.iter().map(|p| p.pair()).collect(),
                partner: theirs.iter().map(|p| p.pair()).collect(),
                reached,
                landed,
            };
        let Some(&from) = fixed.last() else {
            return done(
                &[],
                &[request.start],
                &[request.partner_start],
                false,
                false,
            );
        };
        let pitch = request.width + request.gap;
        // On the pads at the target: route to the straight piece before them.
        let ends = self.landing(request);
        let landing = ends.and_then(|(mine, theirs)| straight_end(mine, theirs, from, pitch));
        let previous = (fixed.len() >= 2).then(|| from.sub(fixed[fixed.len() - 2]));
        let envelope = Request {
            layer: request.layer,
            net: Some(request.net),
            partner: Some(request.partner),
            width: 2.0 * request.width + request.gap,
            start: from,
            target: landing.map_or(request.target, |[_, before]| before),
            diagonal_first: request.diagonal_first,
            corners: request.corners,
            previous,
        };
        let routed = self.route(&envelope);
        let new: Vec<V> = routed.points.iter().map(|p| V::new(p[0], p[1])).collect();
        let fits = |centre: &[V], ends: Option<(V, V)>| -> Option<(Vec<V>, Vec<V>)> {
            let (mine, theirs) = tracks(request, centre, ends);
            // Each net lands on its own side.
            if let (Some((my_end, their_end)), Some(last)) = (ends, mine.iter().rev().nth(1)) {
                if last.dist(my_end) > last.dist(their_end) {
                    return None;
                }
            }
            (track_clear(self, request, request.net, &mine)
                && track_clear(self, request, request.partner, &theirs))
            .then_some((mine, theirs))
        };
        // The whole way onto the pads.
        if routed.reached
            && let (Some(ends), Some([middle, _])) = (ends, landing)
        {
            let mut centre = fixed.clone();
            centre.extend(new.iter().skip(1));
            centre.push(middle);
            let centre = dedupe(centre);
            if let Some((mine, theirs)) = fits(&centre, Some(ends)) {
                return done(&centre, &mine, &theirs, true, true);
            }
        }
        // The longest part of the new path whose tracks keep the rules.
        for keep in (1..=new.len()).rev() {
            let mut centre = fixed.clone();
            centre.extend(new.iter().take(keep).skip(1));
            let centre = dedupe(centre);
            if centre.len() < 2 {
                break;
            }
            if let Some((mine, theirs)) = fits(&centre, None) {
                let reached = routed.reached && keep == new.len() && landing.is_none();
                return done(&centre, &mine, &theirs, reached, false);
            }
        }
        // Nothing of the new path fits: the fixed part (or the launch) only,
        // if that fits.
        if fixed.len() >= 2
            && let Some((mine, theirs)) = fits(&fixed, None)
        {
            return done(&fixed, &mine, &theirs, false, false);
        }
        let centre = if first { Vec::new() } else { fixed };
        done(
            &centre,
            &[request.start],
            &[request.partner_start],
            false,
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_of_pairs() {
        assert_eq!(polarity("USB_DP"), Some(("USB_D", Polarity::Positive)));
        assert_eq!(polarity("USB_DM"), Some(("USB_D", Polarity::Negative)));
        assert_eq!(polarity("TX+"), Some(("TX", Polarity::Positive)));
        assert_eq!(polarity("TX-"), Some(("TX", Polarity::Negative)));
        assert_eq!(polarity("CANH"), Some(("CAN", Polarity::Positive)));
        assert_eq!(polarity("P"), None);
        assert_eq!(polarity("GND"), None);
    }

    #[test]
    fn offsets_keep_their_distance_around_bends() {
        // A 45° bend: both moved lines stay parallel at the distance.
        let line = [V::new(0.0, 0.0), V::new(10.0, 0.0), V::new(20.0, 10.0)];
        for d in [2.0, -2.0] {
            let moved = offset(&line, d);
            assert_eq!(moved.len(), 3);
            for (i, w) in moved.windows(2).enumerate() {
                let (a, b) = (line[i], line[i + 1]);
                for p in w {
                    let dist = crate::geom::point_segment(*p, a, b);
                    // The mitre lies on the extension of both segments.
                    let along = {
                        let dir = b.sub(a).unit();
                        let n = V::new(-dir.y, dir.x);
                        p.sub(a).dot(n)
                    };
                    assert!((along - d).abs() < 1e-9, "{p:?} {along}");
                    assert!(dist >= d.abs() - 1e-9);
                }
            }
        }
    }

    #[test]
    fn tracks_join_the_pair_at_45_degrees() {
        // Pads 1 mm apart, tracks 0.5 mm apart (width 0.2, gap 0.3), the
        // target to the right: both tracks leave to the right and bend in.
        let request = PairRequest {
            layer: CopperLayer::Top,
            net: Id::random(),
            partner: Id::random(),
            width: 200.0,
            gap: 300.0,
            start: V::new(0.0, 0.0),
            partner_start: V::new(0.0, 1000.0),
            centre: Vec::new(),
            target: V::new(10_000.0, 500.0),
            diagonal_first: false,
            corners: Corners::Deg45,
        };
        let [a, b] = launch(&request).unwrap();
        assert_eq!(a, V::new(0.0, 500.0));
        assert!((b.y - 500.0).abs() < 1e-9 && b.x >= 500.0);
        let (mine, theirs) = tracks(&request, &[a, b, V::new(10_000.0, 500.0)], None);
        assert_eq!(mine[0], V::new(0.0, 0.0));
        assert!((mine[1].x - 250.0).abs() < 1e-9 && (mine[1].y - 250.0).abs() < 1e-9);
        assert!((theirs[1].x - 250.0).abs() < 1e-9 && (theirs[1].y - 750.0).abs() < 1e-9);
        assert!((mine.last().unwrap().y - 250.0).abs() < 1e-9);
        assert!((theirs.last().unwrap().y - 750.0).abs() < 1e-9);
    }
}
