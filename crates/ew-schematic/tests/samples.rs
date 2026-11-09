//! Connectivity of a real KiCad project that stays on the developer's
//! machine: set `EW_KICAD_SAMPLE` to a `.kicad_pro` file with its KiCad
//! netlist (`.net`) next to it, then run
//! `cargo test --release -p ew-schematic --test samples -- --ignored --nocapture`.

#![allow(clippy::unwrap_used, clippy::print_stdout, reason = "test output")]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

#[test]
#[ignore = "needs a private KiCad project (EW_KICAD_SAMPLE)"]
fn nets_match_the_kicad_netlist_of_a_real_project() {
    let Ok(sample) = std::env::var("EW_KICAD_SAMPLE") else {
        println!("EW_KICAD_SAMPLE is not set; skipped");
        return;
    };
    let sample = PathBuf::from(sample);
    let imported = ew_kicad::import_project(&sample, None).unwrap();
    let started = Instant::now();
    let connectivity = ew_schematic::analyze(&imported.project);
    println!(
        "{} instances, {} nets, {} pins in {:?}; problems: {:?}",
        connectivity.instances.len(),
        connectivity.nets.len(),
        connectivity.pins.len(),
        started.elapsed(),
        connectivity.problems.iter().take(5).collect::<Vec<_>>()
    );
    let text = std::fs::read_to_string(sample.with_extension("net")).unwrap();
    let (expected, _) = ew_kicad::netlist::parse(&text).unwrap();
    // Compare connectivity (sets of pins), not names.
    let sets = |list: &BTreeMap<String, BTreeSet<(String, String)>>| -> BTreeSet<BTreeSet<(String, String)>> {
        list.values().filter(|nodes| nodes.len() > 1).cloned().collect()
    };
    let actual_list = connectivity.netlist();
    let (want, have) = (sets(&expected.nets), sets(&actual_list));
    let missing: Vec<_> = want.difference(&have).take(10).collect();
    let extra: Vec<_> = have.difference(&want).take(10).collect();
    println!(
        "{} of {} connected nets match",
        want.intersection(&have).count(),
        want.len()
    );
    for net in &missing {
        println!("  missing: {net:?}");
    }
    for net in &extra {
        println!("  extra:   {net:?}");
    }
    let names = expected
        .nets
        .iter()
        .filter(|(name, nodes)| nodes.len() > 1 && actual_list.get(*name) == Some(*nodes))
        .count();
    println!("{names} nets also have the same name");
    let by_nodes: BTreeMap<&BTreeSet<(String, String)>, &String> = actual_list
        .iter()
        .map(|(name, nodes)| (nodes, name))
        .collect();
    let mut shown = 0;
    for (name, nodes) in &expected.nets {
        if nodes.len() > 1 {
            if let Some(ours) = by_nodes.get(nodes) {
                if *ours != name && shown < 25 {
                    println!("  name: kicad {name:?} / ours {ours:?}");
                    shown += 1;
                }
            }
        }
    }
    assert_eq!(want, have);
}

#[test]
#[ignore = "needs a private KiCad project (EW_KICAD_SAMPLE)"]
fn the_erc_of_a_real_project() {
    let Ok(sample) = std::env::var("EW_KICAD_SAMPLE") else {
        println!("EW_KICAD_SAMPLE is not set; skipped");
        return;
    };
    let imported = ew_kicad::import_project(&PathBuf::from(sample), None).unwrap();
    let started = Instant::now();
    let connectivity = ew_schematic::analyze(&imported.project);
    let report = ew_schematic::check(
        &imported.project,
        &connectivity,
        &ew_schematic::erc::settings(&imported.project),
    );
    println!(
        "ERC: {} errors, {} warnings in {:?}",
        report.errors,
        report.warnings,
        started.elapsed()
    );
    let mut codes: BTreeMap<&str, usize> = BTreeMap::new();
    for issue in &report.issues {
        *codes.entry(issue.code.as_str()).or_default() += 1;
    }
    println!("{codes:?}");
    for issue in report.issues.iter().take(40) {
        println!(
            "{:?} {} {} {:?}: {}",
            issue.severity, issue.code, issue.path, issue.at, issue.message
        );
    }
}
