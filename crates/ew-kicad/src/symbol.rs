//! KiCad symbols (`.kicad_sym` libraries and the symbol cache of sheets).

use std::collections::BTreeMap;

use ew_model::id::Id;
use ew_model::library::{
    Pin, PinDirection, PinKind, PinShape, PinText, SymbolDef, SymbolGraphic, SymbolText, SymbolUnit,
};
use ew_model::shape::{Fill, Geometry, LineStyle, Stroke};
use ew_model::units::Point;

use crate::common::{self, angle, nm, up};
use crate::report::ImportReport;
use crate::sexpr::Sexp;

/// Properties that map to fields of [`SymbolDef`] instead of `properties`.
const KNOWN: [&str; 8] = [
    "Reference",
    "Value",
    "Footprint",
    "Datasheet",
    "Description",
    "ki_description",
    "ki_keywords",
    "ki_locked",
];

/// Stroke of a graphic item (`(stroke (width w) (type t))`, or the KiCad 5
/// style `(width w)`).
#[must_use]
pub fn stroke(node: &Sexp<'_>) -> Stroke {
    let stroke = node.get("stroke");
    let width = stroke
        .and_then(|s| s.number("width"))
        .or_else(|| node.number("width"))
        .map_or(0, nm);
    let style = match stroke.and_then(|s| s.value("type")) {
        Some("dash") => LineStyle::Dash,
        Some("dot") => LineStyle::Dot,
        Some("dash_dot" | "dash_dot_dot") => LineStyle::DashDot,
        _ => LineStyle::Solid,
    };
    Stroke { width, style }
}

/// Fill of a symbol graphic (`(fill (type none|outline|background|color))`).
#[must_use]
pub fn fill(node: &Sexp<'_>) -> Fill {
    match node.get("fill").and_then(|f| f.value("type")) {
        Some("outline") => Fill::Solid,
        Some("background" | "color") => Fill::Background,
        _ => Fill::None,
    }
}

/// Library coordinates (y up) of raw millimetres.
#[must_use]
pub fn up_xy(x: f64, y: f64) -> Point {
    Point::new(nm(x), nm(y))
}

/// A graphic item of a symbol (or of a sheet, with another coordinate
/// conversion) as a geometry.
pub fn geometry(node: &Sexp<'_>, convert: &dyn Fn(f64, f64) -> Point) -> Option<Geometry> {
    let at = |child: Option<&Sexp<'_>>| -> Point {
        child.map_or_else(
            || convert(0.0, 0.0),
            |c| convert(c.num(0).unwrap_or(0.0), c.num(1).unwrap_or(0.0)),
        )
    };
    Some(match node.head()? {
        "rectangle" => Geometry::Rect {
            start: at(node.get("start")),
            end: at(node.get("end")),
        },
        "circle" => Geometry::Circle {
            center: at(node.get("center")),
            radius: node.number("radius").map_or(0, nm),
        },
        "arc" => Geometry::Arc {
            start: at(node.get("start")),
            mid: at(node.get("mid")),
            end: at(node.get("end")),
        },
        "polyline" => Geometry::Polyline {
            points: common::points(node.get("pts"), convert),
        },
        "bezier" => {
            let pts = common::points(node.get("pts"), convert);
            if pts.len() < 4 {
                return None;
            }
            Geometry::Bezier {
                start: pts[0],
                control1: pts[1],
                control2: pts[2],
                end: pts[3],
            }
        }
        _ => return None,
    })
}

/// A text angle; some KiCad versions store tenths of a degree.
#[must_use]
pub fn text_angle(at: Option<&Sexp<'_>>) -> ew_model::units::Angle {
    let raw = at.and_then(|a| a.num(2)).unwrap_or(0.0);
    angle(if raw.abs() > 360.0 { raw / 10.0 } else { raw })
}

fn pin_kind(word: &str) -> PinKind {
    match word {
        "input" => PinKind::Input,
        "output" => PinKind::Output,
        "bidirectional" => PinKind::Bidirectional,
        "tri_state" => PinKind::TriState,
        "free" => PinKind::Free,
        "unspecified" => PinKind::Unspecified,
        "power_in" => PinKind::PowerIn,
        "power_out" => PinKind::PowerOut,
        "open_collector" => PinKind::OpenCollector,
        "open_emitter" => PinKind::OpenEmitter,
        "no_connect" => PinKind::NoConnect,
        _ => PinKind::Passive,
    }
}

fn pin_shape(word: &str) -> PinShape {
    match word {
        "inverted" => PinShape::Inverted,
        "clock" => PinShape::Clock,
        "inverted_clock" => PinShape::InvertedClock,
        "input_low" => PinShape::InputLow,
        "clock_low" => PinShape::ClockLow,
        "output_low" => PinShape::OutputLow,
        "edge_clock_high" => PinShape::FallingEdgeClock,
        "non_logic" => PinShape::NonLogic,
        _ => PinShape::Line,
    }
}

fn pin(node: &Sexp<'_>) -> Pin {
    let at = node.get("at");
    let direction = match at
        .and_then(|a| a.num(2))
        .map_or(0, |d| (d.rem_euclid(360.0) / 90.0).round() as i64 % 4)
    {
        1 => PinDirection::Up,
        2 => PinDirection::Left,
        3 => PinDirection::Down,
        _ => PinDirection::Right,
    };
    let name = node.get("name").and_then(|n| n.arg(0)).unwrap_or_default();
    Pin {
        number: node
            .get("number")
            .and_then(|n| n.arg(0))
            .unwrap_or_default()
            .to_owned(),
        name: if name == "~" {
            String::new()
        } else {
            name.to_owned()
        },
        kind: pin_kind(node.arg(0).unwrap_or_default()),
        shape: pin_shape(node.arg(1).unwrap_or_default()),
        at: up(at),
        direction,
        length: node.number("length").map_or(0, nm),
        hidden: node.flag("hide"),
    }
}

/// Unit and body style of a sub-symbol name (`ULN2003A_1_1` → (1, 1)).
fn unit_of(name: &str) -> Option<(u16, u16)> {
    let mut parts = name.rsplitn(3, '_');
    let style = parts.next()?.parse().ok()?;
    let unit = parts.next()?.parse().ok()?;
    Some((unit, style))
}

/// Reads one symbol (`(symbol "Lib:Name" …)`). `base` is the resolved
/// parent of a derived symbol (`extends`).
#[allow(clippy::too_many_lines, reason = "one item kind after the other")]
pub fn symbol_def(
    node: &Sexp<'_>,
    id: Id,
    name: &str,
    base: Option<&SymbolDef>,
    report: &mut ImportReport,
    file: &str,
) -> SymbolDef {
    let mut def = base.cloned().unwrap_or_else(|| SymbolDef {
        id,
        name: name.to_owned(),
        description: String::new(),
        keywords: String::new(),
        prefix: String::new(),
        value: String::new(),
        footprint: None,
        datasheet: String::new(),
        power: false,
        units: Vec::new(),
        properties: BTreeMap::new(),
        pin_text: PinText::default(),
    });
    def.id = id;
    name.clone_into(&mut def.name);
    def.power = def.power || node.get("power").is_some() || node.has_word("power");
    if let Some(numbers) = node.get("pin_numbers") {
        def.pin_text.hide_numbers = numbers.flag("hide");
    }
    if let Some(names) = node.get("pin_names") {
        def.pin_text.hide_names = names.flag("hide");
        if let Some(offset) = names.number("offset") {
            def.pin_text.name_offset = nm(offset);
        }
    }
    for property in node.all("property") {
        let key = property.arg(0).unwrap_or_default();
        let value = property.arg(1).unwrap_or_default();
        match key {
            "Reference" => value
                .trim_end_matches(['?', '*'])
                .clone_into(&mut def.prefix),
            "Value" => value.clone_into(&mut def.value),
            "Footprint" => def.footprint = (!value.is_empty()).then(|| value.to_owned()),
            "Datasheet" => {
                def.datasheet = if value == "~" {
                    String::new()
                } else {
                    value.to_owned()
                };
            }
            "Description" | "ki_description" => value.clone_into(&mut def.description),
            "ki_keywords" => value.clone_into(&mut def.keywords),
            _ if KNOWN.contains(&key) => {}
            _ => {
                def.properties.insert(key.to_owned(), value.to_owned());
            }
        }
    }

    // Graphics and pins live in sub-symbols `Name_unit_style`. Unit 0 is
    // shared by all units; style 2 is the alternate (De Morgan) body.
    let mut parts: Vec<((u16, u16), &Sexp<'_>)> = node
        .all("symbol")
        .filter_map(|sub| unit_of(sub.arg(0).unwrap_or_default()).map(|key| (key, sub)))
        .collect();
    if parts.iter().any(|((_, style), _)| *style > 1) {
        report.info(
            "alternate-body-style",
            Some(file),
            "alternate body styles (De Morgan) are not imported; the normal body is used",
        );
        parts.retain(|((_, style), _)| *style <= 1);
    }
    if parts.is_empty() {
        if def.units.is_empty() {
            def.units.push(SymbolUnit {
                name: String::new(),
                pins: Vec::new(),
                graphics: Vec::new(),
                texts: Vec::new(),
            });
        }
        return def;
    }
    let count = parts
        .iter()
        .map(|((unit, _), _)| *unit)
        .max()
        .unwrap_or(1)
        .max(1);
    def.units = (1..=count)
        .map(|unit| {
            let mut out = SymbolUnit {
                name: String::new(),
                pins: Vec::new(),
                graphics: Vec::new(),
                texts: Vec::new(),
            };
            for ((owner, _), sub) in &parts {
                if *owner != 0 && *owner != unit {
                    continue;
                }
                if let Some(unit_name) = sub.value("unit_name") {
                    unit_name.clone_into(&mut out.name);
                }
                for item in sub.args() {
                    match item.head() {
                        Some("pin") => out.pins.push(pin(item)),
                        Some("text") => {
                            let at = item.get("at");
                            out.texts.push(SymbolText {
                                text: item.arg(0).unwrap_or_default().to_owned(),
                                at: up(at),
                                rotation: text_angle(at),
                                style: common::text_style(item.get("effects")),
                            });
                        }
                        Some("text_box") => {
                            report.info(
                                "text-box",
                                Some(file),
                                "text boxes in symbols are imported as their frame",
                            );
                            out.graphics.push(SymbolGraphic {
                                geometry: Geometry::Rect {
                                    start: up(item.get("start").or_else(|| item.get("at"))),
                                    end: up(item.get("end")),
                                },
                                stroke: stroke(item),
                                fill: fill(item),
                            });
                        }
                        Some(_) => {
                            if let Some(geometry) = geometry(item, &up_xy) {
                                out.graphics.push(SymbolGraphic {
                                    geometry,
                                    stroke: stroke(item),
                                    fill: fill(item),
                                });
                            } else if !matches!(item.head(), Some("unit_name")) {
                                report.warn(
                                    "unsupported-item",
                                    Some(file),
                                    format!(
                                        "symbol item `{}` is not supported",
                                        item.head().unwrap_or("?")
                                    ),
                                );
                            }
                        }
                        None => {}
                    }
                }
            }
            out
        })
        .collect();
    def
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexpr::parse;
    use ew_model::units::um;

    const RESISTOR: &str = r#"(symbol "Device:R"
        (pin_numbers (hide yes)) (pin_names (offset 0))
        (property "Reference" "R" (at 2.032 0 90))
        (property "Value" "R" (at 0 0 90))
        (property "Footprint" "" (at -1.778 0 90) (hide yes))
        (property "Datasheet" "~" (at 0 0 0))
        (property "Description" "Resistor" (at 0 0 0))
        (property "ki_keywords" "R res resistor" (at 0 0 0))
        (property "ki_fp_filters" "R_*" (at 0 0 0))
        (symbol "R_0_1"
            (rectangle (start -1.016 -2.54) (end 1.016 2.54) (stroke (width 0.254) (type default)) (fill (type none))))
        (symbol "R_1_1"
            (pin passive line (at 0 3.81 270) (length 1.27) (name "~" (effects (font (size 1.27 1.27)))) (number "1" (effects (font (size 1.27 1.27)))))
            (pin passive line (at 0 -3.81 90) (length 1.27) (name "~") (number "2"))))"#;

    #[test]
    fn reads_a_resistor() {
        let node = parse(RESISTOR).unwrap();
        let mut report = ImportReport::default();
        let def = symbol_def(&node, Id::from_u128(1), "Device:R", None, &mut report, "x");
        assert_eq!(def.prefix, "R");
        assert_eq!(def.description, "Resistor");
        assert_eq!(def.keywords, "R res resistor");
        assert_eq!(def.datasheet, "");
        assert_eq!(def.footprint, None);
        assert_eq!(
            def.properties.get("ki_fp_filters").map(String::as_str),
            Some("R_*")
        );
        assert!(def.pin_text.hide_numbers);
        assert_eq!(def.pin_text.name_offset, 0);
        assert_eq!(def.units.len(), 1);
        let unit = &def.units[0];
        assert_eq!(unit.graphics.len(), 1);
        assert_eq!(unit.pins.len(), 2);
        let pin = &unit.pins[0];
        assert_eq!(pin.at, Point::new(0, um(3_810)));
        assert_eq!(pin.direction, PinDirection::Down);
        assert_eq!(pin.length, um(1_270));
        assert_eq!(pin.name, "");
        assert_eq!(report.findings.len(), 0);
    }

    #[test]
    fn splits_units_and_skips_the_alternate_body() {
        let text = r#"(symbol "X:Dual"
            (symbol "Dual_0_1" (rectangle (start 0 0) (end 1 1)))
            (symbol "Dual_1_1" (pin input line (at 0 0 0) (length 2.54) (name "A") (number "1")))
            (symbol "Dual_1_2" (pin input inverted (at 0 0 0) (length 2.54) (name "A") (number "1")))
            (symbol "Dual_2_1" (pin output line (at 0 0 180) (length 2.54) (name "B") (number "2") (hide yes))))"#;
        let node = parse(text).unwrap();
        let mut report = ImportReport::default();
        let def = symbol_def(&node, Id::from_u128(2), "X:Dual", None, &mut report, "x");
        assert_eq!(def.units.len(), 2);
        assert_eq!(
            def.units[0].graphics.len(),
            1,
            "shared graphics in every unit"
        );
        assert_eq!(def.units[1].graphics.len(), 1);
        assert_eq!(def.units[0].pins.len(), 1);
        assert_eq!(def.units[0].pins[0].shape, PinShape::Line);
        assert_eq!(def.units[1].pins[0].direction, PinDirection::Left);
        assert!(def.units[1].pins[0].hidden);
        assert_eq!(report.findings.len(), 1);
    }

    #[test]
    fn derived_symbols_inherit_their_parent() {
        let parent = parse(RESISTOR).unwrap();
        let mut report = ImportReport::default();
        let base = symbol_def(
            &parent,
            Id::from_u128(1),
            "Device:R",
            None,
            &mut report,
            "x",
        );
        let child = parse(r#"(symbol "R_Small" (extends "R") (property "Value" "R_Small" (at 0 0 0)) (property "Description" "Small" (at 0 0 0)))"#).unwrap();
        let def = symbol_def(
            &child,
            Id::from_u128(3),
            "Device:R_Small",
            Some(&base),
            &mut report,
            "x",
        );
        assert_eq!(def.value, "R_Small");
        assert_eq!(def.description, "Small");
        assert_eq!(def.units, base.units);
        assert_eq!(def.id, Id::from_u128(3));
    }
}
