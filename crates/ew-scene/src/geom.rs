//! Small 2D geometry helpers in f64 scene units (nanometres).

use std::f64::consts::{FRAC_PI_2, TAU};

use ew_model::units::{Angle, Point};

/// A point in scene coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct P {
    /// x (right).
    pub x: f64,
    /// y (up).
    pub y: f64,
}

impl P {
    /// A point.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Converts a model point.
    #[must_use]
    pub fn of(point: Point) -> Self {
        Self::new(point.x as f64, point.y as f64)
    }

    /// `self + other`.
    #[must_use]
    pub fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }

    /// `self - other`.
    #[must_use]
    pub fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    /// Scales by `factor`.
    #[must_use]
    pub fn scale(self, factor: f64) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }

    /// Rotates counter-clockwise by `angle` radians around the origin.
    #[must_use]
    pub fn rotate(self, angle: f64) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self::new(self.x * cos - self.y * sin, self.x * sin + self.y * cos)
    }

    /// Length of the vector.
    #[must_use]
    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
}

/// An axis-aligned bounding box; [`BBox::EMPTY`] contains nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    /// Minimum x.
    pub min_x: f64,
    /// Minimum y.
    pub min_y: f64,
    /// Maximum x.
    pub max_x: f64,
    /// Maximum y.
    pub max_y: f64,
}

impl BBox {
    /// The empty box.
    pub const EMPTY: Self = Self {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };

    /// A box containing one point.
    #[must_use]
    pub const fn of(p: P) -> Self {
        Self {
            min_x: p.x,
            min_y: p.y,
            max_x: p.x,
            max_y: p.y,
        }
    }

    /// `true` if the box contains nothing.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.min_x > self.max_x || self.min_y > self.max_y
    }

    /// The box grown to contain `p`.
    #[must_use]
    pub const fn with(self, p: P) -> Self {
        Self {
            min_x: self.min_x.min(p.x),
            min_y: self.min_y.min(p.y),
            max_x: self.max_x.max(p.x),
            max_y: self.max_y.max(p.y),
        }
    }

    /// The union of two boxes.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    /// The box grown by `d` on every side (empty stays empty).
    #[must_use]
    pub fn grow(self, d: f64) -> Self {
        if self.is_empty() {
            return self;
        }
        Self {
            min_x: self.min_x - d,
            min_y: self.min_y - d,
            max_x: self.max_x + d,
            max_y: self.max_y + d,
        }
    }

    /// The centre.
    #[must_use]
    pub fn center(self) -> P {
        P::new(
            f64::midpoint(self.min_x, self.max_x),
            f64::midpoint(self.min_y, self.max_y),
        )
    }
}

/// Degrees of a model angle as radians.
#[must_use]
pub fn radians(angle: Angle) -> f64 {
    angle.degrees().to_radians()
}

/// Placement of local coordinates (footprints, symbols): mirror x first, then
/// rotate counter-clockwise, then translate.
#[derive(Debug, Clone, Copy)]
pub struct Xform {
    /// Translation (scene coordinates).
    pub offset: P,
    /// Rotation in radians.
    pub angle: f64,
    /// Mirror x before rotating.
    pub mirror: bool,
}

impl Xform {
    /// A plain translation.
    #[must_use]
    pub const fn translate(offset: P) -> Self {
        Self {
            offset,
            angle: 0.0,
            mirror: false,
        }
    }

    /// Maps a local point.
    #[must_use]
    pub fn apply(&self, p: P) -> P {
        let local = if self.mirror { P::new(-p.x, p.y) } else { p };
        local.rotate(self.angle).add(self.offset)
    }

    /// Maps a local model point.
    #[must_use]
    pub fn point(&self, p: Point) -> P {
        self.apply(P::of(p))
    }

    /// Maps the rotation of a symmetric shape (rectangles, ovals).
    #[must_use]
    pub fn shape_angle(&self, angle: f64) -> f64 {
        let local = if self.mirror { -angle } else { angle };
        normalize(local + self.angle)
    }
}

/// Normalizes an angle into `[0, 2π)`.
#[must_use]
pub fn normalize(angle: f64) -> f64 {
    let a = angle % TAU;
    if a < 0.0 { a + TAU } else { a }
}

/// Normalizes a text rotation (degrees) so that text never reads upside
/// down: the result is in `(-90, 90]`.
#[must_use]
pub fn readable(degrees: f64) -> f64 {
    let mut d = degrees % 360.0;
    if d < 0.0 {
        d += 360.0;
    }
    if d > 90.0 + 1e-6 && d <= 270.0 + 1e-6 {
        d -= 180.0;
    } else if d > 270.0 {
        d -= 360.0;
    }
    d
}

/// The circle through three points as `(center, radius, start, sweep)`;
/// `None` if the points are collinear.
#[must_use]
pub fn arc_through(start: P, mid: P, end: P) -> Option<(P, f64, f64, f64)> {
    let ax = start.x;
    let ay = start.y;
    let bx = mid.x;
    let by = mid.y;
    let cx = end.x;
    let cy = end.y;
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    let scale = start.sub(end).len().max(start.sub(mid).len()).max(1.0);
    if d.abs() < 1e-9 * scale * scale {
        return None;
    }
    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d;
    let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d;
    let center = P::new(ux, uy);
    let radius = start.sub(center).len();
    let angle = |p: P| (p.y - center.y).atan2(p.x - center.x);
    let a_start = angle(start);
    let to_end = normalize(angle(end) - a_start);
    let to_mid = normalize(angle(mid) - a_start);
    let sweep = if to_mid <= to_end {
        to_end
    } else {
        to_end - TAU
    };
    Some((center, radius, a_start, sweep))
}

/// Bounding box of an arc's centre line.
#[must_use]
pub fn arc_bounds(center: P, radius: f64, start: f64, sweep: f64) -> BBox {
    let point = |a: f64| P::new(center.x + radius * a.cos(), center.y + radius * a.sin());
    let mut bbox = BBox::of(point(start)).with(point(start + sweep));
    let (from, to) = if sweep >= 0.0 {
        (start, start + sweep)
    } else {
        (start + sweep, start)
    };
    // Every axis crossing (multiples of 90°) inside the sweep.
    let mut k = (from / FRAC_PI_2).ceil();
    while k * FRAC_PI_2 <= to {
        bbox = bbox.with(point(k * FRAC_PI_2));
        k += 1.0;
    }
    bbox
}

/// Points along a cubic Bézier curve (including both ends).
#[must_use]
pub fn bezier(p0: P, p1: P, p2: P, p3: P, steps: usize) -> Vec<P> {
    let steps = steps.max(1);
    (0..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let u = 1.0 - t;
            let w0 = u * u * u;
            let w1 = 3.0 * u * u * t;
            let w2 = 3.0 * u * t * t;
            let w3 = t * t * t;
            P::new(
                w0 * p0.x + w1 * p1.x + w2 * p2.x + w3 * p3.x,
                w0 * p0.y + w1 * p1.y + w2 * p2.y + w3 * p3.y,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn arc_through_three_points() {
        let (c, r, start, sweep) =
            arc_through(P::new(1.0, 0.0), P::new(0.0, 1.0), P::new(-1.0, 0.0)).unwrap();
        assert!(close(c.x, 0.0) && close(c.y, 0.0) && close(r, 1.0));
        assert!(close(start, 0.0));
        assert!(close(sweep, PI));
        // The same arc the other way round is clockwise.
        let (_, _, start, sweep) =
            arc_through(P::new(-1.0, 0.0), P::new(0.0, 1.0), P::new(1.0, 0.0)).unwrap();
        assert!(close(start, PI));
        assert!(close(sweep, -PI));
        assert!(arc_through(P::new(0.0, 0.0), P::new(1.0, 1.0), P::new(2.0, 2.0)).is_none());
    }

    #[test]
    fn arc_bounds_include_axis_extremes() {
        let b = arc_bounds(P::new(0.0, 0.0), 2.0, 0.0, PI);
        assert!(close(b.max_y, 2.0) && close(b.min_x, -2.0) && close(b.min_y, 0.0));
        let b = arc_bounds(P::new(0.0, 0.0), 1.0, FRAC_PI_2, -PI);
        assert!(close(b.max_x, 1.0) && close(b.min_y, -1.0));
    }

    #[test]
    fn xform_mirrors_then_rotates() {
        let xf = Xform {
            offset: P::new(10.0, 0.0),
            angle: FRAC_PI_2,
            mirror: true,
        };
        let p = xf.apply(P::new(1.0, 0.0));
        assert!(close(p.x, 10.0) && close(p.y, -1.0));
        assert!(close(xf.shape_angle(0.5), FRAC_PI_2 - 0.5));
    }

    #[test]
    fn readable_angles() {
        assert!(close(readable(0.0), 0.0));
        assert!(close(readable(90.0), 90.0));
        assert!(close(readable(180.0), 0.0));
        assert!(close(readable(270.0), 90.0));
        assert!(close(readable(-90.0), 90.0));
        assert!(close(readable(300.0), -60.0));
    }

    #[test]
    fn bezier_hits_end_points() {
        let points = bezier(
            P::new(0.0, 0.0),
            P::new(0.0, 1.0),
            P::new(1.0, 1.0),
            P::new(1.0, 0.0),
            8,
        );
        assert_eq!(points.len(), 9);
        assert_eq!(points[8], P::new(1.0, 0.0));
    }
}
