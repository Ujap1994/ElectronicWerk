//! Drawn geometry shared by schematics, symbols, footprints and boards.

use serde::{Deserialize, Serialize};

use crate::units::{Nm, Point, Rect};

/// The geometry of a drawn shape.
///
/// Serialized with the kind as key, for example
/// `{"line": {"start": [0, 0], "end": [1000000, 0]}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Geometry {
    /// A straight line.
    Line {
        /// Start point.
        start: Point,
        /// End point.
        end: Point,
    },
    /// An axis-aligned rectangle given by two opposite corners.
    Rect {
        /// First corner.
        start: Point,
        /// Opposite corner.
        end: Point,
    },
    /// A circle.
    Circle {
        /// Centre.
        center: Point,
        /// Radius.
        radius: Nm,
    },
    /// A circular arc through three points.
    Arc {
        /// Start point.
        start: Point,
        /// Any point on the arc between start and end.
        mid: Point,
        /// End point.
        end: Point,
    },
    /// An open chain of line segments.
    Polyline {
        /// Vertices in order.
        points: Vec<Point>,
    },
    /// A closed polygon.
    Polygon {
        /// Vertices in order (the last connects back to the first).
        points: Vec<Point>,
    },
    /// A cubic Bézier curve.
    Bezier {
        /// Start point.
        start: Point,
        /// First control point.
        control1: Point,
        /// Second control point.
        control2: Point,
        /// End point.
        end: Point,
    },
}

impl Geometry {
    /// Short name of the geometry kind.
    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Line { .. } => "line",
            Self::Rect { .. } => "rect",
            Self::Circle { .. } => "circle",
            Self::Arc { .. } => "arc",
            Self::Polyline { .. } => "polyline",
            Self::Polygon { .. } => "polygon",
            Self::Bezier { .. } => "bezier",
        }
    }

    /// Every defining point (for circles: the centre ± radius corners).
    #[must_use]
    pub fn points(&self) -> Vec<Point> {
        match self {
            Self::Line { start, end } | Self::Rect { start, end } => vec![*start, *end],
            Self::Circle { center, radius } => vec![
                Point::new(center.x - radius, center.y - radius),
                Point::new(center.x + radius, center.y + radius),
            ],
            Self::Arc { start, mid, end } => vec![*start, *mid, *end],
            Self::Polyline { points } | Self::Polygon { points } => points.clone(),
            Self::Bezier {
                start,
                control1,
                control2,
                end,
            } => vec![*start, *control1, *control2, *end],
        }
    }

    /// An approximate bounding box (exact for everything except arcs, whose
    /// box is the box of their three points).
    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        Rect::from_points(self.points())
    }
}

/// Line style of a schematic stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineStyle {
    /// Continuous line.
    #[default]
    Solid,
    /// Dashed line.
    Dash,
    /// Dotted line.
    Dot,
    /// Dash-dot line.
    DashDot,
}

/// Outline of a schematic shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Stroke {
    /// Line width; 0 means the default schematic line width.
    #[serde(skip_serializing_if = "crate::is_default")]
    pub width: Nm,
    /// Line style.
    #[serde(skip_serializing_if = "crate::is_default")]
    pub style: LineStyle,
}

/// Fill of a closed schematic shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fill {
    /// Not filled.
    #[default]
    None,
    /// Filled with the line colour.
    Solid,
    /// Filled with the body (background) colour.
    Background,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_serializes_with_kind_key() {
        let line = Geometry::Line {
            start: Point::new(0, 0),
            end: Point::new(10, 0),
        };
        let json = serde_json::to_string(&line).unwrap();
        assert_eq!(json, r#"{"line":{"start":[0,0],"end":[10,0]}}"#);
        assert_eq!(serde_json::from_str::<Geometry>(&json).unwrap(), line);
        assert_eq!(line.kind_name(), "line");
    }

    #[test]
    fn bounds() {
        let circle = Geometry::Circle {
            center: Point::new(5, 5),
            radius: 2,
        };
        let rect = circle.bounds().unwrap();
        assert_eq!((rect.min, rect.max), (Point::new(3, 3), Point::new(7, 7)));
        assert!(Geometry::Polygon { points: vec![] }.bounds().is_none());
    }

    #[test]
    fn default_stroke_is_empty_object() {
        assert_eq!(serde_json::to_string(&Stroke::default()).unwrap(), "{}");
        assert_eq!(
            serde_json::from_str::<Stroke>("{}").unwrap(),
            Stroke::default()
        );
    }
}
