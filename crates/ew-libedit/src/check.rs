//! Library checks, modelled on the KiCad library conventions: symbols with
//! pins on the grid and unique numbers, footprints with a courtyard around
//! every pad and a silkscreen clear of the pads, components whose symbol
//! pins and footprint pads match.

use std::collections::{BTreeMap, BTreeSet};

use ew_model::layer::{Layer, LayerSpec};
use ew_model::library::{ComponentDef, FootprintDef, Pin, PinKind, SymbolDef};
use ew_model::pcb::{FootprintTextKind, PadKind};
use ew_model::shape::Geometry;
use ew_model::units::{Nm, Point};
use serde::Serialize;

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// The part is wrong (it will not work or not connect).
    Error,
    /// Probably a mistake.
    Warning,
    /// A hint.
    Info,
}

/// What a finding points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    /// `pin`, `graphic`, `text`, `pad` or `unit`.
    pub kind: String,
    /// Symbol unit (1-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<u16>,
    /// In the alternate body.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub alternate: bool,
    /// Index in the list of its kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

/// One finding of a check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Check code (`pin-off-grid`, `no-courtyard`, …).
    pub code: String,
    /// Severity.
    pub severity: Severity,
    /// What is wrong, in English (the app translates by code).
    pub message: String,
    /// Values for the translated message.
    pub args: BTreeMap<String, String>,
    /// The item concerned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
}

fn finding(
    code: &str,
    severity: Severity,
    message: String,
    args: &[(&str, String)],
    target: Option<Target>,
) -> Finding {
    Finding {
        code: code.to_owned(),
        severity,
        message,
        args: args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        target,
    }
}

fn pin_target(unit: u16, alternate: bool, index: usize) -> Target {
    Target {
        kind: "pin".to_owned(),
        unit: Some(unit),
        alternate,
        index: Some(index),
    }
}

/// The symbol grid: pins end on it (50 mil).
pub const SYMBOL_GRID: Nm = 1_270_000;

fn mm(value: Nm) -> String {
    let text = format!("{:.3}", value as f64 / 1_000_000.0);
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Checks a symbol.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one block per check")]
pub fn check_symbol(def: &SymbolDef) -> Vec<Finding> {
    let mut out = Vec::new();
    if def.prefix.trim().is_empty() {
        out.push(finding(
            "no-prefix",
            Severity::Error,
            "the symbol has no designator prefix".to_owned(),
            &[],
            None,
        ));
    }
    if def.description.trim().is_empty() {
        out.push(finding(
            "no-description",
            Severity::Info,
            "the symbol has no description".to_owned(),
            &[],
            None,
        ));
    }
    if def.power {
        let pins: usize = def.units.iter().map(|u| u.pins.len()).sum();
        if pins != 1 {
            out.push(finding(
                "power-symbol-pins",
                Severity::Error,
                format!("a power symbol needs exactly one pin, this one has {pins}"),
                &[("count", pins.to_string())],
                None,
            ));
        }
    }
    // Pin numbers across units: the same number in two units must be the
    // same pin (a shared supply pin), not two different ones.
    let mut numbers: BTreeMap<&str, (&Pin, u16)> = BTreeMap::new();
    for (u, unit) in def.units.iter().enumerate() {
        let unit_no = u16::try_from(u + 1).unwrap_or(u16::MAX);
        if unit.pins.is_empty() && !def.power {
            out.push(finding(
                "unit-without-pins",
                Severity::Warning,
                format!("unit {unit_no} has no pins"),
                &[("unit", unit_no.to_string())],
                Some(Target {
                    kind: "unit".to_owned(),
                    unit: Some(unit_no),
                    alternate: false,
                    index: None,
                }),
            ));
        }
        let bodies = std::iter::once((false, &unit.pins))
            .chain(unit.alternate.as_ref().map(|b| (true, &b.pins)));
        for (alternate, pins) in bodies {
            let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
            let mut positions: BTreeMap<Point, usize> = BTreeMap::new();
            for (index, pin) in pins.iter().enumerate() {
                let label = format!("{} ({})", pin.number, pin.name);
                if pin.number.trim().is_empty() {
                    out.push(finding(
                        "pin-without-number",
                        Severity::Error,
                        format!("pin {} has no number", pin.name),
                        &[("pin", pin.name.clone())],
                        Some(pin_target(unit_no, alternate, index)),
                    ));
                } else if let Some(&first) = seen.get(pin.number.as_str()) {
                    let other = &pins[first];
                    // Stacked pins (same place, same name) are allowed.
                    if other.at != pin.at || other.name != pin.name {
                        out.push(finding(
                            "duplicate-pin-number",
                            Severity::Error,
                            format!("pin number {} is used twice", pin.number),
                            &[("number", pin.number.clone())],
                            Some(pin_target(unit_no, alternate, index)),
                        ));
                    }
                } else {
                    seen.insert(&pin.number, index);
                }
                if pin.at.x % SYMBOL_GRID != 0 || pin.at.y % SYMBOL_GRID != 0 {
                    out.push(finding(
                        "pin-off-grid",
                        Severity::Warning,
                        format!("pin {label} is not on the 1.27 mm grid"),
                        &[("pin", label.clone())],
                        Some(pin_target(unit_no, alternate, index)),
                    ));
                }
                if pin.length % SYMBOL_GRID != 0 {
                    out.push(finding(
                        "pin-length",
                        Severity::Info,
                        format!(
                            "pin {label} is {} mm long, not a multiple of 1.27 mm",
                            mm(pin.length)
                        ),
                        &[("pin", label.clone()), ("length", mm(pin.length))],
                        Some(pin_target(unit_no, alternate, index)),
                    ));
                }
                if let Some(&other) = positions.get(&pin.at) {
                    let first = &pins[other];
                    if first.name != pin.name && !pin.hidden && !first.hidden {
                        out.push(finding(
                            "overlapping-pins",
                            Severity::Warning,
                            format!(
                                "pins {} and {} are on the same spot",
                                first.number, pin.number
                            ),
                            &[("pin", first.number.clone()), ("other", pin.number.clone())],
                            Some(pin_target(unit_no, alternate, index)),
                        ));
                    }
                } else {
                    positions.insert(pin.at, index);
                }
                if pin.hidden && pin.kind == PinKind::PowerIn && !def.power {
                    out.push(finding(
                        "hidden-power-pin",
                        Severity::Warning,
                        format!(
                            "the supply pin {label} is hidden; it connects to its net name silently"
                        ),
                        &[("pin", label.clone())],
                        Some(pin_target(unit_no, alternate, index)),
                    ));
                }
                if !alternate && !pin.number.is_empty() {
                    match numbers.get(pin.number.as_str()) {
                        Some((first, first_unit))
                            if *first_unit != unit_no
                                && (first.name != pin.name || first.kind != pin.kind) =>
                        {
                            out.push(finding(
                                "duplicate-pin-number",
                                Severity::Error,
                                format!(
                                    "pin number {} is used in units {first_unit} and {unit_no}",
                                    pin.number
                                ),
                                &[("number", pin.number.clone())],
                                Some(pin_target(unit_no, false, index)),
                            ));
                        }
                        Some(_) => {}
                        None => {
                            numbers.insert(&pin.number, (pin, unit_no));
                        }
                    }
                }
            }
        }
        if let Some(body) = &unit.alternate {
            let normal: BTreeSet<&str> = unit.pins.iter().map(|p| p.number.as_str()).collect();
            let alternate: BTreeSet<&str> = body.pins.iter().map(|p| p.number.as_str()).collect();
            if normal != alternate {
                out.push(finding(
                    "alternate-pins-differ",
                    Severity::Error,
                    format!("the alternate body of unit {unit_no} has other pin numbers than the normal one"),
                    &[("unit", unit_no.to_string())],
                    Some(Target {
                        kind: "unit".to_owned(),
                        unit: Some(unit_no),
                        alternate: true,
                        index: None,
                    }),
                ));
            }
        }
    }
    out
}

/// The bounding box of a geometry `[min x, min y, max x, max y]`.
fn geometry_box(geometry: &Geometry) -> Option<[Nm; 4]> {
    let points: Vec<Point> = match geometry {
        Geometry::Line { start, end } | Geometry::Rect { start, end } => vec![*start, *end],
        Geometry::Circle { center, radius } => vec![
            Point::new(center.x - radius, center.y - radius),
            Point::new(center.x + radius, center.y + radius),
        ],
        Geometry::Arc { start, mid, end } => vec![*start, *mid, *end],
        Geometry::Polyline { points } | Geometry::Polygon { points } => points.clone(),
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => vec![*start, *control1, *control2, *end],
    };
    let first = points.first()?;
    let mut b = [first.x, first.y, first.x, first.y];
    for p in &points {
        b = [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)];
    }
    Some(b)
}

/// The segments of a drawn outline (for the silkscreen check).
fn segments(geometry: &Geometry) -> Vec<(Point, Point)> {
    match geometry {
        Geometry::Line { start, end } => vec![(*start, *end)],
        Geometry::Rect { start, end } => {
            let (a, c) = (*start, *end);
            let (b, d) = (Point::new(c.x, a.y), Point::new(a.x, c.y));
            vec![(a, b), (b, c), (c, d), (d, a)]
        }
        Geometry::Polyline { points } => points.windows(2).map(|w| (w[0], w[1])).collect(),
        Geometry::Polygon { points } => {
            let mut out: Vec<(Point, Point)> = points.windows(2).map(|w| (w[0], w[1])).collect();
            if let (Some(first), Some(last)) = (points.first(), points.last()) {
                out.push((*last, *first));
            }
            out
        }
        Geometry::Arc { start, mid, end } => vec![(*start, *mid), (*mid, *end)],
        Geometry::Circle { .. } | Geometry::Bezier { .. } => Vec::new(),
    }
}

/// `true` if the segment a–b touches the box (Liang–Barsky clipping).
fn segment_hits_box(a: Point, b: Point, bx: [f64; 4]) -> bool {
    let (x0, y0) = (a.x as f64, a.y as f64);
    let (dx, dy) = (b.x as f64 - x0, b.y as f64 - y0);
    let mut t0: f64 = 0.0;
    let mut t1: f64 = 1.0;
    for (p, q) in [
        (-dx, x0 - bx[0]),
        (dx, bx[2] - x0),
        (-dy, y0 - bx[1]),
        (dy, bx[3] - y0),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// The box of a pad (rotation ignored except for quarter turns).
fn pad_box(pad: &ew_model::pcb::Pad) -> [Nm; 4] {
    let quarter = (pad.rotation.millidegrees() / 90_000) % 2 != 0;
    let (w, h) = if quarter {
        (pad.size.h, pad.size.w)
    } else {
        (pad.size.w, pad.size.h)
    };
    [
        pad.at.x - w / 2,
        pad.at.y - h / 2,
        pad.at.x + w / 2,
        pad.at.y + h / 2,
    ]
}

fn pad_target(index: usize) -> Target {
    Target {
        kind: "pad".to_owned(),
        unit: None,
        alternate: false,
        index: Some(index),
    }
}

/// Silkscreen clearance from pads.
pub const SILK_CLEARANCE: Nm = 100_000;

/// Checks a footprint.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one block per check")]
pub fn check_footprint(def: &FootprintDef) -> Vec<Finding> {
    let mut out = Vec::new();
    let on = |kind: fn(Layer) -> bool| def.graphics.iter().filter(move |g| kind(g.layer));
    let courtyards: Vec<[Nm; 4]> = on(|l| matches!(l, Layer::Courtyard(_)))
        .filter_map(|g| geometry_box(&g.geometry))
        .collect();
    if courtyards.is_empty() {
        out.push(finding(
            "no-courtyard",
            Severity::Error,
            "the footprint has no courtyard".to_owned(),
            &[],
            None,
        ));
    } else {
        let c = courtyards
            .iter()
            .fold([Nm::MAX, Nm::MAX, Nm::MIN, Nm::MIN], |a, b| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            });
        for (index, pad) in def.pads.iter().enumerate() {
            let b = pad_box(pad);
            if b[0] < c[0] || b[1] < c[1] || b[2] > c[2] || b[3] > c[3] {
                out.push(finding(
                    "pad-outside-courtyard",
                    Severity::Error,
                    format!("pad {} reaches outside the courtyard", pad.number),
                    &[("pad", pad.number.clone())],
                    Some(pad_target(index)),
                ));
            }
        }
    }
    if on(|l| matches!(l, Layer::Fabrication(_))).next().is_none() {
        out.push(finding(
            "no-fab-outline",
            Severity::Warning,
            "the footprint has no outline on the fabrication layer".to_owned(),
            &[],
            None,
        ));
    }
    if !def
        .texts
        .iter()
        .any(|t| t.kind == FootprintTextKind::Designator)
    {
        out.push(finding(
            "no-designator",
            Severity::Error,
            "the footprint has no designator text".to_owned(),
            &[],
            None,
        ));
    }
    if !def.texts.iter().any(|t| t.kind == FootprintTextKind::Value) {
        out.push(finding(
            "no-value",
            Severity::Warning,
            "the footprint has no value text".to_owned(),
            &[],
            None,
        ));
    }
    if def.models.is_empty() {
        out.push(finding(
            "no-3d-model",
            Severity::Info,
            "the footprint has no 3D model".to_owned(),
            &[],
            None,
        ));
    }
    let mut numbered = false;
    for (index, pad) in def.pads.iter().enumerate() {
        let number = pad.number.clone();
        if pad.kind != PadKind::NpThroughHole && number.trim().is_empty() {
            out.push(finding(
                "pad-without-number",
                Severity::Error,
                "a pad has no number".to_owned(),
                &[],
                Some(pad_target(index)),
            ));
        }
        if number == "1" || number == "A1" {
            numbered = true;
        }
        match pad.kind {
            PadKind::Smd | PadKind::Connector => {
                if pad.drill.is_some() {
                    out.push(finding(
                        "smd-pad-with-drill",
                        Severity::Error,
                        format!("SMD pad {number} has a drill"),
                        &[("pad", number.clone())],
                        Some(pad_target(index)),
                    ));
                }
                let paste = pad
                    .layers
                    .iter()
                    .any(|l| matches!(l, LayerSpec::AllPaste | LayerSpec::Layer(Layer::Paste(_))));
                if pad.kind == PadKind::Smd && !paste {
                    out.push(finding(
                        "smd-pad-without-paste",
                        Severity::Warning,
                        format!("SMD pad {number} has no solder paste"),
                        &[("pad", number.clone())],
                        Some(pad_target(index)),
                    ));
                }
            }
            PadKind::ThroughHole | PadKind::NpThroughHole => match pad.drill {
                None => out.push(finding(
                    "hole-without-drill",
                    Severity::Error,
                    format!("through-hole pad {number} has no drill"),
                    &[("pad", number.clone())],
                    Some(pad_target(index)),
                )),
                Some(drill)
                    if pad.kind == PadKind::ThroughHole
                        && drill.diameter >= pad.size.w.min(pad.size.h) =>
                {
                    out.push(finding(
                        "drill-too-large",
                        Severity::Error,
                        format!("the drill of pad {number} is as large as the pad"),
                        &[("pad", number.clone())],
                        Some(pad_target(index)),
                    ));
                }
                Some(_) => {}
            },
        }
    }
    if !def.pads.is_empty()
        && !numbered
        && def.pads.iter().any(|p| p.kind != PadKind::NpThroughHole)
    {
        out.push(finding(
            "no-pad-1",
            Severity::Warning,
            "no pad is numbered 1 (or A1)".to_owned(),
            &[],
            None,
        ));
    }
    // Silkscreen must keep clear of pads (it would be printed on solder).
    let clearance = SILK_CLEARANCE as f64;
    for graphic in on(|l| matches!(l, Layer::Silkscreen(_))) {
        let half = graphic.width as f64 / 2.0;
        for (index, pad) in def.pads.iter().enumerate() {
            let b = pad_box(pad);
            let grown = [
                b[0] as f64 - clearance - half,
                b[1] as f64 - clearance - half,
                b[2] as f64 + clearance + half,
                b[3] as f64 + clearance + half,
            ];
            if segments(&graphic.geometry)
                .iter()
                .any(|(a, c)| segment_hits_box(*a, *c, grown))
            {
                out.push(finding(
                    "silk-over-pad",
                    Severity::Warning,
                    format!(
                        "the silkscreen comes closer than 0.1 mm to pad {}",
                        pad.number
                    ),
                    &[("pad", pad.number.clone())],
                    Some(pad_target(index)),
                ));
                break;
            }
        }
    }
    out
}

/// Checks a component against its symbol and footprint (`None` if missing).
#[must_use]
pub fn check_component(
    def: &ComponentDef,
    symbol: Option<&SymbolDef>,
    footprint: Option<&FootprintDef>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    let Some(symbol) = symbol else {
        out.push(finding(
            "missing-symbol",
            Severity::Error,
            format!("symbol {} is not in a project library", def.symbol),
            &[("name", def.symbol.clone())],
            None,
        ));
        return out;
    };
    if def.mpn.trim().is_empty() {
        out.push(finding(
            "no-mpn",
            Severity::Info,
            "the component has no manufacturer part number".to_owned(),
            &[],
            None,
        ));
    }
    match (&def.footprint, footprint) {
        (None, _) => {
            if !symbol.power {
                out.push(finding(
                    "no-footprint",
                    Severity::Warning,
                    "the component has no footprint".to_owned(),
                    &[],
                    None,
                ));
            }
        }
        (Some(name), None) => {
            out.push(finding(
                "missing-footprint",
                Severity::Error,
                format!("footprint {name} is not in a project library"),
                &[("name", name.clone())],
                None,
            ));
        }
        (Some(_), Some(footprint)) => {
            let pins: BTreeSet<&str> = symbol
                .units
                .iter()
                .flat_map(|u| u.pins.iter().map(|p| p.number.as_str()))
                .collect();
            let pads: BTreeSet<&str> = footprint
                .pads
                .iter()
                .filter(|p| !p.number.is_empty())
                .map(|p| p.number.as_str())
                .collect();
            for pin in pins.difference(&pads) {
                out.push(finding(
                    "pin-without-pad",
                    Severity::Error,
                    format!("pin {pin} has no pad in the footprint"),
                    &[("pin", (*pin).to_owned())],
                    None,
                ));
            }
            for pad in pads.difference(&pins) {
                out.push(finding(
                    "pad-without-pin",
                    Severity::Warning,
                    format!("pad {pad} has no pin in the symbol"),
                    &[("pad", (*pad).to_owned())],
                    None,
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wizard::{Wizard, generate, presets};
    use ew_model::id::Id;
    use ew_model::library::{PinDirection, PinShape, SymbolBody, SymbolUnit};

    fn pin(number: &str, name: &str, x: Nm, y: Nm) -> Pin {
        Pin {
            number: number.to_owned(),
            name: name.to_owned(),
            kind: PinKind::Passive,
            shape: PinShape::Line,
            at: Point::new(x, y),
            direction: PinDirection::Right,
            length: 2_540_000,
            hidden: false,
        }
    }

    fn symbol(pins: Vec<Pin>) -> SymbolDef {
        SymbolDef {
            id: Id::random(),
            name: "X".to_owned(),
            description: "test".to_owned(),
            keywords: String::new(),
            prefix: "U".to_owned(),
            value: String::new(),
            footprint: None,
            datasheet: String::new(),
            power: false,
            units: vec![SymbolUnit {
                pins,
                ..SymbolUnit::default()
            }],
            properties: BTreeMap::new(),
            pin_text: ew_model::library::PinText::default(),
        }
    }

    fn codes(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.code.as_str()).collect()
    }

    #[test]
    fn a_good_symbol_has_no_findings() {
        let def = symbol(vec![pin("1", "A", 0, 0), pin("2", "B", 0, -2_540_000)]);
        assert_eq!(codes(&check_symbol(&def)), Vec::<&str>::new());
    }

    #[test]
    fn symbol_mistakes_are_found() {
        let mut def = symbol(vec![
            pin("1", "A", 0, 0),
            pin("1", "B", 0, -2_540_000),
            pin("3", "C", 500_000, -5_080_000),
            pin("4", "D", 0, 0),
            pin("", "E", 0, -7_620_000),
        ]);
        def.prefix = String::new();
        def.units[0].alternate = Some(SymbolBody {
            pins: vec![pin("1", "A", 0, 0)],
            ..SymbolBody::default()
        });
        let findings = check_symbol(&def);
        let found = codes(&findings).into_iter().collect::<BTreeSet<_>>();
        for code in [
            "no-prefix",
            "duplicate-pin-number",
            "pin-off-grid",
            "overlapping-pins",
            "pin-without-number",
            "alternate-pins-differ",
        ] {
            assert!(found.contains(code), "{code} in {found:?}");
        }
        // Stacked pins (same place, same name) are fine.
        let stacked = symbol(vec![pin("1", "GND", 0, 0), pin("1", "GND", 0, 0)]);
        assert_eq!(codes(&check_symbol(&stacked)), Vec::<&str>::new());
    }

    #[test]
    fn generated_footprints_pass() {
        for preset in presets() {
            let def = generate(&preset.wizard, "x").unwrap();
            let findings: Vec<Finding> = check_footprint(&def)
                .into_iter()
                .filter(|f| f.severity != Severity::Info)
                .collect();
            assert_eq!(codes(&findings), Vec::<&str>::new(), "{}", preset.name);
        }
    }

    #[test]
    fn footprint_mistakes_are_found() {
        let wizard = presets()
            .into_iter()
            .find(|p| p.name == "0603")
            .unwrap()
            .wizard;
        let mut def = generate(&wizard, "x").unwrap();
        def.graphics
            .retain(|g| !matches!(g.layer, Layer::Courtyard(_)));
        def.texts.clear();
        // A silkscreen line right across pad 1.
        def.graphics.push(ew_model::pcb::Graphic {
            layer: Layer::Silkscreen(ew_model::layer::Side::Top),
            geometry: Geometry::Line {
                start: Point::new(-2_000_000, 0),
                end: Point::new(0, 0),
            },
            width: 120_000,
            filled: false,
        });
        let findings = check_footprint(&def);
        let found = codes(&findings).into_iter().collect::<BTreeSet<_>>();
        for code in ["no-courtyard", "no-designator", "no-value", "silk-over-pad"] {
            assert!(found.contains(code), "{code} in {found:?}");
        }
        let Wizard::Chip { .. } = wizard else {
            unreachable!()
        };
    }

    #[test]
    fn components_match_pins_and_pads() {
        let symbol = symbol(vec![
            pin("1", "A", 0, 0),
            pin("2", "B", 0, -2_540_000),
            pin("3", "C", 0, -5_080_000),
        ]);
        let wizard = presets()
            .into_iter()
            .find(|p| p.name == "0603")
            .unwrap()
            .wizard;
        let footprint = generate(&wizard, "R_0603").unwrap();
        let component = ComponentDef {
            id: Id::random(),
            name: "C".to_owned(),
            description: String::new(),
            keywords: String::new(),
            symbol: "X".to_owned(),
            footprint: Some("R_0603".to_owned()),
            value: String::new(),
            manufacturer: String::new(),
            mpn: "ABC".to_owned(),
            datasheet: String::new(),
            properties: BTreeMap::new(),
            suppliers: Vec::new(),
        };
        let found = check_component(&component, Some(&symbol), Some(&footprint));
        assert_eq!(codes(&found), ["pin-without-pad"]);
        assert_eq!(found[0].args["pin"], "3");
        assert_eq!(
            codes(&check_component(&component, None, None)),
            ["missing-symbol"]
        );
    }
}
