//! Performance budgets of the board checks (ROADMAP M7): the full design
//! rules check of an L board (5,000 parts, 200,000 track segments) stays
//! below 5 s, half the 10 s of the requirement (about 1.6 s on a current
//! desktop, 2–3.5 s on shared CI runners); the online check after a local
//! edit of an M board (500 parts, 20,000 track segments) stays below 100 ms.
//!
//! Ignored by default because it only makes sense in release builds. CI runs
//! it with `cargo test --release -p ew-pcb --test budgets -- --ignored`.

#![allow(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    reason = "test helpers may panic; board coordinates fit f64"
)]

use std::time::{Duration, Instant};

use ew_model::document::Document;
use ew_model::pcb::Pcb;
use ew_pcb::drc::{Options, check, merge};
use ew_pcb::geom::{BBox, V};
use ew_synth::{DesignSize, generate};

const FULL_BUDGET: Duration = Duration::from_millis(5_000);
const ONLINE_BUDGET: Duration = Duration::from_millis(100);

fn board(size: DesignSize) -> Pcb {
    let project = generate(size, ew_synth::DEFAULT_SEED);
    project
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
fn large_board_drc_meets_its_budget() {
    let pcb = board(DesignSize::L);
    // A warm-up run, then the best of two: shared CI runners vary by more
    // than a second between runs of the same code (2.1 s to 3.5 s seen).
    let mut full_time = Duration::MAX;
    let mut full = check(&pcb, None, &Options::default());
    for _ in 0..2 {
        let start = Instant::now();
        full = check(&pcb, None, &Options::default());
        full_time = full_time.min(start.elapsed());
    }
    say(&format!(
        "L board DRC: {full_time:?} (budget {FULL_BUDGET:?}), {} findings, {} unrouted",
        full.findings.len(),
        full.counts.unrouted
    ));
    assert!(full_time < FULL_BUDGET);
}

#[test]
#[ignore = "performance budget; run in release mode"]
fn online_drc_meets_its_budget() {
    let best = online_drc(DesignSize::M);
    say(&format!(
        "M board online DRC: {best:?} (budget {ONLINE_BUDGET:?})"
    ));
    assert!(best < ONLINE_BUDGET, "online DRC took {best:?}");
}

#[test]
#[ignore = "budget test: run in release mode (see CI)"]
fn online_drc_of_the_large_board_meets_its_budget() {
    // The goal is 100 ms (ROADMAP); today the connectivity of the whole board
    // (about 135 ms on an Apple M3) is analysed again for every check. This
    // guard keeps it from getting slower until the analysis is incremental.
    const LARGE_ONLINE_BUDGET: Duration = Duration::from_secs(1);
    let best = online_drc(DesignSize::L);
    say(&format!(
        "L board online DRC: {best:?} (guard {LARGE_ONLINE_BUDGET:?}, goal {ONLINE_BUDGET:?})"
    ));
    assert!(best < LARGE_ONLINE_BUDGET, "online DRC took {best:?}");
}

/// The best of three checks of a 5 mm square in the middle of the board,
/// merged into the full result.
fn online_drc(size: DesignSize) -> Duration {
    let pcb = board(size);
    let full = check(&pcb, None, &Options::default());
    // An edit near the middle of the board: a 5 mm square.
    let bounds = pcb.outline_bounds().unwrap();
    let center = V::new(
        (bounds.min.x / 2 + bounds.max.x / 2) as f64,
        (bounds.min.y / 2 + bounds.max.y / 2) as f64,
    );
    let region = BBox::of(center).grow(2.5e6);
    let mut best = Duration::MAX;
    for _ in 0..3 {
        let start = Instant::now();
        let fresh = check(
            &pcb,
            None,
            &Options {
                region: Some(region),
                ..Options::default()
            },
        );
        let merged = merge(&full, fresh, region, 200_000.0);
        best = best.min(start.elapsed());
        assert_eq!(merged.findings.len(), full.findings.len());
    }
    best
}
