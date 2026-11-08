//! Imports the synthetic KiCad 10 project `tests/fixtures/blinky` (made by
//! `scripts/make-kicad-fixture.py`) and checks the result.

#![allow(
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "test helpers may panic; one test checks the whole import"
)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use ew_kicad::{Severity, import_project, library, netlist};
use ew_model::document::Document;
use ew_model::layer::{CopperLayer, Side};
use ew_model::library::SymbolDef;
use ew_model::pcb::Pcb;
use ew_model::units::Point;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/blinky")
        .join(name)
}

fn board(project: &ew_model::project::Project) -> &Pcb {
    project
        .documents
        .values()
        .find_map(|doc| match doc {
            Document::Pcb(pcb) => Some(pcb),
            _ => None,
        })
        .unwrap()
}

#[test]
fn imports_the_demo_project() {
    let imported = import_project(&fixture("blinky.kicad_pro"), None).unwrap();
    let project = &imported.project;
    let report = &imported.report;
    assert_eq!(project.manifest.name, "blinky");
    assert_eq!(project.manifest.revision, "A");
    assert_eq!(
        project.manifest.variables.get("AUTHOR").map(String::as_str),
        Some("Demo Lab")
    );
    assert_eq!(report.total(Severity::Error), 0, "{report:?}");
    let codes: BTreeSet<&str> = report.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(codes.contains("custom-rules"));
    assert!(codes.contains("external-library"));
    assert!(codes.contains("unsupported-item"), "the dimension");

    // Two sheets, linked by the sheet symbol.
    let sheets: Vec<_> = project
        .documents
        .values()
        .filter_map(|doc| match doc {
            Document::Schematic(sheet) => Some(sheet),
            _ => None,
        })
        .collect();
    assert_eq!(sheets.len(), 2);
    let root = sheets
        .iter()
        .find(|sheet| Some(sheet.id) == project.manifest.root_schematic)
        .unwrap();
    assert_eq!(root.name, "Blinky");
    let link = root.sheets.iter().next().unwrap();
    assert!(
        sheets
            .iter()
            .any(|sheet| sheet.id == link.schematic && sheet.name == "Power")
    );
    let paths: Vec<&str> = project
        .manifest
        .documents
        .iter()
        .map(|d| d.path.as_str())
        .collect();
    assert!(paths.contains(&"schematics/blinky.ewsch"));
    assert!(paths.contains(&"schematics/power.ewsch"));
    assert!(paths.contains(&"pcb/blinky.ewpcb"));

    // The board: 4 layers, a part on the bottom, an arc track, a filled zone.
    let pcb = board(project);
    assert_eq!(pcb.stackup.copper_count(), 4);
    assert_eq!(pcb.stackup.finish, "ENIG");
    assert_eq!(pcb.footprints.len(), 8);
    let c2 = pcb
        .footprints
        .iter()
        .find(|fp| fp.designator == "C2")
        .unwrap();
    assert_eq!(c2.side, Side::Bottom);
    assert!(
        c2.graphics
            .iter()
            .all(|g| g.layer.side() != Some(Side::Bottom)),
        "stored as seen from the top"
    );
    assert_eq!(pcb.tracks.iter().filter(|t| t.mid.is_some()).count(), 1);
    assert_eq!(pcb.vias.len(), 2);
    let zone = pcb.zones.iter().next().unwrap();
    assert_eq!(zone.layers, vec![CopperLayer::Inner(1)]);
    assert_eq!(zone.filled.len(), 1);
    assert_eq!(pcb.origin, Point::new(100_000_000, 80_000_000));

    // Symbols linked to their footprints.
    let symbols: Vec<_> = sheets
        .iter()
        .flat_map(|sheet| sheet.symbols.iter())
        .collect();
    for fp in &pcb.footprints {
        let symbol = symbols
            .iter()
            .find(|s| s.designator == fp.designator)
            .unwrap();
        assert_eq!(symbol.component, fp.component, "{}", fp.designator);
    }

    // The project library: design symbols, footprints and the project's own
    // libraries (with the derived symbol resolved).
    let library = project
        .documents
        .values()
        .find_map(|doc| match doc {
            Document::Library(library) => Some(library),
            _ => None,
        })
        .unwrap();
    for name in [
        "Device:R",
        "Timer:NE555P",
        "power:GND",
        "Blinky:TestPoint",
        "Blinky:TestPoint_Small",
    ] {
        assert!(library.symbol(name).is_some(), "{name}");
    }
    assert!(library.symbol("power:GND").unwrap().power);
    assert!(library.footprint("Blinky:TestPoint_Pad_1.0mm").is_some());
    assert!(
        library
            .footprint("Package_SO:SOIC-8_3.9x4.9mm_P1.27mm")
            .is_some()
    );

    // Rules from the project file.
    let rules = project
        .documents
        .values()
        .find_map(|doc| match doc {
            Document::Rules(rules) => Some(rules),
            _ => None,
        })
        .unwrap();
    assert_eq!(rules.net_classes[0].name, "Default");
    assert_eq!(rules.assignments.len(), 2);
    assert_eq!(rules.constraints.min_copper_to_edge, 500_000);
}

#[test]
fn the_board_connects_like_every_netlist() {
    let imported = import_project(&fixture("blinky.kicad_pcb"), None).unwrap();
    let actual = netlist::of_board(board(&imported.project));
    for (file, format) in [
        ("blinky.net", netlist::Format::Kicad),
        ("blinky-netlist.xml", netlist::Format::Xml),
        ("blinky-netlist.csv", netlist::Format::Csv),
    ] {
        let text = std::fs::read_to_string(fixture(file)).unwrap();
        let (expected, detected) = netlist::parse(&text).unwrap();
        assert_eq!(detected, format);
        let comparison = netlist::compare(&expected, &actual);
        assert!(comparison.is_equivalent(), "{file}: {comparison:?}");
        assert_eq!(comparison.same_names, comparison.nets);
    }
}

#[test]
fn symbol_pins_meet_their_wires() {
    let imported = import_project(&fixture("blinky.kicad_sch"), None).unwrap();
    let project = &imported.project;
    let defs: Vec<&SymbolDef> = project
        .documents
        .values()
        .filter_map(|doc| match doc {
            Document::Library(library) => Some(library.symbols.iter().collect::<Vec<_>>()),
            _ => None,
        })
        .flatten()
        .collect();
    let mut checked = 0;
    for doc in project.documents.values() {
        let Document::Schematic(sheet) = doc else {
            continue;
        };
        let ends: BTreeSet<Point> = sheet.wires.iter().flat_map(|w| [w.start, w.end]).collect();
        for symbol in &sheet.symbols {
            let def = defs.iter().find(|d| d.name == symbol.symbol).unwrap();
            for pin in &def.units[0].pins {
                let angle = symbol.rotation.degrees().to_radians();
                let x = if symbol.mirror { -pin.at.x } else { pin.at.x } as f64;
                let y = pin.at.y as f64;
                let end = Point::new(
                    symbol.at.x + (x * angle.cos() - y * angle.sin()).round() as i64,
                    symbol.at.y + (x * angle.sin() + y * angle.cos()).round() as i64,
                );
                assert!(
                    ends.contains(&end),
                    "{} pin {} at {end:?}",
                    symbol.designator,
                    pin.number
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 20);
}

#[test]
fn imports_the_project_libraries_alone() {
    let mut report = ew_kicad::ImportReport::default();
    let symbols = library::import(&fixture("Blinky.kicad_sym"), None, &mut report).unwrap();
    assert_eq!(symbols.symbols.len(), 2);
    let footprints = library::import(&fixture("Blinky.pretty"), None, &mut report).unwrap();
    assert_eq!(footprints.footprints[0].name, "Blinky:TestPoint_Pad_1.0mm");
    assert!(ew_kicad::locate(&fixture("Blinky.pretty")).is_err());
}
