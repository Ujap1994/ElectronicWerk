//! "Select similar" (`document.similar`) on a synthetic board.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
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

#[test]
fn finds_objects_with_the_same_properties() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Similar");
    ew_format::store::create_project(&path, &ew_synth::generate(ew_synth::DesignSize::S, 3))
        .unwrap();
    let mut engine = Engine::without_journal();
    call(&mut engine, "project.open", json!({"path": path}));
    let info = call(&mut engine, "project.info", Value::Null);
    let doc = info["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "pcb")
        .unwrap()["id"]
        .clone();
    let board = call(&mut engine, "document.get", json!({"doc": doc}));
    let tracks = board["tracks"].as_array().unwrap();
    let track = &tracks[0];

    // The fields a track can be compared by, with the net's name.
    let offered = call(
        &mut engine,
        "document.similar",
        json!({"doc": doc, "id": track["id"]}),
    );
    assert_eq!(offered["type"], "track");
    let keys: Vec<&str> = offered["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["key"].as_str().unwrap())
        .collect();
    assert!(
        keys.contains(&"layer") && keys.contains(&"width"),
        "{keys:?}"
    );
    assert!(!keys.contains(&"start") && !keys.contains(&"id"));
    let net = offered["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"] == "net");
    if let Some(net) = net {
        assert!(net["text"].is_string());
    }

    // Every track on the same layer with the same width.
    let found = call(
        &mut engine,
        "document.similar",
        json!({"doc": doc, "id": track["id"], "keys": ["layer", "width"]}),
    );
    let expected = tracks
        .iter()
        .filter(|t| t["layer"] == track["layer"] && t["width"] == track["width"])
        .count();
    assert_eq!(found["ids"].as_array().unwrap().len(), expected);
    assert!(expected > 1);
}
