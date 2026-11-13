//! Polygons with holes as one contour.
//!
//! Gerber regions and many plotters only fill single contours. A polygon
//! with holes becomes one contour by cutting from every hole to the left
//! until the cut meets the outline (or an earlier cut-in hole): the contour
//! runs along the outline to the cut, around the hole and back. The two
//! edges of a cut lie on top of each other, which Gerber explicitly allows
//! ("cut-ins").

use ew_pcb::geom::{V, signed_area2};

/// Orients a ring counter-clockwise (`ccw`) or clockwise.
fn oriented(ring: &[V], ccw: bool) -> Vec<V> {
    let mut out = ring.to_vec();
    if (signed_area2(&out) > 0.0) != ccw {
        out.reverse();
    }
    out
}

/// The x where the edge `p`–`q` crosses the horizontal line through `y`,
/// counting edges half-open in y so that a vertex on the line is met once.
fn crossing(p: V, q: V, y: f64) -> Option<f64> {
    let (low, high) = if p.y <= q.y { (p, q) } else { (q, p) };
    if y < low.y || y >= high.y || (high.y - low.y).abs() < f64::EPSILON {
        return None;
    }
    let t = (y - low.y) / (high.y - low.y);
    Some(low.x + t * (high.x - low.x))
}

/// One contour for an outline with holes: the outline counter-clockwise,
/// every hole clockwise and joined to it by a cut-in.
#[must_use]
pub fn fracture(outline: &[V], holes: &[Vec<V>]) -> Vec<V> {
    let mut contour = oriented(outline, true);
    if contour.len() < 3 {
        return contour;
    }
    // Leftmost vertex of every hole, holes from left to right: a cut to the
    // left can then only meet the outline or holes already joined.
    let mut ordered: Vec<(usize, Vec<V>)> = holes
        .iter()
        .filter(|h| h.len() >= 3)
        .map(|hole| {
            let ring = oriented(hole, false);
            let start = ring
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.x.total_cmp(&b.1.x).then(a.1.y.total_cmp(&b.1.y)))
                .map_or(0, |(i, _)| i);
            (start, ring)
        })
        .collect();
    ordered.sort_by(|a, b| a.1[a.0].x.total_cmp(&b.1[b.0].x));
    for (start, ring) in ordered {
        let h = ring[start];
        // The nearest crossing to the left of the hole.
        let mut best: Option<(usize, f64)> = None;
        let n = contour.len();
        for i in 0..n {
            let (p, q) = (contour[i], contour[(i + 1) % n]);
            if let Some(x) = crossing(p, q, h.y)
                && x <= h.x
                && best.is_none_or(|(_, bx)| x > bx)
            {
                best = Some((i, x));
            }
        }
        let Some((edge, x)) = best else {
            // The hole lies outside the outline: drop it.
            continue;
        };
        let cut = V::new(x, h.y);
        let mut splice = Vec::with_capacity(ring.len() + 4);
        splice.push(cut);
        splice.extend(ring[start..].iter().copied());
        splice.extend(ring[..start].iter().copied());
        splice.push(h);
        splice.push(cut);
        let at = edge + 1;
        contour.splice(at..at, splice);
    }
    contour.dedup_by(|a, b| a.dist(*b) < 0.5);
    if contour.len() > 1 && contour[0].dist(contour[contour.len() - 1]) < 0.5 {
        contour.pop();
    }
    contour
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Vec<V> {
        vec![
            V::new(x, y),
            V::new(x + s, y),
            V::new(x + s, y + s),
            V::new(x, y + s),
        ]
    }

    /// Area by the shoelace formula of a possibly self-touching contour.
    fn area(ring: &[V]) -> f64 {
        signed_area2(ring) / 2.0
    }

    #[test]
    fn holes_are_cut_in() {
        let outline = square(0.0, 0.0, 100.0);
        let holes = vec![square(20.0, 20.0, 10.0), square(60.0, 50.0, 20.0)];
        let contour = fracture(&outline, &holes);
        assert!((area(&contour) - (10_000.0 - 100.0 - 400.0)).abs() < 1e-6);
        // Every hole vertex is on the contour.
        for hole in &holes {
            for p in hole {
                assert!(contour.iter().any(|q| q.dist(*p) < 1e-9));
            }
        }
    }

    #[test]
    fn a_hole_behind_another_hole_joins_through_it() {
        let outline = square(0.0, 0.0, 100.0);
        let holes = vec![square(10.0, 40.0, 20.0), square(50.0, 45.0, 10.0)];
        let contour = fracture(&outline, &holes);
        assert!((area(&contour) - (10_000.0 - 400.0 - 100.0)).abs() < 1e-6);
    }

    #[test]
    fn orientation_of_the_input_does_not_matter() {
        let mut outline = square(0.0, 0.0, 10.0);
        outline.reverse();
        let contour = fracture(&outline, &[square(4.0, 4.0, 2.0)]);
        assert!((area(&contour) - 96.0).abs() < 1e-9);
    }
}
