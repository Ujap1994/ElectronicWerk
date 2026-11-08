//! The part types used by synthetic designs, with their symbols and
//! footprints.

use std::collections::BTreeMap;

use ew_model::collection::Collection;
use ew_model::id::Id;
use ew_model::layer::{Layer, LayerSpec, Side};
use ew_model::library::{
    FootprintDef, Library, Pin, PinDirection, PinKind, SymbolDef, SymbolGraphic, SymbolUnit,
};
use ew_model::pcb::{
    Drill, FootprintAttributes, FootprintText, FootprintTextKind, Graphic, Mounting, Pad, PadKind,
    PadShape,
};
use ew_model::shape::{Fill, Geometry, Stroke};
use ew_model::text::TextStyle;
use ew_model::units::{Angle, Nm, Point, Size, um};

/// Part types of synthetic designs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartKind {
    /// 0603 resistor.
    Resistor,
    /// 0603 capacitor.
    Capacitor,
    /// SOT-23 transistor.
    Transistor,
    /// SOIC-8 IC.
    Ic8,
    /// QFP-64 microcontroller.
    Mcu64,
    /// 1×10 pin header.
    Header10,
}

/// A symbol pin connection point and the direction away from the body.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PinPoint {
    pub(crate) at: Point,
    dx: i64,
    dy: i64,
}

impl PinPoint {
    const fn new(at: Point, dx: i64, dy: i64) -> Self {
        Self { at, dx, dy }
    }

    /// Offset `length` away from the body.
    pub(crate) const fn outward(self, length: Nm) -> Point {
        Point::new(self.dx * length, self.dy * length)
    }

    const fn direction(self) -> PinDirection {
        match (self.dx, self.dy) {
            (-1, _) => PinDirection::Right,
            (1, _) => PinDirection::Left,
            (_, 1) => PinDirection::Down,
            _ => PinDirection::Up,
        }
    }
}

const ALL: [PartKind; 6] = [
    PartKind::Resistor,
    PartKind::Capacitor,
    PartKind::Transistor,
    PartKind::Ic8,
    PartKind::Mcu64,
    PartKind::Header10,
];

impl PartKind {
    /// Picks a kind from a percentage (0–99) with a realistic mix.
    #[must_use]
    pub const fn pick(percent: u32) -> Self {
        match percent {
            0..35 => Self::Resistor,
            35..70 => Self::Capacitor,
            70..80 => Self::Transistor,
            80..92 => Self::Ic8,
            92..95 => Self::Mcu64,
            _ => Self::Header10,
        }
    }

    /// Number of pins and pads.
    #[must_use]
    pub const fn pad_count(self) -> usize {
        match self {
            Self::Resistor | Self::Capacitor => 2,
            Self::Transistor => 3,
            Self::Ic8 => 8,
            Self::Mcu64 => 64,
            Self::Header10 => 10,
        }
    }

    /// Designator prefix.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Resistor => "R",
            Self::Capacitor => "C",
            Self::Transistor => "Q",
            Self::Ic8 | Self::Mcu64 => "U",
            Self::Header10 => "J",
        }
    }

    /// Symbol name in the project library.
    #[must_use]
    pub const fn symbol_name(self) -> &'static str {
        match self {
            Self::Resistor => "Resistor",
            Self::Capacitor => "Capacitor",
            Self::Transistor => "NPN",
            Self::Ic8 => "IC_8",
            Self::Mcu64 => "MCU_64",
            Self::Header10 => "Header_1x10",
        }
    }

    /// Footprint name in the project library.
    #[must_use]
    pub const fn footprint_name(self) -> &'static str {
        match self {
            Self::Resistor => "R_0603",
            Self::Capacitor => "C_0603",
            Self::Transistor => "SOT-23",
            Self::Ic8 => "SOIC-8",
            Self::Mcu64 => "QFP-64",
            Self::Header10 => "PinHeader_1x10_P2.54mm",
        }
    }

    /// Value text.
    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            Self::Resistor => "10k",
            Self::Capacitor => "100n",
            Self::Transistor => "NPN",
            Self::Ic8 => "IC_8",
            Self::Mcu64 => "MCU_64",
            Self::Header10 => "Header",
        }
    }

    /// Two-terminal parts may sit on the bottom side.
    #[must_use]
    pub const fn is_passive(self) -> bool {
        matches!(self, Self::Resistor | Self::Capacitor)
    }

    pub(crate) fn pin_points(self) -> Vec<PinPoint> {
        let p = |x: i64, y: i64, dx: i64, dy: i64| PinPoint::new(Point::new(um(x), um(y)), dx, dy);
        match self {
            Self::Resistor | Self::Capacitor => vec![p(0, 3_810, 0, 1), p(0, -3_810, 0, -1)],
            Self::Transistor => vec![
                p(-5_080, 0, -1, 0),
                p(2_540, -5_080, 0, -1),
                p(2_540, 5_080, 0, 1),
            ],
            Self::Ic8 => {
                let mut pins: Vec<PinPoint> = (0..4)
                    .map(|i| p(-7_620, 3_810 - i * 2_540, -1, 0))
                    .collect();
                pins.extend((0..4).map(|i| p(7_620, -3_810 + i * 2_540, 1, 0)));
                pins
            }
            Self::Mcu64 => {
                let mut pins = Vec::with_capacity(64);
                pins.extend((0..16).map(|i| p(-25_400, 19_050 - i * 2_540, -1, 0)));
                pins.extend((0..16).map(|i| p(-19_050 + i * 2_540, -25_400, 0, -1)));
                pins.extend((0..16).map(|i| p(25_400, -19_050 + i * 2_540, 1, 0)));
                pins.extend((0..16).map(|i| p(19_050 - i * 2_540, 25_400, 0, 1)));
                pins
            }
            Self::Header10 => (0..10)
                .map(|i| p(-5_080, 11_430 - i * 2_540, -1, 0))
                .collect(),
        }
    }

    /// Pad centres and sizes (in pad order).
    fn pad_layout(self) -> Vec<(Point, Size)> {
        let pad =
            |x: i64, y: i64, w: i64, h: i64| (Point::new(um(x), um(y)), Size::new(um(w), um(h)));
        match self {
            Self::Resistor | Self::Capacitor => vec![pad(-800, 0, 900, 950), pad(800, 0, 900, 950)],
            Self::Transistor => vec![
                pad(-950, -1_000, 600, 700),
                pad(950, -1_000, 600, 700),
                pad(0, 1_000, 600, 700),
            ],
            Self::Ic8 => {
                let mut pads: Vec<_> = (0..4)
                    .map(|i| pad(-2_700, 1_905 - i * 1_270, 1_550, 600))
                    .collect();
                pads.extend((0..4).map(|i| pad(2_700, -1_905 + i * 1_270, 1_550, 600)));
                pads
            }
            Self::Mcu64 => {
                let mut pads = Vec::with_capacity(64);
                pads.extend((0..16).map(|i| pad(-5_750, 3_750 - i * 500, 1_500, 300)));
                pads.extend((0..16).map(|i| pad(-3_750 + i * 500, -5_750, 300, 1_500)));
                pads.extend((0..16).map(|i| pad(5_750, -3_750 + i * 500, 1_500, 300)));
                pads.extend((0..16).map(|i| pad(3_750 - i * 500, 5_750, 300, 1_500)));
                pads
            }
            Self::Header10 => (0..10).map(|i| pad(0, -i * 2_540, 1_700, 1_700)).collect(),
        }
    }

    fn symbol(self, id: Id) -> SymbolDef {
        let pins: Vec<Pin> = self
            .pin_points()
            .into_iter()
            .enumerate()
            .map(|(index, point)| Pin {
                number: (index + 1).to_string(),
                name: String::new(),
                kind: if self.is_passive() {
                    PinKind::Passive
                } else {
                    PinKind::Bidirectional
                },
                shape: ew_model::library::PinShape::Line,
                at: point.at,
                direction: point.direction(),
                length: um(2_540),
                hidden: false,
            })
            .collect();
        let half = |value: i64| um(value);
        let (w, h) = match self {
            Self::Resistor | Self::Capacitor => (half(1_016), half(1_270)),
            Self::Transistor => (half(2_540), half(2_540)),
            Self::Ic8 => (half(5_080), half(5_080)),
            Self::Mcu64 => (half(22_860), half(22_860)),
            Self::Header10 => (half(2_540), half(12_700)),
        };
        SymbolDef {
            id,
            name: self.symbol_name().to_owned(),
            description: format!("Synthetic {} symbol", self.symbol_name()),
            keywords: String::new(),
            prefix: self.prefix().to_owned(),
            value: self.value().to_owned(),
            footprint: Some(self.footprint_name().to_owned()),
            datasheet: String::new(),
            power: false,
            units: vec![SymbolUnit {
                name: String::new(),
                pins,
                graphics: vec![SymbolGraphic {
                    geometry: Geometry::Rect {
                        start: Point::new(-w, -h),
                        end: Point::new(w, h),
                    },
                    stroke: Stroke::default(),
                    fill: Fill::Background,
                }],
                texts: Vec::new(),
            }],
            properties: BTreeMap::new(),
        }
    }

    fn footprint(self, id: Id, make_id: &mut dyn FnMut() -> Id) -> FootprintDef {
        let through_hole = self == Self::Header10;
        let pads: Vec<Pad> = self
            .pad_layout()
            .into_iter()
            .enumerate()
            .map(|(index, (at, size))| Pad {
                id: make_id(),
                number: (index + 1).to_string(),
                net: None,
                kind: if through_hole {
                    PadKind::ThroughHole
                } else {
                    PadKind::Smd
                },
                shape: if through_hole {
                    if index == 0 {
                        PadShape::Rect
                    } else {
                        PadShape::Circle
                    }
                } else {
                    PadShape::RoundRect { ratio: 250 }
                },
                at,
                rotation: Angle::ZERO,
                size,
                drill: through_hole.then_some(Drill {
                    diameter: um(1_000),
                    slot_length: None,
                    offset: Point::ORIGIN,
                }),
                layers: if through_hole {
                    vec![LayerSpec::AllCopper, LayerSpec::AllMask]
                } else {
                    vec![
                        LayerSpec::Layer(Layer::TOP_COPPER),
                        LayerSpec::Layer(Layer::SolderMask(Side::Top)),
                        LayerSpec::Layer(Layer::Paste(Side::Top)),
                    ]
                },
                clearance: None,
                mask_margin: None,
                paste_margin: None,
            })
            .collect();
        let bounds = ew_model::units::Rect::from_points(pads.iter().flat_map(|pad| {
            [
                Point::new(pad.at.x - pad.size.w / 2, pad.at.y - pad.size.h / 2),
                Point::new(pad.at.x + pad.size.w / 2, pad.at.y + pad.size.h / 2),
            ]
        }))
        .unwrap_or(ew_model::units::Rect::from_corners(
            Point::ORIGIN,
            Point::ORIGIN,
        ));
        let margin = um(250);
        let courtyard = Geometry::Rect {
            start: Point::new(bounds.min.x - margin, bounds.min.y - margin),
            end: Point::new(bounds.max.x + margin, bounds.max.y + margin),
        };
        FootprintDef {
            id,
            name: self.footprint_name().to_owned(),
            description: format!("Synthetic {} footprint", self.footprint_name()),
            keywords: String::new(),
            pads,
            graphics: vec![Graphic {
                layer: Layer::Courtyard(Side::Top),
                geometry: courtyard,
                width: um(50),
                filled: false,
            }],
            texts: vec![FootprintText {
                kind: FootprintTextKind::Designator,
                text: String::new(),
                layer: Layer::Silkscreen(Side::Top),
                at: Point::new(0, bounds.max.y + um(1_000)),
                rotation: Angle::ZERO,
                style: TextStyle::board(),
                visible: true,
            }],
            models: Vec::new(),
            attributes: FootprintAttributes {
                mounting: if through_hole {
                    Mounting::ThroughHole
                } else {
                    Mounting::Smd
                },
                ..FootprintAttributes::default()
            },
        }
    }
}

/// The library with every synthetic symbol and footprint.
pub(crate) fn library(id: Id, make_id: &mut dyn FnMut() -> Id) -> Library {
    let mut library = Library::new(id, "Project library");
    "Symbols and footprints of the synthetic design".clone_into(&mut library.description);
    library.symbols = ALL
        .iter()
        .map(|kind| kind.symbol(make_id()))
        .collect::<Collection<_>>();
    library.footprints = ALL
        .iter()
        .map(|kind| {
            let footprint_id = make_id();
            kind.footprint(footprint_id, make_id)
        })
        .collect::<Collection<_>>();
    library
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_match_pads() {
        for kind in ALL {
            assert_eq!(kind.pin_points().len(), kind.pad_count(), "{kind:?}");
            assert_eq!(kind.pad_layout().len(), kind.pad_count(), "{kind:?}");
        }
        let mut percentages = [0_usize; 6];
        for percent in 0..100 {
            let index = ALL
                .iter()
                .position(|kind| *kind == PartKind::pick(percent))
                .unwrap();
            percentages[index] += 1;
        }
        assert_eq!(percentages, [35, 35, 10, 12, 3, 5]);
    }
}
