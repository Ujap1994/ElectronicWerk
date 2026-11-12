//! Polygon boolean operations and offsets (with `i_overlay`).
//!
//! A [`Polygons`] value is a list of polygons, each an outline followed by
//! its holes. Inputs may overlap and use any orientation: they are
//! normalized (outlines counter-clockwise, holes clockwise) and combined
//! with the non-zero fill rule, so overlapping polygons unite.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::overlay::FloatOverlay;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};

use crate::geom::{V, signed_area2};

/// Polygons: each one an outline followed by its holes.
pub type Polygons = Vec<Vec<Vec<V>>>;

type Pairs = Vec<Vec<Vec<[f64; 2]>>>;

fn ring_pairs(ring: &[V], ccw: bool) -> Vec<[f64; 2]> {
    let mut pairs: Vec<[f64; 2]> = ring.iter().map(|p| p.pair()).collect();
    if (signed_area2(ring) > 0.0) != ccw {
        pairs.reverse();
    }
    pairs
}

/// Converts polygons into the library's form with normalized orientation.
fn pairs(polygons: &[Vec<Vec<V>>]) -> Pairs {
    polygons
        .iter()
        .filter(|polygon| polygon.first().is_some_and(|outline| outline.len() >= 3))
        .map(|polygon| {
            polygon
                .iter()
                .enumerate()
                .filter(|(_, ring)| ring.len() >= 3)
                .map(|(index, ring)| ring_pairs(ring, index == 0))
                .collect()
        })
        .collect()
}

fn from_pairs(shapes: Pairs) -> Polygons {
    shapes
        .into_iter()
        .map(|shape| {
            shape
                .into_iter()
                .map(|ring| ring.into_iter().map(|[x, y]| V::new(x, y)).collect())
                .collect()
        })
        .collect()
}

fn overlay(subject: &[Vec<Vec<V>>], clip: &[Vec<Vec<V>>], rule: OverlayRule) -> Polygons {
    let subject = pairs(subject);
    let clip = pairs(clip);
    if subject.is_empty() && clip.is_empty() {
        return Vec::new();
    }
    let result = FloatOverlay::with_subj_and_clip(&subject, &clip).overlay(rule, FillRule::NonZero);
    from_pairs(result)
}

/// The union of all polygons.
#[must_use]
pub fn union(polygons: &[Vec<Vec<V>>]) -> Polygons {
    let subject = pairs(polygons);
    if subject.is_empty() {
        return Vec::new();
    }
    from_pairs(subject.simplify_shape(FillRule::NonZero))
}

/// `subject` minus `clip`.
#[must_use]
pub fn difference(subject: &[Vec<Vec<V>>], clip: &[Vec<Vec<V>>]) -> Polygons {
    if clip.is_empty() {
        return union(subject);
    }
    overlay(subject, clip, OverlayRule::Difference)
}

/// The area shared by `subject` and `clip`.
#[must_use]
pub fn intersection(subject: &[Vec<Vec<V>>], clip: &[Vec<Vec<V>>]) -> Polygons {
    if subject.is_empty() || clip.is_empty() {
        return Vec::new();
    }
    overlay(subject, clip, OverlayRule::Intersect)
}

/// The polygons grown (positive `distance`) or shrunk (negative) with
/// round corners. `tolerance` bounds the chord error of the corners.
#[must_use]
pub fn offset(polygons: &[Vec<Vec<V>>], distance: f64, tolerance: f64) -> Polygons {
    let shapes = pairs(polygons);
    if shapes.is_empty() {
        return Vec::new();
    }
    if distance.abs() < 0.5 {
        return from_pairs(shapes);
    }
    // The library splits round joins into pieces of `step · radius`.
    let step = (8.0 * tolerance / distance.abs()).sqrt().clamp(0.02, 0.5);
    let style = OutlineStyle::new(distance).line_join(LineJoin::Round(step));
    // Offsetting needs valid input: simplify first.
    let valid = shapes.simplify_shape(FillRule::NonZero);
    from_pairs(valid.outline(&style))
}

/// Total area of the polygons (outlines minus holes).
#[must_use]
pub fn area(polygons: &[Vec<Vec<V>>]) -> f64 {
    polygons
        .iter()
        .map(|polygon| {
            polygon
                .iter()
                .enumerate()
                .map(|(index, ring)| {
                    let a = signed_area2(ring).abs() / 2.0;
                    if index == 0 { a } else { -a }
                })
                .sum::<f64>()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, size: f64, ccw: bool) -> Vec<Vec<V>> {
        let mut ring = vec![
            V::new(x, y),
            V::new(x + size, y),
            V::new(x + size, y + size),
            V::new(x, y + size),
        ];
        if !ccw {
            ring.reverse();
        }
        vec![ring]
    }

    #[test]
    fn overlapping_polygons_unite_whatever_their_orientation() {
        let a = square(0.0, 0.0, 10.0, true);
        let b = square(5.0, 0.0, 10.0, false);
        let united = union(&[a.clone(), b.clone()]);
        assert_eq!(united.len(), 1);
        assert!((area(&united) - 150.0).abs() < 1e-3);
        let cut = difference(&[square(-10.0, -10.0, 40.0, false)], &[a, b]);
        assert!((area(&cut) - (1600.0 - 150.0)).abs() < 1e-3);
        assert_eq!(cut.len(), 1);
        assert_eq!(cut[0].len(), 2, "the obstacles leave a hole");
    }

    #[test]
    fn intersections_and_offsets() {
        let a = square(0.0, 0.0, 10.0, true);
        let b = square(5.0, 5.0, 10.0, true);
        assert!((area(&intersection(std::slice::from_ref(&a), &[b])) - 25.0).abs() < 1e-3);
        let grown = offset(std::slice::from_ref(&a), 1.0, 0.01);
        // 10×10 plus four 10×1 strips plus four quarter circles.
        let expected = 100.0 + 40.0 + std::f64::consts::PI;
        assert!((area(&grown) - expected).abs() < 0.1, "{}", area(&grown));
        let shrunk = offset(&[a], -2.0, 0.01);
        assert!((area(&shrunk) - 36.0).abs() < 0.1, "{}", area(&shrunk));
        assert_eq!(offset(&[square(0.0, 0.0, 2.0, true)], -1.5, 0.01).len(), 0);
    }
}
