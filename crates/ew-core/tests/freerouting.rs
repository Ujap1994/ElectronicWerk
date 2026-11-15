//! Replay of a real Freerouting run (Freerouting 2.5.0) on the Eagle blinky
//! board: the board's DSN export is still the file Freerouting routed, and
//! the recorded session turns the board into one without errors and without
//! unrouted connections. Record again with `scripts/record-freerouting.sh`
//! after changing the DSN writer.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    reason = "test helpers may panic and take owned JSON for brevity"
)]

use std::path::PathBuf;

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

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

/// The DSN text without the line naming the program version.
fn without_host(text: &str) -> String {
    text.lines()
        .filter(|line| !line.contains("host_cad"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_freerouting_session_routes_the_board() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    let result = call(
        &mut engine,
        "import.kicad",
        json!({
            "source": fixture("../ew-eagle/tests/fixtures/blinky/blinky.sch"),
            "path": dir.path().join("blinky"),
        }),
    );
    let board = result["project"]["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "pcb")
        .unwrap()["id"]
        .clone();

    let exported = call(&mut engine, "pcb.exportSpecctra", json!({"doc": board}));
    let recorded =
        std::fs::read_to_string(fixture("tests/fixtures/freerouting/blinky.dsn")).unwrap();
    assert_eq!(
        without_host(exported["text"].as_str().unwrap()),
        without_host(&recorded),
        "the DSN export changed: record the session again with scripts/record-freerouting.sh"
    );

    let before = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert_ne!(before["airwires"].as_array().unwrap().len(), 0);

    let imported = call(
        &mut engine,
        "pcb.importSession",
        json!({"doc": board, "path": fixture("tests/fixtures/freerouting/blinky.ses")}),
    );
    assert!(imported["tracks"].as_u64().unwrap() > 0, "{imported}");
    call(&mut engine, "pcb.fillZones", json!({"doc": board}));

    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    let errors: Vec<&Value> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["severity"] == "error")
        .collect();
    assert_eq!(errors.len(), 0, "{errors:#?}");
    assert_eq!(report["airwires"].as_array().unwrap().len(), 0);
}
