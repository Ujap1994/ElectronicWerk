//! Dimensions on boards: a measured distance or radius drawn with extension
//! lines, a dimension line, arrows and the value, or a leader with a note.
//!
//! [`draw`] turns a dimension into lines and one text; the screen, the
//! Gerber files and the prints all draw that, so they always agree.

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::layer::Layer;
use crate::project::Units;
use crate::text::{HAlign, TextStyle, VAlign};
use crate::units::{Nm, Point};

/// What a dimension measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DimensionKind {
    /// The distance between two points along the line through them.
    #[default]
    Aligned,
    /// The horizontal distance between two points.
    Horizontal,
    /// The vertical distance between two points.
    Vertical,
    /// The radius of a circle or arc: `start` is the centre, `end` a point
    /// on the circle.
    Radial,
    /// The diameter: `start` is the centre, `end` a point on the circle.
    Diameter,
    /// An arrow from `start` to `end` with a note at `end`.
    Leader,
}

/// A dimension on a board layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dimension {
    /// Object id.
    pub id: Id,
    /// Layer (usually a user, mechanical or fabrication layer).
    pub layer: Layer,
    /// What it measures.
    pub kind: DimensionKind,
    /// First point (the centre of a radius, the arrow tip of a leader).
    pub start: Point,
    /// Second point.
    pub end: Point,
    /// How far the dimension line lies from the measured points: aligned to
    /// the left of `start` → `end`, horizontal upwards, vertical to the
    /// right (negative: the other way). Radius and diameter: how far the
    /// line runs on beyond the circle.
    #[serde(default, skip_serializing_if = "crate::is_default")]
    pub offset: Nm,
    /// The text: `{value}` stands for the measured value; empty: the value
    /// alone. The note of a leader.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Units of the value (none: the project's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<Units>,
    /// Decimals (none: 2 for mm, 1 for mil, 3 for inch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<u8>,
    /// The value without its unit (`25.40` instead of `25.40 mm`).
    #[serde(default, skip_serializing_if = "crate::is_default")]
    pub hide_units: bool,
    /// Text size and stroke.
    pub style: TextStyle,
    /// Line width.
    pub width: Nm,
    /// Locked.
    #[serde(default, skip_serializing_if = "crate::is_default")]
    pub locked: bool,
}

/// A dimension as lines and one text, in board coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionDrawing {
    /// Extension lines, the dimension line and the arrows.
    pub lines: Vec<(Point, Point)>,
    /// The text shown.
    pub text: String,
    /// Where the text is anchored.
    pub at: Point,
    /// Rotation of the text in degrees (always readable: −90° < r ≤ 90°).
    pub rotation: f64,
    /// The style with the alignment for `at`.
    pub style: TextStyle,
    /// The measured value (nm): distance, radius or diameter; 0 for a
    /// leader.
    pub value: f64,
}

/// Angle of the arrow wings to the line.
const ARROW_ANGLE: f64 = 0.4;

#[derive(Debug, Clone, Copy)]
struct V(f64, f64);

impl V {
    fn of(p: Point) -> Self {
        #[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
        Self(p.x as f64, p.y as f64)
    }
    fn add(self, o: Self) -> Self {
        Self(self.0 + o.0, self.1 + o.1)
    }
    fn sub(self, o: Self) -> Self {
        Self(self.0 - o.0, self.1 - o.1)
    }
    fn scale(self, k: f64) -> Self {
        Self(self.0 * k, self.1 * k)
    }
    fn len(self) -> f64 {
        self.0.hypot(self.1)
    }
    fn unit(self) -> Self {
        let l = self.len();
        if l > 0.0 {
            self.scale(1.0 / l)
        } else {
            Self(1.0, 0.0)
        }
    }
    fn rotate(self, angle: f64) -> Self {
        let (s, c) = angle.sin_cos();
        Self(self.0 * c - self.1 * s, self.0 * s + self.1 * c)
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "rounded board coordinates fit i64"
    )]
    fn point(self) -> Point {
        Point::new(self.0.round() as i64, self.1.round() as i64)
    }
}

/// The value in the units, with the decimals and (unless hidden) the unit.
#[must_use]
pub fn format_value(nm: f64, units: Units, precision: Option<u8>, hide_units: bool) -> String {
    let (factor, decimals, symbol) = match units {
        Units::Mm => (1e6, 2, "mm"),
        Units::Mil => (25_400.0, 1, "mil"),
        Units::Inch => (25.4e6, 3, "in"),
    };
    let decimals = usize::from(precision.unwrap_or(decimals).min(6));
    let number = format!("{:.*}", decimals, nm / factor);
    if hide_units {
        number
    } else {
        format!("{number} {symbol}")
    }
}

/// Lines, arrows and the text of a dimension. `project_units` are used when
/// the dimension has no units of its own.
#[allow(clippy::too_many_lines, reason = "one arm per kind of dimension")]
#[must_use]
pub fn draw(dim: &Dimension, project_units: Units) -> DimensionDrawing {
    #[allow(clippy::cast_precision_loss, reason = "lengths fit f64")]
    let (size, offset) = (dim.style.size as f64, dim.offset as f64);
    let arrow = size * 0.9;
    let units = dim.units.unwrap_or(project_units);
    let mut lines: Vec<(Point, Point)> = Vec::new();
    let mut line = |a: V, b: V| lines.push((a.point(), b.point()));
    let (a, b) = (V::of(dim.start), V::of(dim.end));
    // An arrow with its tip at `tip`, its wings back along `back`.
    let arrowhead = |tip: V, back: V, line: &mut dyn FnMut(V, V)| {
        let back = back.unit();
        line(tip, tip.add(back.rotate(ARROW_ANGLE).scale(arrow)));
        line(tip, tip.add(back.rotate(-ARROW_ANGLE).scale(arrow)));
    };
    let label = |value: f64, prefix: &str| {
        let value = format!(
            "{prefix}{}",
            format_value(value, units, dim.precision, dim.hide_units)
        );
        if dim.text.is_empty() {
            value
        } else {
            dim.text.replace("{value}", &value)
        }
    };
    match dim.kind {
        DimensionKind::Aligned | DimensionKind::Horizontal | DimensionKind::Vertical => {
            // The two ends of the dimension line and the measured value.
            let (from, to) = match dim.kind {
                DimensionKind::Horizontal => {
                    let y = a.1 + offset;
                    (V(a.0, y), V(b.0, y))
                }
                DimensionKind::Vertical => {
                    let x = a.0 + offset;
                    (V(x, a.1), V(x, b.1))
                }
                _ => {
                    let normal = b.sub(a).unit().rotate(std::f64::consts::FRAC_PI_2);
                    (a.add(normal.scale(offset)), b.add(normal.scale(offset)))
                }
            };
            let value = to.sub(from).len();
            // Extension lines from the points to a little beyond the line,
            // starting a small gap away from the measured feature.
            for (point, foot) in [(a, from), (b, to)] {
                let along = foot.sub(point);
                if along.len() > size * 0.1 {
                    let dir = along.unit();
                    let gap = (size * 0.4).min(along.len() / 2.0);
                    line(point.add(dir.scale(gap)), foot.add(dir.scale(size * 0.5)));
                }
            }
            if value > 0.0 {
                let u = to.sub(from).unit();
                line(from, to);
                arrowhead(from, u, &mut line);
                arrowhead(to, u.scale(-1.0), &mut line);
            }
            // The text above the middle of the line, readable from below or
            // from the right.
            let u = to.sub(from).unit();
            let mut angle = u.1.atan2(u.0).to_degrees();
            if angle > 90.0 + 1e-9 {
                angle -= 180.0;
            } else if angle <= -90.0 + 1e-9 {
                angle += 180.0;
            }
            #[allow(clippy::cast_precision_loss, reason = "lengths fit f64")]
            let lift = size * 0.25 + dim.width as f64;
            let up = V(1.0, 0.0).rotate(angle.to_radians() + std::f64::consts::FRAC_PI_2);
            let at = from.add(to).scale(0.5).add(up.scale(lift));
            DimensionDrawing {
                lines,
                text: label(value, ""),
                at: at.point(),
                rotation: angle,
                style: dim.style.aligned(HAlign::Center, VAlign::Bottom),
                value,
            }
        }
        DimensionKind::Radial | DimensionKind::Diameter => {
            let radius = b.sub(a).len();
            let u = b.sub(a).unit();
            let beyond = b.add(u.scale(offset.max(0.0)));
            let diameter = dim.kind == DimensionKind::Diameter;
            if diameter {
                let opposite = a.sub(b.sub(a));
                line(opposite, beyond);
                arrowhead(b, u.scale(-1.0), &mut line);
                arrowhead(opposite, u, &mut line);
            } else {
                line(a, beyond);
                arrowhead(b, u.scale(-1.0), &mut line);
            }
            let (value, prefix) = if diameter {
                (radius * 2.0, "Ø ")
            } else {
                (radius, "R ")
            };
            let (at, style) = side_note(beyond, u.0 >= 0.0, size, dim.style);
            DimensionDrawing {
                lines,
                text: label(value, prefix),
                at,
                rotation: 0.0,
                style,
                value,
            }
        }
        DimensionKind::Leader => {
            line(a, b);
            arrowhead(a, b.sub(a), &mut line);
            let right = b.0 >= a.0;
            let shelf = b.add(V(if right { size } else { -size }, 0.0));
            line(b, shelf);
            let (at, style) = side_note(shelf, right, size, dim.style);
            DimensionDrawing {
                lines,
                text: dim.text.clone(),
                at,
                rotation: 0.0,
                style,
                value: 0.0,
            }
        }
    }
}

/// A horizontal note next to the end of a line.
fn side_note(end: V, right: bool, size: f64, style: TextStyle) -> (Point, TextStyle) {
    let at = end.add(V(if right { size * 0.3 } else { -size * 0.3 }, 0.0));
    let h = if right { HAlign::Left } else { HAlign::Right };
    (at.point(), style.aligned(h, VAlign::Center))
}

/// A new dimension with the usual style: 1 mm text, 0.1 mm lines, on
/// `layer`.
#[must_use]
pub fn new_dimension(
    id: Id,
    layer: Layer,
    kind: DimensionKind,
    start: Point,
    end: Point,
    offset: Nm,
) -> Dimension {
    Dimension {
        id,
        layer,
        kind,
        start,
        end,
        offset,
        text: String::new(),
        units: None,
        precision: None,
        hide_units: false,
        style: TextStyle::board(),
        width: 100_000,
        locked: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::mm;

    fn dim(kind: DimensionKind, start: Point, end: Point, offset: Nm) -> Dimension {
        new_dimension(Id::random(), Layer::User(1), kind, start, end, offset)
    }

    #[test]
    fn horizontal_and_vertical_measure_along_one_axis() {
        let d = dim(
            DimensionKind::Horizontal,
            Point::mm(0, 0),
            Point::mm(30, 7),
            mm(5),
        );
        let drawing = draw(&d, Units::Mm);
        assert!((drawing.value - 30e6).abs() < 1.0);
        assert_eq!(drawing.text, "30.00 mm");
        // The dimension line lies 5 mm above the first point.
        assert!(drawing.lines.contains(&(Point::mm(0, 5), Point::mm(30, 5))));
        assert!((drawing.rotation - 0.0).abs() < 1e-9);
        assert!(drawing.at.y > mm(5));
        let v = dim(
            DimensionKind::Vertical,
            Point::mm(0, 0),
            Point::mm(4, 12),
            mm(-3),
        );
        let drawing = draw(&v, Units::Mil);
        assert_eq!(drawing.text, "472.4 mil");
        assert!(
            drawing
                .lines
                .contains(&(Point::mm(-3, 0), Point::mm(-3, 12)))
        );
        assert!((drawing.rotation - 90.0).abs() < 1e-9);
    }

    #[test]
    fn aligned_text_stays_readable() {
        // From right to left: the text is not upside down.
        let d = dim(
            DimensionKind::Aligned,
            Point::mm(10, 0),
            Point::mm(0, 0),
            mm(2),
        );
        let drawing = draw(&d, Units::Mm);
        assert!((drawing.value - 10e6).abs() < 1.0);
        assert!(drawing.rotation.abs() < 1e-9, "{}", drawing.rotation);
        // Two extension lines, the dimension line, two arrows of two wings.
        assert_eq!(drawing.lines.len(), 7);
    }

    #[test]
    fn radius_diameter_and_leader() {
        let r = dim(
            DimensionKind::Radial,
            Point::mm(0, 0),
            Point::mm(3, 4),
            mm(2),
        );
        assert_eq!(draw(&r, Units::Mm).text, "R 5.00 mm");
        let mut d = dim(
            DimensionKind::Diameter,
            Point::mm(0, 0),
            Point::mm(-1, 0),
            0,
        );
        d.text = "{value} 4×".into();
        d.precision = Some(1);
        let drawing = draw(&d, Units::Mm);
        assert_eq!(drawing.text, "Ø 2.0 mm 4×");
        assert_eq!(drawing.style.h_align, HAlign::Right);
        let mut leader = dim(DimensionKind::Leader, Point::mm(0, 0), Point::mm(5, 5), 0);
        leader.text = "Gold fingers".into();
        let drawing = draw(&leader, Units::Mm);
        assert_eq!(drawing.text, "Gold fingers");
        assert_eq!(drawing.value, 0.0);
    }

    #[test]
    fn values_and_units() {
        assert_eq!(format_value(25.4e6, Units::Inch, None, false), "1.000 in");
        assert_eq!(format_value(1.6e6, Units::Mm, Some(1), true), "1.6");
    }
}
