//! Performance budgets (ROADMAP section 6): opening an L design takes less
//! than 3 s and saving it less than 1 s.
//!
//! These tests are ignored by default because they only make sense in
//! release builds. CI runs them with
//! `cargo test --release -p ew-format --test budgets -- --ignored`.

#![allow(clippy::unwrap_used, reason = "test helpers may panic")]

use std::time::{Duration, Instant};

use ew_format::store::{self, SavePlan};
use ew_synth::{DesignSize, generate};

const OPEN_BUDGET: Duration = Duration::from_secs(3);
const SAVE_BUDGET: Duration = Duration::from_secs(1);

fn measure(size: DesignSize) -> (Duration, Duration, u64) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("design");
    let project = generate(size, ew_synth::DEFAULT_SEED);
    let created = store::create_project(&root, &project).unwrap();

    // Best of three runs, to filter out noise on shared CI machines.
    let mut save = Duration::MAX;
    for _ in 0..3 {
        let start = Instant::now();
        store::save_project(
            &root,
            &created.manifest_file,
            &project,
            &SavePlan::everything(&project),
        )
        .unwrap();
        save = save.min(start.elapsed());
    }
    let mut open = Duration::MAX;
    for _ in 0..3 {
        let start = Instant::now();
        let loaded = store::load_project(&root).unwrap();
        open = open.min(start.elapsed());
        assert_eq!(loaded.project.manifest.id, project.manifest.id);
    }
    let bytes = std::fs::metadata(root.join("pcb/board.ewpcb"))
        .unwrap()
        .len();
    (open, save, bytes)
}

#[test]
#[ignore = "performance budget; run in release mode"]
fn large_design_meets_open_and_save_budgets() {
    let (open, save, bytes) = measure(DesignSize::L);
    let report = format!(
        "L design: open {open:?} (budget {OPEN_BUDGET:?}), save {save:?} (budget {SAVE_BUDGET:?}), board file {} MB",
        bytes / 1_000_000
    );
    std::io::Write::write_all(&mut std::io::stdout(), format!("{report}\n").as_bytes()).unwrap();
    assert!(open < OPEN_BUDGET, "{report}");
    assert!(save < SAVE_BUDGET, "{report}");
}
