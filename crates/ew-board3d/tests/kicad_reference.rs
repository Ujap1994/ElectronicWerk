//! The 3D board of the blinky fixture: the body, layers and barrels are
//! checked against the board's own numbers, and the placement of every
//! part against KiCad 10 (`tests/fixtures/blinky-kicad-3d.json`, written by
//! `scripts/make-kicad-3d-reference.py`).

#![allow(clippy::unwrap_used, clippy::print_stderr, reason = "test helpers")]

use std::path::PathBuf;

use ew_board3d::board::{self, BoardOptions, LayerKind, area_mm2};
use ew_board3d::mesh::Mesh;
use ew_board3d::place;
use ew_model::document::Document;
use ew_model::layer::Side;
use ew_model::pcb::Pcb;
use serde_json::Value;

/// KiCad 10 lifts parts by an extra "pad thickness" of 0.05 mm (solder)
/// even with `--no-extra-pad-thickness`; ElectronicWerk puts them on the
/// copper.
const KICAD_LIFT: f64 = 0.05;

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

/// The volume enclosed by a mesh (divergence theorem); only correct if
/// the mesh is closed and every face points outwards.
fn volume(mesh: &Mesh) -> f64 {
    mesh.indices
        .chunks_exact(3)
        .map(|t| {
            let p = |i: u32| mesh.positions[i as usize].map(f64::from);
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum()
}

#[test]
fn the_body_is_the_drilled_outline() {
    let pcb = board();
    let resolve = |text: &str| text.to_owned();
    let board = board::build(&pcb, &resolve, &BoardOptions::default());
    assert!(board.warnings.is_empty(), "{:?}", board.warnings);
    // The fixture is a 40 × 30 mm board, 4 layers.
    let area = area_mm2(&board.area);
    assert!((area - 1200.0).abs() < 0.01, "{area}");
    assert_eq!(board.levels.copper_layers, 4);
    // KiCad's body: two prepregs, a core and the two inner coppers.
    assert!((board.levels.body_top - 1.5558).abs() < 1e-6);
    let drilled: f64 = board
        .holes
        .iter()
        .map(|h| {
            let r = h.diameter / 2e6;
            let slot = h.end.map_or(0.0, |e| h.start.dist(e) / 1e6);
            std::f64::consts::PI * r * r + 2.0 * r * slot
        })
        .sum();
    let expected = (1200.0 - drilled) * board.levels.body_top;
    let got = volume(&board.body);
    assert!(
        (got - expected).abs() < 0.01 * expected,
        "volume {got} vs {expected}"
    );
    // Every layer exists on both sides; barrels for the plated holes.
    for kind in [LayerKind::Copper, LayerKind::Mask, LayerKind::Silk] {
        for side in [Side::Top, Side::Bottom] {
            let layer = board
                .layers
                .iter()
                .find(|l| l.kind == kind && l.side == Some(side))
                .unwrap_or_else(|| panic!("{kind:?} {side:?}"));
            assert!(!layer.mesh.is_empty(), "{}", layer.key());
        }
    }
    assert!(!board.barrels.is_empty());
    // The mask has openings, so it is smaller than the board.
    let mask = board
        .layers
        .iter()
        .find(|l| l.kind == LayerKind::Mask && l.side == Some(Side::Top))
        .unwrap();
    let bounds = mask.mesh.bounds().unwrap();
    assert!((bounds[2] - board.levels.mask(Side::Top)).abs() < 1e-6);
    assert!(
        bounds[0] >= 100.0 - 1e-3 && bounds[3] <= 140.0 + 1e-3,
        "{bounds:?}"
    );
}

#[test]
fn parts_stand_where_kicad_puts_them() {
    let pcb = board();
    let resolve = |text: &str| text.to_owned();
    let board = board::build(&pcb, &resolve, &BoardOptions::default());
    let parts = place::parts(&pcb, &board.levels);
    let reference: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture("tests/fixtures/blinky-kicad-3d.json")).unwrap(),
    )
    .unwrap();
    let expected = reference["parts"].as_object().unwrap();
    assert_eq!(parts.len(), expected.len());
    // KiCad's 3D frame is y = -y(KiCad); ours keeps the page origin. Both
    // agree up to a shift, measured at the board's lower left corner.
    let bbox = board::area_bbox(&board.area);
    let shift = [
        reference["board"]["min"][0].as_f64().unwrap() - bbox.min.x / 1e6,
        reference["board"]["min"][1].as_f64().unwrap() - bbox.min.y / 1e6,
        0.0,
    ];
    assert!((reference["board"]["max"][2].as_f64().unwrap() - board.levels.body_top).abs() < 1e-6);
    for part in &parts {
        let want = &expected[&part.designator];
        assert_eq!(part.models.len(), 1, "{}", part.designator);
        let m = part.models[0].matrix;
        let lift = match part.side {
            Side::Top => KICAD_LIFT,
            Side::Bottom => -KICAD_LIFT,
        };
        for axis in 0..3 {
            let at = want["at"][axis].as_f64().unwrap();
            let got = m.at(axis, 3) + shift[axis] + if axis == 2 { lift } else { 0.0 };
            assert!(
                (got - at).abs() < 1e-4,
                "{} axis {axis}: {got} vs {at}",
                part.designator
            );
            for column in 0..3 {
                let r = want["rotation"][axis][column].as_f64().unwrap();
                assert!(
                    (m.at(axis, column) - r).abs() < 1e-6,
                    "{} rotation {axis},{column}: {} vs {r}",
                    part.designator,
                    m.at(axis, column)
                );
            }
        }
    }
}
