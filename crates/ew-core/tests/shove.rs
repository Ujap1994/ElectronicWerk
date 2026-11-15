//! Push and shove through the engine API: a route in `shove` mode pushes a
//! track of another net aside in the preview, and `pcb.addTracks` with the
//! same mode applies the route and the push in one step that keeps every
//! clearance.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    reason = "test helpers may panic and take owned JSON for brevity"
)]

use std::path::PathBuf;

use ew_core::Engine;
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

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

#[test]
fn a_route_pushes_a_track_aside_and_keeps_the_clearances() {
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
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let net = |name: &str| {
        pcb["nets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["name"] == name)
            .unwrap()["id"]
            .clone()
    };
    let (pushed, pushing) = (net("N$1"), net("N$3"));

    // A free track of N$1 across an empty corner of the board.
    call(
        &mut engine,
        "pcb.addTracks",
        json!({
            "doc": board, "net": pushed, "width": 250_000,
            "segments": [{"layer": "cu.top", "points": [[50 * MM, 8 * MM], [62 * MM, 8 * MM]]}],
        }),
    );

    // N$3 from below up to just under it.
    let request = json!({
        "doc": board, "layer": "cu.top", "net": pushing, "width": 250_000,
        "start": [56 * MM, 3 * MM], "target": [56 * MM, 7_800_000],
        "mode": "shove",
    });
    let route = call(&mut engine, "pcb.route", request);
    assert_eq!(route["reached"], true);
    assert_eq!(
        route["points"],
        json!([[56.0 * 1e6, 3.0 * 1e6], [56.0 * 1e6, 7.8e6]])
    );
    let preview = &route["shove"];
    assert_eq!(preview["hidden"].as_array().unwrap().len(), 1, "{route}");
    assert!(preview["tracks"].as_array().unwrap().len() >= 3);

    // Without shove mode the same request walks around and stops short.
    let around = call(
        &mut engine,
        "pcb.route",
        json!({
            "doc": board, "layer": "cu.top", "net": pushing, "width": 250_000,
            "start": [56 * MM, 3 * MM], "target": [56 * MM, 7_800_000],
        }),
    );
    assert!(around.get("shove").is_none());

    let added = call(
        &mut engine,
        "pcb.addTracks",
        json!({
            "doc": board, "net": pushing, "width": 250_000, "mode": "shove",
            "segments": [{"layer": "cu.top", "points": [[56 * MM, 3 * MM], [56 * MM, 7_800_000]]}],
        }),
    );
    assert!(added["seq"].is_number());

    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    let near: Vec<&Value> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"] == "clearance" || f["code"] == "short")
        .filter(|f| f["at"][0].as_i64().is_some_and(|x| x > 48 * MM))
        .collect();
    assert_eq!(near.len(), 0, "{near:#?}");

    // One step to undo: the pushed track is straight again.
    call(&mut engine, "edit.undo", Value::Null);
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let straight =
        pcb["tracks"].as_array().unwrap().iter().any(|t| {
            t["start"] == json!([50 * MM, 8 * MM]) && t["end"] == json!([62 * MM, 8 * MM])
        });
    assert!(straight);
}
