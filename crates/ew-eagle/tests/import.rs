//! The Eagle import of the synthetic blinky project
//! (`scripts/make-eagle-fixture.py`): the schematic connects exactly the
//! nets Eagle lists, every track ends on a pad or via of its net, and the
//! library imports on its own.

#![allow(clippy::unwrap_used, reason = "test helpers")]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use ew_model::document::Document;
use ew_model::pcb::Pcb;
use ew_model::project::Project;
use ew_pcb::geom::V;
use ew_pcb::items::footprint_xform;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/blinky")
        .join(name)
}

fn board(project: &Project) -> &Pcb {
    project
        .documents
        .values()
        .find_map(|doc| match doc {
            Document::Pcb(pcb) => Some(pcb),
            _ => None,
        })
        .unwrap()
}

/// The nets the Eagle schematic lists: name → (part, pad), parts without a
/// package (supply symbols) left out.
fn eagle_nets() -> BTreeMap<String, BTreeSet<(String, String)>> {
    let text = std::fs::read_to_string(fixture("blinky.sch")).unwrap();
    let document = ew_eagle::parse(&fixture("blinky.sch"), &text).unwrap();
    let schematic =
        ew_eagle::util::path(document.root_element(), &["drawing", "schematic"]).unwrap();
    let mut report = ew_kicad::ImportReport::new("test");
    let library = ew_eagle::library::read(
        ew_eagle::util::path(schematic, &["libraries", "library"]).unwrap(),
        &mut report,
        "test",
    );
    let parts = ew_eagle::schematic::parts(schematic);
    let mut out: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();
    for net in schematic.descendants().filter(|n| n.has_tag_name("net")) {
        for pinref in net.descendants().filter(|n| n.has_tag_name("pinref")) {
            let part = &parts[pinref.attribute("part").unwrap()];
            let device = &library.devices[&(part.deviceset.clone(), part.device.clone())];
            if device.footprint.is_none() {
                continue;
            }
            let unit = device
                .gates
                .iter()
                .position(|g| g == pinref.attribute("gate").unwrap())
                .unwrap();
            let index = device.pin_names[unit]
                .iter()
                .position(|p| p == pinref.attribute("pin").unwrap())
                .unwrap();
            out.entry(net.attribute("name").unwrap().to_owned())
                .or_default()
                .insert((
                    pinref.attribute("part").unwrap().to_owned(),
                    device.def.units[unit].pins[index].number.clone(),
                ));
        }
    }
    out
}

#[test]
fn the_schematic_connects_the_nets_of_eagle() {
    let imported = ew_eagle::import_project(&fixture("blinky.sch"), Some("Blinky")).unwrap();
    let errors: Vec<_> = imported
        .report
        .findings
        .iter()
        .filter(|f| f.severity != ew_kicad::Severity::Info)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    let project = &imported.project;
    let sheets = project
        .documents
        .values()
        .filter(|d| matches!(d, Document::Schematic(_)))
        .count();
    // The root with a sheet symbol for each of the two Eagle sheets.
    assert_eq!(sheets, 3);

    let ours = ew_schematic::connectivity::analyze(project).netlist();
    let expected = eagle_nets();
    // The same groups of pins …
    let groups = |nets: &BTreeMap<String, BTreeSet<(String, String)>>| {
        nets.values()
            .filter(|pins| !pins.is_empty())
            .cloned()
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(groups(&ours), groups(&expected));
    // … with the names Eagle gave them (local nets carry their sheet path).
    for name in ["VCC", "GND", "TIMING", "LED"] {
        let found = ours
            .iter()
            .find(|(net, _)| *net == name || net.ends_with(&format!("/{name}")))
            .map(|(_, pins)| pins);
        assert_eq!(found, expected.get(name), "{name} in {:?}", ours.keys());
    }

    // Parts, values, footprints and the link to the board.
    let symbols: Vec<_> = project
        .documents
        .values()
        .filter_map(|d| match d {
            Document::Schematic(s) => Some(s),
            _ => None,
        })
        .flat_map(|s| s.symbols.iter())
        .collect();
    let u1 = symbols.iter().find(|s| s.designator == "U1").unwrap();
    assert_eq!(u1.value, "NE555");
    assert_eq!(u1.footprint.as_deref(), Some("blinky:SO08"));
    assert_eq!(
        u1.properties.get("MPN").map(String::as_str),
        Some("NE555DR")
    );
    let c2 = symbols.iter().find(|s| s.designator == "C2").unwrap();
    assert!(c2.mirror);
    let gnd = symbols.iter().find(|s| s.designator == "GND1").unwrap();
    assert!(gnd.flags.exclude_from_board && gnd.component.is_none());
    let pcb = board(project);
    // Every part is on the board and linked (the link itself is checked by
    // the board update in the engine tests).
    for symbol in symbols.iter().filter(|s| s.component.is_some()) {
        let fp = pcb
            .footprints
            .iter()
            .find(|f| f.designator == symbol.designator)
            .unwrap();
        assert!(fp.component.is_some(), "{} is linked", symbol.designator);
    }
}

#[test]
fn every_track_ends_on_a_pad_or_via_of_its_net() {
    let imported = ew_eagle::import_project(&fixture("blinky.brd"), None).unwrap();
    let pcb = board(&imported.project);
    assert_eq!(imported.project.manifest.name, "blinky");
    assert_eq!(pcb.nets.len(), 7);
    // Eight parts and the mounting hole.
    assert_eq!(pcb.footprints.len(), 9);
    let c2 = pcb
        .footprints
        .iter()
        .find(|f| f.designator == "C2")
        .unwrap();
    assert_eq!(c2.side, ew_model::layer::Side::Bottom);
    assert_eq!(pcb.vias.len(), 2);
    assert_eq!(pcb.zones.len(), 1);
    assert_eq!(
        pcb.shapes
            .iter()
            .filter(|s| s.layer == ew_model::layer::Layer::Outline)
            .count(),
        4
    );

    let mut anchors: Vec<(V, Option<ew_model::id::Id>)> = Vec::new();
    for fp in &pcb.footprints {
        let xf = footprint_xform(fp);
        for pad in &fp.pads {
            anchors.push((xf.apply(V::of(pad.at)), pad.net));
        }
    }
    for via in &pcb.vias {
        anchors.push((V::of(via.at), via.net));
    }
    assert_eq!(pcb.tracks.len(), 4);
    for track in &pcb.tracks {
        for end in [track.start, track.end] {
            let p = V::of(end);
            assert!(
                anchors
                    .iter()
                    .any(|(a, net)| a.dist(p) < 1_000.0 && *net == track.net),
                "a track end at {end:?} meets no pad or via of its net"
            );
        }
    }
    // Net classes from Eagle's classes.
    let rules = imported
        .project
        .documents
        .values()
        .find_map(|d| match d {
            Document::Rules(r) => Some(r),
            _ => None,
        })
        .unwrap();
    let power = rules
        .net_classes
        .iter()
        .find(|c| c.name == "power")
        .unwrap();
    assert_eq!(power.track_width, 500_000);
    assert_eq!(power.clearance, 300_000);
    assert_eq!(rules.constraints.min_track_width, 152_400);
    let gnd = pcb.nets.iter().find(|n| n.name == "GND").unwrap();
    assert_eq!(gnd.class, "power");
}

#[test]
fn a_library_on_its_own() {
    let mut report = ew_kicad::ImportReport::new("test");
    let library = ew_eagle::library::import(&fixture("blinky.lbr"), &mut report).unwrap();
    assert_eq!(library.footprints.len(), 5);
    assert_eq!(library.symbols.len(), 7);
    let r = library
        .symbols
        .iter()
        .find(|s| s.name == "blinky:R0805")
        .unwrap();
    assert_eq!(r.prefix, "R");
    assert_eq!(r.footprint.as_deref(), Some("blinky:R0805"));
    assert!(r.pin_text.hide_names && r.pin_text.hide_numbers);
    let timer = library
        .symbols
        .iter()
        .find(|s| s.name == "blinky:NE555D")
        .unwrap();
    let numbers: BTreeSet<&str> = timer.units[0]
        .pins
        .iter()
        .map(|p| p.number.as_str())
        .collect();
    assert_eq!(numbers.len(), 8);
    assert!(numbers.contains("8"));
    let gnd = library
        .symbols
        .iter()
        .find(|s| s.name == "blinky:GND")
        .unwrap();
    assert!(gnd.power);
    let led = library
        .footprints
        .iter()
        .find(|f| f.name == "blinky:LED3MM")
        .unwrap();
    assert_eq!(
        led.attributes.mounting,
        ew_model::pcb::Mounting::ThroughHole
    );
    let header = library
        .footprints
        .iter()
        .find(|f| f.name == "blinky:HDR2")
        .unwrap();
    assert!(matches!(
        header.pads[1].shape,
        ew_model::pcb::PadShape::Custom { .. }
    ));

    assert!(ew_eagle::import_project(&fixture("missing.sch"), None).is_err());
}
