//! The KiCad export of a whole project, read back with the KiCad import:
//! the same sheets, parts, wires and labels, and the same netlist.

#![allow(clippy::unwrap_used, reason = "test helpers")]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ew_model::document::Document;
use ew_model::project::Project;
use ew_model::schematic::Schematic;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ew-kicad/tests/fixtures/blinky/blinky.kicad_pro")
}

fn write(dir: &Path, project: &Project) -> ew_kicad::export::KicadProject {
    let exported = ew_kicad::export::export_project(project, "0.15.0");
    for file in &exported.files {
        let path = dir.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.data).unwrap();
    }
    exported
}

fn sheets(project: &Project) -> Vec<&Schematic> {
    let mut out: Vec<&Schematic> = project
        .documents
        .values()
        .filter_map(|doc| match doc {
            Document::Schematic(s) => Some(s),
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[test]
fn a_project_survives_the_round_trip() {
    let original = ew_kicad::import_project(&fixture(), Some("Blinky"))
        .unwrap()
        .project;
    let dir = tempfile::tempdir().unwrap();
    let exported = write(dir.path(), &original);
    let names: BTreeSet<&str> = exported.files.iter().map(|f| f.path.as_str()).collect();
    assert!(names.contains("Blinky.kicad_pro"), "{names:?}");
    assert!(names.contains("Blinky.kicad_sch"));
    assert!(names.contains("Blinky.kicad_pcb"));
    assert!(exported.report.notes.is_empty(), "{:?}", exported.report);

    let back =
        ew_kicad::import_project(&dir.path().join("Blinky.kicad_pro"), Some("Blinky")).unwrap();
    let errors: Vec<_> = back
        .report
        .findings
        .iter()
        .filter(|f| f.severity == ew_kicad::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    let back = back.project;

    // The same netlist: every net with the same pins.
    let before = ew_schematic::connectivity::analyze(&original).netlist();
    let after = ew_schematic::connectivity::analyze(&back).netlist();
    assert_eq!(after, before);

    // The same sheets with the same parts, wires and labels.
    let (a, b) = (sheets(&original), sheets(&back));
    assert_eq!(a.len(), b.len());
    for (one, two) in a.iter().zip(&b) {
        assert_eq!(one.name, two.name);
        assert_eq!(one.page, two.page);
        let parts = |s: &Schematic| {
            let mut parts: Vec<String> = s
                .symbols
                .iter()
                .map(|p| {
                    format!(
                        "{} {} {:?} {:?} {} {} {:?} {:?}",
                        p.designator,
                        p.value,
                        p.at,
                        p.rotation,
                        p.mirror,
                        p.unit,
                        p.footprint,
                        p.flags
                    )
                })
                .collect();
            parts.sort();
            parts
        };
        assert_eq!(parts(two), parts(one), "{}", one.name);
        let wires = |s: &Schematic| {
            s.wires
                .iter()
                .map(|w| (w.start, w.end))
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(wires(two), wires(one));
        let labels = |s: &Schematic| {
            s.labels
                .iter()
                .map(|l| {
                    format!(
                        "{:?}",
                        (l.kind, &l.text, l.at, l.rotation, l.shape, l.style)
                    )
                })
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(labels(two), labels(one));
        let junctions = |s: &Schematic| s.junctions.iter().map(|j| j.at).collect::<BTreeSet<_>>();
        assert_eq!(junctions(two), junctions(one));
        let sheet_symbols = |s: &Schematic| {
            s.sheets
                .iter()
                .map(|x| format!("{:?}", (&x.name, x.at, x.size, x.pins.len())))
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(sheet_symbols(two), sheet_symbols(one));
    }

    // Board and schematic stay linked: every footprint finds its symbol.
    let linked = |p: &Project| {
        let symbols: BTreeSet<_> = sheets(p)
            .iter()
            .flat_map(|s| s.symbols.iter().filter_map(|x| x.component))
            .collect();
        p.documents
            .values()
            .filter_map(|doc| match doc {
                Document::Pcb(pcb) => Some(pcb),
                _ => None,
            })
            .flat_map(|pcb| pcb.footprints.iter())
            .filter(|fp| fp.component.is_some_and(|c| symbols.contains(&c)))
            .count()
    };
    assert_eq!(linked(&back), linked(&original));
    assert!(linked(&original) > 0);
}

/// KiCad's netlist of a schematic: net name → (designator, pin).
fn kicad_netlist(
    cli: &std::ffi::OsStr,
    schematic: &Path,
) -> BTreeSet<(String, BTreeSet<(String, String)>)> {
    let netlist = schematic.with_extension("net");
    let output = std::process::Command::new(cli)
        .args(["sch", "export", "netlist", "--output"])
        .arg(&netlist)
        .arg(schematic)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&netlist).unwrap();
    let tree = ew_kicad::sexpr::parse(&text).unwrap();
    tree.get("nets")
        .map(|nets| {
            nets.all("net")
                .map(|net| {
                    let pins = net
                        .all("node")
                        .map(|node| {
                            (
                                node.value("ref").unwrap_or_default().to_owned(),
                                node.value("pin").unwrap_or_default().to_owned(),
                            )
                        })
                        .collect();
                    (net.value("name").unwrap_or_default().to_owned(), pins)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Local check with KiCad itself: `EW_KICAD_CLI=/path/to/kicad-cli cargo
/// test -p ew-schematic --test kicad_export -- --ignored`. KiCad must read
/// the exported schematic and find the same nets as in the original.
#[test]
#[ignore = "needs a local KiCad installation"]
fn kicad_reads_the_exported_project() {
    let Some(cli) = std::env::var_os("EW_KICAD_CLI") else {
        return;
    };
    let original = ew_kicad::import_project(&fixture(), Some("Blinky"))
        .unwrap()
        .project;
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("original");
    std::fs::create_dir_all(&copy).unwrap();
    for entry in std::fs::read_dir(fixture().parent().unwrap()).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            std::fs::copy(entry.path(), copy.join(entry.file_name())).unwrap();
        }
    }
    let exported = dir.path().join("exported");
    std::fs::create_dir_all(&exported).unwrap();
    write(&exported, &original);
    if let Some(dump) = std::env::var_os("EW_KICAD_EXPORT_DIR") {
        for entry in std::fs::read_dir(&exported).unwrap() {
            let entry = entry.unwrap();
            if entry.path().is_file() {
                std::fs::copy(entry.path(), Path::new(&dump).join(entry.file_name())).unwrap();
            }
        }
    }
    let before = kicad_netlist(&cli, &copy.join("blinky.kicad_sch"));
    let after = kicad_netlist(&cli, &exported.join("Blinky.kicad_sch"));
    assert!(!before.is_empty());
    assert_eq!(after, before);
}
