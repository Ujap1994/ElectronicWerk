//! The acceptance test of the base library against the official KiCad
//! libraries, which stay on the developer's machine. Set
//! `EW_KICAD_LIBRARIES` to a KiCad installation or a folder with the library
//! repositories (optionally `EW_KICAD_MESHES` to their tessellated meshes),
//! then run
//! `cargo test --release -p ew-library --test samples -- --ignored --nocapture`.

#![allow(clippy::unwrap_used, clippy::print_stdout, reason = "test output")]

use std::path::PathBuf;
use std::time::Instant;

use ew_library::index::Query;
use ew_library::{Kind, build, ingest, pack};

/// The acceptance criterion: a search over the whole library answers in
/// less than 50 ms.
const BUDGET_MS: f64 = 50.0;

#[test]
#[ignore = "needs the KiCad libraries (EW_KICAD_LIBRARIES)"]
fn searches_the_kicad_libraries_within_50_ms_and_reproducibly() {
    let Ok(root) = std::env::var("EW_KICAD_LIBRARIES") else {
        println!("EW_KICAD_LIBRARIES is not set; skipped");
        return;
    };
    let mut source = ingest::detect_kicad(&PathBuf::from(root)).unwrap();
    source.meshes = std::env::var("EW_KICAD_MESHES").ok().map(PathBuf::from);
    let a = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let built = build(a.path(), std::slice::from_ref(&source), None, &|_, _, _| {}).unwrap();
    println!(
        "built in {:?}: {:?}",
        started.elapsed(),
        built.library.status().unwrap().counts
    );
    let mut worst = 0.0_f64;
    for text in [
        "r",
        "R_0805",
        "0805",
        "SOT-23",
        "LM358",
        "op amp",
        "usb c receptacle",
        "stm32f4",
        "conn 2x20",
        "capacitor smd 0603",
        "ne555",
        "mounting hole",
        "xyzzy",
        "",
    ] {
        for kind in [
            None,
            Some(Kind::Symbol),
            Some(Kind::Footprint),
            Some(Kind::Model),
        ] {
            let started = Instant::now();
            let result = built
                .library
                .search(&Query {
                    text: text.to_owned(),
                    kind,
                    limit: 200,
                    ..Query::default()
                })
                .unwrap();
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            worst = worst.max(ms);
            println!(
                "{text:>20} {kind:?}: {} results in {ms:.1} ms",
                result.total
            );
        }
    }
    println!("slowest search: {worst:.1} ms");
    assert!(worst < BUDGET_MS, "a search took {worst:.1} ms");

    // A second build gives the same report.
    let b = tempfile::tempdir().unwrap();
    drop(build(b.path(), &[source], None, &|_, _, _| {}).unwrap());
    let report = |dir: &std::path::Path| std::fs::read(dir.join(pack::REPORT_JSON)).unwrap();
    assert!(
        report(a.path()) == report(b.path()),
        "the report is not reproducible"
    );
}
