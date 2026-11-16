//! Stitching vias through the engine API: `pcb.stitch` fills a ground zone
//! with vias where another layer has ground copper too, puts a via fence
//! along tracks, keeps every rule, and removes the free vias again.

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

/// The errors of the DRC that new vias could cause.
fn errors(engine: &mut Engine, board: &Value) -> Vec<String> {
    let report = call(engine, "pcb.drc", json!({"doc": board}));
    let mut codes: Vec<String> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap_or("").to_owned())
        .filter(|code| {
            [
                "clearance",
                "short",
                "hole-to-hole",
                "copper-to-edge",
                "keepout",
            ]
            .contains(&code.as_str())
        })
        .collect();
    codes.sort();
    codes
}

#[test]
#[allow(clippy::too_many_lines, reason = "one scenario")]
fn zones_are_stitched_and_tracks_fenced() {
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
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let inner = pcb["zones"][0].clone();
    assert_eq!(inner["layers"], json!(["cu.in1"]));
    let gnd = inner["net"].clone();

    // A second ground zone on the bottom, filled.
    let bottom = json!({
        "type": "zone", "id": "55555555-5555-4555-8555-555555555555",
        "name": "GND bottom", "net": gnd, "layers": ["cu.bottom"],
        "outline": inner["outline"],
    });
    call(
        &mut engine,
        "edit.apply",
        json!({"label": "zone", "changes": [{"op": "insert", "doc": board, "object": bottom}]}),
    );
    call(&mut engine, "pcb.fillZones", json!({"doc": board}));
    let before = errors(&mut engine, &board);

    // A preview first: nothing changes.
    let preview = call(
        &mut engine,
        "pcb.stitch",
        json!({"doc": board, "zone": inner["id"], "pitch": 4_000_000.0}),
    );
    let count = preview["added"].as_u64().unwrap();
    assert!(count >= 10, "{preview}");
    let vias = |engine: &mut Engine| {
        call(engine, "document.get", json!({"doc": board}))["vias"]
            .as_array()
            .unwrap()
            .len()
    };
    let initial = vias(&mut engine);

    let stitched = call(
        &mut engine,
        "pcb.stitch",
        json!({"doc": board, "zone": inner["id"], "pitch": 4_000_000.0, "staggered": true, "apply": true}),
    );
    let added = stitched["added"].as_u64().unwrap();
    assert!(added >= 10, "{stitched}");
    assert_eq!(vias(&mut engine) as u64, initial as u64 + added);
    assert_eq!(errors(&mut engine, &board), before);

    // A fence of ground vias along the /OUT tracks.
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let out = pcb["nets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "/OUT")
        .unwrap()["id"]
        .clone();
    let tracks: Vec<Value> = pcb["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["net"] == out && t["layer"] == "cu.top")
        .map(|t| t["id"].clone())
        .collect();
    let fenced = call(
        &mut engine,
        "pcb.stitch",
        json!({"doc": board, "tracks": tracks, "net": gnd, "pitch": 1_500_000.0, "apply": true}),
    );
    assert!(fenced["added"].as_u64().unwrap() >= 4, "{fenced}");
    assert_eq!(errors(&mut engine, &board), before);

    // Removing the stitching of the zone leaves the fence outside it.
    let removed = call(
        &mut engine,
        "pcb.stitch",
        json!({"doc": board, "zone": inner["id"], "remove": true, "apply": true}),
    );
    assert!(removed["removed"].as_u64().unwrap() >= added);
    assert_eq!(removed["added"], 0);
    call(&mut engine, "edit.undo", Value::Null);
    call(&mut engine, "edit.undo", Value::Null);
    call(&mut engine, "edit.undo", Value::Null);
    assert_eq!(vias(&mut engine), initial);
}
