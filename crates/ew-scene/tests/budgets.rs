//! Performance budget of the viewer's engine side (ROADMAP section 6): the
//! scene of an L board (about 590,000 primitives) is built in well under the
//! time a user notices when opening a document.
//!
//! Ignored by default because it only makes sense in release builds. CI runs
//! it with `cargo test --release -p ew-scene --test budgets -- --ignored`.

#![allow(clippy::unwrap_used, reason = "test helpers may panic")]

use std::time::{Duration, Instant};

use ew_model::document::DocumentKind;
use ew_scene::{Options, TitleLabels, build};
use ew_synth::{DesignSize, generate};

const SCENE_BUDGET: Duration = Duration::from_millis(500);

#[test]
#[ignore = "performance budget; run in release mode"]
fn large_board_scene_meets_budget() {
    let project = generate(DesignSize::L, ew_synth::DEFAULT_SEED);
    let board = project.documents_of(DocumentKind::Pcb).next().unwrap().id;
    let options = Options {
        resolve: &str::to_owned,
        labels: TitleLabels::default(),
    };
    let mut best = Duration::MAX;
    let mut bytes = 0;
    for _ in 0..3 {
        let start = Instant::now();
        let scene = build(&project, board, &options).unwrap();
        best = best.min(start.elapsed());
        bytes = scene.data.len();
    }
    let report = format!(
        "L board scene: {best:?} (budget {SCENE_BUDGET:?}), {} MB",
        bytes / 1_000_000
    );
    std::io::Write::write_all(&mut std::io::stdout(), format!("{report}\n").as_bytes()).unwrap();
    assert!(best < SCENE_BUDGET, "{report}");
}
