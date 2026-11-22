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

#[test]
fn the_net_inspector_lists_every_net() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Nets");
    ew_format::store::create_project(&path, &ew_synth::generate(ew_synth::DesignSize::S, 4))
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
    let nets = call(&mut engine, "pcb.nets", json!({"doc": doc}));
    let listed = nets["nets"].as_array().unwrap();
    assert_eq!(listed.len(), board["nets"].as_array().unwrap().len());
    // The counts add up to the board's tracks and vias with a net.
    let sum = |key: &str| -> u64 { listed.iter().map(|n| n[key].as_u64().unwrap()).sum() };
    let with_net = |key: &str| {
        board[key]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o.get("net").is_some())
            .count() as u64
    };
    assert_eq!(sum("tracks"), with_net("tracks"));
    assert_eq!(sum("vias"), with_net("vias"));
    assert!(listed.iter().any(|n| n["length"].as_f64().unwrap() > 0.0));
    assert!(listed.iter().all(|n| n["class"].is_string()));
}
