//! Triangle meshes in millimetres, built from polygons in nanometres.

use ew_pcb::geom::{BBox, V, signed_area2};
use ew_pcb::poly;
use rayon::prelude::*;

use crate::mm;

/// Walls whose edges turn by less than this share their normals (smooth
/// cylinders), sharper corners stay sharp.
const SMOOTH_COS: f64 = 0.82; // ≈ 35°

/// A triangle mesh: positions and normals in millimetres, counter-clockwise
/// triangles seen from the side the normals point to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals.
    pub normals: Vec<[f32; 3]>,
    /// Triangles (three indices each).
    pub indices: Vec<u32>,
}

/// Polygons: each one an outline followed by its holes (nanometres).
pub type Polygons = Vec<Vec<Vec<V>>>;

impl Mesh {
    /// `true` without triangles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Number of triangles.
    #[must_use]
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    fn vertex(&mut self, p: [f64; 3], n: [f64; 3]) -> u32 {
        let index = u32::try_from(self.positions.len()).unwrap_or(u32::MAX);
        self.positions.push([p[0] as f32, p[1] as f32, p[2] as f32]);
        self.normals.push([n[0] as f32, n[1] as f32, n[2] as f32]);
        index
    }

    /// Appends another mesh.
    pub fn append(&mut self, other: &Self) {
        let base = u32::try_from(self.positions.len()).unwrap_or(u32::MAX);
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.indices.extend(other.indices.iter().map(|i| base + i));
    }

    /// Flat polygons at height `z` (mm), facing up (`up`) or down.
    pub fn cap(&mut self, polygons: &[Vec<Vec<V>>], z: f64, up: bool) {
        let normal = [0.0, 0.0, if up { 1.0 } else { -1.0 }];
        for polygon in polygons {
            if polygon.len() > MAX_HOLES {
                // Ear clipping slows down with many holes: cut the polygon
                // into tiles first.
                let pieces = tiles(polygon);
                let meshes: Vec<Self> = pieces
                    .par_chunks(16)
                    .map(|chunk| {
                        let mut mesh = Self::default();
                        mesh.cap(chunk, z, up);
                        mesh
                    })
                    .collect();
                for mesh in &meshes {
                    self.append(mesh);
                }
                continue;
            }
            let Some(outline) = polygon.first() else {
                continue;
            };
            if outline.len() < 3 {
                continue;
            }
            if polygon.len() == 1 && is_convex(outline) {
                self.fan(outline, z, up);
                continue;
            }
            let mut coords: Vec<[f64; 2]> = outline.iter().map(|p| [mm(p.x), mm(p.y)]).collect();
            let mut holes: Vec<u32> = Vec::new();
            for hole in polygon.iter().skip(1).filter(|h| h.len() >= 3) {
                holes.push(u32::try_from(coords.len()).unwrap_or(u32::MAX));
                coords.extend(hole.iter().map(|p| [mm(p.x), mm(p.y)]));
            }
            let mut triangles: Vec<u32> = Vec::new();
            earcut::Earcut::new().earcut_slice(&coords, &holes, &mut triangles);
            let base = u32::try_from(self.positions.len()).unwrap_or(u32::MAX);
            for c in &coords {
                self.vertex([c[0], c[1], z], normal);
            }
            for tri in triangles.chunks_exact(3) {
                let (a, b, c) = (
                    coords[tri[0] as usize],
                    coords[tri[1] as usize],
                    coords[tri[2] as usize],
                );
                let ccw = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) > 0.0;
                // Counter-clockwise seen from the side the face looks to.
                if ccw == up {
                    self.indices
                        .extend([base + tri[0], base + tri[1], base + tri[2]]);
                } else {
                    self.indices
                        .extend([base + tri[0], base + tri[2], base + tri[1]]);
                }
            }
        }
    }

    /// A convex ring as a triangle fan.
    fn fan(&mut self, ring: &[V], z: f64, up: bool) {
        let normal = [0.0, 0.0, if up { 1.0 } else { -1.0 }];
        let base = u32::try_from(self.positions.len()).unwrap_or(u32::MAX);
        for p in ring {
            self.vertex([mm(p.x), mm(p.y), z], normal);
        }
        let ccw = signed_area2(ring) > 0.0;
        let n = u32::try_from(ring.len()).unwrap_or(u32::MAX);
        for i in 1..n - 1 {
            if ccw == up {
                self.indices.extend([base, base + i, base + i + 1]);
            } else {
                self.indices.extend([base, base + i + 1, base + i]);
            }
        }
    }

    /// Vertical walls along every ring of `polygons` from `z0` to `z1` (mm),
    /// facing away from the material (outlines outwards, holes inwards).
    pub fn walls(&mut self, polygons: &[Vec<Vec<V>>], z0: f64, z1: f64) {
        for polygon in polygons {
            for (index, ring) in polygon.iter().enumerate() {
                if ring.len() < 3 {
                    continue;
                }
                // Outlines counter-clockwise, holes clockwise: the material
                // is then always on the left of each edge.
                let ccw = signed_area2(ring) > 0.0;
                let mut points: Vec<[f64; 2]> = ring.iter().map(|p| [mm(p.x), mm(p.y)]).collect();
                if ccw != (index == 0) {
                    points.reverse();
                }
                self.ring_wall(&points, z0, z1, true);
            }
        }
    }

    /// A wall along a closed ring of points (mm). The normals point to the
    /// right of the walking direction if `outward`, else to the left.
    pub fn ring_wall(&mut self, points: &[[f64; 2]], z0: f64, z1: f64, outward: bool) {
        let n = points.len();
        if n < 2 {
            return;
        }
        let edge_normal = |i: usize| -> Option<[f64; 2]> {
            let a = points[i % n];
            let b = points[(i + 1) % n];
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let length = dx.hypot(dy);
            (length > 1e-9).then(|| {
                if outward {
                    [dy / length, -dx / length]
                } else {
                    [-dy / length, dx / length]
                }
            })
        };
        let normals: Vec<Option<[f64; 2]>> = (0..n).map(edge_normal).collect();
        let blend = |own: [f64; 2], other: Option<[f64; 2]>| -> [f64; 2] {
            match other {
                Some(o) if own[0] * o[0] + own[1] * o[1] > SMOOTH_COS => {
                    let (x, y) = (own[0] + o[0], own[1] + o[1]);
                    let l = x.hypot(y).max(1e-12);
                    [x / l, y / l]
                }
                _ => own,
            }
        };
        for i in 0..n {
            let Some(own) = normals[i] else {
                continue;
            };
            let prev = (0..n)
                .map(|k| normals[(i + n - 1 - k) % n])
                .find(Option::is_some)
                .flatten();
            let next = (1..=n)
                .map(|k| normals[(i + k) % n])
                .find(Option::is_some)
                .flatten();
            let na = blend(own, prev);
            let nb = blend(own, next);
            let a = points[i];
            let b = points[(i + 1) % n];
            let v0 = self.vertex([a[0], a[1], z0], [na[0], na[1], 0.0]);
            let v1 = self.vertex([b[0], b[1], z0], [nb[0], nb[1], 0.0]);
            let v2 = self.vertex([b[0], b[1], z1], [nb[0], nb[1], 0.0]);
            let v3 = self.vertex([a[0], a[1], z1], [na[0], na[1], 0.0]);
            // Counter-clockwise seen from the normal's side.
            if outward {
                self.indices.extend([v0, v1, v2, v0, v2, v3]);
            } else {
                self.indices.extend([v0, v2, v1, v0, v3, v2]);
            }
        }
    }

    /// An open tube (plated barrel) around a slot or round hole: the core
    /// from `a` to `b` (mm; equal for round holes) with radius `r`, from
    /// `z0` to `z1`, facing inwards.
    pub fn tube(&mut self, a: [f64; 2], b: [f64; 2], r: f64, z0: f64, z1: f64, segments: usize) {
        let ring = stadium_ring(a, b, r, segments);
        // A counter-clockwise ring with the normals to the left points into
        // the tube.
        self.ring_wall(&ring, z0, z1, false);
    }

    /// The axis-aligned bounds `[x0, y0, z0, x1, y1, z1]`, `None` if empty.
    #[must_use]
    pub fn bounds(&self) -> Option<[f64; 6]> {
        let mut out = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
        for p in &self.positions {
            for axis in 0..3 {
                out[axis] = out[axis].min(f64::from(p[axis]));
                out[axis + 3] = out[axis + 3].max(f64::from(p[axis]));
            }
        }
        (!self.positions.is_empty()).then_some(out)
    }
}

/// Polygons with more holes than this are triangulated in tiles.
const MAX_HOLES: usize = 48;

/// Cuts a polygon with many holes into tiles with few holes each.
fn tiles(polygon: &[Vec<V>]) -> Vec<Vec<Vec<V>>> {
    let outline = &polygon[0];
    let holes = &polygon[1..];
    let bbox = BBox::of_points(outline);
    let count = holes.len().div_ceil(MAX_HOLES / 2).max(2);
    let (w, h) = (bbox.max.x - bbox.min.x, bbox.max.y - bbox.min.y);
    // Square-ish tiles: columns × rows ≈ count.
    let columns = ((count as f64 * w / h.max(1.0)).sqrt().ceil() as usize).max(1);
    let rows = count.div_ceil(columns).max(1);
    let (tw, th) = (w / columns as f64, h / rows as f64);
    let hole_boxes: Vec<BBox> = holes.iter().map(|ring| BBox::of_points(ring)).collect();
    (0..columns * rows)
        .into_par_iter()
        .flat_map_iter(|index| {
            let (column, row) = (index % columns, index / columns);
            let x0 = bbox.min.x + tw * column as f64;
            let y0 = bbox.min.y + th * row as f64;
            // The last tiles reach the box edge exactly.
            let x1 = if column + 1 == columns {
                bbox.max.x
            } else {
                x0 + tw
            };
            let y1 = if row + 1 == rows { bbox.max.y } else { y0 + th };
            let tile = BBox::of(V::new(x0, y0)).with(V::new(x1, y1));
            let rect = vec![vec![
                V::new(x0, y0),
                V::new(x1, y0),
                V::new(x1, y1),
                V::new(x0, y1),
            ]];
            let part = poly::intersection(&[vec![outline.clone()]], &[rect]);
            let near: Vec<Vec<Vec<V>>> = holes
                .iter()
                .zip(&hole_boxes)
                .filter(|(_, b)| b.intersects(tile))
                .map(|(ring, _)| vec![ring.clone()])
                .collect();
            if near.is_empty() {
                part
            } else {
                poly::difference(&part, &near)
            }
        })
        .collect()
}

/// `true` if the ring turns one way only (collinear points allowed).
#[must_use]
pub fn is_convex(ring: &[V]) -> bool {
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut turn: Option<bool> = None;
    for i in 0..n {
        let (a, b, c) = (ring[i], ring[(i + 1) % n], ring[(i + 2) % n]);
        let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
        if cross.abs() < 1e-6 {
            continue;
        }
        let left = cross > 0.0;
        match turn {
            None => turn = Some(left),
            Some(own) if own != left => return false,
            Some(_) => {}
        }
    }
    turn.is_some()
}

/// A counter-clockwise ring around a stadium (a circle if `a` = `b`).
#[must_use]
pub fn stadium_ring(a: [f64; 2], b: [f64; 2], r: f64, segments: usize) -> Vec<[f64; 2]> {
    use std::f64::consts::PI;
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy);
    let segments = segments.max(8);
    if length < 1e-9 {
        return (0..segments)
            .map(|i| {
                let t = 2.0 * PI * i as f64 / segments as f64;
                [a[0] + r * t.cos(), a[1] + r * t.sin()]
            })
            .collect();
    }
    let base = dy.atan2(dx);
    let half = segments / 2;
    let mut out = Vec::with_capacity(2 * half + 2);
    for (center, start) in [(b, base - PI / 2.0), (a, base + PI / 2.0)] {
        for i in 0..=half {
            let t = start + PI * i as f64 / half as f64;
            out.push([center[0] + r * t.cos(), center[1] + r * t.sin()]);
        }
    }
    out
}

/// Segments of a full circle of radius `r` (mm) with a chord error below
/// `tolerance` (mm).
#[must_use]
pub fn circle_segments(r: f64, tolerance: f64) -> usize {
    if r <= tolerance {
        return 8;
    }
    let step = 2.0 * (1.0 - tolerance / r).clamp(-1.0, 1.0).acos();
    ((2.0 * std::f64::consts::PI / step.max(1e-3)).ceil() as usize).clamp(8, 128)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, size: f64) -> Vec<V> {
        vec![
            V::new(x, y),
            V::new(x + size, y),
            V::new(x + size, y + size),
            V::new(x, y + size),
        ]
    }

    fn signed_volume(mesh: &Mesh) -> f64 {
        mesh.indices
            .chunks_exact(3)
            .map(|t| {
                let p = |i: u32| mesh.positions[i as usize].map(f64::from);
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0
            })
            .sum()
    }

    #[test]
    fn a_plate_with_a_hole_is_closed_and_faces_out() {
        // 10 mm square with a 2 mm square hole, 1.6 mm thick.
        let mut hole = square(4e6, 4e6, 2e6);
        hole.reverse();
        let polygons = vec![vec![square(0.0, 0.0, 10e6), hole]];
        let mut mesh = Mesh::default();
        mesh.cap(&polygons, 0.0, false);
        mesh.cap(&polygons, 1.6, true);
        mesh.walls(&polygons, 0.0, 1.6);
        // The divergence theorem gives the volume only if every face points
        // outwards.
        let volume = signed_volume(&mesh);
        assert!((volume - (100.0 - 4.0) * 1.6).abs() < 1e-3, "{volume}");
        let bounds = mesh.bounds().unwrap();
        for (got, want) in bounds.iter().zip([0.0, 0.0, 0.0, 10.0, 10.0, 1.6]) {
            assert!((got - want).abs() < 1e-6, "{bounds:?}");
        }
    }

    #[test]
    fn plates_with_many_holes_are_tiled() {
        // A 100 × 50 mm plate with 20 × 10 holes of 1 mm.
        let mut polygon = vec![square(0.0, 0.0, 100e6)];
        polygon[0][1].y = 0.0;
        polygon[0] = vec![
            V::new(0.0, 0.0),
            V::new(100e6, 0.0),
            V::new(100e6, 50e6),
            V::new(0.0, 50e6),
        ];
        for i in 0..20 {
            for j in 0..10 {
                let mut hole = square(2e6 + 5e6 * f64::from(i), 2e6 + 5e6 * f64::from(j), 1e6);
                hole.reverse();
                polygon.push(hole);
            }
        }
        let polygons = vec![polygon];
        let mut mesh = Mesh::default();
        mesh.cap(&polygons, 0.0, false);
        mesh.cap(&polygons, 1.0, true);
        mesh.walls(&polygons, 0.0, 1.0);
        let volume = signed_volume(&mesh);
        assert!((volume - (5000.0 - 200.0)).abs() < 1e-2, "{volume}");
    }

    #[test]
    fn tubes_face_inwards() {
        let mut mesh = Mesh::default();
        mesh.tube([0.0, 0.0], [0.0, 0.0], 1.0, 0.0, 1.0, 32);
        // Every normal points to the axis.
        for (p, n) in mesh.positions.iter().zip(&mesh.normals) {
            assert!(p[0] * n[0] + p[1] * n[1] < 0.0);
        }
        // Inward-facing walls give a negative volume.
        assert!(signed_volume(&mesh) < 0.0);
        let ring = stadium_ring([0.0, 0.0], [2.0, 0.0], 0.5, 16);
        let area: f64 = (0..ring.len())
            .map(|i| {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>()
            / 2.0;
        assert!(area > 0.0, "counter-clockwise");
        assert!(
            (area - (2.0 + std::f64::consts::PI * 0.25)).abs() < 0.05,
            "{area}"
        );
        assert_eq!(circle_segments(0.001, 0.01), 8);
        assert!(circle_segments(10.0, 0.005) > 64);
    }
}
