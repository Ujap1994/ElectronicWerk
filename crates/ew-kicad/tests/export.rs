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

#[test]
fn blind_buried_and_micro_vias_keep_their_layers() {
    use ew_model::layer::CopperLayer;
    use ew_model::pcb::{Via, ViaType};
    use ew_model::units::{Point, um};

    let mut pcb = imported_board();
    assert_eq!(pcb.stackup.copper_count(), 4);
    let spans = [
        (CopperLayer::Top, CopperLayer::Inner(1), false),
        (CopperLayer::Inner(1), CopperLayer::Inner(2), false),
        (CopperLayer::Inner(2), CopperLayer::Bottom, true),
    ];
    for (index, &(from, to, micro)) in spans.iter().enumerate() {
        pcb.vias.insert(Via {
            id: ew_model::id::Id::random(),
            at: Point::mm(200 + i64::try_from(index).unwrap(), 200),
            diameter: um(300),
            drill: um(100),
            net: None,
            from,
            to,
            micro,
            fill: None,
            locked: false,
        });
    }
    let (back, text, _) = round_trip(&pcb);
    assert_eq!(text.matches("(via blind").count(), 2);
    assert_eq!(text.matches("(via micro").count(), 1);
    let kinds: BTreeSet<(i64, ViaType, String, String)> = back
        .vias
        .iter()
        .filter(|v| v.via_type() != ViaType::Through)
        .map(|v| (v.at.x, v.via_type(), v.from.to_string(), v.to.to_string()))
        .collect();
    let expected: BTreeSet<(i64, ViaType, String, String)> = pcb
        .vias
        .iter()
        .filter(|v| v.via_type() != ViaType::Through)
        .map(|v| (v.at.x, v.via_type(), v.from.to_string(), v.to.to_string()))
        .collect();
    assert_eq!(kinds, expected);
    assert_eq!(kinds.len(), 3);
}

#[test]
fn dimensions_survive_the_round_trip() {
    use ew_model::dimension::{DimensionKind, new_dimension};
    use ew_model::id::Id;
    use ew_model::layer::Layer;
    use ew_model::units::{Point, mm};
    let mut pcb = imported_board();
    // The fixture has a dimension of its own: only ours here.
    pcb.dimensions = ew_model::collection::Collection::new();
    let made = [
        (
            DimensionKind::Horizontal,
            Point::mm(0, 0),
            Point::mm(30, 4),
            mm(5),
            "",
        ),
        (
            DimensionKind::Vertical,
            Point::mm(0, 0),
            Point::mm(6, 20),
            mm(-4),
            "{value} max",
        ),
        (
            DimensionKind::Aligned,
            Point::mm(0, 0),
            Point::mm(10, 10),
            mm(3),
            "",
        ),
        (
            DimensionKind::Radial,
            Point::mm(40, 40),
            Point::mm(43, 40),
            mm(2),
            "",
        ),
        (
            DimensionKind::Leader,
            Point::mm(50, 50),
            Point::mm(55, 55),
            0,
            "Gold fingers",
        ),
    ];
    for (kind, start, end, offset, text) in made {
        let mut dim = new_dimension(Id::random(), Layer::User(1), kind, start, end, offset);
        dim.text = text.to_owned();
        pcb.dimensions.insert(dim);
    }
    let (back, file, _report) = round_trip(&pcb);
    assert!(file.contains("(dimension"));
    for (kind, start, end, offset, text) in made {
        let found = back
            .dimensions
            .iter()
            .find(|d| d.kind == kind)
            .unwrap_or_else(|| panic!("{kind:?} is missing"));
        // Positions move with the page; the size and the side stay.
        assert_eq!(found.end - found.start, end - start, "{kind:?}");
        if kind != DimensionKind::Leader {
            assert_eq!(found.offset, offset, "{kind:?}");
        }
        assert_eq!(found.text, text, "{kind:?}");
    }
}

#[test]
fn padstacks_and_via_fills_survive_the_round_trip() {
    use ew_model::layer::Side;
    use ew_model::pcb::{PadLayerShape, PadShape, PadStackLayers, ViaFill};
    use ew_model::units::{Size, um};
    let mut pcb = imported_board();
    let stack = vec![
        PadLayerShape {
            layers: PadStackLayers::Inner,
            shape: PadShape::Circle,
            size: Size::new(um(700), um(700)),
        },
        PadLayerShape {
            layers: PadStackLayers::Back,
            shape: PadShape::RoundRect { ratio: 250 },
            size: Size::new(um(1_100), um(900)),
        },
    ];
    let mut changed = Vec::new();
    for side in [Side::Top, Side::Bottom] {
        let id = pcb
            .footprints
            .iter()
            .find(|f| f.side == side)
            .map(|f| f.id)
            .unwrap();
        let fp = pcb.footprints.get_mut(id).unwrap();
        fp.pads[0].stack.clone_from(&stack);
        changed.push((id, fp.pads[0].clone()));
    }
    let fills = [
        ViaFill::Tented,
        ViaFill::TentedCovered,
        ViaFill::Plugged,
        ViaFill::PluggedCovered,
        ViaFill::Filled,
        ViaFill::FilledCovered,
        ViaFill::FilledCapped,
    ];
    let first = pcb.vias.iter().next().cloned().unwrap();
    let mut vias = Vec::new();
    for index in 0..fills.len() {
        let mut via = first.clone();
        via.id = ew_model::id::Id::random();
        via.at = ew_model::units::Point::mm(150 + i64::try_from(index).unwrap(), 150);
        vias.push(via.id);
        pcb.vias.insert(via);
    }
    for (id, fill) in vias.iter().zip(fills) {
        pcb.vias.get_mut(*id).unwrap().fill = Some(fill);
    }
    let (back, text, _) = round_trip(&pcb);
    assert_eq!(text.matches("(padstack").count(), 2);
    if let Some(path) = std::env::var_os("EW_PADSTACK_OUT") {
        std::fs::write(path, &text).unwrap();
    }
    for (id, pad) in changed {
        let fp = back.footprints.get(id).unwrap();
        let again = &fp.pads[0];
        assert_eq!(
            (&again.shape, again.size),
            (&pad.shape, pad.size),
            "{:?}",
            fp.side
        );
        assert_eq!(again.stack, pad.stack, "{:?}", fp.side);
    }
    for (id, fill) in vias.iter().zip(fills) {
        assert_eq!(back.vias.get(*id).unwrap().fill, Some(fill));
    }
}
