//! The starter symbols every new project gets: the common parts of a first
//! circuit, drawn like KiCad's symbols (pins on the 1.27 mm grid), so a
//! schematic can be drawn before the base library is set up.

use std::collections::BTreeMap;

use ew_model::id::Id;
use ew_model::library::{
    Pin, PinDirection, PinKind, PinShape, PinText, SymbolDef, SymbolGraphic, SymbolUnit,
};
use ew_model::shape::{Fill, Geometry, Stroke};
use ew_model::units::{Nm, Point};

/// Micrometres to nanometres.
const fn um(value: i64) -> Nm {
    value * 1_000
}

fn p(x: i64, y: i64) -> Point {
    Point::new(um(x), um(y))
}

fn line(a: Point, b: Point) -> SymbolGraphic {
    SymbolGraphic {
        geometry: Geometry::Line { start: a, end: b },
        stroke: Stroke::default(),
        fill: Fill::None,
    }
}

fn thick(a: Point, b: Point) -> SymbolGraphic {
    SymbolGraphic {
        stroke: Stroke {
            width: um(508),
            ..Stroke::default()
        },
        ..line(a, b)
    }
}

fn rect(a: Point, b: Point, fill: Fill) -> SymbolGraphic {
    SymbolGraphic {
        geometry: Geometry::Rect { start: a, end: b },
        stroke: Stroke::default(),
        fill,
    }
}

fn polygon(points: &[Point], fill: Fill) -> SymbolGraphic {
    SymbolGraphic {
        geometry: Geometry::Polygon {
            points: points.to_vec(),
        },
        stroke: Stroke::default(),
        fill,
    }
}

fn polyline(points: &[Point]) -> SymbolGraphic {
    SymbolGraphic {
        geometry: Geometry::Polyline {
            points: points.to_vec(),
        },
        stroke: Stroke::default(),
        fill: Fill::None,
    }
}

fn circle(center: Point, radius: Nm, fill: Fill) -> SymbolGraphic {
    SymbolGraphic {
        geometry: Geometry::Circle { center, radius },
        stroke: Stroke::default(),
        fill,
    }
}

fn arc(start: Point, mid: Point, end: Point) -> SymbolGraphic {
    SymbolGraphic {
        geometry: Geometry::Arc { start, mid, end },
        stroke: Stroke::default(),
        fill: Fill::None,
    }
}

fn pin(
    number: &str,
    name: &str,
    kind: PinKind,
    at: Point,
    direction: PinDirection,
    length: Nm,
) -> Pin {
    Pin {
        number: number.to_owned(),
        name: name.to_owned(),
        kind,
        shape: PinShape::Line,
        at,
        direction,
        length,
        hidden: false,
    }
}

struct Spec<'a> {
    name: &'a str,
    description: &'a str,
    keywords: &'a str,
    prefix: &'a str,
    value: &'a str,
    pins: Vec<Pin>,
    graphics: Vec<SymbolGraphic>,
    hide_names: bool,
    power: bool,
}

fn symbol(spec: Spec<'_>) -> SymbolDef {
    SymbolDef {
        id: Id::random(),
        name: spec.name.to_owned(),
        description: spec.description.to_owned(),
        keywords: spec.keywords.to_owned(),
        prefix: spec.prefix.to_owned(),
        value: spec.value.to_owned(),
        footprint: None,
        datasheet: String::new(),
        power: spec.power,
        units: vec![SymbolUnit {
            name: String::new(),
            pins: spec.pins,
            graphics: spec.graphics,
            texts: Vec::new(),
            alternate: None,
        }],
        properties: BTreeMap::new(),
        pin_text: PinText {
            hide_numbers: true,
            hide_names: spec.hide_names,
            ..PinText::default()
        },
    }
}

/// Two vertical passive pins 7.62 mm apart (R, C, L).
fn two_pins(length: Nm) -> Vec<Pin> {
    vec![
        pin(
            "1",
            "~",
            PinKind::Passive,
            p(0, 3_810),
            PinDirection::Down,
            length,
        ),
        pin(
            "2",
            "~",
            PinKind::Passive,
            p(0, -3_810),
            PinDirection::Up,
            length,
        ),
    ]
}

/// A diode body (cathode bar on the left) for D and LED.
fn diode() -> (Vec<Pin>, Vec<SymbolGraphic>) {
    let pins = vec![
        pin(
            "1",
            "K",
            PinKind::Passive,
            p(-3_810, 0),
            PinDirection::Right,
            um(2_540),
        ),
        pin(
            "2",
            "A",
            PinKind::Passive,
            p(3_810, 0),
            PinDirection::Left,
            um(2_540),
        ),
    ];
    let graphics = vec![
        polygon(
            &[
                p(1_270, 1_270),
                p(1_270, -1_270),
                p(-1_270, 0),
                p(1_270, 1_270),
            ],
            Fill::None,
        ),
        thick(p(-1_270, 1_270), p(-1_270, -1_270)),
        line(p(1_270, 0), p(-1_270, 0)),
    ];
    (pins, graphics)
}

/// A connector with one row of `count` pins (2.54 mm pitch).
fn connector(count: i64) -> SymbolDef {
    let pins = (0..count)
        .map(|i| {
            pin(
                &(i + 1).to_string(),
                &format!("Pin_{}", i + 1),
                PinKind::Passive,
                p(-5_080, -2_540 * i),
                PinDirection::Right,
                um(3_810),
            )
        })
        .collect();
    symbol(Spec {
        name: if count == 2 {
            "Conn_01x02"
        } else {
            "Conn_01x03"
        },
        description: "Connector, one row of pins",
        keywords: "connector header terminal",
        prefix: "J",
        value: if count == 2 {
            "Conn_01x02"
        } else {
            "Conn_01x03"
        },
        pins,
        graphics: vec![rect(
            p(-1_270, 1_270),
            p(1_270, -2_540 * count + 1_270),
            Fill::Background,
        )],
        hide_names: true,
        power: false,
    })
}

/// The starter symbols (new ids on every call).
#[must_use]
#[allow(clippy::too_many_lines, reason = "one block per symbol")]
pub fn starter_symbols() -> Vec<SymbolDef> {
    let (d_pins, d_graphics) = diode();
    let (led_pins, mut led_graphics) = diode();
    for (x, y) in [(0, 1_778), (1_270, 1_778)] {
        led_graphics.push(polyline(&[p(x - 762, y), p(x + 254, y + 1_016)]));
        led_graphics.push(polyline(&[
            p(x - 254, y + 1_016),
            p(x + 254, y + 1_016),
            p(x + 254, y + 508),
        ]));
    }
    vec![
        symbol(Spec {
            name: "R",
            description: "Resistor",
            keywords: "resistor R",
            prefix: "R",
            value: "10k",
            pins: two_pins(um(1_270)),
            graphics: vec![rect(p(-1_016, 2_540), p(1_016, -2_540), Fill::None)],
            hide_names: true,
            power: false,
        }),
        symbol(Spec {
            name: "C",
            description: "Capacitor",
            keywords: "capacitor C cap",
            prefix: "C",
            value: "100n",
            pins: two_pins(um(2_794)),
            graphics: vec![
                thick(p(-2_032, 762), p(2_032, 762)),
                thick(p(-2_032, -762), p(2_032, -762)),
            ],
            hide_names: true,
            power: false,
        }),
        symbol(Spec {
            name: "C_Polarized",
            description: "Polarized capacitor (electrolytic); pin 1 is +",
            keywords: "capacitor electrolytic polarized elko",
            prefix: "C",
            value: "10u",
            pins: two_pins(um(2_794)),
            graphics: vec![
                rect(p(-2_286, 1_016), p(2_286, 508), Fill::None),
                rect(p(-2_286, -508), p(2_286, -1_016), Fill::Solid),
                line(p(-1_778, 2_286), p(-762, 2_286)),
                line(p(-1_270, 2_794), p(-1_270, 1_778)),
            ],
            hide_names: true,
            power: false,
        }),
        symbol(Spec {
            name: "L",
            description: "Inductor",
            keywords: "inductor coil choke L",
            prefix: "L",
            value: "10u",
            pins: two_pins(um(1_270)),
            graphics: (0..4)
                .map(|i| {
                    let top = 2_540 - 1_270 * i;
                    arc(p(0, top), p(635, top - 635), p(0, top - 1_270))
                })
                .collect(),
            hide_names: true,
            power: false,
        }),
        symbol(Spec {
            name: "D",
            description: "Diode; pin 1 is the cathode",
            keywords: "diode rectifier",
            prefix: "D",
            value: "1N4148",
            pins: d_pins,
            graphics: d_graphics,
            hide_names: true,
            power: false,
        }),
        symbol(Spec {
            name: "LED",
            description: "Light emitting diode; pin 1 is the cathode",
            keywords: "LED light emitting diode",
            prefix: "D",
            value: "red",
            pins: led_pins,
            graphics: led_graphics,
            hide_names: true,
            power: false,
        }),
        symbol(Spec {
            name: "Q_NPN_BCE",
            description: "NPN transistor, pins base, collector, emitter",
            keywords: "transistor NPN BJT",
            prefix: "Q",
            value: "BC547",
            pins: vec![
                pin(
                    "1",
                    "B",
                    PinKind::Input,
                    p(-5_080, 0),
                    PinDirection::Right,
                    um(5_715),
                ),
                pin(
                    "2",
                    "C",
                    PinKind::Passive,
                    p(2_540, 5_080),
                    PinDirection::Down,
                    um(2_540),
                ),
                pin(
                    "3",
                    "E",
                    PinKind::Passive,
                    p(2_540, -5_080),
                    PinDirection::Up,
                    um(2_540),
                ),
            ],
            graphics: vec![
                circle(p(1_270, 0), um(2_819), Fill::Background),
                thick(p(635, 1_905), p(635, -1_905)),
                line(p(635, 635), p(2_540, 2_540)),
                line(p(635, -635), p(2_540, -2_540)),
                polygon(
                    &[
                        p(1_524, -2_032),
                        p(2_032, -1_524),
                        p(2_540, -2_540),
                        p(1_524, -2_032),
                    ],
                    Fill::Solid,
                ),
            ],
            hide_names: true,
            power: false,
        }),
        connector(2),
        connector(3),
        symbol(Spec {
            name: "SW_Push",
            description: "Push button, closes while pressed",
            keywords: "switch push button tactile",
            prefix: "SW",
            value: "SW_Push",
            pins: vec![
                pin(
                    "1",
                    "1",
                    PinKind::Passive,
                    p(-5_080, 0),
                    PinDirection::Right,
                    um(2_540),
                ),
                pin(
                    "2",
                    "2",
                    PinKind::Passive,
                    p(5_080, 0),
                    PinDirection::Left,
                    um(2_540),
                ),
            ],
            graphics: vec![
                circle(p(-2_032, 0), um(508), Fill::None),
                circle(p(2_032, 0), um(508), Fill::None),
                line(p(-2_540, 1_270), p(2_540, 1_270)),
                line(p(0, 1_270), p(0, 3_048)),
            ],
            hide_names: true,
            power: false,
        }),
        symbol(Spec {
            name: "PWR_FLAG",
            description: "Marks a net as powered (for the electrical rules check)",
            keywords: "power flag supply",
            prefix: "#FLG",
            value: "PWR_FLAG",
            pins: vec![Pin {
                hidden: true,
                ..pin("1", "pwr", PinKind::PowerOut, p(0, 0), PinDirection::Up, 0)
            }],
            graphics: vec![
                line(p(0, 0), p(0, 1_270)),
                polygon(
                    &[
                        p(0, 1_270),
                        p(-1_016, 1_905),
                        p(0, 2_540),
                        p(1_016, 1_905),
                        p(0, 1_270),
                    ],
                    Fill::None,
                ),
            ],
            hide_names: true,
            power: true,
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_pins_are_on_the_grid() {
        let grid = um(1_270);
        let symbols = starter_symbols();
        assert_eq!(symbols.len(), 11);
        for def in &symbols {
            for pin in &def.units[0].pins {
                assert_eq!(pin.at.x % grid, 0, "{} pin {}", def.name, pin.number);
                assert_eq!(pin.at.y % grid, 0, "{} pin {}", def.name, pin.number);
            }
        }
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"R") && names.contains(&"PWR_FLAG"));
    }
}
