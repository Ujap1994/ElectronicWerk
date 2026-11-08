//! Units, coordinates, ids and layer names shared by every importer.

use std::collections::HashSet;

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::text::{HAlign, TextStyle, VAlign};
use ew_model::units::{Angle, Nm, Point};

use crate::sexpr::Sexp;

/// Millimetres (KiCad) to nanometres.
#[must_use]
pub fn nm(mm: f64) -> Nm {
    (mm * 1e6).round() as Nm
}

/// A KiCad angle in degrees (normalized to `[0, 360)`).
#[must_use]
pub fn angle(degrees: f64) -> Angle {
    let normalized = degrees.rem_euclid(360.0);
    Angle::from_degrees_f64(normalized).unwrap_or(Angle::ZERO)
}

/// Placement of KiCad coordinates in ElectronicWerk coordinates. KiCad
/// pages, sheets and boards have y pointing down from the top-left page
/// corner; ElectronicWerk has y pointing up from the bottom-left corner.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    /// Page height (nm).
    pub height: Nm,
}

impl Frame {
    /// Converts a page position.
    #[must_use]
    pub fn point(self, x: f64, y: f64) -> Point {
        Point::new(nm(x), self.height - nm(y))
    }

    /// `(at x y …)` or `(xy x y)` as a page position.
    #[must_use]
    pub fn at(self, node: Option<&Sexp<'_>>) -> Point {
        node.map_or(Point::ORIGIN, |n| {
            self.point(n.num(0).unwrap_or(0.0), n.num(1).unwrap_or(0.0))
        })
    }
}

/// Footprint-local KiCad coordinates (y down) as ElectronicWerk local
/// coordinates (y up).
#[must_use]
pub fn local(x: f64, y: f64) -> Point {
    Point::new(nm(x), -nm(y))
}

/// `(at x y)` of a footprint item as local coordinates.
#[must_use]
pub fn local_at(node: Option<&Sexp<'_>>) -> Point {
    node.map_or(Point::ORIGIN, |n| {
        local(n.num(0).unwrap_or(0.0), n.num(1).unwrap_or(0.0))
    })
}

/// Symbol library coordinates are already y up.
#[must_use]
pub fn up(node: Option<&Sexp<'_>>) -> Point {
    node.map_or(Point::ORIGIN, |n| {
        Point::new(nm(n.num(0).unwrap_or(0.0)), nm(n.num(1).unwrap_or(0.0)))
    })
}

/// The points of a `(pts (xy …) (xy …))` list, converted with `convert`.
/// Arcs inside point lists (`(arc (start) (mid) (end))`) are replaced by
/// their three points.
pub fn points(pts: Option<&Sexp<'_>>, convert: &dyn Fn(f64, f64) -> Point) -> Vec<Point> {
    let mut out = Vec::new();
    let Some(pts) = pts else {
        return out;
    };
    for item in pts.args() {
        if item.is("xy") {
            out.push(convert(
                item.num(0).unwrap_or(0.0),
                item.num(1).unwrap_or(0.0),
            ));
        } else if item.is("arc") {
            for name in ["start", "mid", "end"] {
                if let Some(p) = item.get(name) {
                    out.push(convert(p.num(0).unwrap_or(0.0), p.num(1).unwrap_or(0.0)));
                }
            }
        }
    }
    out
}

/// A KiCad text style from `(effects (font (size h w) (thickness t) bold
/// italic) (justify left bottom))`.
#[must_use]
pub fn text_style(effects: Option<&Sexp<'_>>) -> TextStyle {
    let mut style = TextStyle::schematic();
    let Some(effects) = effects else {
        return style;
    };
    if let Some(font) = effects.get("font") {
        if let Some(size) = font.get("size") {
            style.size = nm(size.num(0).unwrap_or(1.27));
        }
        style.thickness = font.number("thickness").map_or(0, nm);
        style.bold = font.flag("bold");
        style.italic = font.flag("italic");
    }
    if let Some(justify) = effects.get("justify") {
        if justify.has_word("left") {
            style.h_align = HAlign::Left;
        } else if justify.has_word("right") {
            style.h_align = HAlign::Right;
        }
        if justify.has_word("top") {
            style.v_align = VAlign::Top;
        } else if justify.has_word("bottom") {
            style.v_align = VAlign::Bottom;
        }
    }
    style
}

/// `true` if the text is hidden (`hide` in `effects` or `(hide yes)`).
#[must_use]
pub fn hidden(node: &Sexp<'_>) -> bool {
    node.flag("hide")
        || node
            .get("effects")
            .is_some_and(|effects| effects.flag("hide"))
}

/// Parses a UUID (`"0a64e4a4-91f7-…"`).
#[must_use]
pub fn parse_uuid(text: &str) -> Option<Id> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    u128::from_str_radix(&hex, 16).ok().map(Id::from_u128)
}

/// A stable id derived from text (FNV-1a, formatted as a UUID version 8).
#[must_use]
pub fn derived_id(parts: &[&str]) -> Id {
    let mut a: u64 = 0xcbf2_9ce4_8422_2325;
    let mut b: u64 = 0x8422_2325_cbf2_9ce4;
    for part in parts {
        for byte in part.bytes().chain(std::iter::once(0)) {
            a = (a ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            b = (b ^ u64::from(byte.rotate_left(3))).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    let mut value = (u128::from(a) << 64) | u128::from(b);
    // Version 8 (custom) and the RFC 4122 variant.
    value = (value & !(0xF_u128 << 76)) | (0x8_u128 << 76);
    value = (value & !(0x3_u128 << 62)) | (0x2_u128 << 62);
    Id::from_u128(value)
}

/// Hands out ids: KiCad's own UUIDs where possible, stable derived ids
/// otherwise, never the same id twice.
#[derive(Debug, Default)]
pub struct Ids {
    used: HashSet<Id>,
}

impl Ids {
    /// The KiCad UUID of `node` (`(uuid "…")`), or an id derived from
    /// `fallback`, made unique.
    pub fn of(&mut self, node: &Sexp<'_>, fallback: &[&str]) -> Id {
        let kicad = node
            .value("uuid")
            .or_else(|| node.value("tstamp"))
            .and_then(parse_uuid);
        self.take(kicad, fallback)
    }

    /// `preferred` if it is unused, otherwise an id derived from `fallback`.
    pub fn take(&mut self, preferred: Option<Id>, fallback: &[&str]) -> Id {
        if let Some(id) = preferred {
            if self.used.insert(id) {
                return id;
            }
        }
        let mut counter = 0_u32;
        loop {
            let suffix = counter.to_string();
            let mut parts = fallback.to_vec();
            parts.push(&suffix);
            let id = derived_id(&parts);
            if self.used.insert(id) {
                return id;
            }
            counter += 1;
        }
    }
}

/// A KiCad board layer name as an ElectronicWerk layer.
#[must_use]
pub fn layer(name: &str) -> Option<Layer> {
    if let Some(copper) = copper(name) {
        return Some(Layer::Copper(copper));
    }
    let sided = |side: Side| -> Option<Layer> {
        let rest = &name[2..];
        Some(match rest {
            "SilkS" | "Silkscreen" => Layer::Silkscreen(side),
            "Mask" => Layer::SolderMask(side),
            "Paste" => Layer::Paste(side),
            "Adhes" | "Adhesive" => Layer::Adhesive(side),
            "Fab" => Layer::Fabrication(side),
            "CrtYd" | "Courtyard" => Layer::Courtyard(side),
            _ => return None,
        })
    };
    if let Some(rest) = name.strip_prefix("User.") {
        // KiCad 9+: User.1 … User.n after the four classic user layers.
        let n: u8 = rest.parse().ok()?;
        return (n <= 28).then(|| Layer::User(n + 4));
    }
    match name {
        "Edge.Cuts" => Some(Layer::Outline),
        "Margin" => Some(Layer::Mechanical(1)),
        "Dwgs.User" | "User.Drawings" => Some(Layer::User(1)),
        "Cmts.User" | "User.Comments" => Some(Layer::User(2)),
        "Eco1.User" | "User.Eco1" => Some(Layer::User(3)),
        "Eco2.User" | "User.Eco2" => Some(Layer::User(4)),
        _ if name.starts_with("F.") => sided(Side::Top),
        _ if name.starts_with("B.") => sided(Side::Bottom),
        _ => None,
    }
}

/// A KiCad copper layer name (`F.Cu`, `In3.Cu`, `B.Cu`).
#[must_use]
pub fn copper(name: &str) -> Option<CopperLayer> {
    match name {
        "F.Cu" => Some(CopperLayer::Top),
        "B.Cu" => Some(CopperLayer::Bottom),
        _ => {
            let n: u8 = name.strip_prefix("In")?.strip_suffix(".Cu")?.parse().ok()?;
            (1..=62).contains(&n).then_some(CopperLayer::Inner(n))
        }
    }
}

/// A pad layer, including the KiCad wildcards.
#[must_use]
pub fn layer_specs(name: &str) -> Vec<LayerSpec> {
    match name {
        "*.Cu" => vec![LayerSpec::AllCopper],
        "*.Mask" => vec![LayerSpec::AllMask],
        "*.Paste" => vec![LayerSpec::AllPaste],
        "F&B.Cu" => vec![
            LayerSpec::Layer(Layer::TOP_COPPER),
            LayerSpec::Layer(Layer::BOTTOM_COPPER),
        ],
        "*.SilkS" => vec![
            LayerSpec::Layer(Layer::Silkscreen(Side::Top)),
            LayerSpec::Layer(Layer::Silkscreen(Side::Bottom)),
        ],
        _ => layer(name).map(LayerSpec::Layer).into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexpr::parse;

    #[test]
    fn converts_units_angles_and_page_positions() {
        assert_eq!(nm(1.27), 1_270_000);
        assert_eq!(nm(-0.0001), -100);
        assert_eq!(angle(-90.0), Angle::DEG_270);
        assert_eq!(angle(450.0), Angle::DEG_90);
        let frame = Frame { height: nm(297.0) };
        assert_eq!(frame.point(10.0, 7.0), Point::new(nm(10.0), nm(290.0)));
        assert_eq!(local(1.0, 2.0), Point::new(nm(1.0), -nm(2.0)));
    }

    #[test]
    fn maps_layers() {
        assert_eq!(layer("F.Cu"), Some(Layer::TOP_COPPER));
        assert_eq!(layer("In2.Cu"), Some(Layer::Copper(CopperLayer::Inner(2))));
        assert_eq!(layer("B.SilkS"), Some(Layer::Silkscreen(Side::Bottom)));
        assert_eq!(layer("F.Silkscreen"), Some(Layer::Silkscreen(Side::Top)));
        assert_eq!(layer("F.CrtYd"), Some(Layer::Courtyard(Side::Top)));
        assert_eq!(layer("Edge.Cuts"), Some(Layer::Outline));
        assert_eq!(layer("User.3"), Some(Layer::User(7)));
        assert_eq!(layer("Bogus"), None);
        assert_eq!(layer_specs("*.Cu"), vec![LayerSpec::AllCopper]);
        assert_eq!(layer_specs("F&B.Cu").len(), 2);
    }

    #[test]
    fn ids_are_stable_and_unique() {
        let uuid = "0a64e4a4-91f7-4c05-9e44-1c5c9b8c7d2e";
        let id = parse_uuid(uuid).unwrap();
        assert_eq!(id.to_string(), uuid);
        assert_eq!(derived_id(&["a", "b"]), derived_id(&["a", "b"]));
        assert_ne!(derived_id(&["a", "b"]), derived_id(&["ab"]));
        let mut ids = Ids::default();
        let text = format!("(x (uuid \"{uuid}\"))");
        let node = parse(&text).unwrap();
        assert_eq!(ids.of(&node, &["x"]), id);
        let again = ids.of(&node, &["x"]);
        assert_ne!(again, id, "a duplicate uuid gets a derived id");
    }

    #[test]
    fn reads_text_styles() {
        let node =
            parse("(effects (font (size 1.5 1.4) (thickness 0.2) bold) (justify left bottom))")
                .unwrap();
        let style = text_style(Some(&node));
        assert_eq!(style.size, nm(1.5));
        assert!(style.bold);
        assert_eq!(
            (style.h_align, style.v_align),
            (HAlign::Left, VAlign::Bottom)
        );
        let hidden_old = parse("(property (effects (font (size 1 1)) hide))").unwrap();
        assert!(hidden(&hidden_old));
    }
}
