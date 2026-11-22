//! A synthetic system of three boards (M18): a carrier board, a shield
//! stacked on it through a 2 × 8 header and a front panel board joined by
//! a four-wire cable. It holds the mistakes the system check finds: two
//! swapped signals on the stacking header, a signal that meets an
//! unconnected pin, and a wire whose signals have different names on both
//! boards (fixed with an alias). The boards have no schematics; their nets
//! are on the pads.

use ew_libedit::wizard::{Wizard, generate as footprint};
use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::layer::{Layer, Side};
use ew_model::library::FootprintDef;
use ew_model::pcb::{
    BoardShape, FootprintInstance, Model3d, Net, Pcb, STANDARD_THICKNESS, Scale3, Stackup,
};
use ew_model::project::{DocumentRef, Project};
use ew_model::rules::Rules;
use ew_model::shape::Geometry;
use ew_model::system::{
    BoardSource, Cable, CableEnd, ConnectorRef, Link, Pairing, System, SystemBoard, Wire, WireEnd,
};
use ew_model::units::{Angle, Point, Vec3, mm};

use crate::rng::Rng;

const SOCKET: &str = "${KICAD10_3DMODEL_DIR}/Connector_PinSocket_2.54mm.3dshapes/PinSocket_2x08_P2.54mm_Vertical.step";
const HEADER: &str = "${KICAD10_3DMODEL_DIR}/Connector_PinHeader_2.54mm.3dshapes/PinHeader_2x08_P2.54mm_Vertical.step";
const HEADER_1X4: &str = "${KICAD10_3DMODEL_DIR}/Connector_PinHeader_2.54mm.3dshapes/PinHeader_1x04_P2.54mm_Vertical.step";
const SOIC: &str = "${KICAD10_3DMODEL_DIR}/Package_SO.3dshapes/SOIC-8_3.9x4.9mm_P1.27mm.step";

/// The signals of the 2 × 8 stacking header, pin by pin, on the carrier.
const CARRIER_HEADER: [&str; 16] = [
    "+5V", "+5V", "GND", "GND", "SDA", "SCL", "D2", "D3", "D4", "D5", "A0", "A1", "RESET", "3V3",
    "GND", "GND",
];

/// The same header on the shield: SDA and SCL swapped, A0 not connected.
const SHIELD_HEADER: [&str; 16] = [
    "+5V", "+5V", "GND", "GND", "SCL", "SDA", "D2", "D3", "D4", "D5", "", "A1", "RESET", "3V3",
    "GND", "GND",
];

/// A footprint without pads (only if a wizard fails, which it does not
/// for these fixed values).
fn empty(name: &str) -> FootprintDef {
    FootprintDef {
        id: Id::random(),
        name: name.to_owned(),
        description: String::new(),
        keywords: String::new(),
        pads: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        models: Vec::new(),
        attributes: ew_model::pcb::FootprintAttributes::default(),
    }
}

fn header(rows: u32, pins: u32, name: &str) -> FootprintDef {
    let mut def = footprint(
        &Wizard::Header {
            rows,
            pins_per_row: pins,
            pitch: 2_540_000,
            drill: 1_000_000,
            pad: 1_700_000,
        },
        name,
    )
    .unwrap_or_else(|_| empty(name));
    name.clone_into(&mut def.name);
    def
}

fn soic() -> FootprintDef {
    let name = "SOIC-8_3.9x4.9mm_P1.27mm";
    let mut def = ew_libedit::wizard::presets()
        .into_iter()
        .find(|p| p.name == "SOIC-8")
        .and_then(|p| footprint(&p.wizard, name).ok())
        .unwrap_or_else(|| empty(name));
    name.clone_into(&mut def.name);
    def
}

/// A footprint on a board from a definition, with the nets of its pads by
/// pin number (`""`: not connected).
#[allow(clippy::too_many_arguments, reason = "one footprint in one call")]
fn place(
    def: &FootprintDef,
    designator: &str,
    value: &str,
    at: Point,
    side: Side,
    model: &str,
    nets: &[(String, Id)],
    signals: &[&str],
    id: &mut impl FnMut() -> Id,
) -> FootprintInstance {
    let mut pads = def.pads.clone();
    for pad in &mut pads {
        pad.id = id();
        let index = pad
            .number
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1));
        pad.net = index
            .and_then(|i| signals.get(i))
            .filter(|s| !s.is_empty())
            .and_then(|s| nets.iter().find(|(n, _)| n == s).map(|(_, net)| *net));
    }
    FootprintInstance {
        id: id(),
        component: None,
        footprint: def.name.clone(),
        designator: designator.to_owned(),
        value: value.to_owned(),
        at,
        rotation: Angle::ZERO,
        side,
        locked: false,
        pads,
        graphics: def.graphics.clone(),
        texts: def.texts.clone(),
        models: if model.is_empty() {
            Vec::new()
        } else {
            vec![Model3d {
                path: model.to_owned(),
                offset: Vec3::default(),
                rotation: [Angle::ZERO; 3],
                scale: Scale3::default(),
                hidden: false,
            }]
        },
        properties: std::collections::BTreeMap::new(),
        attributes: def.attributes,
    }
}

/// A board with a rectangular outline and the nets it needs.
fn board(
    name: &str,
    width: i64,
    height: i64,
    signals: &[&str],
    id: &mut impl FnMut() -> Id,
) -> (Pcb, Vec<(String, Id)>) {
    let mut pcb = Pcb::new(id(), name, Stackup::standard(2, STANDARD_THICKNESS));
    pcb.shapes.insert(BoardShape {
        id: id(),
        layer: Layer::Outline,
        geometry: Geometry::Rect {
            start: Point::ORIGIN,
            end: Point::new(mm(width), mm(height)),
        },
        width: 100_000,
        filled: false,
    });
    let mut nets = Vec::new();
    for signal in signals {
        if signal.is_empty() || nets.iter().any(|(n, _): &(String, Id)| n == signal) {
            continue;
        }
        let net = Net {
            id: id(),
            name: (*signal).to_owned(),
            class: if crate_power(signal) {
                "Power".to_owned()
            } else {
                ew_model::rules::DEFAULT_NET_CLASS.to_owned()
            },
        };
        nets.push(((*signal).to_owned(), net.id));
        pcb.nets.insert(net);
    }
    (pcb, nets)
}

fn crate_power(signal: &str) -> bool {
    matches!(signal, "+5V" | "GND" | "3V3")
}

/// The system project: boards `Carrier`, `Shield` and `Front panel` and the
/// system `Station`.
#[must_use]
#[allow(clippy::too_many_lines, reason = "the whole example in one piece")]
pub fn generate(seed: u64) -> Project {
    let mut rng = Rng::new(seed);
    let mut id = || Id::from_random_bytes(rng.bytes());
    let mut project = Project::new(id(), "Station");
    "A".clone_into(&mut project.manifest.revision);
    "Synthetic system of three boards (generated, not a real circuit)."
        .clone_into(&mut project.manifest.description);
    let socket = header(2, 8, "PinSocket_2x08_P2.54mm_Vertical");
    let pins = header(2, 8, "PinHeader_2x08_P2.54mm_Vertical");
    let small = header(1, 4, "PinHeader_1x04_P2.54mm_Vertical");
    let ic = soic();

    // The carrier: the socket for the shield, the cable connector and a
    // controller.
    let mut carrier_signals: Vec<&str> = CARRIER_HEADER.to_vec();
    carrier_signals.extend(["LED", "BUTTON"]);
    let (mut carrier, carrier_nets) = board("Carrier", 80, 50, &carrier_signals, &mut id);
    carrier.footprints.insert(place(
        &socket,
        "J1",
        "Shield",
        Point::new(mm(40), mm(30)),
        Side::Top,
        SOCKET,
        &carrier_nets,
        &CARRIER_HEADER,
        &mut id,
    ));
    carrier.footprints.insert(place(
        &small,
        "J2",
        "Panel",
        Point::new(mm(8), mm(8)),
        Side::Top,
        HEADER_1X4,
        &carrier_nets,
        &["+5V", "GND", "LED", "BUTTON"],
        &mut id,
    ));
    carrier.footprints.insert(place(
        &ic,
        "U1",
        "MCU",
        Point::new(mm(62), mm(15)),
        Side::Top,
        SOIC,
        &carrier_nets,
        &["SDA", "SCL", "D2", "GND", "D3", "D4", "D5", "+5V"],
        &mut id,
    ));

    // The shield: the header on its bottom side, above the socket.
    let (mut shield, shield_nets) = board("Shield", 60, 40, &SHIELD_HEADER, &mut id);
    shield.footprints.insert(place(
        &pins,
        "P1",
        "Carrier",
        Point::new(mm(30), mm(20)),
        Side::Bottom,
        HEADER,
        &shield_nets,
        &SHIELD_HEADER,
        &mut id,
    ));
    shield.footprints.insert(place(
        &ic,
        "U1",
        "Sensor",
        Point::new(mm(45), mm(28)),
        Side::Top,
        SOIC,
        &shield_nets,
        &["SDA", "SCL", "", "GND", "", "", "", "3V3"],
        &mut id,
    ));

    // The front panel: its end of the cable.
    let (mut panel, panel_nets) = board(
        "Front panel",
        60,
        20,
        &["+5V", "GND", "LED", "BTN"],
        &mut id,
    );
    panel.footprints.insert(place(
        &small,
        "J1",
        "Cable",
        Point::new(mm(30), mm(10)),
        Side::Top,
        HEADER_1X4,
        &panel_nets,
        &["+5V", "GND", "LED", "BTN"],
        &mut id,
    ));

    // The system.
    let ids = [id(), id(), id()];
    let thickness = STANDARD_THICKNESS;
    let mut system = System::new(id(), "Station");
    system.boards = vec![
        SystemBoard {
            id: ids[0],
            name: "Carrier".to_owned(),
            source: BoardSource::Local { doc: carrier.id },
            quantity: 1,
            position: Vec3::default(),
            rotation: [Angle::ZERO; 3],
            hidden: false,
        },
        SystemBoard {
            id: ids[1],
            name: "Shield".to_owned(),
            source: BoardSource::Local { doc: shield.id },
            quantity: 1,
            // P1 (at 30, 20 on the shield) over J1 (at 40, 30 on the
            // carrier), on the 8.5 mm socket.
            position: Vec3 {
                x: mm(10),
                y: mm(10),
                z: thickness + 8_500_000,
            },
            rotation: [Angle::ZERO; 3],
            hidden: false,
        },
        SystemBoard {
            id: ids[2],
            name: "Front panel".to_owned(),
            source: BoardSource::Local { doc: panel.id },
            quantity: 1,
            position: Vec3 {
                x: mm(10),
                y: mm(-40),
                z: 0,
            },
            rotation: [Angle::ZERO; 3],
            hidden: false,
        },
    ];
    system.links = vec![Link {
        id: id(),
        a: ConnectorRef {
            board: ids[0],
            designator: "J1".to_owned(),
        },
        b: ConnectorRef {
            board: ids[1],
            designator: "P1".to_owned(),
        },
        pairing: Pairing::Number,
    }];
    let (x1, x2) = (id(), id());
    system.cables = vec![Cable {
        id: id(),
        name: "W1".to_owned(),
        part: "4-wire cable".to_owned(),
        length: mm(150),
        ends: vec![
            CableEnd {
                id: x1,
                label: "X1".to_owned(),
                connector: Some(ConnectorRef {
                    board: ids[0],
                    designator: "J2".to_owned(),
                }),
                housing: "Socket 1x04 2.54 mm".to_owned(),
            },
            CableEnd {
                id: x2,
                label: "X2".to_owned(),
                connector: Some(ConnectorRef {
                    board: ids[2],
                    designator: "J1".to_owned(),
                }),
                housing: "Socket 1x04 2.54 mm".to_owned(),
            },
        ],
        wires: ["red", "black", "yellow", "green"]
            .iter()
            .enumerate()
            .map(|(i, color)| Wire {
                id: id(),
                from: WireEnd {
                    end: x1,
                    pin: (i + 1).to_string(),
                },
                to: WireEnd {
                    end: x2,
                    pin: (i + 1).to_string(),
                },
                signal: String::new(),
                color: (*color).to_owned(),
                gauge: "AWG 24".to_owned(),
                length: None,
            })
            .collect(),
    }];

    let rules = Rules::new(id(), "Design rules");
    let mut add = |kind: DocumentKind, path: &str, document: Document| {
        let reference = DocumentRef {
            id: document.id(),
            kind,
            path: path.to_owned(),
        };
        project
            .add_document(reference, usize::MAX, document)
            .unwrap_or_default();
    };
    add(
        DocumentKind::Pcb,
        "pcb/carrier.ewpcb",
        Document::Pcb(carrier),
    );
    add(DocumentKind::Pcb, "pcb/shield.ewpcb", Document::Pcb(shield));
    add(
        DocumentKind::Pcb,
        "pcb/front-panel.ewpcb",
        Document::Pcb(panel),
    );
    add(
        DocumentKind::System,
        "systems/station.ewsys",
        Document::System(system),
    );
    add(
        DocumentKind::Rules,
        "rules/rules.ewrules",
        Document::Rules(rules),
    );
    project
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_three_boards_and_a_system() {
        let project = generate(1);
        assert_eq!(project.documents_of(DocumentKind::Pcb).count(), 3);
        let system = project
            .documents_of(DocumentKind::System)
            .find_map(|e| match project.document(e.id) {
                Some(Document::System(s)) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| System::new(Id::from_u128(0), ""));
        assert_eq!(system.boards.len(), 3);
        assert_eq!(system.cables[0].wires.len(), 4);
        let carrier = project
            .documents_of(DocumentKind::Pcb)
            .find_map(|e| match project.document(e.id) {
                Some(Document::Pcb(p)) if p.name == "Carrier" => Some(p.clone()),
                _ => None,
            })
            .unwrap_or_else(|| {
                Pcb::new(
                    Id::from_u128(0),
                    "",
                    Stackup::standard(2, STANDARD_THICKNESS),
                )
            });
        let j1 = carrier.footprints.iter().find(|f| f.designator == "J1");
        assert_eq!(j1.map(|f| f.pads.len()), Some(16));
        assert!(j1.is_some_and(|f| f.pads.iter().all(|p| p.net.is_some())));
    }
}
