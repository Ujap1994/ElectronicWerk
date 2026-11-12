//! The library editors through the engine API: creating symbols and
//! footprints (also with a wizard), editing them with undo, renaming with
//! references, components, checks and editor scenes.

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

fn call_err(engine: &mut Engine, method: &str, params: Value) -> String {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    reply["error"]["code"].as_str().unwrap_or("none").to_owned()
}

/// A new project; returns the engine, the library and the top sheet.
fn project(dir: &std::path::Path) -> (Engine, String, String) {
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": dir.join("Lib"), "name": "Lib", "template": "empty"}),
    );
    let library = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "library")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let root = snapshot["rootSchematic"].as_str().unwrap().to_owned();
    (engine, library, root)
}

const MM: i64 = 1_000_000;

#[test]
#[allow(clippy::too_many_lines, reason = "one editing session")]
fn symbols_footprints_and_components() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, lib, root) = project(dir.path());
    let list = call(&mut engine, "libedit.list", json!({"doc": lib}));
    assert!(
        list["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == "R")
    );
    assert_eq!(list["footprints"].as_array().unwrap().len(), 0);

    // A footprint from a wizard preset.
    let presets = call(&mut engine, "libedit.presets", Value::Null);
    let soic = presets
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "SOIC-8")
        .unwrap()["wizard"]
        .clone();
    let preview = call(&mut engine, "libedit.wizard", json!({"wizard": soic}));
    assert_eq!(preview["name"], "SOIC-8_3.9x4.9mm_P1.27mm");
    assert_eq!(preview["def"]["pads"].as_array().unwrap().len(), 8);
    let created = call(
        &mut engine,
        "libedit.create",
        json!({"doc": lib, "kind": "footprint", "name": "SOIC-8", "wizard": soic}),
    );
    assert_eq!(created["history"]["undoLabel"], "create-library-item");
    assert_eq!(
        call_err(
            &mut engine,
            "libedit.create",
            json!({"doc": lib, "kind": "footprint", "name": "SOIC-8"})
        ),
        "conflict"
    );
    let item = call(
        &mut engine,
        "libedit.item",
        json!({"doc": lib, "kind": "footprint", "name": "SOIC-8"}),
    );
    assert!(
        item["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["severity"] == "info"),
        "{item}"
    );

    // A footprint edit: move pad 1, then undo.
    let moved = call(
        &mut engine,
        "libedit.footprint",
        json!({"doc": lib, "name": "SOIC-8", "edit": {"op": "move", "items": [{"kind": "pad", "index": 0}], "delta": [0, MM]}}),
    );
    assert_eq!(moved["history"]["undoLabel"], "edit-footprint");
    assert_eq!(moved["select"], json!([{"kind": "pad", "index": 0}]));
    call(&mut engine, "edit.undo", Value::Null);

    // A new symbol with two pins in a body, a second unit and an alternate body.
    call(
        &mut engine,
        "libedit.create",
        json!({"doc": lib, "kind": "symbol", "name": "Opamp"}),
    );
    for (number, y) in [("1", 0), ("2", -2_540_000)] {
        call(
            &mut engine,
            "libedit.symbol",
            json!({"doc": lib, "name": "Opamp", "edit": {"op": "addPin", "pin": {
                "number": number, "name": format!("IN{number}"), "kind": "input",
                "at": [-5_080_000, y], "direction": "right", "length": 2_540_000
            }}}),
        );
    }
    call(
        &mut engine,
        "libedit.symbol",
        json!({"doc": lib, "name": "Opamp", "edit": {"op": "setPins", "indices": [0, 1], "patch": {"length": 3_810_000}}}),
    );
    let unit = call(
        &mut engine,
        "libedit.symbol",
        json!({"doc": lib, "name": "Opamp", "edit": {"op": "addUnit", "copy": true}}),
    );
    assert_eq!(unit["body"], json!({"unit": 2, "alternate": false}));
    let alternate = call(
        &mut engine,
        "libedit.symbol",
        json!({"doc": lib, "name": "Opamp", "body": {"unit": 1}, "edit": {"op": "addAlternate"}}),
    );
    assert_eq!(alternate["body"]["alternate"], true);
    call(
        &mut engine,
        "libedit.setProps",
        json!({"doc": lib, "kind": "symbol", "name": "Opamp", "props": {"prefix": "U", "footprint": "SOIC-8", "description": "Dual op amp"}}),
    );
    assert_eq!(
        call_err(
            &mut engine,
            "libedit.setProps",
            json!({"doc": lib, "kind": "symbol", "name": "Opamp", "props": {"name": "X"}})
        ),
        "invalidParams"
    );
    let item = call(
        &mut engine,
        "libedit.item",
        json!({"doc": lib, "kind": "symbol", "name": "Opamp"}),
    );
    let def = &item["def"];
    assert_eq!(def["units"].as_array().unwrap().len(), 2);
    assert_eq!(def["units"][0]["pins"][0]["length"], 3_810_000);
    assert!(def["units"][0]["alternate"].is_object());

    // Editor scenes.
    let scene = call(
        &mut engine,
        "view.item",
        json!({"doc": lib, "kind": "symbol", "name": "Opamp", "unit": 1, "encoding": "none"}),
    );
    assert_eq!(scene["stats"]["objects"], 2);
    let scene = call(
        &mut engine,
        "view.item",
        json!({"doc": lib, "kind": "footprint", "name": "SOIC-8", "encoding": "none"}),
    );
    assert!(scene["stats"]["objects"].as_u64().unwrap() > 8);

    // A component, placed on the sheet with its data.
    call(
        &mut engine,
        "libedit.create",
        json!({"doc": lib, "kind": "component", "name": "LM358DR"}),
    );
    call(
        &mut engine,
        "libedit.setProps",
        json!({"doc": lib, "kind": "component", "name": "LM358DR", "props": {
            "symbol": "Opamp", "footprint": "SOIC-8", "value": "LM358", "manufacturer": "TI", "mpn": "LM358DR",
            "suppliers": [{"supplier": "Mouser", "partNumber": "595-LM358DR"}]
        }}),
    );
    let checks = call(
        &mut engine,
        "libedit.check",
        json!({"doc": lib, "kind": "component"}),
    );
    let codes: Vec<&str> = checks[0]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap())
        .collect();
    // Pins 1–4 of the symbol against pads 1–8: four pads without pins.
    assert_eq!(
        codes.iter().filter(|c| **c == "pad-without-pin").count(),
        4,
        "{codes:?}"
    );
    let placed = call(
        &mut engine,
        "schematic.place",
        json!({"doc": root, "component": "LM358DR", "at": [25_400_000, 25_400_000], "annotate": true}),
    );
    let id = placed["select"][0].as_str().unwrap().to_owned();
    let sheet = call(&mut engine, "document.get", json!({"doc": root}));
    let symbol = sheet["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id.as_str())
        .unwrap()
        .clone();
    assert_eq!(
        (
            symbol["symbol"].as_str(),
            symbol["value"].as_str(),
            symbol["footprint"].as_str()
        ),
        (Some("Opamp"), Some("LM358"), Some("SOIC-8"))
    );
    assert_eq!(symbol["properties"]["MPN"], "LM358DR");
    assert_eq!(symbol["properties"]["Supplier 1 PN"], "595-LM358DR");

    // Renaming the footprint follows every reference; deleting a used item needs force.
    let renamed = call(
        &mut engine,
        "libedit.rename",
        json!({"doc": lib, "kind": "footprint", "name": "SOIC-8", "new": "SO-8"}),
    );
    assert_eq!(
        renamed["changes"], 4,
        "footprint, symbol default, component, placed symbol"
    );
    let sheet = call(&mut engine, "document.get", json!({"doc": root}));
    assert_eq!(
        sheet["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == id.as_str())
            .unwrap()["footprint"],
        "SO-8"
    );
    assert_eq!(
        call_err(
            &mut engine,
            "libedit.delete",
            json!({"doc": lib, "kind": "symbol", "name": "Opamp"})
        ),
        "conflict"
    );
    let deleted = call(
        &mut engine,
        "libedit.delete",
        json!({"doc": lib, "kind": "symbol", "name": "Opamp", "force": true}),
    );
    assert_eq!(deleted["usage"]["placed"], 1);
    assert_eq!(deleted["history"]["undoLabel"], "delete-library-item");
}
