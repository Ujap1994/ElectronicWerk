//! STEP export of the blinky fixture with the synthetic models of the
//! library fixture: STEP boxes for resistors and the SOIC (merged exactly), a
//! GLB mesh for the capacitors (faceted), nothing for the LED and the header
//! (reported as missing).
//!
//! The file is compared with `tests/fixtures/blinky-assembly.step`, which
//! the desktop app's tests read back with Open CASCADE
//! (`apps/desktop/src/engine-host/step-export.test.ts`). After an intended
//! change, write it again with `UPDATE_FIXTURES=1 cargo test -p ew-board3d
//! --test step_export`.

#![allow(clippy::unwrap_used, reason = "test helpers")]

use std::path::PathBuf;

use ew_board3d::board::{self, BoardOptions};
use ew_board3d::glb;
use ew_board3d::place;
use ew_board3d::step::{self, ModelData, Origin, StepOptions, part21};
use ew_model::document::Document;
use ew_model::pcb::Pcb;

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

fn models(path: &str) -> Option<ModelData> {
    let library = fixture("../ew-library/tests/fixtures");
    if path.contains("R_0805") {
        let text =
            std::fs::read_to_string(library.join("kicad/3dmodels/Demo.3dshapes/R_0805.step"))
                .unwrap();
        Some(ModelData::Step(text))
    } else if path.contains("SOIC-8") {
        let text =
            std::fs::read_to_string(library.join("kicad/3dmodels/Demo.3dshapes/SOIC-8.step"))
                .unwrap();
        Some(ModelData::Step(text))
    } else if path.contains("C_0805") {
        let bytes = std::fs::read(library.join("meshes/Demo.3dshapes/R_0805_Red.glb")).unwrap();
        Some(ModelData::Mesh(glb::read(&bytes).unwrap()))
    } else {
        None
    }
}

#[test]
fn the_blinky_assembly_matches_its_reference() {
    let pcb = board();
    let resolve = |text: &str| text.to_owned();
    let built = board::build(&pcb, &resolve, &BoardOptions::default());
    let parts = place::parts(&pcb, &built.levels);
    let options = StepOptions {
        origin: Origin::Drill,
        include_dnp: false,
        name: "blinky.step".into(),
        created: String::new(),
        version: "test".into(),
    };
    let (text, report) = step::export(&pcb, &built, &parts, &mut models, &options);
    assert!(report.exact_body, "the blinky outline and holes are exact");
    assert_eq!(report.exact, 2, "R_0805 and SOIC-8 merged once each");
    assert_eq!(report.faceted, 1, "C_0805 from its mesh");
    assert_eq!(report.placed, 6, "4 resistors/capacitors… placed");
    assert_eq!(report.missing.len(), 2, "{:?}", report.missing);

    // The file is valid Part 21 with one assembly root.
    let file = part21::parse(&text).unwrap();
    let roots = file.roots();
    assert_eq!(roots.len(), 1, "one root: the board assembly");
    let usages = file
        .records
        .iter()
        .filter(|r| r.kind() == "NEXT_ASSEMBLY_USAGE_OCCURRENCE")
        .count();
    // Body + 6 parts + the SOIC's own sub-assembly links, if any.
    assert!(usages >= 7, "{usages}");
    // Every reference points to a record.
    let ids = file.by_id();
    for record in &file.records {
        for reference in part21::references(&record.body) {
            assert!(
                ids.contains_key(&reference),
                "#{} → #{reference}",
                record.id
            );
        }
    }

    let reference = fixture("tests/fixtures/blinky-assembly.step");
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        std::fs::write(&reference, &text).unwrap();
    }
    let expected = std::fs::read_to_string(&reference).unwrap_or_default();
    assert!(
        expected == text,
        "the STEP output changed; check it and run with UPDATE_FIXTURES=1"
    );
}

/// With the models of a local KiCad installation (`EW_KICAD_3DMODELS`):
/// writes `EW_STEP_OUT` for a comparison with `kicad-cli pcb export step`.
#[test]
#[ignore = "needs a KiCad installation"]
fn the_blinky_assembly_with_kicad_models() {
    let Some(dir) = std::env::var_os("EW_KICAD_3DMODELS") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let pcb = board();
    let resolve = |text: &str| text.to_owned();
    let built = board::build(&pcb, &resolve, &BoardOptions::default());
    let parts = place::parts(&pcb, &built.levels);
    let mut kicad = |path: &str| -> Option<ModelData> {
        let rest = path.split_once('}').map_or(path, |(_, rest)| rest);
        let text = std::fs::read_to_string(dir.join(rest.trim_start_matches('/'))).ok()?;
        Some(ModelData::Step(text))
    };
    let options = StepOptions {
        origin: Origin::Absolute,
        ..StepOptions::default()
    };
    let (text, report) = step::export(&pcb, &built, &parts, &mut kicad, &options);
    assert_eq!(report.placed, 8, "{report:?}");
    assert!(report.missing.is_empty(), "{report:?}");
    if let Some(out) = std::env::var_os("EW_STEP_OUT") {
        std::fs::write(out, text).unwrap();
    }
}

#[test]
fn glb_and_vrml_place_the_meshes() {
    use ew_board3d::export::{self, ExportOptions, Format};
    let pcb = board();
    let resolve = |text: &str| text.to_owned();
    let mut meshes = |path: &str| -> Option<ModelData> {
        let bytes = std::fs::read(fixture(
            "../ew-library/tests/fixtures/meshes/Demo.3dshapes/R_0805_Red.glb",
        ))
        .ok()?;
        path.contains("0805")
            .then(|| ModelData::Mesh(glb::read(&bytes).unwrap()))
    };
    let options = ExportOptions {
        format: Format::Glb,
        ..ExportOptions::default()
    };
    let (bytes, report) = export::export(&pcb, &resolve, &mut meshes, &options);
    // R1–R3, C1, C2 and D1 use 0805 models.
    assert_eq!(report.placed, 6, "{report:?}");
    let parts = glb::read(&bytes).unwrap();
    // The board's meshes plus one primitive per model group.
    assert!(parts.len() >= 5, "{}", parts.len());
    let options = ExportOptions {
        format: Format::Vrml,
        ..ExportOptions::default()
    };
    let (bytes, report) = export::export(&pcb, &resolve, &mut meshes, &options);
    assert_eq!(report.placed, 6);
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with("#VRML V2.0 utf8"));
    assert!(text.contains("DEF R1 Transform"));
    assert_eq!(text.matches("USE M").count(), 3, "six parts, three models");
}
