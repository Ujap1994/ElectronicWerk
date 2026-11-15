//! The assembly assistant: the order of the parts and the saved progress.

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

fn designators(group: &Value) -> Vec<&str> {
    group["designators"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d.as_str().unwrap())
        .collect()
}

#[test]
fn plans_the_assembly_and_saves_the_progress() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Board");
    let mut engine = Engine::without_journal();
    call(
        &mut engine,
        "project.create",
        json!({"path": root, "name": "Board", "template": "board-2l",
               "board": {"width": 50_000_000, "height": 40_000_000, "layers": 2, "thickness": 1_600_000}}),
    );
    let mut ids = Vec::new();
    for (symbol, value, footprint) in [
        ("R", "10k", "0805"),
        ("R", "330", "0805"),
        ("R", "10k", "0805"),
        ("C", "100n", "0402"),
        ("C", "10u", "1206"),
        ("LED", "red", "0805"),
        ("R", "1M", "0805"),
        ("R", "jumper", "Pin header 1x02 2.54 mm"),
    ] {
        let placed = call(
            &mut engine,
            "design.place",
            json!({"symbol": symbol, "value": value, "footprint": footprint}),
        );
        ids.push(placed["id"].clone());
    }
    // R4 is not populated.
    let sheet = call(&mut engine, "project.info", Value::Null)["rootSchematic"].clone();
    call(
        &mut engine,
        "edit.apply",
        json!({"changes": [{"op": "patch", "doc": sheet, "id": ids[6], "fields": {"flags": {"dnp": true}}}]}),
    );
    call(&mut engine, "design.updateBoard", Value::Null);
    call(&mut engine, "design.arrange", Value::Null);
    // C2 goes on the bottom.
    call(
        &mut engine,
        "design.placeFootprint",
        json!({"designator": "C2", "at": [40, 10], "side": "bottom"}),
    );

    let plan = call(&mut engine, "assembly.plan", Value::Null);
    assert_eq!(plan["board"], "Board");
    assert_eq!(plan["dnp"], json!(["R4"]));
    assert_eq!(plan["total"], 7);
    assert_eq!(plan["done"], 0);
    let groups = plan["groups"].as_array().unwrap();
    // SMD first, top first, smallest first: 0402, then the 0805 groups,
    // then the bottom; the through-hole header comes last.
    assert_eq!(designators(&groups[0]), ["C1"]);
    assert_eq!(groups[0]["footprint"], "Chip_0402_1005Metric");
    let r10k = groups.iter().find(|g| g["value"] == "10k").unwrap();
    assert_eq!(designators(r10k), ["R1", "R3"]);
    assert_eq!(r10k["count"], 2);
    assert_eq!(r10k["ids"].as_array().unwrap().len(), 2);
    let bottom = &groups[groups.len() - 2];
    assert_eq!(bottom["side"], "bottom");
    assert_eq!(designators(bottom), ["C2"]);
    let last = groups.last().unwrap();
    assert_eq!(last["mounting"], "throughHole");
    assert_eq!(designators(last), ["R5"]);
    assert_eq!(groups.len(), 6);

    // Bottom first, by count.
    let other = call(
        &mut engine,
        "assembly.plan",
        json!({"firstSide": "bottom", "order": "count"}),
    );
    assert_eq!(designators(&other["groups"][0]), ["C2"]);
    assert_eq!(designators(&other["groups"][1]), ["R1", "R3"]);

    // Ticking off: saved at once, an event, no undo step.
    call(&mut engine, "project.save", Value::Null);
    let marked = call(
        &mut engine,
        "assembly.mark",
        json!({"designators": ["R1", "C1"], "placed": true}),
    );
    assert_eq!(marked["placed"], json!(["C1", "R1"]));
    let events = engine.take_events();
    assert!(
        events
            .iter()
            .any(|e| e["type"] == "assembly" && e["placed"] == json!(["C1", "R1"]))
    );
    let info = call(&mut engine, "project.info", Value::Null);
    assert_eq!(info["history"]["dirty"], false);
    let file: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("assembly.json")).unwrap())
            .unwrap();
    assert_eq!(file["format"], "electronicwerk.assembly");
    let board_id = plan["boardId"].as_str().unwrap();
    assert_eq!(file["boards"][board_id]["placed"], json!(["C1", "R1"]));

    // A new engine reads it back.
    let mut later = Engine::without_journal();
    call(&mut later, "project.open", json!({"path": root}));
    let plan = call(&mut later, "assembly.plan", Value::Null);
    assert_eq!(plan["done"], 2);
    assert_eq!(plan["groups"][0]["placed"], 1);
    call(
        &mut later,
        "assembly.mark",
        json!({"designators": ["R1"], "placed": false}),
    );
    assert_eq!(call(&mut later, "assembly.plan", Value::Null)["done"], 1);

    // Finished: counted, and the next build starts empty.
    let reset = call(&mut later, "assembly.reset", json!({"completed": true}));
    assert_eq!(reset["completed"], 1);
    assert_eq!(reset["placed"], json!([]));
    assert!(
        error(
            &mut later,
            "assembly.mark",
            json!({"designators": ["Q9"], "placed": true})
        )
        .contains("no footprint Q9")
    );
    assert!(error(&mut later, "assembly.plan", json!({"board": "Nope"})).contains("no board"));
}
