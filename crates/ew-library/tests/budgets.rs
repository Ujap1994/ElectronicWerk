//! Performance budget of the search (run in CI with
//! `cargo test --release -p ew-library --test budgets -- --ignored --nocapture`).
//!
//! 50,000 synthetic entries with KiCad-like names stand in for the base
//! library; the real libraries are measured locally (`tests/samples.rs`).

#![allow(
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::cast_precision_loss,
    reason = "test output"
)]

use std::time::Instant;

use ew_library::entry::{Entry, Kind, Print};
use ew_library::index::{Library, Query, SourceInfo};

/// Budget per search in milliseconds (the acceptance criterion is 50 ms on
/// a typical computer; CI runners are slower).
const BUDGET_MS: f64 = 100.0;

const WORDS: &[&str] = &[
    "resistor",
    "capacitor",
    "inductor",
    "diode",
    "schottky",
    "zener",
    "led",
    "transistor",
    "mosfet",
    "regulator",
    "amplifier",
    "operational",
    "comparator",
    "connector",
    "header",
    "socket",
    "switch",
    "crystal",
    "oscillator",
    "fuse",
    "relay",
    "sensor",
    "temperature",
    "microcontroller",
    "memory",
    "flash",
    "interface",
    "usb",
    "uart",
    "ethernet",
    "power",
    "smd",
    "tht",
    "dual",
    "quad",
    "low",
    "noise",
    "high",
    "speed",
    "precision",
];
const PREFIXES: &[&str] = &[
    "R", "C", "L", "D", "LED", "Q", "U", "J", "SW", "F", "Y", "K",
];
const PACKAGES: &[&str] = &[
    "0402_1005Metric",
    "0603_1608Metric",
    "0805_2012Metric",
    "1206_3216Metric",
    "SOT-23",
    "SOT-23-5",
    "SOIC-8_3.9x4.9mm_P1.27mm",
    "TSSOP-20_4.4x6.5mm_P0.65mm",
    "QFN-32-1EP_5x5mm",
    "DIP-8_W7.62mm",
    "PinHeader_1x10_P2.54mm",
    "PinHeader_2x20_P2.54mm",
    "LQFP-64_10x10mm_P0.5mm",
];

struct Random(u64);

impl Random {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).unwrap_or(0) % n
    }
}

fn entry(kind: Kind, library: String, name: String, description: String) -> Entry {
    Entry {
        kind,
        source: 0,
        library,
        package: name.clone(),
        name,
        description,
        keywords: String::new(),
        value: String::new(),
        mpn: String::new(),
        manufacturer: String::new(),
        datasheet: String::new(),
        params: String::new(),
        pins: 2,
        units: 1,
        mounting: Some("smd".to_owned()),
        size: None,
        model: None,
        file: None,
        mesh: None,
        data: None,
        print: Print::None,
    }
}

fn synthetic(count: usize) -> Vec<Entry> {
    let mut random = Random(42);
    let kinds = [Kind::Symbol, Kind::Footprint, Kind::Model];
    (0..count)
        .map(|index| {
            let kind = kinds[index % 3];
            let prefix = PREFIXES[random.next(PREFIXES.len())];
            let package = PACKAGES[random.next(PACKAGES.len())];
            let name = format!("{prefix}_{package}_{index}");
            let description = (0..8)
                .map(|_| WORDS[random.next(WORDS.len())])
                .collect::<Vec<_>>()
                .join(" ");
            let library = format!("{}_{}", prefix, ["SMD", "THT", "Misc"][random.next(3)]);
            entry(kind, library, name, description)
        })
        .collect()
}

#[test]
#[ignore = "performance budget; run in release mode"]
fn searches_answer_within_the_budget() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let (library, _) =
        Library::build(dir.path(), &[SourceInfo::default()], synthetic(50_000), 50).unwrap();
    println!("built 50,000 entries in {:?}", started.elapsed());
    let mut worst = 0.0_f64;
    for text in [
        "r",
        "R_0805",
        "0805",
        "SOT-23",
        "LQFP 64",
        "op amp",
        "connector 2x20",
        "usb",
        "low noise",
        "smd capacitor 0603",
        "xyzzy",
        "",
    ] {
        for kind in [None, Some(Kind::Footprint)] {
            let mut best = f64::MAX;
            for _ in 0..3 {
                let started = Instant::now();
                let result = library
                    .search(&Query {
                        text: text.to_owned(),
                        kind,
                        limit: 200,
                        ..Query::default()
                    })
                    .unwrap();
                best = best.min(started.elapsed().as_secs_f64() * 1000.0);
                assert!(result.total > 0 || text == "xyzzy", "{text}");
            }
            println!("{text:>20} {kind:?}: {best:.1} ms");
            worst = worst.max(best);
        }
    }
    println!("slowest search: {worst:.1} ms (budget {BUDGET_MS} ms)");
    assert!(worst < BUDGET_MS);
}
