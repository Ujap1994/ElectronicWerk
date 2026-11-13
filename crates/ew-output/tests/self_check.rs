//! Every kind of copper a board can have — rotated and rounded pads,
//! trapezoids, custom pads, slots, arcs, zone fills with holes, parts on the
//! bottom — written as Gerber and drill files, read back and compared with
//! the exact shapes of the board, pixel by pixel.

#![allow(
    clippy::unwrap_used,
    clippy::print_stderr,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "test helpers"
)]

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, Side};
use ew_model::pcb::{BoardShape, FootprintInstance, Net, Pcb, Stackup, Track, Via, Zone, ZoneFill};
use ew_model::shape::Geometry;
use ew_model::units::{Point, mm, um};
use ew_output::art::{ArtOptions, BoardArt};
use ew_output::drill::{DrillOptions, drill_files};
use ew_output::gerber::{Context, GerberOptions, gerbers};
use ew_output::raster::Bitmap;
use ew_output::read;
use ew_pcb::geom::V;
use ew_pcb::items::Board;
use serde_json::json;

const PIXEL: f64 = 10_000.0;

fn footprint(value: serde_json::Value) -> FootprintInstance {
    serde_json::from_value(value).unwrap()
}

fn test_board() -> Pcb {
    let mut pcb = Pcb::new(Id::random(), "test", Stackup::standard(2, mm(1) + um(600)));
    let gnd = Id::random();
    let sig = Id::random();
    for (id, name) in [(gnd, "GND"), (sig, "SIG,1")] {
        pcb.nets.insert(Net {
            id,
            name: name.into(),
            class: String::new(),
        });
    }
    pcb.shapes.insert(BoardShape {
        id: Id::random(),
        layer: Layer::Outline,
        geometry: Geometry::Rect {
            start: Point::mm(0, 0),
            end: Point::mm(40, 30),
        },
        width: um(100),
        filled: false,
    });
    let n = |id: Id| json!(id);
    pcb.footprints.insert(footprint(json!({
        "id": Id::random(),
        "footprint": "Mixed",
        "designator": "U1",
        "at": [mm(12), mm(15)],
        "rotation": 30,
        "pads": [
            { "id": Id::random(), "number": "1", "net": n(sig), "shape": "rect",
              "at": [mm(-4), mm(3)], "size": [um(1500), um(600)], "layers": ["cu.top", "mask.top", "paste.top"] },
            { "id": Id::random(), "number": "2", "shape": { "roundRect": { "ratio": 250 } },
              "at": [mm(-1), mm(3)], "rotation": 15, "size": [um(1600), um(900)], "layers": ["cu.top", "mask.top"] },
            { "id": Id::random(), "number": "3", "net": n(gnd), "shape": "oval",
              "at": [mm(2), mm(3)], "rotation": 45, "size": [mm(2), mm(1)], "layers": ["cu.top", "mask.top"] },
            { "id": Id::random(), "number": "4", "shape": { "trapezoid": { "delta": [um(400), 0] } },
              "at": [mm(-4), mm(-2)], "size": [mm(2), um(1200)], "layers": ["cu.top"] },
            { "id": Id::random(), "number": "5", "shape": { "custom": { "points": [
                [0, 0], [mm(2), 0], [mm(2), um(600)], [um(600), um(600)], [um(600), mm(2)], [0, mm(2)]
              ] } },
              "at": [mm(0), mm(-3)], "size": [um(600), um(600)], "layers": ["cu.top"] },
            { "id": Id::random(), "number": "6", "shape": "circle",
              "at": [mm(4), mm(-2)], "size": [um(1200), um(1200)], "layers": ["cu.top"] }
        ]
    })));
    pcb.footprints.insert(footprint(json!({
        "id": Id::random(),
        "footprint": "Header",
        "designator": "J1",
        "at": [mm(30), mm(12)],
        "rotation": 90,
        "side": "bottom",
        "pads": [
            { "id": Id::random(), "number": "1", "net": n(gnd), "kind": "throughHole", "shape": "rect",
              "at": [0, 0], "size": [um(1700), um(1700)], "drill": { "diameter": mm(1) },
              "layers": ["cu.*", "mask.*"] },
            { "id": Id::random(), "number": "2", "net": n(sig), "kind": "throughHole", "shape": "oval",
              "at": [mm(3), 0], "size": [mm(3), um(1500)],
              "drill": { "diameter": um(700), "slotLength": um(2000) },
              "layers": ["cu.*", "mask.*"] },
            { "id": Id::random(), "number": "3", "shape": "rect",
              "at": [mm(6), mm(1)], "size": [mm(2), mm(1)], "layers": ["cu.top", "mask.top"] }
        ]
    })));
    pcb.footprints.insert(footprint(json!({
        "id": Id::random(),
        "footprint": "Hole",
        "designator": "H1",
        "at": [mm(35), mm(25)],
        "pads": [
            { "id": Id::random(), "number": "", "kind": "npThroughHole", "shape": "circle",
              "at": [0, 0], "size": [mm(3), mm(3)], "drill": { "diameter": mm(3) },
              "layers": ["cu.*", "mask.*"] }
        ]
    })));
    pcb.tracks.insert(Track {
        id: Id::random(),
        layer: CopperLayer::Top,
        net: Some(sig),
        width: um(300),
        start: Point::mm(4, 4),
        end: Point::mm(10, 4),
        mid: None,
        locked: false,
    });
    pcb.tracks.insert(Track {
        id: Id::random(),
        layer: CopperLayer::Top,
        net: Some(sig),
        width: um(400),
        start: Point::mm(20, 4),
        end: Point::mm(26, 4),
        mid: Some(Point::mm(23, 7)),
        locked: false,
    });
    pcb.tracks.insert(Track {
        id: Id::random(),
        layer: CopperLayer::Bottom,
        net: Some(sig),
        width: um(250),
        start: Point::mm(26, 4),
        end: Point::mm(30, 8),
        mid: Some(Point::new(mm(29), mm(5))),
        locked: false,
    });
    pcb.vias.insert(Via {
        id: Id::random(),
        at: Point::mm(26, 4),
        diameter: um(800),
        drill: um(400),
        net: Some(sig),
        from: CopperLayer::Top,
        to: CopperLayer::Bottom,
        micro: false,
        locked: false,
    });
    let zone = Zone {
        id: Id::random(),
        name: "GND".into(),
        net: Some(gnd),
        layers: vec![CopperLayer::Top, CopperLayer::Bottom],
        outline: vec![
            Point::mm(1, 1),
            Point::mm(39, 1),
            Point::mm(39, 29),
            Point::mm(1, 29),
        ],
        holes: Vec::new(),
        priority: 0,
        keepout: None,
        fill: ZoneFill::default(),
        filled: Vec::new(),
    };
    let zone_id = zone.id;
    pcb.zones.insert(zone);
    let fills = ew_pcb::fill::fill(&pcb, None, None);
    for filled in fills {
        if let Some(z) = pcb.zones.get_mut(filled.zone) {
            z.filled = filled.areas;
        }
    }
    let holes: usize = pcb
        .zones
        .get(zone_id)
        .unwrap()
        .filled
        .iter()
        .map(|a| a.holes.len())
        .sum();
    assert!(holes > 0, "the fill should have holes around other nets");
    pcb
}

fn context() -> Context {
    Context {
        version: "test".into(),
        created: String::new(),
        project: "test".into(),
        guid: "x".into(),
        revision: "1".into(),
        stem: "test".into(),
    }
}

#[test]
fn gerbers_draw_the_exact_copper() {
    let pcb = test_board();
    let resolve = |s: &str| s.to_owned();
    let art = BoardArt::new(&pcb, ArtOptions::default(), &resolve);
    let options = GerberOptions {
        layers: vec![
            Layer::Copper(CopperLayer::Top),
            Layer::Copper(CopperLayer::Bottom),
        ],
        job_file: false,
        ..GerberOptions::default()
    };
    let files = gerbers(&art, &options, &context());
    let board = Board::new(&pcb);
    let bounds = [-1e6, -1e6, 41e6, 31e6];
    for (file, layer) in files.iter().zip([CopperLayer::Top, CopperLayer::Bottom]) {
        let text = String::from_utf8(file.data.clone()).unwrap();
        let image = read::read(&text).unwrap();
        assert!(image.warnings.is_empty(), "{:?}", image.warnings);
        let ours = Bitmap::of(&image, bounds, PIXEL, V::new(0.0, 0.0));
        let bit = board.layer_bit(layer).unwrap();
        let mut exact = Bitmap::new(bounds, PIXEL);
        // A mounting hole without a copper ring has no copper to plot.
        let bare_hole = |item: &&ew_pcb::items::CopperItem| match item.item {
            ew_pcb::items::ItemRef::Pad { footprint, index } => pcb
                .footprints
                .get(footprint)
                .and_then(|fp| fp.pads.get(index))
                .is_some_and(|pad| pad.kind == ew_model::pcb::PadKind::NpThroughHole),
            _ => false,
        };
        for item in board
            .copper
            .iter()
            .filter(|c| c.layers & bit != 0)
            .filter(|c| !bare_hole(c))
        {
            exact.paint(&item.shape, true, V::new(0.0, 0.0));
        }
        let differ = ours.difference(&exact);
        let ratio = differ as f64 / exact.count() as f64;
        eprintln!(
            "{}: {} px, differ {differ} ({:.4} %)",
            file.name,
            exact.count(),
            ratio * 100.0
        );
        if let Ok(dir) = std::env::var("EW_RASTER_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::write(dir.join(format!("{}-gerber.pgm", file.name)), ours.pgm()).unwrap();
            std::fs::write(dir.join(format!("{}-exact.pgm", file.name)), exact.pgm()).unwrap();
        }
        assert!(
            ratio < 0.0005,
            "{}: {:.4} % differ",
            file.name,
            ratio * 100.0
        );
    }
    // Net names with commas are escaped.
    let top = String::from_utf8(files[0].data.clone()).unwrap();
    assert!(top.contains("%TO.N,SIG\\u002C1*%"));
}

#[test]
fn drill_files_hold_every_hole() {
    let pcb = test_board();
    let files = drill_files(&pcb, DrillOptions::default(), "test", "test", "");
    let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["test-PTH.drl", "test-NPTH.drl"]);
    let pth = read::read(&String::from_utf8(files[0].data.clone()).unwrap()).unwrap();
    let npth = read::read(&String::from_utf8(files[1].data.clone()).unwrap()).unwrap();
    // Two plated pad holes (one a slot) and the via; one mounting hole.
    assert_eq!(pth.objects.len(), 3);
    assert_eq!(npth.objects.len(), 1);
    let board = Board::new(&pcb);
    for hole in &board.holes {
        let image = if hole.plated { &pth } else { &npth };
        let center = hole.shape.anchor();
        assert!(
            image.objects.iter().any(|o| o.shape.contains(center)),
            "hole at {center:?}"
        );
    }
    let slot = String::from_utf8(files[0].data.clone()).unwrap();
    assert!(slot.contains("G85"), "{slot}");
}

#[test]
fn bottom_parts_go_to_the_bottom_layers() {
    let pcb = test_board();
    let resolve = |s: &str| s.to_owned();
    let art = BoardArt::new(&pcb, ArtOptions::default(), &resolve);
    // Pad 3 of J1 is an SMD pad on the "top" of a part placed on the bottom.
    let bottom = art.features(Layer::Copper(CopperLayer::Bottom));
    let top = art.features(Layer::Copper(CopperLayer::Top));
    let pin = |features: &[ew_output::art::Feature], number: &str| {
        features.iter().any(|f| {
            f.attributes()
                .pin
                .as_ref()
                .is_some_and(|(d, n)| d == "J1" && n == number)
        })
    };
    assert!(pin(&bottom, "3") && !pin(&top, "3"));
    assert!(pin(&bottom, "1") && pin(&top, "1"));
    let mask = art.features(Layer::SolderMask(Side::Bottom));
    assert_eq!(mask.len(), 4, "J1 pads 1–3 and the mounting hole");
}
