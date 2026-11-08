//! The `library.*` engine methods with the synthetic library of
//! `crates/ew-library/tests/fixtures`.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    reason = "test helpers may panic and take owned JSON for brevity"
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ew_core::Engine;
use serde_json::{Value, json};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ew-library/tests/fixtures")
}

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
    reply["error"]["code"].as_str().unwrap_or("none").to_owned()
}

fn source() -> Value {
    json!({"kicad": fixtures().join("kicad"), "meshes": fixtures().join("meshes"), "version": "test"})
}

fn find(engine: &mut Engine, text: &str, kind: &str) -> i64 {
    let result = call(
        engine,
        "library.search",
        json!({"text": text, "kind": kind, "limit": 5}),
    );
    result["items"][0]["id"].as_i64().unwrap()
}

#[test]
fn builds_in_the_background_with_progress_events() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    call(&mut engine, "library.open", json!({"dir": dir.path()}));
    let status = call(&mut engine, "library.status", Value::Null);
    assert_eq!(status["installed"], false);
    assert_eq!(
        error(&mut engine, "library.search", json!({"text": "R"})),
        "notFound"
    );

    assert_eq!(
        call(&mut engine, "library.build", json!({"sources": [source()]}))["started"],
        true
    );
    let started = Instant::now();
    let mut events = Vec::new();
    while !events
        .iter()
        .any(|e: &Value| e["action"] == "built" || e["action"] == "failed")
    {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "build timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
        events.extend(engine.take_events());
    }
    assert!(events.iter().any(|e| e["action"] == "progress"));
    let built = events.iter().find(|e| e["action"] == "built").unwrap();
    assert_eq!(built["status"]["status"]["counts"]["footprint"], 6);
    assert!(!dir.path().join("staging").exists());

    let status = call(&mut engine, "library.status", Value::Null);
    assert_eq!(status["installed"], true);
    assert_eq!(status["status"]["pending"], 3);
}

#[test]
fn searches_previews_decides_and_adds_to_a_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    call(&mut engine, "library.open", json!({"dir": dir.path()}));
    call(
        &mut engine,
        "library.build",
        json!({"sources": [source()], "wait": true}),
    );

    let footprint = find(&mut engine, "R_0805", "footprint");
    let details = call(&mut engine, "library.get", json!({"id": footprint}));
    assert_eq!(details["name"], "R_0805");
    assert_eq!(details["aliases"], json!(["Demo_Copy:R_0805"]));
    let facets = call(&mut engine, "library.facets", json!({"kind": "symbol"}));
    assert_eq!(facets.as_array().unwrap().len(), 2);

    // Previews come as scenes, like documents.
    let (reply, data) = engine.dispatch_binary(
        &json!({"method": "library.preview", "params": {"id": footprint}}).to_string(),
    );
    let reply: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(reply["result"]["kind"], "pcb");
    assert!(data.unwrap().len() > 100);
    let symbol = find(&mut engine, "LM_A", "symbol");
    let scene = call(&mut engine, "library.preview", json!({"id": symbol}));
    assert_eq!(scene["kind"], "schematic");
    assert!(scene["data"].as_str().unwrap().len() > 100);
    let model = find(&mut engine, "SOIC", "model");
    assert_eq!(
        error(&mut engine, "library.preview", json!({"id": model})),
        "invalidParams"
    );
    let mesh = call(&mut engine, "library.get", json!({"id": model}))["mesh"].clone();
    let cached = call(&mut engine, "library.mesh", json!({"hash": mesh}));
    assert_eq!(cached["exists"], true);

    // Review a near-duplicate group.
    let page = call(
        &mut engine,
        "library.groups",
        json!({"class": "near", "decision": "pending"}),
    );
    assert_eq!(page["total"], 3);
    let key = page["groups"][0]["key"].clone();
    let group = call(
        &mut engine,
        "library.decide",
        json!({"key": key, "decision": "separate"}),
    );
    assert_eq!(group["decision"], "separate");
    assert!(
        engine
            .take_events()
            .iter()
            .any(|e| e["action"] == "changed")
    );
    let report = call(&mut engine, "library.report", json!({"format": "markdown"}));
    assert!(report.as_str().unwrap().contains("Near duplicates"));

    // Add a symbol (with its default footprint, if any) to a project.
    let project = tempfile::tempdir().unwrap();
    call(
        &mut engine,
        "project.create",
        json!({"path": project.path().join("Demo"), "name": "Demo", "template": "board-2l"}),
    );
    let added = call(
        &mut engine,
        "library.add",
        json!({"ids": [symbol, footprint]}),
    );
    assert_eq!(added["symbols"], 1);
    assert_eq!(added["footprints"], 1);
    let again = call(&mut engine, "library.add", json!({"ids": [symbol]}));
    assert_eq!(again["symbols"], 0);
    assert_eq!(again["skipped"], 1);
}
