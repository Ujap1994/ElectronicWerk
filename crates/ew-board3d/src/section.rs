//! Cross-sections of triangle meshes with a horizontal plane: the helpers
//! of the enclosure check (M17) read the board outline, the bosses and the
//! material at the board plane from them.
//!
//! The loops come out oriented like the solid: outlines counter-clockwise,
//! holes clockwise (seen from above), when the mesh's triangles face
//! outwards.

use std::collections::HashMap;

use crate::collide::{P, Tri};

/// A closed loop in the plane (mm).
pub type Loop = Vec<[f64; 2]>;

/// The point where the plane `z` cuts the edge `a`–`b`, computed the same
/// way from both triangles that share the edge.
fn cut(a: P, b: P, z: f64) -> [f64; 2] {
    let (a, b) = if (a[0], a[1], a[2]) <= (b[0], b[1], b[2]) {
        (a, b)
    } else {
        (b, a)
    };
    let t = (z - a[2]) / (b[2] - a[2]);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn key(p: [f64; 2]) -> (i64, i64) {
    #[allow(clippy::cast_possible_truncation, reason = "millimetres, rounded")]
    ((p[0] * 1e5).round() as i64, (p[1] * 1e5).round() as i64)
}

/// The cross-section of triangles with the plane `z`: closed loops,
/// outlines counter-clockwise and holes clockwise. Open chains (meshes
/// that are not closed) are left out.
#[must_use]
pub fn slice<'a, I>(triangles: I, z: f64) -> Vec<Loop>
where
    I: IntoIterator<Item = &'a Tri>,
{
    let mut segments: Vec<([f64; 2], [f64; 2])> = Vec::new();
    for t in triangles {
        // Points on the plane count as above it, so a plane through a
        // vertex still cuts each edge once.
        let above = t.map(|p| p[2] >= z);
        let ups = above.iter().filter(|a| **a).count();
        if ups == 0 || ups == 3 {
            continue;
        }
        let mut points = Vec::with_capacity(2);
        for i in 0..3 {
            let j = (i + 1) % 3;
            if above[i] != above[j] {
                points.push(cut(t[i], t[j], z));
            }
        }
        let [a, b] = [points[0], points[1]];
        // Walk with the solid on the left: along the triangle's normal
        // turned by a quarter.
        let e1 = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
        let e2 = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2]];
        let along = [-n[1], n[0]];
        let d = [b[0] - a[0], b[1] - a[1]];
        if key(a) == key(b) {
            continue;
        }
        if d[0] * along[0] + d[1] * along[1] >= 0.0 {
            segments.push((a, b));
        } else {
            segments.push((b, a));
        }
    }
    chain(&segments)
}

/// Joins segments into closed loops.
fn chain(segments: &[([f64; 2], [f64; 2])]) -> Vec<Loop> {
    let mut starts: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, (a, _)) in segments.iter().enumerate() {
        starts.entry(key(*a)).or_default().push(i);
    }
    let mut used = vec![false; segments.len()];
    let mut loops = Vec::new();
    for first in 0..segments.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let origin = key(segments[first].0);
        let mut ring = vec![segments[first].0];
        let mut end = segments[first].1;
        let mut closed = false;
        while ring.len() <= segments.len() {
            if key(end) == origin {
                closed = true;
                break;
            }
            let next = starts
                .get(&key(end))
                .and_then(|list| list.iter().copied().find(|i| !used[*i]));
            let Some(next) = next else {
                break;
            };
            used[next] = true;
            ring.push(segments[next].0);
            end = segments[next].1;
        }
        if closed && ring.len() >= 3 {
            loops.push(ring);
        }
    }
    loops
}

/// Twice the signed area of a loop (positive: counter-clockwise).
#[must_use]
pub fn area2(ring: &[[f64; 2]]) -> f64 {
    let mut sum = 0.0;
    for (i, a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        sum += a[0] * b[1] - b[0] * a[1];
    }
    sum
}

/// `true` if a point lies inside a loop (even-odd).
#[must_use]
pub fn contains(ring: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    let mut j = ring.len().wrapping_sub(1);
    for (i, a) in ring.iter().enumerate() {
        let b = ring[j];
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0];
            if p[0] < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// The centroid of a loop.
#[must_use]
pub fn centroid(ring: &[[f64; 2]]) -> [f64; 2] {
    let a = area2(ring);
    if a.abs() < 1e-12 {
        let n = ring.len().max(1) as f64;
        return [
            ring.iter().map(|p| p[0]).sum::<f64>() / n,
            ring.iter().map(|p| p[1]).sum::<f64>() / n,
        ];
    }
    let (mut cx, mut cy) = (0.0, 0.0);
    for (i, p) in ring.iter().enumerate() {
        let q = ring[(i + 1) % ring.len()];
        let f = p[0] * q[1] - q[0] * p[1];
        cx += (p[0] + q[0]) * f;
        cy += (p[1] + q[1]) * f;
    }
    [cx / (3.0 * a), cy / (3.0 * a)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collide::box_triangles;
    use crate::place::Matrix;

    #[test]
    fn cuts_boxes_into_loops() {
        let mut tris = box_triangles([0.0; 3], [20.0, 10.0, 5.0], &Matrix::IDENTITY);
        // A second box beside it, and one above the plane.
        tris.extend(box_triangles(
            [30.0, 0.0, 0.0],
            [32.0, 2.0, 5.0],
            &Matrix::IDENTITY,
        ));
        tris.extend(box_triangles(
            [0.0, 0.0, 6.0],
            [1.0, 1.0, 7.0],
            &Matrix::IDENTITY,
        ));
        let loops = slice(&tris, 2.5);
        assert_eq!(loops.len(), 2);
        for ring in &loops {
            assert!(area2(ring) > 0.0, "outlines run counter-clockwise");
        }
        let areas: Vec<f64> = loops.iter().map(|r| area2(r) / 2.0).collect();
        assert!(areas.iter().any(|a| (a - 200.0).abs() < 1e-9));
        assert!(areas.iter().any(|a| (a - 4.0).abs() < 1e-9));
        // A plane through the top face still gives the loop.
        assert_eq!(slice(&tris[..12], 5.0).len(), 1);
        assert_eq!(slice(&tris[..12], 5.1).len(), 0);
        let big = loops.iter().find(|r| area2(r) > 100.0).unwrap_or(&loops[0]);
        assert!(contains(big, [10.0, 5.0]));
        assert!(!contains(big, [25.0, 5.0]));
        let c = centroid(big);
        assert!((c[0] - 10.0).abs() < 1e-9 && (c[1] - 5.0).abs() < 1e-9);
    }

    #[test]
    fn holes_run_clockwise() {
        // A box with an inner box facing in: a frame.
        let mut tris = box_triangles([0.0; 3], [20.0, 20.0, 5.0], &Matrix::IDENTITY);
        let inner = box_triangles([5.0, 5.0, -1.0], [15.0, 15.0, 6.0], &Matrix::IDENTITY);
        tris.extend(inner.into_iter().map(|[a, b, c]| [a, c, b]));
        let loops = slice(&tris, 2.0);
        assert_eq!(loops.len(), 2);
        assert_eq!(loops.iter().filter(|r| area2(r) < 0.0).count(), 1);
    }
}
