//! KiCad export, read back with the KiCad import: the blinky fixture comes
//! out of a round trip (import → export → import) with the same design.

#![allow(clippy::unwrap_used, clippy::too_many_lines, reason = "test helpers")]

use std::collections::BTreeSet;
use std::path::PathBuf;

use ew_kicad::common::Ids;
use ew_kicad::export::board::board_file;
use ew_kicad::export::{ExportReport, Title};
use ew_kicad::report::ImportReport;
use ew_kicad::sexpr::parse;
use ew_model::document::Document;
use ew_model::pcb::Pcb;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blinky/blinky.kicad_pro")
}

fn imported_board() -> Pcb {
    let imported = ew_kicad::import_project(&fixture(), Some("Blinky")).unwrap();
    imported
        .project
        .documents
        .into_values()
        .find_map(|doc| match doc {
            Document::Pcb(pcb) => Some(pcb),
            _ => None,
        })
        .unwrap()
}

fn title() -> Title<'static> {
    Title {
        project: "Blinky",
        revision: "A",
        date: "2026-11-15",
        company: "",
        library: "Blinky",
        version: "0.15.0",
    }
}

/// Writes a board and reads it back.
fn round_trip(pcb: &Pcb) -> (Pcb, String, ExportReport) {
    let mut report = ExportReport::default();
    let text = board_file(pcb, &title(), &|_| None, &mut report);
    let tree = parse(&text).unwrap();
    let mut import = ImportReport::new("test".to_owned());
    let back = ew_kicad::pcb::board(&tree, &pcb.name, &mut Ids::default(), &mut import, "test");
    (back.pcb, text, report)
}

#[test]
fn a_board_survives_the_round_trip() {
    let pcb = imported_board();
    let (back, text, report) = round_trip(&pcb);
    assert!(text.starts_with("(kicad_pcb\n\t(version 20240108)"));
    assert!(report.notes.is_empty(), "{report:?}");

    assert_eq!(back.stackup, pcb.stackup);
    assert_eq!(back.origin, pcb.origin);
    let names = |p: &Pcb| {
        p.nets
            .iter()
            .map(|n| n.name.clone())
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(names(&back), names(&pcb));

    // Footprints: the same place, side, rotation, pads (with their nets),
    // graphics, texts and models.
    let net_name = |p: &Pcb, id: Option<ew_model::id::Id>| {
        id.and_then(|id| p.nets.get(id)).map(|n| n.name.clone())
    };
    assert_eq!(back.footprints.len(), pcb.footprints.len());
    for fp in &pcb.footprints {
        let other = back
            .footprints
            .iter()
            .find(|f| f.designator == fp.designator)
            .unwrap();
        assert_eq!(other.at, fp.at, "{}", fp.designator);
        assert_eq!(other.rotation, fp.rotation, "{}", fp.designator);
        assert_eq!(other.side, fp.side, "{}", fp.designator);
        assert_eq!(other.value, fp.value);
        assert_eq!(other.attributes, fp.attributes, "{}", fp.designator);
        assert_eq!(other.graphics, fp.graphics, "{}", fp.designator);
        assert_eq!(other.models, fp.models, "{}", fp.designator);
        assert_eq!(other.texts, fp.texts, "{}", fp.designator);
        assert_eq!(other.pads.len(), fp.pads.len());
        for (a, b) in fp.pads.iter().zip(&other.pads) {
            let strip = |pad: &ew_model::pcb::Pad| ew_model::pcb::Pad {
                id: ew_model::id::Id::from_u128(0),
                net: None,
                ..pad.clone()
            };
            assert_eq!(strip(b), strip(a), "{} pad {}", fp.designator, a.number);
            assert_eq!(net_name(&back, b.net), net_name(&pcb, a.net));
        }
    }

    // Board items.
    let key = |p: &Pcb| {
        let mut tracks: Vec<_> = p
            .tracks
            .iter()
            .map(|t| (t.layer, t.width, t.start, t.end, t.mid, net_name(p, t.net)))
            .collect();
        tracks.sort();
        tracks
    };
    assert_eq!(key(&back), key(&pcb));
    let vias = |p: &Pcb| {
        let mut vias: Vec<_> = p
            .vias
            .iter()
            .map(|v| (v.at, v.diameter, v.drill, v.from, v.to, net_name(p, v.net)))
            .collect();
        vias.sort();
        vias
    };
    assert_eq!(vias(&back), vias(&pcb));
    let shapes = |p: &Pcb| {
        let mut shapes: Vec<_> = p
            .shapes
            .iter()
            .map(|s| format!("{:?}", (s.layer, &s.geometry, s.width, s.filled)))
            .collect();
        shapes.sort();
        shapes
    };
    assert_eq!(shapes(&back), shapes(&pcb));
    let zones = |p: &Pcb| {
        let mut zones: Vec<_> = p
            .zones
            .iter()
            .map(|z| {
                format!(
                    "{:?}",
                    (
                        &z.layers,
                        &z.outline,
                        &z.holes,
                        z.priority,
                        z.keepout,
                        net_name(p, z.net)
                    )
                )
            })
            .collect();
        zones.sort();
        zones
    };
    assert_eq!(zones(&back), zones(&pcb));
    let texts = |p: &Pcb| {
        let mut texts: Vec<_> = p
            .texts
            .iter()
            .map(|t| {
                format!(
                    "{:?}",
                    (&t.text, t.layer, t.at, t.rotation, &t.style, t.mirrored)
                )
            })
            .collect();
        texts.sort();
        texts
    };
    assert_eq!(texts(&back), texts(&pcb));
}

/// Local check with KiCad itself: `EW_KICAD_CLI=/path/to/kicad-cli cargo
/// test -p ew-kicad --test export -- --ignored`. KiCad must load the board
/// and its design rules check must run.
#[test]
#[ignore = "needs a local KiCad installation"]
fn kicad_opens_the_board() {
    let Some(cli) = std::env::var_os("EW_KICAD_CLI") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let pcb = imported_board();
    let mut report = ExportReport::default();
    let path = dir.path().join("Blinky.kicad_pcb");
    std::fs::write(&path, board_file(&pcb, &title(), &|_| None, &mut report)).unwrap();
    let out = dir.path().join("drc.json");
    let output = std::process::Command::new(&cli)
        .args(["pcb", "drc", "--format", "json", "--output"])
        .arg(&out)
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let drc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let counts = |key: &str| drc[key].as_array().map_or(0, Vec::len);
    std::io::Write::write_all(
        &mut std::io::stdout(),
        format!(
            "KiCad DRC: {} violations, {} unconnected, {} schematic parity\n",
            counts("violations"),
            counts("unconnected_items"),
            counts("schematic_parity")
        )
        .as_bytes(),
    )
    .unwrap();
}
