//! Creepage: the shortest path between two copper shapes along the surface
//! of the board. It cannot cross the board edge, a cut-out, a milled slot or
//! a non-plated hole, so it goes around them and can be much longer than
//! the straight distance (the clearance through the air).
//!
//! The path is found on a visibility graph of the corners of the obstacles
//! near the two shapes: from the nearest point of the first shape to
//! obstacle corners, from corner to corner, and to the nearest point of the
//! second shape, each step a straight line that crosses no obstacle edge.

#![allow(
    clippy::many_single_char_names,
    reason = "geometry: points, vectors and the pair"
)]

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::geom::{BBox, Core, Shape, V, closest_on_segment};

/// An obstacle: a closed ring the path may not cross (counted from the
/// inside of the board: cut-outs and holes; the board outline itself).
#[derive(Debug, Clone)]
pub struct Obstacle {
    /// The corners, in order.
    pub ring: Vec<V>,
    /// The board outline: the path stays inside it (for the others, the
    /// path stays outside).
    pub outline: bool,
}

/// The nearest point of a shape's edge to `p` (or `p` itself inside it).
#[must_use]
pub fn nearest_on(shape: &Shape, p: V) -> V {
    let r = shape.radius;
    let grow = |q: V| {
        let d = p.sub(q);
        if d.len() <= r {
            p
        } else {
            q.add(d.unit().scale(r))
        }
    };
    match &shape.core {
        Core::Point(c) => grow(*c),
        Core::Segment(a, b) => grow(closest_on_segment(p, *a, *b)),
        Core::Poly(poly) => {
            if poly.contains(p) {
                return p;
            }
            let mut best = poly.outline()[0];
            let mut best_d = f64::INFINITY;
            for ring in poly.rings() {
                let n = ring.len();
                for i in 0..n {
                    let q = closest_on_segment(p, ring[i], ring[(i + 1) % n]);
                    let d = q.dist(p);
                    if d < best_d {
                        best_d = d;
                        best = q;
                    }
                }
            }
            grow(best)
        }
    }
}

/// `true` if the open segments `a`–`b` and `c`–`d` cross (touching at an
/// end or running along each other does not count).
fn crosses(a: V, b: V, c: V, d: V) -> bool {
    let d1 = b.sub(a).cross(c.sub(a));
    let d2 = b.sub(a).cross(d.sub(a));
    let d3 = d.sub(c).cross(a.sub(c));
    let d4 = d.sub(c).cross(b.sub(c));
    let eps = 1e-3;
    ((d1 > eps && d2 < -eps) || (d1 < -eps && d2 > eps))
        && ((d3 > eps && d4 < -eps) || (d3 < -eps && d4 > eps))
}

/// `true` if `p` lies inside the ring (even-odd rule).
fn inside(ring: &[V], p: V) -> bool {
    let mut odd = false;
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            odd = !odd;
        }
    }
    odd
}

/// `true` if the straight step from `a` to `b` stays on the surface.
fn clear(a: V, b: V, obstacles: &[Obstacle]) -> bool {
    let mid = a.add(b).scale(0.5);
    for obstacle in obstacles {
        let ring = &obstacle.ring;
        let n = ring.len();
        for i in 0..n {
            if crosses(a, b, ring[i], ring[(i + 1) % n]) {
                return false;
            }
        }
        // A chord between two corners of one obstacle crosses none of its
        // edges: its middle tells.
        if inside(ring, mid) != obstacle.outline {
            return false;
        }
    }
    true
}

#[derive(PartialEq)]
struct Node(f64, usize);

impl Eq for Node {}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.total_cmp(&self.0)
    }
}

/// The creepage distance between two shapes, if it is below `limit`
/// (`None`: at least `limit`). `straight` is their distance through the
/// air when it is below `limit`.
#[must_use]
pub fn creepage(
    a: &Shape,
    b: &Shape,
    straight: f64,
    obstacles: &[Obstacle],
    limit: f64,
) -> Option<f64> {
    // The nearest points of the two shapes (alternating projection).
    let mut pa = nearest_on(a, b.anchor());
    let mut pb = nearest_on(b, pa);
    for _ in 0..4 {
        pa = nearest_on(a, pb);
        pb = nearest_on(b, pa);
    }
    let area = a.bbox().union(b.bbox()).grow(limit);
    let near: Vec<Obstacle> = obstacles
        .iter()
        .filter(|o| o.outline || BBox::of_points(&o.ring).intersects(area))
        .cloned()
        .collect();
    if clear(pa, pb, &near) {
        return (straight < limit).then_some(straight.max(0.0));
    }
    // Corners of the obstacles near the two shapes.
    let corners: Vec<V> = near
        .iter()
        .flat_map(|o| o.ring.iter().copied())
        .filter(|p| area.contains(BBox::of(*p)))
        .collect();
    let n = corners.len();
    let mut cost = vec![f64::INFINITY; n];
    let mut heap = BinaryHeap::new();
    for (i, corner) in corners.iter().enumerate() {
        let from = nearest_on(a, *corner);
        let d = from.dist(*corner);
        if d < limit && clear(from, *corner, &near) {
            cost[i] = d;
            heap.push(Node(d, i));
        }
    }
    let mut best = limit;
    while let Some(Node(d, i)) = heap.pop() {
        if d > cost[i] || d >= best {
            continue;
        }
        let here = corners[i];
        let to = nearest_on(b, here);
        let total = d + to.dist(here);
        if total < best && clear(here, to, &near) {
            best = total;
        }
        for (j, next) in corners.iter().enumerate() {
            let step = d + here.dist(*next);
            if j != i && step < cost[j] && step < best && clear(here, *next, &near) {
                cost[j] = step;
                heap.push(Node(step, j));
            }
        }
    }
    (best < limit).then_some(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MM: f64 = 1e6;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<V> {
        vec![
            V::new(x0, y0),
            V::new(x1, y0),
            V::new(x1, y1),
            V::new(x0, y1),
        ]
    }

    #[test]
    fn a_slot_lengthens_the_path() {
        // Two pads 4 mm apart (edge to edge) across a slot 1 mm wide and
        // 10 mm long: the path goes around the end of the slot.
        let a = Shape::disc(V::new(0.0, 0.0), 1.0 * MM);
        let b = Shape::disc(V::new(6.0 * MM, 0.0), 1.0 * MM);
        let board = Obstacle {
            ring: rect(-20.0 * MM, -20.0 * MM, 20.0 * MM, 20.0 * MM),
            outline: true,
        };
        let straight = 4.0 * MM;
        // Without the slot the creepage is the straight distance.
        assert_eq!(
            creepage(&a, &b, straight, std::slice::from_ref(&board), 8.0 * MM),
            Some(straight)
        );
        let slot = Obstacle {
            ring: rect(2.5 * MM, -5.0 * MM, 3.5 * MM, 5.0 * MM),
            outline: false,
        };
        let path = creepage(&a, &b, straight, &[board.clone(), slot.clone()], 20.0 * MM).unwrap();
        // From each pad to a corner of the slot's end, and over its 1 mm
        // wide end: 2 × (√(2.5² + 5²) − 1) + 1 mm.
        let expected = 2.0 * (2.5_f64.hypot(5.0) - 1.0) * MM + 1.0 * MM;
        assert!((path - expected).abs() < 0.3 * MM, "{path} vs {expected}");
        // Asked for less than the path: fine.
        assert_eq!(creepage(&a, &b, straight, &[board, slot], 8.0 * MM), None);
    }

    #[test]
    fn far_shapes_are_fine() {
        let a = Shape::disc(V::new(0.0, 0.0), 1.0 * MM);
        let b = Shape::stadium(
            V::new(10.0 * MM, -2.0 * MM),
            V::new(10.0 * MM, 2.0 * MM),
            0.5 * MM,
        );
        assert_eq!(creepage(&a, &b, 8.5 * MM, &[], 5.0 * MM), None);
        let p = nearest_on(&b, V::new(0.0, 0.0));
        assert!((p.x - 9.5 * MM).abs() < 1.0 && p.y.abs() < 1.0);
    }
}
