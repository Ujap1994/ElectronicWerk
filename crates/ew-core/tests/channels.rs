//! Channels through the engine API: the synthetic design with a sheet used
//! three times; `pcb.copyChannel` gives channels 2 and 3 the placement and
//! routing of channel 1 in one undo step.

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

fn unrouted(engine: &mut Engine, board: &Value) -> usize {
    let report = call(engine, "pcb.drc", json!({"doc": board}));
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"] == "unrouted")
        .count()
}

#[test]
fn the_layout_of_channel_1_goes_to_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let project = ew_synth::channels::generate(3, 7);
    let created = ew_format::store::create_project(&dir.path().join("Channels"), &project).unwrap();
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.open",
        json!({"path": created.root.join(&created.manifest_file)}),
    );
    let board = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "pcb")
        .unwrap()["id"]
        .clone();

    let channels = call(&mut engine, "pcb.channels", json!({"doc": board}));
    let channel = &channels[0];
    assert_eq!(channel["name"], "RC filter");
    let keys: Vec<String> = channel["instances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["key"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(keys.len(), 3);
    assert_eq!(channel["instances"][0]["name"], "/Channel 1/");
    let before = unrouted(&mut engine, &board);

    // Counted first, then copied.
    let counted = call(
        &mut engine,
        "pcb.copyChannel",
        json!({"doc": board, "sheet": channel["sheet"], "from": keys[0]}),
    );
    assert_eq!(counted["moved"], 4);
    assert_eq!(counted["tracks"], 4);
    let copied = call(
        &mut engine,
        "pcb.copyChannel",
        json!({"doc": board, "sheet": channel["sheet"], "from": keys[0], "apply": true}),
    );
    assert_eq!(copied["skipped"], json!([]));
    // The OUT nets of channels 2 and 3 are routed now.
    assert_eq!(unrouted(&mut engine, &board), before - 2);

    // R2 sits to C2 as R1 to C1.
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let at = |designator: &str| {
        let fp = pcb["footprints"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["designator"] == designator)
            .unwrap();
        (fp["at"][0].as_i64().unwrap(), fp["at"][1].as_i64().unwrap())
    };
    let (r1, c1, r2, c2) = (at("R1"), at("C1"), at("R2"), at("C2"));
    assert_eq!((c1.0 - r1.0, c1.1 - r1.1), (c2.0 - r2.0, c2.1 - r2.1));

    // One undo.
    call(&mut engine, "edit.undo", Value::Null);
    assert_eq!(unrouted(&mut engine, &board), before);
}
