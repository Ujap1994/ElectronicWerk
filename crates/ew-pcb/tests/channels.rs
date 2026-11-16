//! Channels: a sheet used twice gives two groups of footprints, and the
//! layout of one is copied to the other with the other's nets.

#![allow(clippy::unwrap_used, reason = "test helpers")]

use std::collections::BTreeMap;

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::layer::CopperLayer;
use ew_model::pcb::{FootprintInstance, Net, Pcb, STANDARD_THICKNESS, Stackup, Track};
use ew_model::project::{DocumentRef, Project};
use ew_model::schematic::{PlacedSymbol, Schematic, SheetSymbol, instance_key};
use ew_model::units::Point;
use ew_pcb::channels::{channels, copy_layout};
use ew_pcb::sync::part_key;
use serde_json::json;

fn symbol(designators: &[(&str, &str)]) -> PlacedSymbol {
    let instances: BTreeMap<String, String> = designators
        .iter()
        .map(|(k, d)| ((*k).to_owned(), (*d).to_owned()))
        .collect();
    serde_json::from_value(json!({
        "id": Id::random(), "symbol": "R", "unit": 1, "at": [0, 0],
        "designator": "R?", "value": "1k", "footprint": "R_0805",
        "instances": instances,
    }))
    .unwrap()
}

fn footprint(component: Id, designator: &str, at: Point, nets: [Id; 2]) -> FootprintInstance {
    serde_json::from_value(json!({
        "id": Id::random(), "component": component, "footprint": "R_0805",
        "designator": designator, "value": "1k", "at": at,
        "pads": [
            {"id": Id::random(), "number": "1", "net": nets[0], "shape": "rect",
             "at": [-1_000_000, 0], "size": [1_000_000, 1_200_000], "layers": ["cu.top"]},
            {"id": Id::random(), "number": "2", "net": nets[1], "shape": "rect",
             "at": [1_000_000, 0], "size": [1_000_000, 1_200_000], "layers": ["cu.top"]},
        ],
    }))
    .unwrap()
}

#[test]
#[allow(clippy::too_many_lines, reason = "one project built by hand")]
fn the_layout_of_one_channel_is_copied_to_the_other() {
    let mut project = Project::new(Id::random(), "Channels");
    let (root_id, child_id, board_id) = (Id::random(), Id::random(), Id::random());
    let (s1, s2) = (Id::random(), Id::random());
    let (k1, k2) = (instance_key(&[s1]), instance_key(&[s2]));

    // The top sheet uses the channel sheet twice.
    let mut root = Schematic::new(root_id, "Top");
    for (id, name) in [(s1, "Channel 1"), (s2, "Channel 2")] {
        let sheet: SheetSymbol = serde_json::from_value(json!({
            "id": id, "name": name, "schematic": child_id,
            "at": [0, 0], "size": [10_000_000, 10_000_000],
        }))
        .unwrap();
        root.sheets.insert(sheet);
    }
    let mut child = Schematic::new(child_id, "Channel");
    let ra = symbol(&[(&k1, "R1"), (&k2, "R3")]);
    let rb = symbol(&[(&k1, "R2"), (&k2, "R4")]);
    child.symbols.insert(ra.clone());
    child.symbols.insert(rb.clone());

    // The board: channel 1 placed and routed, channel 2 somewhere else.
    let mut pcb = Pcb::new(board_id, "Board", Stackup::standard(2, STANDARD_THICKNESS));
    let net = |pcb: &mut Pcb, name: &str| {
        let id = Id::random();
        pcb.nets.insert(Net {
            id,
            name: name.into(),
            class: String::new(),
        });
        id
    };
    let (n1, n2, gnd, a1, a2) = (
        net(&mut pcb, "N1"),
        net(&mut pcb, "N2"),
        net(&mut pcb, "GND"),
        net(&mut pcb, "A1"),
        net(&mut pcb, "A2"),
    );
    pcb.footprints.insert(footprint(
        part_key(&ra, &k1),
        "R1",
        Point::mm(10, 10),
        [a1, n1],
    ));
    pcb.footprints.insert(footprint(
        part_key(&rb, &k1),
        "R2",
        Point::mm(10, 14),
        [n1, gnd],
    ));
    pcb.footprints.insert(footprint(
        part_key(&ra, &k2),
        "R3",
        Point::mm(40, 30),
        [a2, n2],
    ));
    pcb.footprints.insert(footprint(
        part_key(&rb, &k2),
        "R4",
        Point::mm(50, 31),
        [n2, gnd],
    ));
    // R1 pad 2 to R2 pad 1 (net N1), with a bend.
    for (start, end) in [
        (Point::mm(11, 10), Point::mm(12, 11)),
        (Point::mm(12, 11), Point::mm(12, 13)),
        (Point::mm(12, 13), Point::mm(9, 14)),
    ] {
        pcb.tracks.insert(Track {
            id: Id::random(),
            layer: CopperLayer::Top,
            net: Some(n1),
            width: 250_000,
            start,
            end,
            mid: None,
            locked: false,
        });
    }

    let mut add = |id: Id, kind: DocumentKind, path: &str, document: Document| {
        let index = project.manifest.documents.len();
        project
            .add_document(
                DocumentRef {
                    id,
                    kind,
                    path: path.into(),
                },
                index,
                document,
            )
            .unwrap();
    };
    add(
        root_id,
        DocumentKind::Schematic,
        "schematics/top.ewsch",
        Document::Schematic(root),
    );
    add(
        child_id,
        DocumentKind::Schematic,
        "schematics/channel.ewsch",
        Document::Schematic(child),
    );
    add(
        board_id,
        DocumentKind::Pcb,
        "pcb/board.ewpcb",
        Document::Pcb(pcb.clone()),
    );
    project.manifest.root_schematic = Some(root_id);

    let connectivity = ew_schematic::analyze(&project);
    let found = channels(&project, &connectivity, &pcb);
    assert_eq!(found.len(), 1, "{found:?}");
    let channel = &found[0];
    assert_eq!(channel.name, "Channel");
    assert_eq!(channel.instances.len(), 2);
    let second = channel.instances.iter().find(|i| i.key == k2).unwrap();
    let mut designators = second.designators.clone();
    designators.sort();
    assert_eq!(designators, ["R3", "R4"]);

    let copied = copy_layout(&pcb, channel, &k1, std::slice::from_ref(&k2), true);
    assert_eq!(copied.skipped.len(), 0);
    assert_eq!(copied.footprints.len(), 2);
    let r3 = copied
        .footprints
        .iter()
        .find(|f| f.designator == "R3")
        .unwrap();
    let r4 = copied
        .footprints
        .iter()
        .find(|f| f.designator == "R4")
        .unwrap();
    // The same arrangement, at the corner where channel 2 was.
    assert_eq!(r3.at, Point::mm(40, 30));
    assert_eq!(r4.at, Point::mm(40, 34));
    // The routing comes with the nets of channel 2.
    assert_eq!(copied.tracks.len(), 3);
    assert!(copied.tracks.iter().all(|t| t.net == Some(n2)));
    assert!(copied.tracks.iter().any(|t| t.start == Point::mm(41, 30)));
    assert!(copied.tracks.iter().any(|t| t.end == Point::mm(39, 34)));
    assert_eq!(copied.vias.len(), 0);
}
