//! Performance budgets of the 3D board (ROADMAP M9): building the 3D board
//! of an M board (500 parts, 20,000 track segments) stays below 1 s and of
//! an L board (5,000 parts, 200,000 track segments) below 5 s (about 60 ms
//! and 0.7 s on a current desktop), and placing the parts of an L board
//! after a move stays below 50 ms, well inside the 250 ms of the live
//! update.
//!
//! Ignored by default because it only makes sense in release builds. CI runs
//! it with `cargo test --release -p ew-board3d --test budgets -- --ignored`.

#![allow(clippy::unwrap_used, reason = "test helpers may panic")]

use std::time::{Duration, Instant};

use ew_board3d::board::{self, BoardOptions};
use ew_board3d::place;
use ew_board3d::stack::Levels;
use ew_model::document::Document;
use ew_model::pcb::Pcb;
use ew_synth::{DesignSize, generate};

const BUILD_BUDGET: Duration = Duration::from_millis(1_000);
const LARGE_BUDGET: Duration = Duration::from_millis(5_000);
const PLACE_BUDGET: Duration = Duration::from_millis(50);

fn board(size: DesignSize) -> Pcb {
    generate(size, ew_synth::DEFAULT_SEED)
        .documents
        .into_values()
        .find_map(|doc| match doc {
            Document::Pcb(pcb) => Some(pcb),
            _ => None,
        })
        .unwrap()
}

fn say(text: &str) {
    std::io::Write::write_all(&mut std::io::stdout(), format!("{text}\n").as_bytes()).unwrap();
}

#[test]
#[ignore = "performance budget; run in release mode"]
fn the_3d_board_meets_its_budgets() {
    let resolve = |text: &str| text.to_owned();
    let medium = board(DesignSize::M);
    let mut best = Duration::MAX;
    let mut triangles = 0;
    for _ in 0..2 {
        let start = Instant::now();
        let built = board::build(&medium, &resolve, &BoardOptions::default());
        best = best.min(start.elapsed());
        triangles = built.body.triangles()
            + built.barrels.triangles()
            + built
                .layers
                .iter()
                .map(|l| l.mesh.triangles())
                .sum::<usize>();
    }
    say(&format!(
        "M board 3D build: {best:?} (budget {BUILD_BUDGET:?}), {triangles} triangles"
    ));

    let large = board(DesignSize::L);
    let start = Instant::now();
    let built = board::build(&large, &resolve, &BoardOptions::default());
    let large_time = start.elapsed();
    say(&format!(
        "L board 3D build: {large_time:?} (budget {LARGE_BUDGET:?}), {} layer meshes",
        built.layers.len()
    ));
    let levels = Levels::of(&large.stackup);
    let mut place_best = Duration::MAX;
    for _ in 0..3 {
        let start = Instant::now();
        let parts = place::parts(&large, &levels);
        place_best = place_best.min(start.elapsed());
        assert_eq!(parts.len(), large.footprints.len());
    }
    say(&format!(
        "L board placements: {place_best:?} (budget {PLACE_BUDGET:?})"
    ));
    assert!(best < BUILD_BUDGET, "3D build took {best:?}");
    assert!(large_time < LARGE_BUDGET, "L 3D build took {large_time:?}");
    assert!(place_best < PLACE_BUDGET, "placements took {place_best:?}");
}
