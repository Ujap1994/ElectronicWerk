//! The 3D assembly and the enclosure check through the engine API: a box
//! enclosure made for the demo board, bodies moved into the board and its
//! parts, the findings in the design rules check, a STEP file added from
//! outside the project.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "test code"
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

fn error(engine: &mut Engine, method: &str, params: Value) -> String {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    reply["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

fn doc_of(snapshot: &Value, kind: &str) -> String {
    snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == kind)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn codes(findings: &Value) -> Vec<String> {
    findings
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"].as_str().unwrap().starts_with("mechanical-"))
        .map(|f| f["code"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn checks_the_board_in_its_enclosure() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Demo");
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": root, "name": "Demo", "template": "demo"}),
    );
    let board = doc_of(&snapshot, "pcb");

    // A box made to fit: no findings.
    let made = call(&mut engine, "mechanical.enclosure", json!({"doc": board}));
    let ids: Vec<String> = made["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(root.join("mechanical/enclosure-bottom.step").is_file());
    assert!(root.join("mechanical/enclosure-lid.step").is_file());
    let list = call(&mut engine, "mechanical.list", json!({"doc": board}));
    let bodies = list["bodies"].as_array().unwrap();
    assert_eq!(bodies.len(), 2);
    assert_eq!(bodies[0]["exists"], true);
    assert_eq!(bodies[0]["path"], "mechanical/enclosure-bottom.step");
    assert_eq!(bodies[1]["kind"], "lid");
    assert_eq!(bodies[1]["opacity"], 40);
    assert_eq!(list["clearance"], 0.5);
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    assert_eq!(checked["errors"], 0, "{checked}");
    assert_eq!(checked["warnings"], 0, "{checked}");
    assert_eq!(checked["unchecked"], json!([]));
    assert_eq!(checked["tessellate"], json!([]));

    // The lid 6 mm lower cuts into the tallest parts (and the board).
    let lid = &bodies[1];
    let z = lid["position"][2].as_i64().unwrap();
    let mut position = lid["position"].clone();
    position[2] = json!(z - 6_000_000);
    call(
        &mut engine,
        "mechanical.update",
        json!({"doc": board, "id": ids[1], "position": position}),
    );
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    assert!(checked["errors"].as_u64().unwrap() >= 1, "{checked}");
    let collision = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "mechanical-collision" && f["targets"][1].is_object())
        .unwrap()
        .clone();
    assert_eq!(collision["targets"][0]["id"], ids[1].as_str());
    assert!(collision["targets"][1]["id"].is_string());
    assert_eq!(collision["args"]["body"], "Enclosure lid");
    assert!(collision["point"][2].as_f64().unwrap() > 1.0);
    assert!(
        collision["message"]
            .as_str()
            .unwrap()
            .ends_with("collides with Enclosure lid")
    );

    // The design rules check reports them too.
    let drc = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(codes(&drc["findings"]).contains(&"mechanical-collision".to_owned()));

    // Undo puts the lid back.
    call(&mut engine, "edit.undo", Value::Null);
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    assert_eq!(checked["errors"], 0, "{checked}");

    // The bottom shell 1 mm to the right: a wall cuts into the board.
    let bottom = &bodies[0];
    let mut position = bottom["position"].clone();
    position[0] = json!(position[0].as_i64().unwrap() + 1_000_000);
    call(
        &mut engine,
        "mechanical.update",
        json!({"doc": board, "id": ids[0], "position": position}),
    );
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    let board_hit = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"].as_str().unwrap().ends_with("/board"))
        .unwrap()
        .clone();
    assert_eq!(board_hit["code"], "mechanical-collision");
    assert_eq!(
        board_hit["message"],
        "The board collides with Enclosure bottom"
    );

    // 0.3 mm instead: a clearance finding (minimum 0.5 mm).
    position[0] = json!(bottom["position"][0].as_i64().unwrap() + 300_000);
    call(
        &mut engine,
        "mechanical.update",
        json!({"doc": board, "id": ids[0], "position": position}),
    );
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    let board_hit = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"].as_str().unwrap().ends_with("/board"))
        .unwrap()
        .clone();
    assert_eq!(board_hit["code"], "mechanical-clearance");
    assert_eq!(board_hit["severity"], "warning");
    assert_eq!(board_hit["args"]["distance"], "0.2 mm");
    // A smaller minimum clears it.
    let checked = call(
        &mut engine,
        "mechanical.check",
        json!({"doc": board, "clearance": 0.1}),
    );
    assert_eq!(checked["warnings"], 0, "{checked}");

    // Bodies that are not checked, or hidden and not checked.
    call(
        &mut engine,
        "mechanical.update",
        json!({"doc": board, "id": ids[0], "check": false, "opacity": 30}),
    );
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    assert_eq!(checked["findings"], json!([]));
    assert!(
        error(
            &mut engine,
            "mechanical.update",
            json!({"doc": board, "id": ids[0], "opacity": 0})
        )
        .contains("opacity")
    );

    // A STEP file from outside the project is copied in.
    let outside = dir.path().join("Front panel.step");
    std::fs::copy(root.join("mechanical/enclosure-lid.step"), &outside).unwrap();
    let added = call(
        &mut engine,
        "mechanical.add",
        json!({"doc": board, "file": outside}),
    );
    assert_eq!(added["path"], "mechanical/Front panel.step");
    assert!(root.join("mechanical/Front panel.step").is_file());
    let list = call(&mut engine, "mechanical.list", json!({"doc": board}));
    let panel = list["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == added["id"])
        .unwrap()
        .clone();
    assert_eq!(panel["kind"], "panel");
    assert_eq!(panel["name"], "Front panel");
    assert!(
        error(
            &mut engine,
            "mechanical.add",
            json!({"doc": board, "file": dir.path().join("missing.step")})
        )
        .contains("missing.step")
    );
    let not_step = dir.path().join("notes.txt");
    std::fs::write(&not_step, "hello").unwrap();
    assert!(
        error(
            &mut engine,
            "mechanical.add",
            json!({"doc": board, "file": not_step})
        )
        .contains("STEP")
    );

    // Removing keeps the files; saving and opening again keeps the bodies.
    call(
        &mut engine,
        "mechanical.remove",
        json!({"doc": board, "ids": [added["id"]]}),
    );
    assert!(root.join("mechanical/Front panel.step").is_file());
    call(&mut engine, "project.save", Value::Null);
    let mut later = Engine::without_journal();
    call(&mut later, "project.open", json!({"path": root}));
    let list = call(&mut later, "mechanical.list", json!({"doc": board}));
    assert_eq!(list["bodies"].as_array().unwrap().len(), 2);
    assert_eq!(list["bodies"][0]["check"], false);
    assert_eq!(list["bodies"][0]["opacity"], 30);
}
