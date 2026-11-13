//! Our Gerber and drill files of the blinky fixture compared with the files
//! KiCad 10 writes for the same board (`tests/fixtures/blinky-kicad`): both
//! are read back, rasterized at 20 µm and compared pixel by pixel.

#![allow(
    clippy::unwrap_used,
    clippy::print_stderr,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "test helpers"
)]

use std::path::PathBuf;

use ew_model::document::Document;
use ew_model::layer::{CopperLayer, Layer, Side};
use ew_model::pcb::Pcb;
use ew_output::art::{ArtOptions, BoardArt};
use ew_output::drill::{DrillOptions, drill_files};
use ew_output::gerber::{Context, GerberOptions, gerbers};
use ew_output::raster::Bitmap;
use ew_output::read::{self, Image};
use ew_pcb::geom::V;

const PIXEL: f64 = 20_000.0;

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn board() -> Pcb {
    let imported = ew_kicad::import_project(
        &fixture("../ew-kicad/tests/fixtures/blinky/blinky.kicad_pro"),
        None,
    )
    .unwrap();
    imported
        .project
        .documents
        .values()
        .find_map(|d| match d {
            Document::Pcb(pcb) => Some(pcb.clone()),
            _ => None,
        })
        .unwrap()
}

fn context() -> Context {
    Context {
        version: "test".into(),
        created: String::new(),
        project: "blinky".into(),
        guid: "00000000-0000-0000-0000-000000000000".into(),
        revision: "A".into(),
        stem: "blinky".into(),
    }
}

fn reference(name: &str) -> Image {
    let text =
        std::fs::read_to_string(fixture(&format!("tests/fixtures/blinky-kicad/{name}"))).unwrap();
    read::read(&text).unwrap()
}

struct Ours {
    files: Vec<(String, Image)>,
}

impl Ours {
    fn new(pcb: &Pcb) -> Self {
        let resolve = |s: &str| s.to_owned();
        let art = BoardArt::new(pcb, ArtOptions::default(), &resolve);
        let options = GerberOptions {
            layers: vec![
                Layer::Copper(CopperLayer::Top),
                Layer::Copper(CopperLayer::Inner(1)),
                Layer::Copper(CopperLayer::Inner(2)),
                Layer::Copper(CopperLayer::Bottom),
                Layer::SolderMask(Side::Top),
                Layer::SolderMask(Side::Bottom),
                Layer::Paste(Side::Top),
                Layer::Paste(Side::Bottom),
                Layer::Silkscreen(Side::Top),
                Layer::Silkscreen(Side::Bottom),
                Layer::Outline,
            ],
            ..GerberOptions::default()
        };
        let mut files: Vec<(String, Image)> = gerbers(&art, &options, &context())
            .into_iter()
            .filter(|f| !f.name.ends_with(".gbrjob"))
            .map(|f| {
                let text = String::from_utf8(f.data).unwrap();
                let image = read::read(&text).unwrap();
                assert!(
                    image.warnings.is_empty(),
                    "{}: {:?}",
                    f.name,
                    image.warnings
                );
                (f.name, image)
            })
            .collect();
        for f in drill_files(pcb, DrillOptions { merge: true }, "blinky", "test", "") {
            let text = String::from_utf8(f.data).unwrap();
            files.push((f.name, read::read(&text).unwrap()));
        }
        Self { files }
    }

    fn get(&self, suffix: &str) -> &Image {
        &self
            .files
            .iter()
            .find(|(name, _)| name.ends_with(suffix))
            .unwrap_or_else(|| panic!("no file {suffix}"))
            .1
    }
}

/// The offset that moves the KiCad image onto ours (KiCad's y axis points
/// down from the page corner, ours up), from the board outlines.
fn alignment(ours: &Image, kicad: &Image) -> V {
    let (a, b) = (ours.bounds().unwrap(), kicad.bounds().unwrap());
    V::new(a[0] - b[0], a[1] - b[1])
}

/// Pixels that differ, relative to the pixels of the reference.
fn compare(ours: &Image, kicad: &Image, offset: V, name: &str) -> f64 {
    let b = ours.bounds().or_else(|| kicad.bounds()).unwrap();
    let k = kicad.bounds().unwrap_or(b);
    let bounds = [
        b[0].min(k[0] + offset.x) - 1e6,
        b[1].min(k[1] + offset.y) - 1e6,
        b[2].max(k[2] + offset.x) + 1e6,
        b[3].max(k[3] + offset.y) + 1e6,
    ];
    let mine = Bitmap::of(ours, bounds, PIXEL, V::new(0.0, 0.0));
    let theirs = Bitmap::of(kicad, bounds, PIXEL, offset);
    let differ = mine.difference(&theirs);
    let ratio = differ as f64 / theirs.count().max(1) as f64;
    eprintln!(
        "{name}: ours {} px, KiCad {} px, differ {differ} px ({:.3} %)",
        mine.count(),
        theirs.count(),
        ratio * 100.0
    );
    if let Ok(dir) = std::env::var("EW_RASTER_DIR") {
        let dir = PathBuf::from(dir);
        std::fs::write(dir.join(format!("{name}-ours.pgm")), mine.pgm()).unwrap();
        std::fs::write(dir.join(format!("{name}-kicad.pgm")), theirs.pgm()).unwrap();
    }
    ratio
}

#[test]
fn gerbers_match_kicad() {
    let pcb = board();
    let ours = Ours::new(&pcb);
    let offset = alignment(ours.get("-Outline.gm1"), &reference("Edge_Cuts.gm1"));
    // Copper, mask, paste and outline match to the pixel apart from curve
    // approximation along edges.
    for (suffix, kicad, limit) in [
        ("-Copper_Top.gtl", "F_Cu.gtl", 0.01),
        ("-Copper_In1.g1", "In1_Cu.g1", 0.01),
        ("-Copper_In2.g2", "In2_Cu.g2", 0.01),
        ("-Copper_Bottom.gbl", "B_Cu.gbl", 0.01),
        ("-Soldermask_Top.gts", "F_Mask.gts", 0.01),
        ("-Soldermask_Bottom.gbs", "B_Mask.gbs", 0.01),
        ("-Paste_Top.gtp", "F_Paste.gtp", 0.01),
        ("-Paste_Bottom.gbp", "B_Paste.gbp", 0.01),
        ("-Outline.gm1", "Edge_Cuts.gm1", 0.02),
        ("blinky.drl", "blinky.drl", 0.01),
    ] {
        let ratio = compare(ours.get(suffix), &reference(kicad), offset, kicad);
        assert!(ratio <= limit, "{kicad}: {:.3} % differ", ratio * 100.0);
    }
    // Silkscreen texts use another font: only the graphics and the place of
    // the texts can match.
    for (suffix, kicad) in [
        ("-Silkscreen_Top.gto", "F_Silkscreen.gto"),
        ("-Silkscreen_Bottom.gbo", "B_Silkscreen.gbo"),
    ] {
        let ratio = compare(ours.get(suffix), &reference(kicad), offset, kicad);
        assert!(ratio <= 0.8, "{kicad}: {:.3} % differ", ratio * 100.0);
    }
}

#[test]
fn net_and_pad_attributes() {
    let pcb = board();
    let resolve = |s: &str| s.to_owned();
    let art = BoardArt::new(&pcb, ArtOptions::default(), &resolve);
    let options = GerberOptions {
        layers: vec![Layer::Copper(CopperLayer::Top)],
        ..GerberOptions::default()
    };
    let files = gerbers(&art, &options, &context());
    let top = String::from_utf8(files[0].data.clone()).unwrap();
    for expected in [
        "%TF.FileFunction,Copper,L1,Top*%",
        "%TO.P,U1,1*%",
        "%TO.N,GND*%",
        "%TO.N,/TRIG*%",
        "%TA.AperFunction,SMDPad,CuDef*%",
        "%TA.AperFunction,ComponentPad*%",
        "%TA.AperFunction,ViaPad*%",
        "%TA.AperFunction,Conductor*%",
    ] {
        assert!(top.contains(expected), "missing {expected}");
    }
    let job = files.iter().find(|f| f.name.ends_with(".gbrjob")).unwrap();
    let job: serde_json::Value = serde_json::from_slice(&job.data).unwrap();
    assert_eq!(job["GeneralSpecs"]["LayerNumber"], 4);
    assert_eq!(job["FilesAttributes"][0]["FileFunction"], "Copper,L1,Top");
}

#[test]
fn ipc356_matches_kicad() {
    let pcb = board();
    let text = ew_output::ipc356::netlist(&pcb, true, "test");
    assert!(text.ends_with("999\n"));
    let ours = text;
    let kicad =
        std::fs::read_to_string(fixture("tests/fixtures/blinky-kicad/blinky.d356")).unwrap();
    let records = |text: &str| -> Vec<String> {
        let mut lines: Vec<String> = text
            .lines()
            .filter(|l| l.starts_with('3'))
            .map(|l| l.trim_end().to_owned())
            .collect();
        lines.sort();
        lines
    };
    // The fixture's via at (137, 108) is stored on GND but overlaps a pad
    // of /LED_A (one of the fixture's deliberate shorts); KiCad moves it to
    // the pad's net when it loads the board, we keep the stored net.
    let short = |line: &String| line.contains("VIA") && line.contains("X+014567");
    let (mut ours, mut kicad) = (records(&ours), records(&kicad));
    assert!(ours.iter().any(|l| short(l) && l.starts_with("317GND")));
    ours.retain(|l| !short(l));
    kicad.retain(|l| !short(l));
    assert_eq!(ours, kicad);
}

/// Checks that XML is well formed and returns the element names in order.
fn elements(xml: &str) -> Vec<String> {
    let mut stack: Vec<String> = Vec::new();
    let mut names = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        let end = rest[start..].find('>').unwrap() + start;
        let tag = &rest[start + 1..end];
        rest = &rest[end + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        // Quotes in attributes are balanced.
        assert_eq!(tag.matches('"').count() % 2, 0, "{tag}");
        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(stack.pop().as_deref(), Some(name), "closing {name}");
        } else {
            let name = tag
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap()
                .to_owned();
            names.push(name.clone());
            if !tag.ends_with('/') {
                stack.push(name);
            }
        }
    }
    assert!(stack.is_empty(), "unclosed {stack:?}");
    names
}

#[test]
fn ipc2581_is_complete() {
    let pcb = board();
    let resolve = |s: &str| s.to_owned();
    let art = BoardArt::new(&pcb, ArtOptions::default(), &resolve);
    let xml = ew_output::ipc2581::write(
        &art,
        &ew_output::ipc2581::Context {
            version: "test".into(),
            created: "2026-11-13T08:00:00".into(),
        },
    );
    let names = elements(&xml);
    let count = |name: &str| names.iter().filter(|n| *n == name).count();
    assert_eq!(count("IPC-2581"), 1);
    assert_eq!(count("Component"), pcb.footprints.len());
    assert_eq!(count("RefDes"), pcb.footprints.len());
    let mut packages: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for fp in &pcb.footprints {
        packages.insert(fp.footprint.as_str(), fp.pads.len());
    }
    assert_eq!(count("Package"), packages.len(), "one per footprint name");
    assert_eq!(count("Pin"), packages.values().sum::<usize>());
    let pads: usize = pcb.footprints.iter().map(|f| f.pads.len()).sum();
    assert!(count("PinRef") >= pads, "every pad in a net and on copper");
    assert_eq!(count("Hole"), 4, "two vias and two header holes");
    assert!(count("Contour") >= 1, "the GND plane");
    assert!(xml.contains("<LogicalNet name=\"GND\">"));
    assert!(xml.contains("<Layer name=\"DRILL_1_4\" layerFunction=\"DRILL\""));
    if let Ok(dir) = std::env::var("EW_RASTER_DIR") {
        std::fs::write(PathBuf::from(dir).join("blinky.ipc2581.xml"), &xml).unwrap();
    }
}

#[test]
fn prints_of_the_blinky_project() {
    use ew_output::print::{self, AssemblyPrint, BoardPrint, SchematicPrint};
    let imported = ew_kicad::import_project(
        &fixture("../ew-kicad/tests/fixtures/blinky/blinky.kicad_pro"),
        None,
    )
    .unwrap();
    let project = imported.project;
    let options = ew_scene::Options {
        resolve: &|s: &str| s.to_owned(),
        labels: ew_scene::TitleLabels::default(),
    };
    let sheets = print::schematic_pages(&project, &options, SchematicPrint::default());
    assert_eq!(sheets.len(), 2, "root sheet and power sheet");
    assert!(sheets[0].items.len() > 100);
    assert!(sheets[0].labels.iter().any(|l| l.text == "U1"));
    let pcb = board();
    let resolve = |s: &str| s.to_owned();
    let art = BoardArt::new(&pcb, ArtOptions::default(), &resolve);
    let boards = print::board_pages(&art, &BoardPrint::default(), "blinky");
    assert_eq!(boards.len(), 1);
    let assembly = print::assembly_pages(&art, &AssemblyPrint::default(), "blinky");
    assert_eq!(assembly.len(), 2, "C2 is on the bottom");
    assert!(assembly[1].labels.iter().any(|l| l.text == "C2"));
    if let Ok(dir) = std::env::var("EW_RASTER_DIR") {
        let dir = PathBuf::from(dir);
        for (name, pages) in [
            ("schematic", sheets),
            ("board", boards),
            ("assembly", assembly),
        ] {
            for (i, page) in pages.iter().enumerate() {
                std::fs::write(
                    dir.join(format!("{name}-{i}.svg")),
                    ew_output::svg::write(page, name),
                )
                .unwrap();
            }
            let doc = print::document(name, pages, "test", "");
            std::fs::write(dir.join(format!("{name}.pdf")), ew_output::pdf::write(&doc)).unwrap();
        }
    }
}
