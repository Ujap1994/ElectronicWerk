//! Reading Eagle XML: attributes, units, orientations, layers, arcs and
//! texts.

use ew_model::layer::{CopperLayer, Layer, Side};
use ew_model::shape::Geometry;
use ew_model::text::{HAlign, TextStyle, VAlign};
use ew_model::units::{Angle, Nm, Point};
use roxmltree::Node;

/// Millimetres (Eagle files are always in millimetres) to nanometres.
#[must_use]
pub fn nm(mm: f64) -> Nm {
    (mm * 1e6).round() as Nm
}

/// A number attribute (0 if missing or not a number).
#[must_use]
pub fn num(node: Node<'_, '_>, name: &str) -> f64 {
    opt(node, name).unwrap_or(0.0)
}

/// A number attribute, if there is one.
#[must_use]
pub fn opt(node: Node<'_, '_>, name: &str) -> Option<f64> {
    node.attribute(name).and_then(|v| v.trim().parse().ok())
}

/// A text attribute (empty if missing).
#[must_use]
pub fn attr<'a>(node: Node<'a, '_>, name: &str) -> &'a str {
    node.attribute(name).unwrap_or_default()
}

/// `yes`/`no` attribute with a default.
#[must_use]
pub fn flag(node: Node<'_, '_>, name: &str, default: bool) -> bool {
    match node.attribute(name) {
        Some("yes") => true,
        Some("no") => false,
        _ => default,
    }
}

/// A point from `x`/`y` attributes (or `x1`/`y1`, …).
#[must_use]
pub fn point(node: Node<'_, '_>, x: &str, y: &str) -> Point {
    Point::new(nm(num(node, x)), nm(num(node, y)))
}

/// The child elements with a tag name.
pub fn children<'a, 'input: 'a>(
    node: Node<'a, 'input>,
    tag: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    node.children()
        .filter(move |n| n.is_element() && n.has_tag_name(tag))
}

/// The first child element with a tag name.
#[must_use]
pub fn child<'a, 'input>(node: Node<'a, 'input>, tag: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|n| n.is_element() && n.has_tag_name(tag))
}

/// The element reached through a path of child tag names.
#[must_use]
pub fn path<'a, 'input>(node: Node<'a, 'input>, tags: &[&str]) -> Option<Node<'a, 'input>> {
    let mut current = node;
    for tag in tags {
        current = child(current, tag)?;
    }
    Some(current)
}

/// An Eagle orientation such as `R90`, `MR180` or `SMR270`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rot {
    /// Degrees counter-clockwise.
    pub angle: f64,
    /// Mirrored at the y axis (after turning).
    pub mirror: bool,
}

impl Rot {
    /// Reads the `rot` attribute.
    #[must_use]
    pub fn of(node: Node<'_, '_>) -> Self {
        Self::parse(attr(node, "rot"))
    }

    /// Parses `R90`, `MR180`, `SR0`, `SMR270` (spin is ignored).
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mirror = text.contains('M');
        let angle = text
            .trim_start_matches(['S', 'M', 'R'])
            .parse::<f64>()
            .unwrap_or(0.0);
        Self { angle, mirror }
    }

    /// The rotation in ElectronicWerk's order (mirror first, then turn):
    /// Eagle turns first and mirrors after, which is the same as mirroring
    /// first and turning the other way.
    #[must_use]
    pub fn model_angle(self) -> Angle {
        let degrees = if self.mirror { -self.angle } else { self.angle };
        Angle::from_degrees_f64(degrees.rem_euclid(360.0)).unwrap_or(Angle::ZERO)
    }

    /// Places a local point (relative to an origin) the way Eagle does.
    #[must_use]
    pub fn apply(self, p: Point) -> Point {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (x, y) = (p.x as f64, p.y as f64);
        let rx = x.mul_add(cos, -(y * sin));
        let ry = x.mul_add(sin, y * cos);
        let rx = if self.mirror { -rx } else { rx };
        Point::new(rx.round() as Nm, ry.round() as Nm)
    }
}

/// A line, or an arc when `curve` (degrees, counter-clockwise from the
/// start) is set.
#[must_use]
pub fn line_or_arc(start: Point, end: Point, curve: f64) -> Geometry {
    if curve.abs() < 1e-6 || start == end {
        return Geometry::Line { start, end };
    }
    let (sx, sy) = (start.x as f64, start.y as f64);
    let (ex, ey) = (end.x as f64, end.y as f64);
    let (dx, dy) = (ex - sx, ey - sy);
    let chord = dx.hypot(dy);
    let theta = curve.to_radians();
    // The centre lies on the perpendicular through the middle of the chord.
    let h = (chord / 2.0) / (theta / 2.0).tan();
    let (mx, my) = (f64::midpoint(sx, ex), f64::midpoint(sy, ey));
    let (nx, ny) = (-dy / chord, dx / chord);
    let (cx, cy) = (mx + nx * h, my + ny * h);
    let (sin, cos) = (theta / 2.0).sin_cos();
    let (rx, ry) = (sx - cx, sy - cy);
    let mid = Point::new(
        (cx + rx * cos - ry * sin).round() as Nm,
        (cy + rx * sin + ry * cos).round() as Nm,
    );
    Geometry::Arc { start, mid, end }
}

/// The geometry of a `wire` element.
#[must_use]
pub fn wire_geometry(node: Node<'_, '_>) -> Geometry {
    line_or_arc(
        point(node, "x1", "y1"),
        point(node, "x2", "y2"),
        num(node, "curve"),
    )
}

/// The points of a `polygon` element (curved edges become their ends).
#[must_use]
pub fn vertices(node: Node<'_, '_>) -> Vec<Point> {
    children(node, "vertex")
        .map(|v| point(v, "x", "y"))
        .collect()
}

/// A text style from `size`, `ratio` and `align`.
#[must_use]
pub fn text_style(node: Node<'_, '_>) -> TextStyle {
    let size = nm(opt(node, "size").unwrap_or(1.778));
    let ratio = opt(node, "ratio").unwrap_or(8.0);
    let (v_align, h_align) = match attr(node, "align") {
        "bottom-center" => (VAlign::Bottom, HAlign::Center),
        "bottom-right" => (VAlign::Bottom, HAlign::Right),
        "center-left" => (VAlign::Center, HAlign::Left),
        "center" => (VAlign::Center, HAlign::Center),
        "center-right" => (VAlign::Center, HAlign::Right),
        "top-left" => (VAlign::Top, HAlign::Left),
        "top-center" => (VAlign::Top, HAlign::Center),
        "top-right" => (VAlign::Top, HAlign::Right),
        _ => (VAlign::Bottom, HAlign::Left),
    };
    TextStyle {
        size,
        thickness: (size as f64 * ratio / 100.0).round() as Nm,
        h_align,
        v_align,
        ..TextStyle::default()
    }
}

/// The ElectronicWerk layer of an Eagle board layer number (`inner` maps
/// inner copper layer numbers to their place in the stack).
#[must_use]
pub fn board_layer(number: u32, inner: &dyn Fn(u32) -> Option<u8>) -> Option<Layer> {
    let top = Side::Top;
    let bottom = Side::Bottom;
    Some(match number {
        1 => Layer::Copper(CopperLayer::Top),
        2..=15 => Layer::Copper(CopperLayer::Inner(inner(number)?)),
        16 => Layer::Copper(CopperLayer::Bottom),
        20 | 46 => Layer::Outline,
        21 | 25 => Layer::Silkscreen(top),
        22 | 26 => Layer::Silkscreen(bottom),
        27 | 51 => Layer::Fabrication(top),
        28 | 52 => Layer::Fabrication(bottom),
        29 => Layer::SolderMask(top),
        30 => Layer::SolderMask(bottom),
        31 => Layer::Paste(top),
        32 => Layer::Paste(bottom),
        35 => Layer::Adhesive(top),
        36 => Layer::Adhesive(bottom),
        39 => Layer::Courtyard(top),
        40 => Layer::Courtyard(bottom),
        47 => Layer::User(1),
        48 => Layer::User(2),
        49 => Layer::User(3),
        100..=127 => Layer::User(u8::try_from(number - 95).ok()?),
        _ => return None,
    })
}

/// Millimetres for names (`3.2`).
#[must_use]
pub fn fmt_mm(value: Nm) -> String {
    let text = format!("{:.3}", value as f64 / 1e6);
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Pin and via restring Eagle uses when no diameter is given: a quarter of
/// the drill on each side, between 0.254 and 0.508 mm.
#[must_use]
pub fn auto_diameter(drill: f64) -> f64 {
    drill + 2.0 * (drill * 0.25).clamp(0.254, 0.508)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientations() {
        let p = Point::new(2_000_000, 1_000_000);
        assert_eq!(
            Rot::parse("R90").apply(p),
            Point::new(-1_000_000, 2_000_000)
        );
        assert_eq!(
            Rot::parse("MR0").apply(p),
            Point::new(-2_000_000, 1_000_000)
        );
        // Turned first, then mirrored (as KiCad reads Eagle files).
        assert_eq!(
            Rot::parse("MR90").apply(p),
            Point::new(1_000_000, 2_000_000)
        );
        assert_eq!(Rot::parse("MR90").model_angle(), Angle::DEG_270);
        assert_eq!(Rot::parse("SR45").angle, 45.0);
    }

    #[test]
    fn quarter_arc() {
        let arc = line_or_arc(Point::new(1_000_000, 0), Point::new(0, 1_000_000), 90.0);
        let Geometry::Arc { mid, .. } = arc else {
            panic!("an arc")
        };
        assert_eq!(mid, Point::new(707_107, 707_107));
        let back = line_or_arc(Point::new(1_000_000, 0), Point::new(0, 1_000_000), -270.0);
        let Geometry::Arc { mid, .. } = back else {
            panic!("an arc")
        };
        assert_eq!(mid, Point::new(-707_107, -707_107));
    }
}
