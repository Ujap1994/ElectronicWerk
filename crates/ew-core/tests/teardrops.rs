//! Teardrops through the engine API: `pcb.teardrops` adds a filled
//! teardrop zone where tracks end in pads and vias, keeps every clearance,
//! replaces them when it runs again, keeps its settings in the design
//! rules and removes them on request.

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

fn teardrops(engine: &mut Engine, board: &Value) -> Vec<Value> {
    let pcb = call(engine, "document.get", json!({"doc": board}));
    pcb["zones"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|z| z["teardrop"] == true)
        .cloned()
        .collect()
}

fn problems(engine: &mut Engine, board: &Value) -> usize {
    let report = call(engine, "pcb.drc", json!({"doc": board}));
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| ["clearance", "short"].contains(&f["code"].as_str().unwrap_or("")))
        .count()
}

#[test]
fn teardrops_are_made_filled_replaced_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    let result = call(
        &mut engine,
        "import.kicad",
        json!({
            "source": fixture("../ew-kicad/tests/fixtures/blinky/blinky.kicad_pro"),
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
    let before = problems(&mut engine, &board);

    let made = call(&mut engine, "pcb.teardrops", json!({"doc": board}));
    let added = made["added"].as_u64().unwrap();
    assert!(added >= 4, "{made}");
    let zones = teardrops(&mut engine, &board);
    assert_eq!(zones.len() as u64, added);
    for zone in &zones {
        assert!(!zone["filled"].as_array().unwrap().is_empty(), "{zone}");
        assert_eq!(zone["priority"], 1000);
    }
    assert_eq!(problems(&mut engine, &board), before);

    // Again with straight sides and vias left out: the old ones are replaced
    // and the settings are kept in the rules.
    let settings = json!({
        "pads": true, "vias": false, "lengthRatio": 50, "maxLength": 1_000_000,
        "widthRatio": 100, "maxWidth": 2_000_000, "curved": false,
    });
    let again = call(
        &mut engine,
        "pcb.teardrops",
        json!({"doc": board, "settings": settings}),
    );
    assert_eq!(again["removed"].as_u64().unwrap(), added);
    let straight = teardrops(&mut engine, &board);
    assert_eq!(straight.len() as u64, again["added"].as_u64().unwrap());
    assert!(
        straight
            .iter()
            .all(|z| z["outline"].as_array().unwrap().len() == 4)
    );
    let rules = call(&mut engine, "pcb.rules", json!({"doc": board}))["rules"].clone();
    assert_eq!(rules["teardrops"]["curved"], false);
    assert_eq!(rules["teardrops"]["vias"], false);

    // One undo brings the first teardrops back.
    call(&mut engine, "edit.undo", Value::Null);
    assert_eq!(teardrops(&mut engine, &board).len() as u64, added);

    // Removed.
    let removed = call(
        &mut engine,
        "pcb.teardrops",
        json!({"doc": board, "remove": true}),
    );
    assert_eq!(removed["added"], 0);
    assert_eq!(teardrops(&mut engine, &board).len(), 0);
}
