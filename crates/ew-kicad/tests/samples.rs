//! Regression tests against real KiCad projects that stay on the developer's
//! machine (they are never committed). Set `EW_KICAD_SAMPLE` to a
//! `.kicad_pro` file with a `.net` netlist exported next to it, and
//! optionally `EW_KICAD_LIBRARIES` to a folder with the official KiCad
//! symbol and footprint libraries, then run
//! `cargo test --release -p ew-kicad --test samples -- --ignored --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "test output and geometry checks"
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ew_kicad::{ImportReport, Severity, import_project, library, netlist};
use ew_model::document::Document;
use ew_model::library::{PinDirection, SymbolDef};
use ew_model::units::Point;

fn pin_end(symbol: &ew_model::schematic::PlacedSymbol, at: Point) -> Point {
    let angle = symbol.rotation.degrees().to_radians();
    let x = if symbol.mirror { -at.x } else { at.x } as f64;
    let y = at.y as f64;
    let (sin, cos) = angle.sin_cos();
    Point::new(
        symbol.at.x + (x * cos - y * sin).round() as i64,
        symbol.at.y + (x * sin + y * cos).round() as i64,
    )
}

#[test]
#[ignore = "needs a private KiCad project (EW_KICAD_SAMPLE)"]
fn imports_a_real_project() {
    let Ok(sample) = std::env::var("EW_KICAD_SAMPLE") else {
        println!("EW_KICAD_SAMPLE is not set; skipped");
        return;
    };
    let sample = PathBuf::from(sample);
    let started = Instant::now();
    let imported = import_project(&sample, None).unwrap();
    println!("import: {:?}", started.elapsed());
    let report = &imported.report;
    println!("{}", report.to_markdown("KiCad import"));
    assert_eq!(report.total(Severity::Error), 0, "import errors");

    // Electrical check: the board connects like KiCad's netlist.
    let net_file = sample.with_extension("net");
    let text = std::fs::read_to_string(&net_file).unwrap();
    let (expected, _) = netlist::parse(&text).unwrap();
    let pcb = imported
        .project
        .documents
        .values()
        .find_map(|doc| match doc {
            Document::Pcb(pcb) => Some(pcb),
            _ => None,
        })
        .unwrap();
    let comparison = netlist::compare(&expected, &netlist::of_board(pcb));
    println!(
        "nets: {} of {} match ({} with the same name), {} differences, missing {:?}",
        comparison.matching_nets,
        comparison.nets,
        comparison.same_names,
        comparison.differences.len(),
        comparison.missing_components
    );
    for difference in comparison.differences.iter().take(20) {
        println!("  {difference:?}");
    }
    assert!(comparison.is_equivalent());

    // Geometric check: pin ends of placed symbols meet wires, labels,
    // junctions, other pins or no-connect flags.
    let defs: Vec<&SymbolDef> = imported
        .project
        .documents
        .values()
        .filter_map(|doc| match doc {
            Document::Library(library) => Some(library.symbols.iter().collect::<Vec<_>>()),
            _ => None,
        })
        .flatten()
        .collect();
    let mut total = 0;
    let mut connected = 0;
    for doc in imported.project.documents.values() {
        let Document::Schematic(sheet) = doc else {
            continue;
        };
        let mut anchors: BTreeSet<Point> = BTreeSet::new();
        for wire in &sheet.wires {
            anchors.insert(wire.start);
            anchors.insert(wire.end);
        }
        anchors.extend(sheet.labels.iter().map(|l| l.at));
        anchors.extend(sheet.junctions.iter().map(|j| j.at));
        anchors.extend(sheet.no_connects.iter().map(|n| n.at));
        let mut pins: Vec<Point> = Vec::new();
        for symbol in &sheet.symbols {
            let Some(def) = defs.iter().find(|d| d.name == symbol.symbol) else {
                continue;
            };
            let Some(unit) = def.units.get(usize::from(symbol.unit.max(1) - 1)) else {
                continue;
            };
            for pin in &unit.pins {
                let _ = PinDirection::Right;
                pins.push(pin_end(symbol, pin.at));
            }
        }
        for (index, pin) in pins.iter().enumerate() {
            total += 1;
            let other_pin = pins.iter().enumerate().any(|(i, p)| i != index && p == pin);
            if anchors.contains(pin) || other_pin {
                connected += 1;
            }
        }
    }
    let share = f64::from(connected) / f64::from(total.max(1));
    println!(
        "pins meeting a connection: {connected} of {total} ({:.1} %)",
        share * 100.0
    );
    assert!(share > 0.9, "symbol placement looks wrong");
}

#[test]
#[ignore = "needs the KiCad libraries (EW_KICAD_LIBRARIES)"]
fn reads_every_official_library() {
    let Ok(root) = std::env::var("EW_KICAD_LIBRARIES") else {
        println!("EW_KICAD_LIBRARIES is not set; skipped");
        return;
    };
    let root = Path::new(&root);
    let mut report = ImportReport::default();
    let started = Instant::now();
    let mut symbols = 0;
    let mut footprints = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            match extension {
                "kicad_sym" | "kicad_symdir" | "pretty" => {
                    let lib = library::import(&path, None, &mut report).unwrap();
                    symbols += lib.symbols.len();
                    footprints += lib.footprints.len();
                }
                _ if path.is_dir() && !path.to_string_lossy().contains("3dshapes") => {
                    stack.push(path);
                }
                _ => {}
            }
        }
    }
    report.finish();
    println!(
        "{symbols} symbols and {footprints} footprints in {:?}; {} errors, {} warnings",
        started.elapsed(),
        report.total(Severity::Error),
        report.total(Severity::Warning)
    );
    for finding in report.findings.iter().take(30) {
        println!("  {finding:?}");
    }
    assert_eq!(report.total(Severity::Error), 0);
}
