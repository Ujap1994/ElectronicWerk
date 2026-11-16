//! Teardrops: a smooth widening of a track where it meets a pad or via.
//! They make the joint stronger (against cracks from heat and bending, and
//! against a drill that hits the pad off-centre) and the change of width
//! gentler for fast signals.
//!
//! A teardrop is a small copper zone of the track's net, marked as a
//! teardrop. It starts across the pad at the track's end (as wide as the
//! settings allow, never wider than the pad), and narrows to the track's
//! width some way beyond the pad's edge, with curved or straight sides.
//! Teardrops are made again from scratch whenever the generator runs, and
//! filled like other zones, so they keep their clearance to other nets.

use ew_model::id::Id;
use ew_model::pcb::{IslandRemoval, PadConnection, Pcb, Track, Zone, ZoneFill};
use ew_model::rules::{Rules, TeardropSettings};

use crate::geom::{BBox, Shape, V};
use crate::items::ItemRef;
use crate::route::{Corners, Request, Router};

/// Teardrop zones fill before other zones.
pub const PRIORITY: u32 = 1_000;

/// Points on each curved side.
const CURVE_STEPS: usize = 8;

/// How far a shape reaches from `from` in the direction `dir` (nm).
fn extent(shape: &Shape, from: V, dir: V) -> f64 {
    let (mut lo, mut hi) = (0.0, 20_000_000.0);
    if !shape.contains(from) {
        return 0.0;
    }
    for _ in 0..40 {
        let mid = f64::midpoint(lo, hi);
        if shape.contains(from.add(dir.scale(mid))) {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo < 100.0 {
            break;
        }
    }
    lo
}

/// A quadratic Bézier curve from `a` to `b` around `control`, without `a`.
fn curve(a: V, control: V, b: V) -> Vec<V> {
    (1..=CURVE_STEPS)
        .map(|i| {
            #[allow(clippy::cast_precision_loss, reason = "few steps")]
            let t = i as f64 / CURVE_STEPS as f64;
            let u = 1.0 - t;
            a.scale(u * u)
                .add(control.scale(2.0 * u * t))
                .add(b.scale(t * t))
        })
        .collect()
}

/// The outline of a teardrop.
#[derive(Debug, Clone, PartialEq)]
struct Drop {
    /// The base on the pad: (left, right) seen along the track.
    base: (V, V),
    /// Where it meets the track's edges.
    tip: (V, V),
    /// Where the track leaves the pad, on its edges (the curves' controls).
    edge: (V, V),
}

impl Drop {
    fn outline(&self, curved: bool) -> Vec<V> {
        let mut points = vec![self.base.0];
        if curved {
            points.extend(curve(self.base.0, self.edge.0, self.tip.0));
        } else {
            points.push(self.tip.0);
        }
        points.push(self.tip.1);
        if curved {
            let mut back = curve(self.tip.1, self.edge.1, self.base.1);
            back.pop();
            points.extend(back);
        }
        points.push(self.base.1);
        points
    }
}

impl Router<'_> {
    /// The teardrop where `track` ends at `end` inside the pad or via
    /// `item`, if one fits.
    #[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
    fn teardrop(
        &self,
        track: &Track,
        end: V,
        other: V,
        item: usize,
        settings: &TeardropSettings,
    ) -> Option<Vec<V>> {
        let shape = &self.board.copper[item].shape;
        let length = end.dist(other);
        if length < 1.0 {
            return None;
        }
        let dir = other.sub(end).unit();
        let left = V::new(-dir.y, dir.x);
        let width = track.width as f64;
        let (reach_left, reach_right) = (
            extent(shape, end, left),
            extent(shape, end, left.scale(-1.0)),
        );
        let size = reach_left + reach_right;
        // Tracks about as wide as the pad gain nothing.
        if width >= 0.8 * size {
            return None;
        }
        let out = extent(shape, end, dir);
        let mut long = (size * f64::from(settings.length_ratio) / 100.0)
            .min(settings.max_length as f64)
            .min(length - out);
        let mut wide = (size * f64::from(settings.width_ratio) / 100.0)
            .min(settings.max_width as f64)
            .max(width);
        let request = Request {
            layer: track.layer,
            net: track.net,
            partner: None,
            width: 1.0,
            start: end,
            target: other,
            diagonal_first: false,
            corners: Corners::Deg45,
            previous: None,
        };
        let skip_item = self.board.copper[item].item;
        let skip_track = ItemRef::Track { id: track.id };
        for _ in 0..3 {
            if long < width / 2.0 {
                return None;
            }
            let half = wide / 2.0;
            let pad_edge = end.add(dir.scale(out));
            let tip = pad_edge.add(dir.scale(long));
            let drop = Drop {
                base: (
                    end.add(left.scale(half.min(reach_left))),
                    end.sub(left.scale(half.min(reach_right))),
                ),
                tip: (
                    tip.add(left.scale(width / 2.0)),
                    tip.sub(left.scale(width / 2.0)),
                ),
                edge: (
                    pad_edge.add(left.scale(width / 2.0)),
                    pad_edge.sub(left.scale(width / 2.0)),
                ),
            };
            let outline = drop.outline(settings.curved);
            let mut ring = outline.clone();
            ring.push(outline[0]);
            let clear = ring.windows(2).all(|w| {
                self.clear_skipping(&request, w[0], w[1], &|i| i == skip_item || i == skip_track)
            });
            if clear {
                return Some(outline);
            }
            long *= 0.7;
            wide = (wide * 0.7).max(width);
        }
        None
    }
}

/// The teardrops of a board: one at every end of a straight track that
/// lies in a pad or via of its net (as the settings choose), as zones.
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
pub fn teardrops(pcb: &Pcb, rules: Option<&Rules>, settings: &TeardropSettings) -> Vec<Zone> {
    let router = Router::new(pcb, rules);
    let mut zones = Vec::new();
    for track in pcb
        .tracks
        .iter()
        .filter(|t| t.mid.is_none() && t.net.is_some())
    {
        let Some(bit) = router.board.layer_bit(track.layer) else {
            continue;
        };
        let (a, b) = (V::of(track.start), V::of(track.end));
        for (end, other) in [(a, b), (b, a)] {
            let found = router.board.copper_near(BBox::of(end)).find(|&index| {
                let item = &router.board.copper[index];
                let wanted = match item.item {
                    ItemRef::Pad { .. } => settings.pads,
                    ItemRef::Via { .. } => settings.vias,
                    _ => false,
                };
                wanted
                    && item.net == track.net
                    && item.layers & bit != 0
                    && item.shape.contains(end)
            });
            let Some(index) = found else {
                continue;
            };
            let Some(outline) = router.teardrop(track, end, other, index, settings) else {
                continue;
            };
            let class = router.class_of(track.net);
            zones.push(Zone {
                id: Id::random(),
                name: String::new(),
                net: track.net,
                layers: vec![track.layer],
                outline: outline.into_iter().map(V::point).collect(),
                holes: Vec::new(),
                priority: PRIORITY,
                keepout: None,
                fill: ZoneFill {
                    clearance: class.clearance,
                    min_width: (track.width / 4).max(10_000),
                    pad_connection: PadConnection::Solid,
                    island_removal: IslandRemoval::Never,
                    ..ZoneFill::default()
                },
                filled: Vec::new(),
                teardrop: true,
            });
        }
    }
    zones
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curved_sides_run_from_the_base_to_the_tip() {
        let drop = Drop {
            base: (V::new(0.0, 500.0), V::new(0.0, -500.0)),
            tip: (V::new(1_000.0, 100.0), V::new(1_000.0, -100.0)),
            edge: (V::new(400.0, 100.0), V::new(400.0, -100.0)),
        };
        let straight = drop.outline(false);
        assert_eq!(straight.len(), 4);
        let curved = drop.outline(true);
        assert_eq!(curved.len(), 2 * CURVE_STEPS + 2);
        assert_eq!(curved[0], drop.base.0);
        assert_eq!(curved[CURVE_STEPS], drop.tip.0);
        assert_eq!(*curved.last().unwrap_or(&V::default()), drop.base.1);
        // The sides bend inwards: halfway they are narrower than a straight
        // side would be.
        let half = curved[CURVE_STEPS / 2];
        assert!(half.y < 300.0, "{half:?}");
        let extent = extent(
            &Shape::disc(V::new(0.0, 0.0), 300.0),
            V::new(0.0, 0.0),
            V::new(1.0, 0.0),
        );
        assert!((extent - 300.0).abs() < 200.0);
    }
}
