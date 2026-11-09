//! The connectivity of the imported KiCad demo project matches the netlist
//! that KiCad would export (`crates/ew-kicad/tests/fixtures/blinky`).

#![allow(clippy::unwrap_used, reason = "test helpers may panic")]

use std::path::PathBuf;

use ew_schematic::analyze;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ew-kicad/tests/fixtures/blinky")
        .join(name)
}

#[test]
fn nets_match_the_kicad_netlist() {
    let imported = ew_kicad::import_project(&fixture("blinky.kicad_pro"), None).unwrap();
    let connectivity = analyze(&imported.project);
    assert!(
        connectivity.problems.is_empty(),
        "{:?}",
        connectivity.problems
    );
    assert_eq!(connectivity.instances.len(), 2);
    assert_eq!(connectivity.instances[1].name, "/Power/");

    let text = std::fs::read_to_string(fixture("blinky.net")).unwrap();
    let (expected, _) = ew_kicad::netlist::parse(&text).unwrap();
    let actual = connectivity.netlist();
    let multi = |list: &std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<(String, String)>,
    >| {
        list.iter()
            .filter(|(_, nodes)| nodes.len() > 1)
            .map(|(name, nodes)| (name.clone(), nodes.clone()))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    assert_eq!(multi(&actual), multi(&expected.nets));
}

#[test]
fn the_demo_project_passes_the_erc() {
    let imported = ew_kicad::import_project(&fixture("blinky.kicad_pro"), None).unwrap();
    let connectivity = analyze(&imported.project);
    let settings = ew_schematic::erc::settings(&imported.project);
    let report = ew_schematic::check(&imported.project, &connectivity, &settings);
    // Like KiCad: the demo has no power flags, so the power inputs of the
    // timer are not driven. Nothing else is wrong.
    let codes: Vec<(&str, &str)> = report
        .issues
        .iter()
        .map(|i| {
            (
                i.code.as_str(),
                i.args.get("net").map_or("", String::as_str),
            )
        })
        .collect();
    assert_eq!(
        codes,
        [("power-not-driven", "VCC"), ("power-not-driven", "GND")]
    );
}
