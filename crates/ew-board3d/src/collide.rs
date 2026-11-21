//! Overlaps and distances between triangle meshes: the geometry of the
//! enclosure check (M17).
//!
//! A [`Solid`] holds the triangles of one body in board coordinates (mm)
//! with a search tree. [`meet`] tells how another body meets it: an
//! **overlap** (an edge of one body passes through a face of the other, or
//! one body lies inside the other) or a **gap** closer than a limit with the
//! two closest points (a gap of 0 is touching, as a board resting on the
//! bosses of its enclosure).
//!
//! Touching is told from overlapping with a tolerance of [`TOUCH`]: faces
//! that lie on each other count as touching, edges must cross a face by more
//! than that to count as an overlap.

use rstar::{AABB, RTree, RTreeObject};

use crate::mesh::Mesh;
use crate::place::Matrix;

/// A point or vector (mm).
pub type P = [f64; 3];
/// A triangle.
pub type Tri = [P; 3];

/// Distances below this count as touching (mm); edges must cross a face by
/// more than this to count as an overlap.
pub const TOUCH: f64 = 0.001;

/// An axis-aligned box (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Lower corner.
    pub min: P,
    /// Upper corner.
    pub max: P,
}

impl Bounds {
    /// No points.
    pub const EMPTY: Self = Self {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };

    /// The box of some points.
    #[must_use]
    pub fn of(points: &[P]) -> Self {
        let mut out = Self::EMPTY;
        for p in points {
            out.add(*p);
        }
        out
    }

    /// Adds a point.
    pub fn add(&mut self, p: P) {
        for i in 0..3 {
            self.min[i] = self.min[i].min(p[i]);
            self.max[i] = self.max[i].max(p[i]);
        }
    }

    /// Both boxes.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        let mut out = *self;
        out.add(other.min);
        out.add(other.max);
        out
    }

    /// `true` without points.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        (0..3).any(|i| self.min[i] > self.max[i])
    }

    /// Grown by `d` on every side.
    #[must_use]
    pub fn grow(&self, d: f64) -> Self {
        Self {
            min: [self.min[0] - d, self.min[1] - d, self.min[2] - d],
            max: [self.max[0] + d, self.max[1] + d, self.max[2] + d],
        }
    }

    /// `true` if the boxes share a point.
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        (0..3).all(|i| self.min[i] <= other.max[i] && other.min[i] <= self.max[i])
    }

    /// `true` if `other` lies inside.
    #[must_use]
    pub fn contains(&self, other: &Self) -> bool {
        (0..3).all(|i| self.min[i] <= other.min[i] && other.max[i] <= self.max[i])
    }

    /// The centre.
    #[must_use]
    pub fn center(&self) -> P {
        [
            f64::midpoint(self.min[0], self.max[0]),
            f64::midpoint(self.min[1], self.max[1]),
            f64::midpoint(self.min[2], self.max[2]),
        ]
    }

    fn envelope(&self) -> AABB<P> {
        AABB::from_corners(self.min, self.max)
    }

    /// The box of a mesh's points under a matrix.
    #[must_use]
    pub fn of_mesh(mesh: &Mesh, matrix: &Matrix) -> Self {
        let mut local = Self::EMPTY;
        for p in &mesh.positions {
            local.add([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]);
        }
        if local.is_empty() {
            return local;
        }
        // The eight corners: a box around the turned box.
        let mut out = Self::EMPTY;
        for i in 0..8 {
            let corner = [
                if i & 1 == 0 {
                    local.min[0]
                } else {
                    local.max[0]
                },
                if i & 2 == 0 {
                    local.min[1]
                } else {
                    local.max[1]
                },
                if i & 4 == 0 {
                    local.min[2]
                } else {
                    local.max[2]
                },
            ];
            out.add(matrix.apply(corner));
        }
        out
    }
}

fn tri_bounds(t: &Tri) -> Bounds {
    Bounds::of(t)
}

struct Entry {
    index: u32,
    envelope: AABB<P>,
}

impl RTreeObject for Entry {
    type Envelope = AABB<P>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// The triangles of one body in board coordinates with a search tree.
pub struct Solid {
    /// Triangles.
    pub triangles: Vec<Tri>,
    /// Their box.
    pub bounds: Bounds,
    tree: RTree<Entry>,
}

impl std::fmt::Debug for Solid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Solid")
            .field("triangles", &self.triangles.len())
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

impl Solid {
    /// A body of triangles.
    #[must_use]
    pub fn new(triangles: Vec<Tri>) -> Self {
        let mut bounds = Bounds::EMPTY;
        let entries: Vec<Entry> = triangles
            .iter()
            .enumerate()
            .map(|(index, t)| {
                let b = tri_bounds(t);
                bounds = bounds.union(&b);
                Entry {
                    index: u32::try_from(index).unwrap_or(u32::MAX),
                    envelope: b.envelope(),
                }
            })
            .collect();
        Self {
            triangles,
            bounds,
            tree: RTree::bulk_load(entries),
        }
    }

    /// A mesh placed by a matrix.
    #[must_use]
    pub fn of_mesh(mesh: &Mesh, matrix: &Matrix) -> Self {
        Self::new(triangles(mesh, matrix).collect())
    }

    /// `true` without triangles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// The triangles whose boxes meet a box.
    fn near<'a>(&'a self, b: &Bounds) -> impl Iterator<Item = &'a Tri> + 'a {
        self.tree
            .locate_in_envelope_intersecting(b.envelope())
            .map(|e| &self.triangles[e.index as usize])
    }

    /// `true` if any triangle comes into a box.
    #[must_use]
    pub fn reaches(&self, b: &Bounds) -> bool {
        self.near(b).next().is_some()
    }

    /// The distance from a point to the surface (up to `limit`).
    fn surface_distance(&self, p: P, limit: f64) -> f64 {
        let around = Bounds { min: p, max: p }.grow(limit);
        self.near(&around)
            .map(|t| length(sub(p, closest_on_triangle(p, t))))
            .fold(limit, f64::min)
    }

    /// The distance along a ray (`dir` of unit length) to the first face,
    /// up to `max`.
    #[must_use]
    pub fn ray(&self, p: P, dir: P, max: f64) -> Option<f64> {
        let end = add(p, scale(dir, max));
        let span = Bounds::of(&[p, end]);
        self.near(&span)
            .filter_map(|t| ray_distance(p, dir, t))
            .filter(|d| *d <= max)
            .reduce(f64::min)
    }

    /// `true` if a point lies inside the closed surface (the number of
    /// faces a ray from it crosses is odd).
    #[must_use]
    pub fn contains(&self, p: P) -> bool {
        let b = &self.bounds;
        if !(0..3).all(|i| b.min[i] <= p[i] && p[i] <= b.max[i]) {
            return false;
        }
        // A slightly tilted ray, so it rarely meets an edge exactly.
        let dir = unit([1.0, 0.013_7, 0.007_31]);
        let reach = b.max[0] - p[0] + 1.0;
        let end = add(p, scale(dir, reach / dir[0]));
        let ray = Bounds::of(&[p, end]);
        self.near(&ray).filter(|t| ray_hits(p, dir, t)).count() % 2 == 1
    }
}

/// The triangles of a mesh under a matrix.
pub fn triangles<'a>(mesh: &'a Mesh, matrix: &'a Matrix) -> impl Iterator<Item = Tri> + 'a {
    let point = move |i: u32| {
        let q = mesh.positions[i as usize];
        matrix.apply([f64::from(q[0]), f64::from(q[1]), f64::from(q[2])])
    };
    mesh.indices
        .chunks_exact(3)
        .map(move |t| [point(t[0]), point(t[1]), point(t[2])])
}

/// How two bodies meet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Meeting {
    /// They overlap: an edge of one passes through a face of the other, or
    /// one lies inside the other. `at` is a point of the overlap.
    Overlap {
        /// A point in both bodies.
        at: P,
    },
    /// Their surfaces come closer than the limit (0: touching).
    Gap {
        /// Distance (mm).
        distance: f64,
        /// The closest point on the first body.
        a: P,
        /// The closest point on the second body.
        b: P,
    },
}

/// How a body (its triangles in board coordinates and their box) meets a
/// solid, up to a distance `limit`; `None` if they are farther apart.
pub fn meet<I>(a: I, a_bounds: &Bounds, b: &Solid, limit: f64) -> Option<Meeting>
where
    I: IntoIterator<Item = Tri>,
{
    if a_bounds.is_empty() || b.is_empty() || !a_bounds.grow(limit).intersects(&b.bounds) {
        return None;
    }
    let region = b.bounds.grow(limit);
    let mut best: Option<(f64, P, P)> = None;
    let mut probes: Vec<P> = Vec::new();
    for t in a {
        if probes.len() < 3 {
            probes.push(t[0]);
        }
        let tb = tri_bounds(&t);
        if !tb.intersects(&region) {
            continue;
        }
        let reach = best.map_or(limit, |(d, _, _)| d.max(TOUCH));
        for q in b.near(&tb.grow(reach)) {
            if let Some(at) = crossing(&t, q) {
                return Some(Meeting::Overlap { at });
            }
            let (d, pa, pb) = triangle_distance(&t, q);
            if d <= limit && best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, pa, pb));
            }
        }
    }
    // A body inside the other touches no face of it.
    if a_bounds.intersects(&b.bounds) {
        for p in probes {
            if b.surface_distance(p, 2.0 * TOUCH) >= 2.0 * TOUCH && b.contains(p) {
                return Some(Meeting::Overlap { at: p });
            }
        }
    }
    best.map(|(distance, a, b)| Meeting::Gap { distance, a, b })
}

// ---------------------------------------------------------------------------
// Vector helpers.

fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(a: P) -> f64 {
    dot(a, a).sqrt()
}

fn unit(a: P) -> P {
    let l = length(a);
    if l == 0.0 { a } else { scale(a, 1.0 / l) }
}

// ---------------------------------------------------------------------------
// Triangle geometry (C. Ericson, Real-Time Collision Detection, 5.1).

/// The point of a triangle closest to `p`.
#[must_use]
pub fn closest_on_triangle(p: P, t: &Tri) -> P {
    let [a, b, c] = *t;
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return add(a, scale(ab, v));
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return add(a, scale(ac, w));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return add(b, scale(sub(c, b), w));
    }
    let denom = va + vb + vc;
    if denom.abs() < f64::MIN_POSITIVE {
        return a;
    }
    let v = vb / denom;
    let w = vc / denom;
    add(a, add(scale(ab, v), scale(ac, w)))
}

/// The closest points of two segments.
#[must_use]
pub fn closest_on_segments(p1: P, q1: P, p2: P, q2: P) -> (P, P) {
    let d1 = sub(q1, p1);
    let d2 = sub(q2, p2);
    let r = sub(p1, p2);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);
    let eps = 1e-18;
    let (s, t);
    if a <= eps && e <= eps {
        return (p1, p2);
    }
    if a <= eps {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = dot(d1, r);
        if e <= eps {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = dot(d1, d2);
            let denom = a * e - b * b;
            let mut s0 = if denom > eps {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    (add(p1, scale(d1, s)), add(p2, scale(d2, t)))
}

/// Where an edge of one triangle passes through the other by more than
/// [`TOUCH`] on both sides.
fn crossing(t: &Tri, q: &Tri) -> Option<P> {
    for (u, v) in [(t, q), (q, t)] {
        let raw = cross(sub(v[1], v[0]), sub(v[2], v[0]));
        if dot(raw, raw) < 1e-24 {
            continue;
        }
        let n = unit(raw);
        for i in 0..3 {
            let s0 = u[i];
            let s1 = u[(i + 1) % 3];
            let d0 = dot(sub(s0, v[0]), n);
            let d1 = dot(sub(s1, v[0]), n);
            if !((d0 > TOUCH && d1 < -TOUCH) || (d0 < -TOUCH && d1 > TOUCH)) {
                continue;
            }
            let x = add(s0, scale(sub(s1, s0), d0 / (d0 - d1)));
            if inside(x, v, n) {
                return Some(x);
            }
        }
    }
    None
}

/// `true` if a point of a triangle's plane lies inside it (not on its
/// edges).
fn inside(x: P, t: &Tri, n: P) -> bool {
    (0..3).all(|i| {
        let a = t[i];
        let b = t[(i + 1) % 3];
        let edge = sub(b, a);
        let len = length(edge);
        len > 0.0 && dot(cross(edge, sub(x, a)), n) / len > 1e-9
    })
}

/// The distance of two triangles that do not cross, with the closest
/// points.
#[must_use]
pub fn triangle_distance(t: &Tri, q: &Tri) -> (f64, P, P) {
    let mut best = (f64::INFINITY, t[0], q[0]);
    let mut take = |a: P, b: P| {
        let d = length(sub(a, b));
        if d < best.0 {
            best = (d, a, b);
        }
    };
    for p in t {
        take(*p, closest_on_triangle(*p, q));
    }
    for p in q {
        take(closest_on_triangle(*p, t), *p);
    }
    for i in 0..3 {
        for j in 0..3 {
            let (a, b) = closest_on_segments(t[i], t[(i + 1) % 3], q[j], q[(j + 1) % 3]);
            take(a, b);
        }
    }
    best
}

/// Where a ray from `p` along `dir` meets a triangle (the distance in
/// units of `dir`).
fn ray_distance(p: P, dir: P, t: &Tri) -> Option<f64> {
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let h = cross(dir, e2);
    let a = dot(e1, h);
    if a.abs() < 1e-14 {
        return None;
    }
    let f = 1.0 / a;
    let s = sub(p, t[0]);
    let u = f * dot(s, h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = f * dot(dir, q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let d = f * dot(e2, q);
    (d > 1e-9).then_some(d)
}

/// `true` if a ray from `p` along `dir` passes through a triangle.
fn ray_hits(p: P, dir: P, t: &Tri) -> bool {
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let h = cross(dir, e2);
    let a = dot(e1, h);
    if a.abs() < 1e-14 {
        return false;
    }
    let f = 1.0 / a;
    let s = sub(p, t[0]);
    let u = f * dot(s, h);
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = cross(s, e1);
    let v = f * dot(dir, q);
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    f * dot(e2, q) > 1e-9
}

/// A box as twelve triangles facing out.
#[must_use]
pub fn box_triangles(min: P, max: P, matrix: &Matrix) -> Vec<Tri> {
    let c = |i: usize| {
        matrix.apply([
            if i & 1 == 0 { min[0] } else { max[0] },
            if i & 2 == 0 { min[1] } else { max[1] },
            if i & 4 == 0 { min[2] } else { max[2] },
        ])
    };
    // Faces as corner indices, counter-clockwise seen from outside.
    let faces = [
        [0, 2, 3, 1], // bottom (z min)
        [4, 5, 7, 6], // top
        [0, 1, 5, 4], // y min
        [2, 6, 7, 3], // y max
        [0, 4, 6, 2], // x min
        [1, 3, 7, 5], // x max
    ];
    let mut out = Vec::with_capacity(12);
    for f in faces {
        out.push([c(f[0]), c(f[1]), c(f[2])]);
        out.push([c(f[0]), c(f[2]), c(f[3])]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(min: P, size: f64) -> Solid {
        Solid::new(box_triangles(
            min,
            [min[0] + size, min[1] + size, min[2] + size],
            &Matrix::IDENTITY,
        ))
    }

    fn of(s: &Solid) -> (Vec<Tri>, Bounds) {
        (s.triangles.clone(), s.bounds)
    }

    #[test]
    fn tells_gaps_touching_and_overlaps() {
        let a = cube([0.0, 0.0, 0.0], 10.0);
        // 0.3 mm apart along x.
        let b = cube([10.3, 2.0, 2.0], 5.0);
        let (tris, bounds) = of(&b);
        match meet(tris, &bounds, &a, 0.5) {
            Some(Meeting::Gap {
                distance,
                a: pa,
                b: pb,
            }) => {
                assert!((distance - 0.3).abs() < 1e-9, "{distance}");
                assert!((pa[0] - 10.3).abs() < 1e-9 && (pb[0] - 10.0).abs() < 1e-9);
            }
            other => panic!("{other:?}"),
        }
        // Farther than the limit.
        assert_eq!(meet(tris_of(&b), &b.bounds, &a, 0.2), None);
        // Resting on the face: touching, not overlapping.
        let c = cube([2.0, 2.0, 10.0], 5.0);
        match meet(tris_of(&c), &c.bounds, &a, 0.5) {
            Some(Meeting::Gap { distance, .. }) => assert!(distance < TOUCH),
            other => panic!("{other:?}"),
        }
        // Cutting in by 1 mm.
        let d = cube([9.0, 2.0, 2.0], 5.0);
        assert!(matches!(
            meet(tris_of(&d), &d.bounds, &a, 0.5),
            Some(Meeting::Overlap { .. })
        ));
        // Inside without touching a face.
        let e = cube([3.0, 3.0, 3.0], 2.0);
        assert!(matches!(
            meet(tris_of(&e), &e.bounds, &a, 0.5),
            Some(Meeting::Overlap { .. })
        ));
        assert!(a.contains([5.0, 5.0, 5.0]));
        assert!(!a.contains([15.0, 5.0, 5.0]));
    }

    fn tris_of(s: &Solid) -> Vec<Tri> {
        s.triangles.clone()
    }

    #[test]
    fn hollow_bodies_do_not_contain_their_cavity() {
        // A closed box with walls of 1 mm: outer 20, inner 18 (two
        // surfaces, the inner one facing in).
        let mut tris = box_triangles([0.0; 3], [20.0; 3], &Matrix::IDENTITY);
        let inner = box_triangles([1.0; 3], [19.0; 3], &Matrix::IDENTITY);
        tris.extend(inner.into_iter().map(|[a, b, c]| [a, c, b]));
        let shell = Solid::new(tris);
        assert!(!shell.contains([10.0, 10.0, 10.0]));
        assert!(shell.contains([0.5, 10.0, 10.0]));
        let part = cube([5.0, 5.0, 5.0], 2.0);
        assert_eq!(meet(tris_of(&part), &part.bounds, &shell, 0.5), None);
        let near_wall = cube([16.8, 5.0, 5.0], 2.0);
        match meet(tris_of(&near_wall), &near_wall.bounds, &shell, 0.5) {
            Some(Meeting::Gap { distance, .. }) => assert!((distance - 0.2).abs() < 1e-9),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn segment_and_point_distances() {
        let (a, b) =
            closest_on_segments([0.0; 3], [1.0, 0.0, 0.0], [0.5, 1.0, -1.0], [0.5, 1.0, 1.0]);
        assert_eq!(a, [0.5, 0.0, 0.0]);
        assert_eq!(b, [0.5, 1.0, 0.0]);
        let t = [[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let c = closest_on_triangle([0.2, 0.2, 3.0], &t);
        assert!(length(sub(c, [0.2, 0.2, 0.0])) < 1e-12, "{c:?}");
        assert_eq!(closest_on_triangle([-1.0, -1.0, 0.0], &t), [0.0; 3]);
    }
}
