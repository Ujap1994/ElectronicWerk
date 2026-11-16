//! A small synthetic project with a repeated sheet: a top sheet that uses
//! one "RC filter" sheet several times (a resistor and a capacitor per
//! use). On the board, the first channel is placed and routed, the others
//! are placed in another arrangement and not routed, so copying the layout
//! of a channel can be shown and tested.

use std::collections::BTreeMap;

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, Side};
use ew_model::outputs::OutputJob;
use ew_model::pcb::{BoardShape, FootprintInstance, Net, Pcb, STANDARD_THICKNESS, Stackup, Track};
use ew_model::project::{DocumentRef, Project};
use ew_model::rules::Rules;
use ew_model::schematic::{
    ComponentFlags, Label, LabelKind, PlacedSymbol, Schematic, SheetSymbol, Wire, instance_key,
};
use ew_model::shape::Geometry;
use ew_model::units::{Angle, Point, Size, mm, um};

use crate::parts::{self, PartKind};
use crate::rng::Rng;

/// The board link of a part in a use of a sheet (as the board sync makes
/// it: the symbol id for the top sheet, a derived id below).
fn part_key(symbol: Id, instance: &str) -> Id {
    if instance == "/" {
        symbol
    } else {
        Id::derived(&["part", &symbol.to_string(), instance])
    }
}

/// A project with one sheet used `count` times (at least 2).
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "the whole small project in one piece"
)]
pub fn generate(count: usize, seed: u64) -> Project {
    let count = count.max(2);
    let mut rng = Rng::new(seed);
    let mut id = || Id::from_random_bytes(rng.bytes());
    let mut project = Project::new(id(), "Channels");
    "A".clone_into(&mut project.manifest.revision);
    "Synthetic design with a repeated sheet (generated, not a real circuit)."
        .clone_into(&mut project.manifest.description);
    let library = parts::library(id(), &mut id);

    // The repeated sheet: IN — R — OUT — C — GND.
    let mut child = Schematic::new(id(), "RC filter");
    let kinds = [PartKind::Resistor, PartKind::Capacitor];
    let nets = [["IN", "OUT"], ["OUT", "GND"]];
    let mut symbols = Vec::new();
    for (index, kind) in kinds.iter().enumerate() {
        let at = Point::new(mm(60) + i64::try_from(index).unwrap_or(0) * mm(40), mm(150));
        let symbol_id = id();
        symbols.push(symbol_id);
        child.symbols.insert(PlacedSymbol {
            id: symbol_id,
            symbol: kind.symbol_name().to_owned(),
            unit: 1,
            at,
            rotation: Angle::ZERO,
            mirror: false,
            designator: String::new(),
            value: kind.value().to_owned(),
            footprint: Some(kind.footprint_name().to_owned()),
            component: None,
            properties: BTreeMap::new(),
            fields: Vec::new(),
            flags: ComponentFlags::default(),
            instances: BTreeMap::new(),
            alternate: false,
        });
        for (pin, net) in kind.pin_points().into_iter().zip(nets[index]) {
            let start = at + pin.at;
            let end = start + pin.outward(um(2_540));
            child.wires.insert(Wire {
                id: id(),
                start,
                end,
            });
            let out = pin.outward(1);
            let rotation = match (out.x, out.y) {
                (-1, _) => Angle::DEG_180,
                (_, 1) => Angle::DEG_90,
                (_, -1) => Angle::DEG_270,
                _ => Angle::ZERO,
            };
            child.labels.insert(Label {
                id: id(),
                kind: if net == "GND" {
                    LabelKind::Global
                } else {
                    LabelKind::Local
                },
                text: net.to_owned(),
                at: end,
                rotation,
                shape: ew_model::schematic::PortShape::Passive,
                style: ew_model::text::TextStyle::default(),
            });
        }
    }

    // The top sheet uses it `count` times.
    let mut root = Schematic::new(id(), "Root");
    let mut uses = Vec::new();
    for k in 0..count {
        let sheet = id();
        let index = i64::try_from(k).unwrap_or(0);
        root.sheets.insert(SheetSymbol {
            id: sheet,
            name: format!("Channel {}", k + 1),
            schematic: child.id,
            at: Point::new(mm(30) + index * mm(50), mm(200)),
            size: Size::mm(40, 25),
            pins: Vec::new(),
        });
        uses.push((instance_key(&[sheet]), format!("Channel {}", k + 1)));
    }
    // Per-use designators: R1/C1, R2/C2, …
    let mut placed: Vec<PlacedSymbol> = child.symbols.iter().cloned().collect();
    placed.sort_by_key(|s| s.at.x);
    for symbol in &mut placed {
        let prefix = if symbol.symbol == PartKind::Resistor.symbol_name() {
            "R"
        } else {
            "C"
        };
        for (k, (key, _)) in uses.iter().enumerate() {
            symbol
                .instances
                .insert(key.clone(), format!("{prefix}{}", k + 1));
        }
        symbol.designator = format!("{prefix}1");
        child.symbols.insert(symbol.clone());
    }

    // The board.
    let mut pcb = Pcb::new(id(), "Board", Stackup::standard(2, STANDARD_THICKNESS));
    let width = mm(20) + i64::try_from(count).unwrap_or(2) * mm(20);
    pcb.shapes.insert(BoardShape {
        id: id(),
        layer: Layer::Outline,
        geometry: Geometry::Rect {
            start: Point::ORIGIN,
            end: Point::new(width, mm(40)),
        },
        width: um(100),
        filled: false,
    });
    let gnd = id();
    pcb.nets.insert(Net {
        id: gnd,
        name: "GND".into(),
        class: String::new(),
    });
    for (k, (key, name)) in uses.iter().enumerate() {
        let local = |net: &str| format!("/{name}/{net}");
        let (input, output) = (id(), id());
        for (net, label) in [(input, "IN"), (output, "OUT")] {
            pcb.nets.insert(Net {
                id: net,
                name: local(label),
                class: String::new(),
            });
        }
        let x = mm(15) + i64::try_from(k).unwrap_or(0) * mm(20);
        // Channel 1: the capacitor below the resistor, routed. The others:
        // side by side, not routed.
        let (r_at, c_at, c_rotation) = if k == 0 {
            (
                Point::new(x, mm(28)),
                Point::new(x + mm(4), mm(20)),
                Angle::DEG_270,
            )
        } else {
            (
                Point::new(x, mm(12)),
                Point::new(x + mm(8), mm(12)),
                Angle::ZERO,
            )
        };
        for (symbol, kind, at, rotation, pad_nets) in [
            (
                placed[0].id,
                PartKind::Resistor,
                r_at,
                Angle::ZERO,
                [input, output],
            ),
            (
                placed[1].id,
                PartKind::Capacitor,
                c_at,
                c_rotation,
                [output, gnd],
            ),
        ] {
            let Some(definition) = library.footprint(kind.footprint_name()) else {
                continue;
            };
            let mut pads = definition.pads.clone();
            for (pad, net) in pads.iter_mut().zip(pad_nets) {
                pad.id = id();
                pad.net = Some(net);
            }
            let prefix = if kind == PartKind::Resistor { "R" } else { "C" };
            pcb.footprints.insert(FootprintInstance {
                id: id(),
                component: Some(part_key(symbol, key)),
                footprint: definition.name.clone(),
                designator: format!("{prefix}{}", k + 1),
                value: kind.value().to_owned(),
                at,
                rotation,
                side: Side::Top,
                locked: false,
                pads,
                graphics: definition.graphics.clone(),
                texts: definition.texts.clone(),
                models: Vec::new(),
                properties: BTreeMap::new(),
                attributes: definition.attributes,
            });
        }
        if k == 0 {
            // R pad 2 down to C pad 1: a 45° diagonal, then straight.
            let pad = |kind: PartKind, number: usize, at: Point, rotation: Angle| {
                library
                    .footprint(kind.footprint_name())
                    .and_then(|d| d.pads.get(number))
                    .map(|p| at + crate::rotate(p.at, rotation))
            };
            if let (Some(a), Some(b)) = (
                pad(PartKind::Resistor, 1, r_at, Angle::ZERO),
                pad(PartKind::Capacitor, 0, c_at, c_rotation),
            ) {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let m = dx.abs().min(dy.abs());
                let mid = Point::new(a.x + dx.signum() * m, a.y + dy.signum() * m);
                for (start, end) in [(a, mid), (mid, b)] {
                    if start != end {
                        pcb.tracks.insert(Track {
                            id: id(),
                            layer: CopperLayer::Top,
                            net: Some(output),
                            width: um(250),
                            start,
                            end,
                            mid: None,
                            locked: false,
                        });
                    }
                }
            }
        }
    }

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
    let root_id = root.id;
    add(
        DocumentKind::Schematic,
        "schematics/root.ewsch",
        Document::Schematic(root),
    );
    add(
        DocumentKind::Schematic,
        "schematics/rc-filter.ewsch",
        Document::Schematic(child),
    );
    add(DocumentKind::Pcb, "pcb/board.ewpcb", Document::Pcb(pcb));
    add(
        DocumentKind::Library,
        "library/project.ewlib",
        Document::Library(library),
    );
    add(
        DocumentKind::Rules,
        "rules/design.ewrules",
        Document::Rules(Rules::new(id(), "Design rules")),
    );
    let job = OutputJob::fabrication(id(), "Fabrication", &mut id);
    add(
        DocumentKind::OutputJob,
        "outputs/fabrication.ewout",
        Document::OutputJob(job),
    );
    project.manifest.root_schematic = Some(root_id);
    project
}
