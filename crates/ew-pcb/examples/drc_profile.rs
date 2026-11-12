//! Prints where the time of a DRC on a synthetic board goes.
#![allow(
    clippy::unwrap_used,
    clippy::print_stdout,
    missing_docs,
    reason = "dev tool"
)]
use std::collections::BTreeMap;
use std::time::Instant;

use ew_model::document::Document;
use ew_pcb::drc::{Options, check};

fn main() {
    let size = std::env::args().nth(1).unwrap_or_else(|| "l".into());
    let project = ew_synth::generate(size.parse().unwrap(), ew_synth::DEFAULT_SEED);
    let pcb = project
        .documents
        .into_values()
        .find_map(|d| match d {
            Document::Pcb(p) => Some(p),
            _ => None,
        })
        .unwrap();
    println!(
        "tracks {} vias {} footprints {} zones {}",
        pcb.tracks.len(),
        pcb.vias.len(),
        pcb.footprints.len(),
        pcb.zones.len()
    );
    let t = Instant::now();
    let board = ew_pcb::items::Board::new(&pcb);
    println!(
        "board: {:?} ({} copper, {} holes, {} silk)",
        t.elapsed(),
        board.copper.len(),
        board.holes.len(),
        board.silk.len()
    );
    let t = Instant::now();
    let conn = ew_pcb::connectivity::analyze(&board);
    println!(
        "connectivity: {:?} ({} airwires, {} shorts)",
        t.elapsed(),
        conn.airwires.len(),
        conn.shorts.len()
    );
    let t = Instant::now();
    let fills = ew_pcb::fill::fill(&pcb, None, None);
    println!(
        "fill: {:?} ({} zones, {} areas, {} points)",
        t.elapsed(),
        fills.len(),
        fills.iter().map(|f| f.areas.len()).sum::<usize>(),
        fills
            .iter()
            .flat_map(|f| f.areas.iter())
            .map(|a| a.points.len() + a.holes.iter().map(Vec::len).sum::<usize>())
            .sum::<usize>()
    );
    let t = Instant::now();
    let report = check(&pcb, None, &Options::default());
    println!("check: {:?}", t.elapsed());
    let mut codes: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &report.findings {
        *codes.entry(&f.code).or_default() += 1;
    }
    println!("{codes:?}");
    for code in codes.keys() {
        if let Some(f) = report.findings.iter().find(|f| f.code == *code) {
            println!("  {code}: {}", f.message);
        }
    }
}

#[allow(dead_code, reason = "called from main via the fill flag")]
fn _unused() {}
