//! Stitching vias: free vias (no track ends at them) of one net.
//!
//! - **Zone stitching** fills a copper zone with a grid of vias of its net,
//!   so its copper connects to the copper of the same net on the other
//!   layers with many short paths (a low-impedance ground, heat spreading).
//! - **A via fence** puts vias of one net (usually ground) along both sides
//!   of tracks, which shields them (RF lines, fast clocks).
//!
//! A via goes only where it keeps every clearance and the hole distances,
//! stays out of keep-outs and off the copper of its own net (pads, tracks,
//! vias), and keeps its distance to the board edge. In a zone it also lies
//! completely in the zone's copper and in copper of the same net on another
//! layer.

use ew_model::id::Id;
use ew_model::layer::CopperLayer;
use ew_model::pcb::{Pcb, Track, Via, Zone};

use crate::geom::{BBox, Shape, V, point_segment};
use crate::items::ItemRef;
use crate::route::Router;

/// The size of stitching vias.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViaSize {
    /// Pad diameter (nm).
    pub diameter: f64,
    /// Drill (nm).
    pub drill: f64,
}

/// `true` if `p` lies in the polygon at least `margin` inside its edges.
fn inside(outline: &[V], holes: &[Vec<V>], p: V, margin: f64) -> bool {
    let crossing = |ring: &[V]| {
        let mut inside = false;
        for i in 0..ring.len() {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
        }
        inside
    };
    let clear = |ring: &[V]| {
        (0..ring.len()).all(|i| point_segment(p, ring[i], ring[(i + 1) % ring.len()]) >= margin)
    };
    crossing(outline) && clear(outline) && holes.iter().all(|hole| !crossing(hole) && clear(hole))
}

/// `true` if `p` lies in the filled copper of a zone on a layer, at least
/// `margin` inside.
fn in_fill(zone: &Zone, layer: CopperLayer, p: V, margin: f64) -> bool {
    zone.filled.iter().filter(|a| a.layer == layer).any(|area| {
        let outline: Vec<V> = area.points.iter().map(|q| V::of(*q)).collect();
        let holes: Vec<Vec<V>> = area
            .holes
            .iter()
            .map(|h| h.iter().map(|q| V::of(*q)).collect())
            .collect();
        inside(&outline, &holes, p, margin)
    })
}

/// The vias of a net that no track ends at.
#[must_use]
pub fn free_vias(pcb: &Pcb, net: Id) -> Vec<&Via> {
    pcb.vias
        .iter()
        .filter(|via| via.net == Some(net))
        .filter(|via| {
            !pcb.tracks
                .iter()
                .any(|t| t.net == Some(net) && (t.start == via.at || t.end == via.at))
        })
        .collect()
}

impl Router<'_> {
    /// `true` if a stitching via of `net` fits at `at`.
    #[must_use]
    #[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
    pub fn stitch_fits(&self, net: Id, at: V, size: ViaSize, placed: &[V]) -> bool {
        if !self.via_fits(Some(net), at, size.diameter, None) {
            return false;
        }
        let r = size.diameter / 2.0;
        let shape = Shape::disc(at, r);
        let constraints = &self.rules.constraints;
        let clearance = self.class_of(Some(net)).clearance as f64;
        // Off the copper of its own net (except zones).
        for index in self.board.copper_near(shape.bbox().grow(clearance)) {
            let item = &self.board.copper[index];
            if item.net == Some(net)
                && !matches!(item.item, ItemRef::Zone { .. })
                && shape.gap(&item.shape, clearance).is_some()
            {
                return false;
            }
        }
        // Hole distances, to the holes of the board and to the new vias.
        let hole_gap = constraints.min_hole_to_hole as f64;
        let drill = Shape::disc(at, size.drill / 2.0);
        for index in self.board.holes_near(drill.bbox().grow(hole_gap)) {
            if drill
                .gap(&self.board.holes[index].shape, hole_gap)
                .is_some()
            {
                return false;
            }
        }
        if placed
            .iter()
            .any(|p| p.dist(at) < size.drill + hole_gap.max(clearance))
        {
            return false;
        }
        // Keep-outs that forbid vias, and the board edge.
        if self
            .board
            .keepouts
            .iter()
            .any(|k| k.rules.vias && shape.gap(&k.shape, 0.0).is_some())
        {
            return false;
        }
        let edge = constraints.min_copper_to_edge as f64;
        self.board.outline.edge_gap(&shape, edge).is_none()
    }

    /// Stitching vias for a zone: a grid of `pitch` (every other row moved
    /// by half a pitch when `staggered`), where the via lies in the zone's
    /// copper and in copper of the same net on another layer.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "grid counts are small"
    )]
    pub fn stitch_zone(
        &self,
        pcb: &Pcb,
        zone: &Zone,
        size: ViaSize,
        pitch: f64,
        staggered: bool,
    ) -> Vec<V> {
        let Some(net) = zone.net else {
            return Vec::new();
        };
        if pitch <= 0.0 || zone.outline.len() < 3 {
            return Vec::new();
        }
        let points: Vec<V> = zone.outline.iter().map(|p| V::of(*p)).collect();
        let bbox = BBox::of_points(&points);
        let r = size.diameter / 2.0;
        let others: Vec<&Zone> = pcb
            .zones
            .iter()
            .filter(|z| z.net == Some(net) && z.keepout.is_none())
            .collect();
        let mut placed: Vec<V> = Vec::new();
        let (w, h) = (bbox.max.x - bbox.min.x, bbox.max.y - bbox.min.y);
        let (cols, rows) = ((w / pitch) as usize + 1, (h / pitch) as usize + 1);
        // The grid is centred in the zone.
        let x0 = bbox.min.x + (w - (cols - 1) as f64 * pitch) / 2.0;
        let y0 = bbox.min.y + (h - (rows - 1) as f64 * pitch) / 2.0;
        for row in 0..rows {
            let shift = if staggered && row % 2 == 1 {
                pitch / 2.0
            } else {
                0.0
            };
            for col in 0..cols {
                let at = V::new(x0 + col as f64 * pitch + shift, y0 + row as f64 * pitch);
                let on = |layer: CopperLayer| {
                    others
                        .iter()
                        .any(|z| z.layers.contains(&layer) && in_fill(z, layer, at, r))
                };
                let here = zone
                    .layers
                    .iter()
                    .find(|&&layer| in_fill(zone, layer, at, r));
                let Some(&layer) = here else {
                    continue;
                };
                let elsewhere = self
                    .board
                    .stack
                    .iter()
                    .any(|&other| other != layer && on(other));
                if elsewhere && self.stitch_fits(net, at, size, &placed) {
                    placed.push(at);
                }
            }
        }
        placed
    }

    /// A via fence of `net` along both sides of tracks: vias `pitch` apart,
    /// `gap` from the track's edge to the via's edge.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "counts along a track are small"
    )]
    pub fn fence(&self, tracks: &[&Track], net: Id, size: ViaSize, pitch: f64, gap: f64) -> Vec<V> {
        let mut placed: Vec<V> = Vec::new();
        if pitch <= 0.0 {
            return placed;
        }
        for track in tracks.iter().filter(|t| t.mid.is_none()) {
            let (a, b) = (V::of(track.start), V::of(track.end));
            let length = a.dist(b);
            if length < 1.0 {
                continue;
            }
            let dir = b.sub(a).unit();
            let left = V::new(-dir.y, dir.x);
            let offset = track.width as f64 / 2.0 + gap + size.diameter / 2.0;
            let count = (length / pitch).floor() as usize + 1;
            let start = (length - (count - 1) as f64 * pitch) / 2.0;
            for i in 0..count {
                let along = a.add(dir.scale(start + i as f64 * pitch));
                for side in [1.0, -1.0] {
                    let at = along.add(left.scale(side * offset));
                    // Not next to a via of the fence placed for another
                    // track (at corners), nor too close to any track of
                    // the fenced set.
                    if placed.iter().any(|p| p.dist(at) < 0.75 * pitch) {
                        continue;
                    }
                    let near_track = tracks.iter().any(|t| {
                        point_segment(at, V::of(t.start), V::of(t.end))
                            < t.width as f64 / 2.0 + gap + size.diameter / 2.0 - 1.0
                    });
                    if !near_track && self.stitch_fits(net, at, size, &placed) {
                        placed.push(at);
                    }
                }
            }
        }
        placed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_in_polygons_with_a_margin() {
        let square = [
            V::new(0.0, 0.0),
            V::new(10.0, 0.0),
            V::new(10.0, 10.0),
            V::new(0.0, 10.0),
        ];
        let hole = vec![
            V::new(4.0, 4.0),
            V::new(6.0, 4.0),
            V::new(6.0, 6.0),
            V::new(4.0, 6.0),
        ];
        assert!(inside(&square, &[], V::new(2.0, 2.0), 1.5));
        assert!(!inside(&square, &[], V::new(1.0, 5.0), 1.5));
        assert!(!inside(
            &square,
            std::slice::from_ref(&hole),
            V::new(5.0, 5.0),
            0.0
        ));
        assert!(!inside(
            &square,
            std::slice::from_ref(&hole),
            V::new(5.0, 6.5),
            1.0
        ));
        assert!(inside(&square, &[hole], V::new(5.0, 8.0), 1.0));
    }
}
