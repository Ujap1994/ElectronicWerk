//! Helpers from the enclosure (M17), read from cross-sections and rays:
//!
//! - [`cavity`]: the room inside the enclosure at the board plane, for the
//!   board outline.
//! - [`material`]: enclosure material at the board plane (posts, ribs) —
//!   keep-out areas.
//! - [`bosses`] and [`mounting`]: the bosses under (or over) the board
//!   against its mounting holes.
//! - [`openings`]: connectors that reach over the board edge, checked for
//!   an opening in front of them.
//! - [`height_areas`]: how tall parts may be on each side of the board,
//!   from rays to the enclosure.
//!
//! Lengths are millimetres unless a name says otherwise; polygons for the
//! board use nanometres like the rest of the board model.

use ew_model::id::Id;
use ew_model::layer::Side;
use ew_pcb::geom::V;
use ew_pcb::poly::{self, Polygons};
use serde::Serialize;

use crate::collide::{Bounds, Solid};
use crate::mechanical::{PartShape, Shape};
use crate::section::{self, Loop};

const NM: f64 = 1e6;

fn to_nm(ring: &[[f64; 2]]) -> Vec<V> {
    ring.iter().map(|p| V::new(p[0] * NM, p[1] * NM)).collect()
}

fn to_mm(ring: &[V]) -> Loop {
    ring.iter().map(|p| [p.x / NM, p.y / NM]).collect()
}

/// The cross-section of solids at height `z` as united polygons (nm).
#[must_use]
pub fn section_polygons(solids: &[&Solid], z: f64) -> Polygons {
    let rings: Polygons = solids
        .iter()
        .flat_map(|s| section::slice(&s.triangles, z))
        .map(|ring| vec![to_nm(&ring)])
        .collect();
    poly::union(&rings)
}

/// The room inside the enclosure at height `z`: the hole of the section
/// that contains `near` (else the largest hole), as a counter-clockwise
/// loop in mm.
#[must_use]
pub fn cavity(solids: &[&Solid], z: f64, near: [f64; 2]) -> Option<Loop> {
    let united = section_polygons(solids, z);
    let mut holes: Vec<Loop> = united
        .iter()
        .flat_map(|polygon| polygon.iter().skip(1))
        .map(|ring| {
            let mut ring = to_mm(ring);
            if section::area2(&ring) < 0.0 {
                ring.reverse();
            }
            ring
        })
        .collect();
    if let Some(index) = holes.iter().position(|h| section::contains(h, near)) {
        return Some(holes.swap_remove(index));
    }
    holes
        .into_iter()
        .max_by(|a, b| section::area2(a).total_cmp(&section::area2(b)))
}

/// Enclosure material at height `z` inside the board area (nm).
#[must_use]
pub fn material(solids: &[&Solid], z: f64, board: &Polygons) -> Polygons {
    let section = section_polygons(solids, z);
    if section.is_empty() || board.is_empty() {
        return Vec::new();
    }
    poly::intersection(&section, board)
        .into_iter()
        .filter(|p| poly::area(std::slice::from_ref(p)) > 0.01 * NM * NM)
        .collect()
}

/// A boss (or post) of the enclosure where it meets the board.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Boss {
    /// The body.
    pub body: Id,
    /// Below or above the board.
    pub side: Side,
    /// Centre (mm): of its hole, else of the boss.
    pub center: [f64; 2],
    /// Diameter of a circle of the same area (mm).
    pub diameter: f64,
    /// Diameter of the hole in it (mm), if any.
    pub hole: Option<f64>,
}

/// The bosses of a body: small pieces of its section at height `z` (at
/// most `largest` mm across).
#[must_use]
pub fn bosses(body: Id, solid: &Solid, z: f64, side: Side, largest: f64) -> Vec<Boss> {
    let united = section_polygons(&[solid], z);
    let mut out = Vec::new();
    for polygon in &united {
        let Some(outline) = polygon.first() else {
            continue;
        };
        let ring = to_mm(outline);
        let b = ring.iter().fold(Bounds::EMPTY, |mut b, p| {
            b.add([p[0], p[1], 0.0]);
            b
        });
        if b.max[0] - b.min[0] > largest || b.max[1] - b.min[1] > largest {
            continue;
        }
        let area = section::area2(&ring).abs() / 2.0;
        let hole = polygon.get(1).map(|h| to_mm(h));
        let center = hole
            .as_ref()
            .map_or_else(|| section::centroid(&ring), |h| section::centroid(h));
        out.push(Boss {
            body,
            side,
            center,
            diameter: 2.0 * (area / std::f64::consts::PI).sqrt(),
            hole: hole
                .map(|h| 2.0 * (section::area2(&h).abs() / 2.0 / std::f64::consts::PI).sqrt()),
        });
    }
    out
}

/// A mounting hole of the board.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MountingHole {
    /// Centre (mm).
    pub center: [f64; 2],
    /// Diameter (mm).
    pub diameter: f64,
    /// The footprint it belongs to.
    pub footprint: Option<Id>,
}

/// What does not fit between bosses and mounting holes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum MountIssue {
    /// A boss under the board without a hole over it.
    BossWithoutHole {
        /// The boss.
        boss: Boss,
    },
    /// A mounting hole without a boss.
    HoleWithoutBoss {
        /// The hole.
        hole: MountingHole,
    },
    /// Hole and boss do not line up.
    Offset {
        /// The boss.
        boss: Boss,
        /// The hole.
        hole: MountingHole,
        /// Distance of the centres (mm).
        distance: f64,
    },
    /// The board's hole is smaller than the screw hole of the boss.
    TooSmall {
        /// The boss.
        boss: Boss,
        /// The hole.
        hole: MountingHole,
    },
}

/// Compares the bosses with the board's mounting holes. Centres closer
/// than `tolerance` (mm) line up; bosses outside the board are left out.
/// Holes without a boss are reported only if the enclosure has bosses.
#[must_use]
pub fn mounting(
    bosses: &[Boss],
    holes: &[MountingHole],
    board: &Polygons,
    tolerance: f64,
) -> Vec<MountIssue> {
    let on_board = |c: [f64; 2]| {
        let p = V::new(c[0] * NM, c[1] * NM);
        board.iter().any(|polygon| {
            polygon
                .first()
                .is_some_and(|outline| section::contains(&to_mm(outline), [p.x / NM, p.y / NM]))
        })
    };
    let bosses: Vec<&Boss> = bosses.iter().filter(|b| on_board(b.center)).collect();
    let mut out = Vec::new();
    let mut used = vec![false; holes.len()];
    for boss in &bosses {
        let nearest = holes
            .iter()
            .enumerate()
            .map(|(i, h)| {
                (
                    i,
                    (h.center[0] - boss.center[0]).hypot(h.center[1] - boss.center[1]),
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match nearest {
            Some((i, d)) if d <= boss.diameter / 2.0 => {
                used[i] = true;
                let hole = holes[i].clone();
                if d > tolerance {
                    out.push(MountIssue::Offset {
                        boss: (*boss).clone(),
                        hole,
                        distance: d,
                    });
                } else if boss.hole.is_some_and(|screw| hole.diameter + 1e-6 < screw) {
                    out.push(MountIssue::TooSmall {
                        boss: (*boss).clone(),
                        hole,
                    });
                }
            }
            _ => out.push(MountIssue::BossWithoutHole {
                boss: (*boss).clone(),
            }),
        }
    }
    if !bosses.is_empty() {
        for (i, hole) in holes.iter().enumerate() {
            if !used[i] {
                out.push(MountIssue::HoleWithoutBoss { hole: hole.clone() });
            }
        }
    }
    out
}

/// A connector over the board edge with no opening in front of it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocked {
    /// The part.
    pub part: Id,
    /// The body in front of it.
    pub body: Id,
    /// Where (the middle of the part's face, mm).
    pub point: [f64; 3],
}

/// Parts reaching more than `overhang` mm over the board's box: rays from
/// their outer face (3 × 3 over it) that all meet a body within `reach`
/// mean there is no opening for them.
#[must_use]
pub fn openings(
    parts: &[PartShape],
    board: [f64; 4],
    solids: &[(Id, &Solid)],
    overhang: f64,
    reach: f64,
) -> Vec<Blocked> {
    let mut out = Vec::new();
    for part in parts.iter().filter(|p| !p.stand_in) {
        let bounds = part.shapes.iter().fold(Bounds::EMPTY, |b, s| {
            let sb = match s {
                Shape::Mesh(mesh, matrix) => Bounds::of_mesh(mesh, matrix),
                Shape::Box(tris) => {
                    let mut b = Bounds::EMPTY;
                    for t in tris {
                        for p in t {
                            b.add(*p);
                        }
                    }
                    b
                }
            };
            b.union(&sb)
        });
        if bounds.is_empty() {
            continue;
        }
        // The sides it reaches over: (axis, direction).
        let sides = [
            (0, -1.0, board[0] - bounds.min[0]),
            (0, 1.0, bounds.max[0] - board[2]),
            (1, -1.0, board[1] - bounds.min[1]),
            (1, 1.0, bounds.max[1] - board[3]),
        ];
        for (axis, sign, over) in sides {
            if over <= overhang {
                continue;
            }
            let other = 1 - axis;
            let face = if sign < 0.0 {
                bounds.min[axis] - 0.01
            } else {
                bounds.max[axis] + 0.01
            };
            let mut dir = [0.0; 3];
            dir[axis] = sign;
            let mut blocked_by: Option<Id> = None;
            let mut all = true;
            for i in 1..=3 {
                for j in 1..=3 {
                    let u = bounds.min[other]
                        + (bounds.max[other] - bounds.min[other]) * f64::from(i) / 4.0;
                    let w = bounds.min[2] + (bounds.max[2] - bounds.min[2]) * f64::from(j) / 4.0;
                    let mut p = [0.0; 3];
                    p[axis] = face;
                    p[other] = u;
                    p[2] = w;
                    let hit = solids
                        .iter()
                        .filter_map(|(id, s)| s.ray(p, dir, reach).map(|d| (*id, d)))
                        .min_by(|a, b| a.1.total_cmp(&b.1));
                    match hit {
                        Some((id, _)) => {
                            blocked_by.get_or_insert(id);
                        }
                        None => all = false,
                    }
                }
            }
            if all && let Some(body) = blocked_by {
                let mut point = [0.0; 3];
                point[axis] = face;
                point[other] = f64::midpoint(bounds.min[other], bounds.max[other]);
                point[2] = f64::midpoint(bounds.min[2], bounds.max[2]);
                out.push(Blocked {
                    part: part.id,
                    body,
                    point,
                });
            }
        }
    }
    out
}

/// An area where parts may be at most `height` tall.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightArea {
    /// The board side.
    pub side: Side,
    /// Free height above (below) the board surface (mm).
    pub height: f64,
    /// The area (nm).
    pub polygons: Polygons,
}

/// How tall parts may be over the board: rays up from the top face and
/// down from the bottom face on a grid of `step` mm, the free heights
/// rounded down to `quantum` mm, cells of the same height united. Cells
/// with nothing above them give no area.
#[must_use]
#[allow(clippy::too_many_arguments, reason = "geometry inputs")]
pub fn height_areas(
    board: &Polygons,
    top: f64,
    bottom: f64,
    solids: &[&Solid],
    step: f64,
    quantum: f64,
    reach: f64,
) -> Vec<HeightArea> {
    let Some(bbox) =
        board
            .iter()
            .filter_map(|p| p.first())
            .flatten()
            .fold(None, |b: Option<[f64; 4]>, v| {
                let (x, y) = (v.x / NM, v.y / NM);
                Some(b.map_or([x, y, x, y], |b| {
                    [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
                }))
            })
    else {
        return Vec::new();
    };
    let step = step.max(0.2);
    let quantum = quantum.max(0.05);
    let outer: Vec<Loop> = board
        .iter()
        .filter_map(|p| p.first())
        .map(|r| to_mm(r))
        .collect();
    let holes: Vec<Loop> = board
        .iter()
        .flat_map(|p| p.iter().skip(1))
        .map(|r| to_mm(r))
        .collect();
    let inside = |p: [f64; 2]| {
        outer.iter().any(|r| section::contains(r, p))
            && !holes.iter().any(|r| section::contains(r, p))
    };
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "grid size"
    )]
    let (nx, ny) = (
        (((bbox[2] - bbox[0]) / step).ceil() as usize).max(1),
        (((bbox[3] - bbox[1]) / step).ceil() as usize).max(1),
    );
    let mut out = Vec::new();
    for side in [Side::Top, Side::Bottom] {
        let (z, dir) = match side {
            Side::Top => (top, [0.0, 0.0, 1.0]),
            Side::Bottom => (bottom, [0.0, 0.0, -1.0]),
        };
        // Free height per cell, quantized (in quanta).
        let mut levels: std::collections::BTreeMap<i64, Polygons> =
            std::collections::BTreeMap::new();
        for j in 0..ny {
            for i in 0..nx {
                #[allow(clippy::cast_precision_loss, reason = "grid indices")]
                let (x0, y0) = (bbox[0] + i as f64 * step, bbox[1] + j as f64 * step);
                let c = [x0 + step / 2.0, y0 + step / 2.0];
                if !inside(c) {
                    continue;
                }
                let start = [c[0], c[1], z];
                let free = if solids.iter().any(|s| s.contains(start)) {
                    Some(0.0)
                } else {
                    solids
                        .iter()
                        .filter_map(|s| s.ray(start, dir, reach))
                        .reduce(f64::min)
                };
                let Some(free) = free else {
                    continue;
                };
                #[allow(clippy::cast_possible_truncation, reason = "heights in quanta")]
                let level = (free / quantum).floor() as i64;
                let cell = vec![to_nm(&[
                    [x0, y0],
                    [x0 + step, y0],
                    [x0 + step, y0 + step],
                    [x0, y0 + step],
                ])];
                levels.entry(level).or_default().push(cell);
            }
        }
        for (level, cells) in levels {
            let united = poly::intersection(&poly::union(&cells), board);
            if united.is_empty() {
                continue;
            }
            #[allow(clippy::cast_precision_loss, reason = "heights in quanta")]
            out.push(HeightArea {
                side,
                height: level as f64 * quantum,
                polygons: united,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collide::box_triangles;
    use crate::place::Matrix;

    fn part(id: u128, min: [f64; 3], max: [f64; 3]) -> PartShape {
        PartShape {
            id: Id::from_u128(id),
            designator: format!("J{id}"),
            shapes: vec![Shape::Box(box_triangles(min, max, &Matrix::IDENTITY))],
            stand_in: false,
        }
    }

    #[test]
    fn connectors_need_an_opening() {
        // A wall at x 32…34 with an opening from y 4 to 12, z 0 to 6.
        let mut tris = box_triangles([32.0, -10.0, -5.0], [34.0, 4.0, 15.0], &Matrix::IDENTITY);
        tris.extend(box_triangles(
            [32.0, 12.0, -5.0],
            [34.0, 30.0, 15.0],
            &Matrix::IDENTITY,
        ));
        tris.extend(box_triangles(
            [32.0, 4.0, 6.0],
            [34.0, 12.0, 15.0],
            &Matrix::IDENTITY,
        ));
        tris.extend(box_triangles(
            [32.0, 4.0, -5.0],
            [34.0, 12.0, 0.0],
            &Matrix::IDENTITY,
        ));
        let wall = Solid::new(tris);
        let board = [0.0, 0.0, 30.0, 20.0];
        let solids = [(Id::from_u128(9), &wall)];
        // In front of the opening, in front of the wall, and inside.
        let parts = [
            part(1, [27.0, 6.0, 1.6], [31.0, 10.0, 4.0]),
            part(2, [27.0, 14.0, 1.6], [31.0, 18.0, 4.0]),
            part(3, [10.0, 6.0, 1.6], [14.0, 10.0, 4.0]),
        ];
        let blocked = openings(&parts, board, &solids, 0.3, 10.0);
        assert_eq!(blocked.len(), 1, "{blocked:?}");
        assert_eq!(blocked[0].part, Id::from_u128(2));
        assert_eq!(blocked[0].body, Id::from_u128(9));
    }

    #[test]
    fn heights_under_a_lid() {
        // A plate 10 mm over a 20 × 10 mm board, lower (5 mm) over x < 5.
        let mut tris = box_triangles([-5.0, -5.0, 11.6], [25.0, 15.0, 13.0], &Matrix::IDENTITY);
        tris.extend(box_triangles(
            [-5.0, -5.0, 6.6],
            [5.0, 15.0, 11.6],
            &Matrix::IDENTITY,
        ));
        let lid = Solid::new(tris);
        let board: Polygons = vec![vec![to_nm(&[
            [0.0, 0.0],
            [20.0, 0.0],
            [20.0, 10.0],
            [0.0, 10.0],
        ])]];
        let areas = height_areas(&board, 1.6, 0.0, &[&lid], 1.0, 0.5, 100.0);
        let top: Vec<&HeightArea> = areas.iter().filter(|a| a.side == Side::Top).collect();
        assert_eq!(top.len(), 2, "{areas:?}");
        let heights: Vec<f64> = top.iter().map(|a| a.height).collect();
        assert!(
            heights.contains(&5.0) && heights.contains(&10.0),
            "{heights:?}"
        );
        let low = top
            .iter()
            .find(|a| (a.height - 5.0).abs() < 1e-9)
            .map_or(0.0, |a| poly::area(&a.polygons));
        assert!((low / NM / NM - 50.0).abs() < 1e-6, "{low}");
        assert!(
            areas.iter().all(|a| a.side == Side::Top),
            "nothing under the board"
        );
    }
}
