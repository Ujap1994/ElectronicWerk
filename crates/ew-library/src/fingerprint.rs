//! Source-independent fingerprints of symbols and footprints (ADR-0005).
//!
//! A fingerprint is a hash of a *canonical form*: coordinates are rounded to
//! 1 µm, footprints are moved so that their pads are centred on the origin
//! and turned by the quarter turn that gives the smallest canonical form,
//! and everything that does not change the object (names, texts, ids, the
//! order of the items) is left out. Two objects from different libraries,
//! tools or units therefore get the same fingerprint if they are the same.

use std::fmt::Write as _;

use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::library::{FootprintDef, SymbolDef};
use ew_model::pcb::{PadKind, PadShape};
use ew_model::shape::{Fill, Geometry};
use ew_model::units::{Nm, Point};
use sha2::{Digest, Sha256};

/// Rounding grid of canonical coordinates: 1 µm.
pub const GRID: Nm = 1_000;

/// Default tolerance for near duplicates: 0.05 mm (in µm, like canonical
/// coordinates).
pub const TOLERANCE_UM: i64 = 50;

/// A short, stable hash of canonical text (128 bits as hex).
#[must_use]
pub fn hash(text: &str) -> String {
    hex16(&Sha256::digest(text.as_bytes()))
}

/// The first 16 bytes of a digest as hex.
pub(crate) fn hex16(digest: &[u8]) -> String {
    let mut out = String::with_capacity(32);
    for byte in digest.iter().take(16) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Rounds nanometres to the canonical grid (µm).
const fn um(value: Nm) -> i64 {
    (value + GRID / 2).div_euclid(GRID)
}

/// Turns a point by `quarters` quarter turns counterclockwise.
const fn turn(x: i64, y: i64, quarters: u8) -> (i64, i64) {
    match quarters % 4 {
        1 => (-y, x),
        2 => (-x, -y),
        3 => (y, -x),
        _ => (x, y),
    }
}

// --- Footprints -----------------------------------------------------------

/// A pad in canonical form (µm, millidegrees).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonPad {
    /// Pad number.
    pub number: String,
    /// 0 SMD, 1 plated hole, 2 non-plated hole, 3 edge connector.
    pub kind: u8,
    /// Shape with its parameters (`rect`, `rr250`, …).
    pub shape: String,
    /// Centre.
    pub x: i64,
    /// Centre.
    pub y: i64,
    /// Width.
    pub w: i64,
    /// Height.
    pub h: i64,
    /// Rotation (millidegrees, normalized for symmetric shapes).
    pub rotation: i64,
    /// Hole: diameter, slot length, offset x, offset y.
    pub drill: Option<[i64; 4]>,
    /// Copper and mask/paste sides as bits.
    pub layers: u8,
}

/// The fingerprint of a footprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootprintPrint {
    /// Pads and drawn graphics (silkscreen, fabrication, courtyard).
    pub exact: String,
    /// Pads only.
    pub pads: String,
    /// Bucket for tolerance comparisons: pad count, numbers and kinds.
    pub coarse: String,
    /// The canonical pads (for tolerance comparisons).
    pub canon: Vec<CanonPad>,
}

fn pad_kind(kind: PadKind) -> u8 {
    match kind {
        PadKind::Smd => 0,
        PadKind::ThroughHole => 1,
        PadKind::NpThroughHole => 2,
        PadKind::Connector => 3,
    }
}

fn pad_layers(layers: &[LayerSpec]) -> u8 {
    const COPPER_TOP: u8 = 1;
    const COPPER_BOTTOM: u8 = 2;
    const COPPER_INNER: u8 = 4;
    const MASK_TOP: u8 = 8;
    const MASK_BOTTOM: u8 = 16;
    const PASTE_TOP: u8 = 32;
    const PASTE_BOTTOM: u8 = 64;
    let mut bits = 0;
    for spec in layers {
        bits |= match spec {
            LayerSpec::AllCopper => COPPER_TOP | COPPER_BOTTOM | COPPER_INNER,
            LayerSpec::AllMask => MASK_TOP | MASK_BOTTOM,
            LayerSpec::AllPaste => PASTE_TOP | PASTE_BOTTOM,
            LayerSpec::Layer(layer) => match layer {
                Layer::Copper(CopperLayer::Top) => COPPER_TOP,
                Layer::Copper(CopperLayer::Bottom) => COPPER_BOTTOM,
                Layer::Copper(CopperLayer::Inner(_)) => COPPER_INNER,
                Layer::SolderMask(Side::Top) => MASK_TOP,
                Layer::SolderMask(Side::Bottom) => MASK_BOTTOM,
                Layer::Paste(Side::Top) => PASTE_TOP,
                Layer::Paste(Side::Bottom) => PASTE_BOTTOM,
                _ => 0,
            },
        };
    }
    bits
}

fn shape_key(shape: &PadShape) -> String {
    match shape {
        PadShape::Circle => "circle".to_owned(),
        PadShape::Rect => "rect".to_owned(),
        PadShape::Oval => "oval".to_owned(),
        PadShape::RoundRect { ratio } => format!("rr{ratio}"),
        PadShape::Trapezoid { delta } => format!("trap{},{}", um(delta.w), um(delta.h)),
        PadShape::Custom { points } => {
            let points: Vec<(i64, i64)> = points.iter().map(|p| (um(p.x), um(p.y))).collect();
            format!("custom{}", hash(&format!("{points:?}")))
        }
    }
}

/// Puts the rotation of a pad into a normal form: shapes that look the same
/// after half a turn lose it, and a pad turned by 90° becomes an unturned
/// pad with swapped width and height.
fn normalize_pad(pad: &mut CanonPad) {
    pad.rotation = pad.rotation.rem_euclid(360_000);
    let plain_hole = pad
        .drill
        .is_none_or(|[_, slot, ox, oy]| slot == 0 && ox == 0 && oy == 0);
    if !plain_hole {
        return;
    }
    match pad.shape.as_str() {
        "circle" => pad.rotation = 0,
        s if s == "rect" || s == "oval" || s.starts_with("rr") => {
            pad.rotation %= 180_000;
            if pad.rotation == 90_000 {
                std::mem::swap(&mut pad.w, &mut pad.h);
                pad.rotation = 0;
            }
        }
        _ => {}
    }
}

/// Turns pads by quarter turns, centres them and sorts them.
fn place_pads(pads: &[CanonPad], quarters: u8) -> Vec<CanonPad> {
    let mut out: Vec<CanonPad> = pads
        .iter()
        .map(|pad| {
            let (x, y) = turn(pad.x, pad.y, quarters);
            let mut pad = CanonPad {
                x,
                y,
                rotation: pad.rotation + i64::from(quarters) * 90_000,
                ..pad.clone()
            };
            normalize_pad(&mut pad);
            pad
        })
        .collect();
    if let Some((cx, cy)) = centre(out.iter().map(|p| (p.x, p.y))) {
        for pad in &mut out {
            pad.x -= cx;
            pad.y -= cy;
        }
    }
    out.sort();
    out
}

/// Centre of the bounding box of points (rounded down).
fn centre(points: impl Iterator<Item = (i64, i64)>) -> Option<(i64, i64)> {
    let mut bounds: Option<(i64, i64, i64, i64)> = None;
    for (x, y) in points {
        bounds = Some(match bounds {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    }
    bounds.map(|(x0, y0, x1, y1)| ((x0 + x1).div_euclid(2), (y0 + y1).div_euclid(2)))
}

fn write_pads(out: &mut String, pads: &[CanonPad]) {
    for pad in pads {
        let _ = write!(
            out,
            "P{}|{}|{}|{},{}|{}x{}|{}|{:?}|{};",
            pad.number,
            pad.kind,
            pad.shape,
            pad.x,
            pad.y,
            pad.w,
            pad.h,
            pad.rotation,
            pad.drill,
            pad.layers
        );
    }
}

fn layer_class(layer: Layer) -> Option<u8> {
    Some(match layer {
        Layer::Silkscreen(Side::Top) => 1,
        Layer::Silkscreen(Side::Bottom) => 2,
        Layer::Fabrication(Side::Top) => 3,
        Layer::Fabrication(Side::Bottom) => 4,
        Layer::Courtyard(Side::Top) => 5,
        Layer::Courtyard(Side::Bottom) => 6,
        Layer::Copper(_) => 7,
        Layer::SolderMask(_) | Layer::Paste(_) => 8,
        _ => return None,
    })
}

/// Canonical text of a geometry after turning and moving it.
fn geometry_text(geometry: &Geometry, quarters: u8, offset: (i64, i64)) -> String {
    let p = |point: Point| {
        let (x, y) = turn(um(point.x), um(point.y), quarters);
        (x - offset.0, y - offset.1)
    };
    match geometry {
        Geometry::Line { start, end } => {
            let (a, b) = (p(*start), p(*end));
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            format!("L{a:?}{b:?}")
        }
        Geometry::Rect { start, end } => {
            let (a, b) = (p(*start), p(*end));
            format!(
                "R{:?}{:?}",
                (a.0.min(b.0), a.1.min(b.1)),
                (a.0.max(b.0), a.1.max(b.1))
            )
        }
        Geometry::Circle { center, radius } => format!("C{:?}{}", p(*center), um(*radius)),
        Geometry::Arc { start, mid, end } => {
            let (a, m, b) = (p(*start), p(*mid), p(*end));
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            format!("A{a:?}{m:?}{b:?}")
        }
        Geometry::Polyline { points } => {
            let mut list: Vec<_> = points.iter().map(|q| p(*q)).collect();
            if list.last() < list.first() {
                list.reverse();
            }
            format!("Y{list:?}")
        }
        Geometry::Polygon { points } => {
            let mut list: Vec<_> = points.iter().map(|q| p(*q)).collect();
            if let Some(start) = list
                .iter()
                .enumerate()
                .min_by_key(|(_, point)| **point)
                .map(|(index, _)| index)
            {
                list.rotate_left(start);
            }
            format!("G{list:?}")
        }
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => format!(
            "B{:?}{:?}{:?}{:?}",
            p(*start),
            p(*control1),
            p(*control2),
            p(*end)
        ),
    }
}

/// The fingerprint of a footprint.
#[must_use]
pub fn footprint(def: &FootprintDef) -> FootprintPrint {
    let pads: Vec<CanonPad> = def
        .pads
        .iter()
        .map(|pad| {
            let mut canon = CanonPad {
                number: pad.number.clone(),
                kind: pad_kind(pad.kind),
                shape: shape_key(&pad.shape),
                x: um(pad.at.x),
                y: um(pad.at.y),
                w: um(pad.size.w),
                h: um(pad.size.h),
                rotation: i64::from(pad.rotation.millidegrees()),
                drill: pad.drill.as_ref().map(|d| {
                    [
                        um(d.diameter),
                        d.slot_length.map_or(0, um),
                        um(d.offset.x),
                        um(d.offset.y),
                    ]
                }),
                layers: pad_layers(&pad.layers),
            };
            normalize_pad(&mut canon);
            canon
        })
        .collect();

    // The quarter turn with the smallest canonical pads (graphics decide
    // for footprints without pads).
    let mut best: Option<(String, u8, Vec<CanonPad>)> = None;
    for quarters in 0..4 {
        let placed = place_pads(&pads, quarters);
        let mut text = String::new();
        write_pads(&mut text, &placed);
        if pads.is_empty() {
            text = graphics_text(def, quarters, graphics_centre(def, quarters));
        }
        if best.as_ref().is_none_or(|(b, _, _)| text < *b) {
            best = Some((text, quarters, placed));
        }
    }
    let (pad_text, quarters, canon) = best.unwrap_or_default();
    // The same move as the pads: the centre of the turned pad centres.
    let offset = centre(pads.iter().map(|pad| turn(pad.x, pad.y, quarters)))
        .unwrap_or_else(|| graphics_centre(def, quarters));
    let graphics = graphics_text(def, quarters, offset);
    let mut numbers: Vec<&str> = def.pads.iter().map(|p| p.number.as_str()).collect();
    numbers.sort_unstable();
    let mut kinds: Vec<u8> = pads.iter().map(|p| p.kind).collect();
    kinds.sort_unstable();
    FootprintPrint {
        exact: hash(&format!("{pad_text}#{graphics}")),
        pads: hash(&pad_text),
        coarse: format!("{}|{}|{kinds:?}", def.pads.len(), numbers.join(",")),
        canon,
    }
}

fn graphics_centre(def: &FootprintDef, quarters: u8) -> (i64, i64) {
    let points = def
        .graphics
        .iter()
        .flat_map(|g| geometry_points(&g.geometry));
    centre(points.map(|point| turn(um(point.x), um(point.y), quarters))).unwrap_or((0, 0))
}

fn geometry_points(geometry: &Geometry) -> Vec<Point> {
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

fn graphics_text(def: &FootprintDef, quarters: u8, offset: (i64, i64)) -> String {
    let mut items: Vec<String> = def
        .graphics
        .iter()
        .filter_map(|graphic| {
            let class = layer_class(graphic.layer)?;
            Some(format!(
                "{class}|{}|{}|{}",
                geometry_text(&graphic.geometry, quarters, offset),
                um(graphic.width),
                u8::from(graphic.filled)
            ))
        })
        .collect();
    items.sort_unstable();
    items.join(";")
}

/// `true` if two canonical pad lists match within `tolerance` µm, in any
/// quarter turn of `b`.
#[must_use]
pub fn pads_close(a: &[CanonPad], b: &[CanonPad], tolerance: i64) -> bool {
    if a.len() != b.len() {
        return false;
    }
    (0..4).any(|quarters| {
        let turned = place_pads(b, quarters);
        a.iter().zip(&turned).all(|(p, q)| {
            p.number == q.number
                && p.kind == q.kind
                && p.shape == q.shape
                && p.layers == q.layers
                && (p.x - q.x).abs() <= tolerance
                && (p.y - q.y).abs() <= tolerance
                && (p.w - q.w).abs() <= tolerance
                && (p.h - q.h).abs() <= tolerance
                && (p.rotation - q.rotation).abs() <= 1_000
                && match (p.drill, q.drill) {
                    (None, None) => true,
                    (Some(d), Some(e)) => d.iter().zip(&e).all(|(u, v)| (u - v).abs() <= tolerance),
                    _ => false,
                }
        })
    })
}

// --- Symbols --------------------------------------------------------------

/// The fingerprint of a symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolPrint {
    /// The complete drawing: pins, graphics and texts of every unit.
    pub drawing: String,
    /// The pins only (number, name, type, position, direction, length).
    pub pins: String,
    /// The pin numbers (to compare symbols with the same name).
    pub numbers: String,
}

/// The fingerprint of a symbol.
#[must_use]
pub fn symbol(def: &SymbolDef) -> SymbolPrint {
    let mut pins = String::new();
    let mut drawing = String::new();
    let mut numbers: Vec<String> = Vec::new();
    for (index, unit) in def.units.iter().enumerate() {
        let mut unit_pins: Vec<String> = unit
            .pins
            .iter()
            .map(|pin| {
                numbers.push(pin.number.clone());
                format!(
                    "{}|{}|{:?}|{},{}|{:?}|{}|{}",
                    pin.number,
                    pin.name.trim().to_lowercase(),
                    pin.kind,
                    um(pin.at.x),
                    um(pin.at.y),
                    pin.direction,
                    um(pin.length),
                    u8::from(pin.hidden)
                )
            })
            .collect();
        unit_pins.sort_unstable();
        let _ = write!(pins, "U{index}:{};", unit_pins.join(";"));
        let mut graphics: Vec<String> = unit
            .graphics
            .iter()
            .map(|graphic| {
                let fill = match graphic.fill {
                    Fill::None => 0,
                    Fill::Solid => 1,
                    Fill::Background => 2,
                };
                format!("{}|{fill}", geometry_text(&graphic.geometry, 0, (0, 0)))
            })
            .chain(unit.texts.iter().map(|text| {
                format!(
                    "T{}|{},{}|{}",
                    text.text,
                    um(text.at.x),
                    um(text.at.y),
                    text.rotation.millidegrees()
                )
            }))
            .collect();
        graphics.sort_unstable();
        let _ = write!(drawing, "U{index}:{};", graphics.join(";"));
    }
    numbers.sort_unstable();
    numbers.dedup();
    SymbolPrint {
        drawing: hash(&format!("{pins}#{drawing}")),
        pins: hash(&pins),
        numbers: hash(&numbers.join(",")),
    }
}

#[cfg(test)]
mod tests {
    use ew_model::id::Id;
    use ew_model::pcb::{Drill, Pad};
    use ew_model::units::{Angle, Size, mm};

    use super::*;

    fn pad(number: &str, x: f64, y: f64, w: f64, h: f64, rotation: i64) -> Pad {
        Pad {
            id: Id::random(),
            number: number.to_owned(),
            net: None,
            kind: PadKind::Smd,
            shape: PadShape::RoundRect { ratio: 250 },
            at: Point::new(mm_f(x), mm_f(y)),
            rotation: Angle::from_degrees(rotation),
            size: Size::new(mm_f(w), mm_f(h)),
            drill: None,
            layers: vec![LayerSpec::Layer(Layer::Copper(CopperLayer::Top))],
            clearance: None,
            mask_margin: None,
            paste_margin: None,
        }
    }

    fn mm_f(value: f64) -> Nm {
        (value * 1e6).round() as Nm
    }

    fn two_pads(name: &str, pads: Vec<Pad>) -> FootprintDef {
        FootprintDef {
            id: Id::random(),
            name: name.to_owned(),
            description: String::new(),
            keywords: String::new(),
            pads,
            graphics: Vec::new(),
            texts: Vec::new(),
            models: Vec::new(),
            attributes: ew_model::pcb::FootprintAttributes::default(),
        }
    }

    #[test]
    fn origin_rotation_and_names_do_not_matter() {
        let a = two_pads(
            "A:R_0805",
            vec![
                pad("1", -0.9, 0.0, 1.0, 1.4, 0),
                pad("2", 0.9, 0.0, 1.0, 1.4, 0),
            ],
        );
        // The same footprint turned by 90° and moved.
        let b = two_pads(
            "B:Other",
            vec![
                pad("1", 5.0, 2.1, 1.0, 1.4, 90),
                pad("2", 5.0, 3.9, 1.0, 1.4, 90),
            ],
        );
        let (pa, pb) = (footprint(&a), footprint(&b));
        assert_eq!(pa.pads, pb.pads);
        assert_eq!(pa.exact, pb.exact);
        // Swapped pad numbers are a different footprint.
        let c = two_pads(
            "C:Swapped",
            vec![
                pad("2", -0.9, 0.0, 1.0, 1.4, 0),
                pad("1", 0.9, 0.0, 1.0, 1.4, 0),
            ],
        );
        // (mirror symmetric: swapping numbers equals a half turn)
        assert_eq!(footprint(&c).pads, pa.pads);
        let d = two_pads(
            "D:Wider",
            vec![
                pad("1", -0.95, 0.0, 1.0, 1.4, 0),
                pad("2", 0.95, 0.0, 1.0, 1.4, 0),
            ],
        );
        let pd = footprint(&d);
        assert_ne!(pd.pads, pa.pads);
        assert!(pads_close(&pa.canon, &pd.canon, TOLERANCE_UM));
        assert!(!pads_close(&pa.canon, &pd.canon, 10));
        assert_eq!(pa.coarse, pd.coarse);
    }

    #[test]
    fn graphics_change_only_the_exact_print() {
        let mut a = two_pads(
            "A:R",
            vec![
                pad("1", -0.9, 0.0, 1.0, 1.4, 0),
                pad("2", 0.9, 0.0, 1.0, 1.4, 0),
            ],
        );
        let b = a.clone();
        a.graphics.push(ew_model::pcb::Graphic {
            layer: Layer::Silkscreen(Side::Top),
            geometry: Geometry::Line {
                start: Point::new(mm(-1), mm(1)),
                end: Point::new(mm(1), mm(1)),
            },
            width: mm_f(0.12),
            filled: false,
        });
        let (pa, pb) = (footprint(&a), footprint(&b));
        assert_eq!(pa.pads, pb.pads);
        assert_ne!(pa.exact, pb.exact);
    }

    #[test]
    fn holes_and_kinds_are_part_of_the_print() {
        let mut a = two_pads("A:H", vec![pad("1", 0.0, 0.0, 1.7, 1.7, 0)]);
        a.pads[0].kind = PadKind::ThroughHole;
        a.pads[0].shape = PadShape::Circle;
        a.pads[0].drill = Some(Drill {
            diameter: mm(1),
            slot_length: None,
            offset: Point::default(),
        });
        let mut b = a.clone();
        b.pads[0].drill = Some(Drill {
            diameter: mm_f(0.9),
            slot_length: None,
            offset: Point::default(),
        });
        let (pa, pb) = (footprint(&a), footprint(&b));
        assert_ne!(pa.pads, pb.pads);
        assert!(!pads_close(&pa.canon, &pb.canon, TOLERANCE_UM));
        assert!(pads_close(&pa.canon, &pb.canon, 100));
    }

    #[test]
    fn symbol_prints_ignore_names_and_order() {
        use ew_model::library::{Pin, PinDirection, PinKind, PinShape, SymbolUnit};
        let pin = |number: &str, y: i64| Pin {
            number: number.to_owned(),
            name: "~".to_owned(),
            kind: PinKind::Passive,
            shape: PinShape::Line,
            at: Point::new(0, mm(y)),
            direction: if y > 0 {
                PinDirection::Down
            } else {
                PinDirection::Up
            },
            length: mm(1),
            hidden: false,
        };
        let a = SymbolDef {
            id: Id::random(),
            name: "Device:R".to_owned(),
            description: String::new(),
            keywords: String::new(),
            prefix: "R".to_owned(),
            value: "R".to_owned(),
            footprint: None,
            datasheet: String::new(),
            power: false,
            units: vec![SymbolUnit {
                pins: vec![pin("1", 3), pin("2", -3)],
                ..SymbolUnit::default()
            }],
            properties: std::collections::BTreeMap::new(),
            pin_text: ew_model::library::PinText::default(),
        };
        let mut b = a.clone();
        b.name = "Other:R_Small".to_owned();
        b.units[0].pins.reverse();
        let (pa, pb) = (symbol(&a), symbol(&b));
        assert_eq!(pa, pb);
        b.units[0].pins[0].kind = PinKind::Input;
        let pb = symbol(&b);
        assert_ne!(pa.pins, pb.pins);
        assert_eq!(pa.numbers, pb.numbers);
    }
}
