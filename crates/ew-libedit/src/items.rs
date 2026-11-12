//! Addressing items of a symbol or footprint, and the geometric transforms
//! shared by the symbol and footprint operations.

use ew_model::shape::Geometry;
use ew_model::units::{Angle, Point};
use serde::{Deserialize, Serialize};

/// The kind of an item of a definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    /// A symbol pin.
    Pin,
    /// A drawn graphic.
    Graphic,
    /// A text.
    Text,
    /// A footprint pad.
    Pad,
    /// A 3D model of a footprint.
    Model,
}

impl ItemKind {
    /// The tag of this kind in the ids of editor scenes.
    #[must_use]
    pub const fn tag(self) -> u64 {
        match self {
            Self::Pin => 1,
            Self::Graphic => 2,
            Self::Text => 3,
            Self::Pad => 4,
            Self::Model => 5,
        }
    }
}

/// One item: its kind and its index in the list of that kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRef {
    /// Kind.
    pub kind: ItemKind,
    /// Index.
    pub index: usize,
}

impl ItemRef {
    /// A reference.
    #[must_use]
    pub const fn new(kind: ItemKind, index: usize) -> Self {
        Self { kind, index }
    }
}

/// How items are turned or mirrored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    /// 90° counterclockwise.
    Left,
    /// 90° clockwise.
    Right,
    /// Mirror left to right.
    Horizontal,
    /// Mirror top to bottom.
    Vertical,
}

impl Orientation {
    /// Maps a vector (no translation).
    #[must_use]
    pub const fn vector(self, v: Point) -> Point {
        match self {
            Self::Left => Point::new(-v.y, v.x),
            Self::Right => Point::new(v.y, -v.x),
            Self::Horizontal => Point::new(-v.x, v.y),
            Self::Vertical => Point::new(v.x, -v.y),
        }
    }

    /// Maps a point about `center`.
    #[must_use]
    pub fn point(self, p: Point, center: Point) -> Point {
        center + self.vector(p - center)
    }

    /// The new rotation of something that has an orientation (pads, texts).
    #[must_use]
    pub fn angle(self, angle: Angle) -> Angle {
        let deg = i64::from(angle.millidegrees());
        match self {
            Self::Left => Angle::from_millidegrees(deg + 90_000),
            Self::Right => Angle::from_millidegrees(deg - 90_000),
            Self::Horizontal => Angle::from_millidegrees(180_000 - deg),
            Self::Vertical => Angle::from_millidegrees(-deg),
        }
    }

    /// `true` for the mirrors.
    #[must_use]
    pub const fn mirrors(self) -> bool {
        matches!(self, Self::Horizontal | Self::Vertical)
    }
}

/// Applies `f` to every point of a geometry.
pub fn transform_geometry(geometry: &mut Geometry, f: &dyn Fn(Point) -> Point) {
    match geometry {
        Geometry::Line { start, end } | Geometry::Rect { start, end } => {
            *start = f(*start);
            *end = f(*end);
        }
        Geometry::Circle { center, .. } => *center = f(*center),
        Geometry::Arc { start, mid, end } => {
            *start = f(*start);
            *mid = f(*mid);
            *end = f(*end);
        }
        Geometry::Polyline { points } | Geometry::Polygon { points } => {
            for p in points {
                *p = f(*p);
            }
        }
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => {
            for p in [start, control1, control2, end] {
                *p = f(*p);
            }
        }
    }
}

/// The points of a geometry (to find a centre).
#[must_use]
pub fn geometry_points(geometry: &Geometry) -> Vec<Point> {
    match geometry {
        Geometry::Line { start, end } | Geometry::Rect { start, end } => vec![*start, *end],
        Geometry::Circle { center, .. } => vec![*center],
        Geometry::Arc { start, mid, end } => vec![*start, *mid, *end],
        Geometry::Polyline { points } | Geometry::Polygon { points } => points.clone(),
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => vec![*start, *control1, *control2, *end],
    }
}

/// The centre of the bounding box of points, rounded to `grid`.
#[must_use]
pub fn center_of(points: &[Point], grid: i64) -> Point {
    if points.is_empty() {
        return Point::ORIGIN;
    }
    let min_x = points.iter().map(|p| p.x).min().unwrap_or(0);
    let max_x = points.iter().map(|p| p.x).max().unwrap_or(0);
    let min_y = points.iter().map(|p| p.y).min().unwrap_or(0);
    let max_y = points.iter().map(|p| p.y).max().unwrap_or(0);
    let snap = |v: i64| {
        if grid <= 0 {
            v
        } else {
            (v as f64 / grid as f64).round() as i64 * grid
        }
    };
    Point::new(
        snap(min_x + (max_x - min_x) / 2),
        snap(min_y + (max_y - min_y) / 2),
    )
}

/// The next number after `number` with `step`: the trailing digits count
/// (`A9` + 1 = `A10`, `7` − 2 = `5`); a number without digits gets one.
#[must_use]
pub fn next_number(number: &str, step: i64) -> String {
    let digits = number.len() - number.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (prefix, value) = number.split_at(number.len() - digits);
    let value: i64 = value.parse().unwrap_or(0);
    format!("{prefix}{}", (value + step).max(0))
}

/// Removes the indices from a list (in any order, ignoring repeats and
/// indices out of range).
pub fn remove_indices<T>(list: &mut Vec<T>, indices: &[usize]) {
    let mut sorted: Vec<usize> = indices
        .iter()
        .copied()
        .filter(|&i| i < list.len())
        .collect();
    sorted.sort_unstable();
    sorted.dedup();
    for index in sorted.into_iter().rev() {
        list.remove(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_count_on() {
        assert_eq!(next_number("1", 1), "2");
        assert_eq!(next_number("A9", 1), "A10");
        assert_eq!(next_number("7", -2), "5");
        assert_eq!(next_number("GND", 1), "GND1");
    }

    #[test]
    fn orientations() {
        let c = Point::new(10, 10);
        assert_eq!(
            Orientation::Left.point(Point::new(20, 10), c),
            Point::new(10, 20)
        );
        assert_eq!(
            Orientation::Horizontal.point(Point::new(20, 10), c),
            Point::new(0, 10)
        );
        assert_eq!(
            Orientation::Left.angle(Angle::from_degrees(270)),
            Angle::ZERO
        );
        assert_eq!(
            Orientation::Horizontal.angle(Angle::ZERO),
            Angle::from_degrees(180)
        );
        let mut list = vec![0, 1, 2, 3, 4];
        remove_indices(&mut list, &[3, 1, 3, 9]);
        assert_eq!(list, [0, 2, 4]);
    }
}
