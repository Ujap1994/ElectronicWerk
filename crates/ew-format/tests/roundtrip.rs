//! Save → load round trips on synthetic designs: the loaded project equals
//! the saved one, and saving it again produces byte-identical files.

#![allow(clippy::unwrap_used, reason = "test helpers may panic")]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ew_format::store::{self, SavePlan};
use ew_synth::{DesignSize, generate};
use proptest::prelude::*;

fn read_all(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                files.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    files
}

fn round_trip(size: DesignSize, seed: u64) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("design");
    let project = generate(size, seed);
    let created = store::create_project(&root, &project).unwrap();
    let first = read_all(&root);

    let loaded = store::load_project(&root).unwrap();
    assert_eq!(loaded.project, project);
    assert_eq!(loaded.hashes, created.hashes);

    store::save_project(
        &root,
        &loaded.manifest_file,
        &loaded.project,
        &SavePlan::everything(&loaded.project),
    )
    .unwrap();
    assert_eq!(read_all(&root), first, "saving again changed bytes");
    assert!(ew_format::validate::validate_project(&loaded.project).is_valid());
}

#[test]
fn medium_design_round_trips() {
    round_trip(DesignSize::M, ew_synth::DEFAULT_SEED);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn small_designs_round_trip(seed in any::<u64>()) {
        round_trip(DesignSize::S, seed);
    }
}

#[test]
fn files_are_diff_friendly() {
    let dir = tempfile::tempdir().unwrap();
    let project = generate(DesignSize::S, 3);
    store::create_project(dir.path(), &project).unwrap();
    let board = fs::read_to_string(dir.path().join("pcb/board.ewpcb")).unwrap();
    // One track per line.
    let track_lines = board
        .lines()
        .filter(|line| line.contains("\"width\": 250000"))
        .count();
    assert!(track_lines > 100, "{track_lines}");
    assert!(board.lines().all(|line| !line.ends_with(' ')));
    assert!(!board.contains('\r'));
}
