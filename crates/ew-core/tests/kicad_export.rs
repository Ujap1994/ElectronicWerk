//! `project.exportKicad`: a project made in ElectronicWerk (power ports,
//! its own symbols and footprints) as a KiCad project, read back with the
//! KiCad import with the same netlist.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    reason = "test helpers may panic and take owned JSON for brevity"
)]

use ew_core::Engine;
use serde_json::{Value, json};

fn call(engine: &mut Engine, method: &str, params: Value) -> Value {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    assert!(reply.get("error").is_none(), "{method}: {reply}");
    reply["result"].clone()
}

#[test]
fn a_demo_project_goes_to_kicad_and_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    call(
        &mut engine,
        "project.create",
        json!({"path": dir.path().join("Demo"), "name": "Sensor Board", "template": "demo"}),
    );
    // Two power ports on the root sheet.
    let snapshot = call(&mut engine, "project.info", Value::Null);
    let root = snapshot["rootSchematic"].as_str().unwrap().to_owned();
    for (n, net, style) in [(1, "GND", "ground"), (2, "+3V3", "bar")] {
        let port = json!({
            "type": "powerPort", "id": format!("00000000-0000-4000-8000-00000000f0{n:02}"),
            "net": net, "at": [n * 10_160_000, 20_320_000], "style": style
        });
        call(
            &mut engine,
            "edit.apply",
            json!({"changes": [{"op": "insert", "doc": root, "object": port}]}),
        );
    }
    let out = dir.path().join("kicad");
    let result = call(&mut engine, "project.exportKicad", json!({"path": out}));
    let files: Vec<&str> = result["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for expected in [
        "Sensor_Board.kicad_pro",
        "Sensor_Board.kicad_sch",
        "Sensor_Board.kicad_pcb",
        "Sensor_Board.kicad_sym",
        "sym-lib-table",
        "fp-lib-table",
    ] {
        assert!(files.contains(&expected), "{expected} in {files:?}");
    }
    assert!(files.iter().any(|f| f.starts_with("Sensor_Board.pretty/")));
    assert!(out.join("Sensor_Board.kicad_pro").exists());
    assert_eq!(result["project"], json!(out.join("Sensor_Board.kicad_pro")));

    // The power ports are KiCad power symbols.
    let sheets: String = files
        .iter()
        .filter(|f| f.ends_with(".kicad_sch"))
        .map(|f| std::fs::read_to_string(out.join(f)).unwrap())
        .collect();
    assert!(sheets.contains("(symbol \"power:"), "power symbols");
    assert!(sheets.contains("(power)"));

    // KiCad's view of the project: the same nets.
    let session = engine_project(&mut engine, dir.path());
    let back = ew_kicad::import_project(&out.join("Sensor_Board.kicad_pro"), None).unwrap();
    let before = ew_schematic::connectivity::analyze(&session).netlist();
    let after = ew_schematic::connectivity::analyze(&back.project).netlist();
    assert_eq!(after, before);

    // A folder with files is refused.
    let reply: Value =
        serde_json::from_str(&engine.dispatch(
            &json!({"method": "project.exportKicad", "params": {"path": out}}).to_string(),
        ))
        .unwrap();
    assert_eq!(reply["error"]["code"], "invalidParams");
}

/// The open project as the engine saved it.
fn engine_project(engine: &mut Engine, dir: &std::path::Path) -> ew_model::project::Project {
    call(engine, "project.save", json!({}));
    ew_format::store::load_project(&dir.join("Demo"))
        .unwrap()
        .project
}

fn eagle_fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ew-eagle/tests/fixtures/blinky")
        .join(name)
}

#[test]
fn an_eagle_project_and_library_come_in() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    let probe = call(
        &mut engine,
        "import.kicadProbe",
        json!({"source": eagle_fixture("blinky.brd")}),
    );
    assert_eq!(probe["format"], "eagle");
    assert_eq!(probe["schematic"], true);
    assert_eq!(probe["board"], true);
    assert_eq!(probe["name"], "blinky");

    let target = dir.path().join("Blinky");
    let result = call(
        &mut engine,
        "import.kicad",
        json!({"source": eagle_fixture("blinky.sch"), "path": target, "name": "Blinky"}),
    );
    assert_eq!(result["reportFile"], "eagle-import-report.md");
    assert!(target.join("eagle-import-report.md").exists());
    assert_eq!(result["report"]["counts"]["footprints"], 8);
    let docs = result["project"]["documents"].as_array().unwrap();
    assert!(docs.iter().any(|d| d["kind"] == "pcb"));
    assert_eq!(docs.iter().filter(|d| d["kind"] == "schematic").count(), 3);

    // The board matches the schematic: nothing to update.
    let board = docs.iter().find(|d| d["kind"] == "pcb").unwrap()["id"].clone();
    let preview = call(&mut engine, "pcb.syncPreview", json!({"doc": board}));
    let changes = preview["changes"].as_array().map_or(0, Vec::len);
    assert_eq!(changes, 0, "{preview}");

    // The library on its own: everything is already in the project.
    let library = call(
        &mut engine,
        "import.library",
        json!({"source": eagle_fixture("blinky.lbr")}),
    );
    assert!(library["report"].is_object());
    assert_eq!(library["symbols"], 0, "{library}");
    assert_eq!(library["footprints"], 0);
}
