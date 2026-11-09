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

#[test]
fn exported_netlists_read_back() {
    let imported = ew_kicad::import_project(&fixture("blinky.kicad_pro"), None).unwrap();
    let connectivity = analyze(&imported.project);
    let text = ew_schematic::export::kicad_netlist(&imported.project, &connectivity);
    let (exported, _) = ew_kicad::netlist::parse(&text).unwrap();
    let expected = std::fs::read_to_string(fixture("blinky.net")).unwrap();
    let (expected, _) = ew_kicad::netlist::parse(&expected).unwrap();
    assert_eq!(exported.nets, expected.nets);
    assert_eq!(exported.components, expected.components);

    let csv = ew_schematic::export::csv_netlist(&connectivity);
    assert!(csv.starts_with("Net,Designator,Pin\n/CV,C2,1\n"), "{csv}");

    let bom = ew_schematic::export::bom_csv(&imported.project, &connectivity);
    let lines: Vec<&str> = bom.lines().collect();
    assert_eq!(
        lines[0],
        "Item,Quantity,Designators,Value,Footprint,Symbol,Description,Manufacturer,MPN,DNP"
    );
    assert_eq!(lines.len(), 9, "{bom}");
}
