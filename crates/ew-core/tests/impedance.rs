//! The impedance view through the engine API: `pcb.impedance` computes with
//! the saved rules or with rules that are still being edited, and
//! `pcb.impedanceCalc` gives the impedance of any width on a layer.

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

#[test]
fn edited_rules_are_computed_before_they_are_saved() {
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

    // The saved rules have no profile; the interfaces are offered anyway.
    let saved = call(&mut engine, "pcb.impedance", json!({"doc": board}));
    assert_eq!(saved["profiles"], json!([]));
    assert!(
        saved["presets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "usb2" && p["target"] == 90)
    );
    let top = &saved["layers"][0];
    assert_eq!(top["layer"], "cu.top");
    assert_eq!(top["structure"]["kind"], "microstrip");
    assert!(top["structure"]["epsilonR"].as_f64().unwrap() > 3.0);

    // A profile that exists only in the dialog.
    let mut rules = call(&mut engine, "pcb.rules", json!({"doc": board}))["rules"].clone();
    rules["impedanceProfiles"] = json!([
        {"name": "RF", "target": 50, "tolerance": 10, "preset": "rf50"},
    ]);
    rules["netClasses"][0]["impedance"] = json!("RF");
    let edited = call(
        &mut engine,
        "pcb.impedance",
        json!({"doc": board, "rules": rules}),
    );
    let layout = &edited["profiles"][0]["layers"][0]["layout"];
    let width = layout["width"].as_f64().unwrap();
    let ohms = layout["impedance"].as_f64().unwrap();
    assert!((45.0..55.0).contains(&ohms), "{layout}");
    assert_eq!(layout["manual"], false);

    // The calculator agrees with the profile's width.
    let calc = call(
        &mut engine,
        "pcb.impedanceCalc",
        json!({"doc": board, "layer": "cu.top", "width": width}),
    );
    assert!((calc["impedance"].as_f64().unwrap() - ohms).abs() < 1e-6);

    // Nothing was saved.
    let again = call(&mut engine, "pcb.impedance", json!({"doc": board}));
    assert_eq!(again["profiles"], json!([]));

    // Saved, a route of the class gets the profile's width on each layer.
    call(&mut engine, "pcb.setRules", json!({"props": rules}));
    let start = call(
        &mut engine,
        "pcb.routeStart",
        json!({"doc": board, "at": [0, 0], "layer": "cu.top"}),
    );
    assert_eq!(start["profile"], "RF");
    let widths = start["layerWidths"].as_object().unwrap();
    assert!((widths["cu.top"].as_f64().unwrap() - width).abs() < 1.0);
    assert!(widths.contains_key("cu.bottom"));

    // Each segment of a route may have its own width.
    let net = call(&mut engine, "document.get", json!({"doc": board}))["nets"][0]["id"].clone();
    call(
        &mut engine,
        "pcb.addTracks",
        json!({
            "doc": board, "net": net, "width": 250_000,
            "segments": [
                {"layer": "cu.top", "points": [[0, 0], [1_000_000, 0]], "width": 300_000},
                {"layer": "cu.bottom", "points": [[1_000_000, 0], [2_000_000, 0]]},
            ],
        }),
    );
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let width_at = |x: i64| {
        pcb["tracks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["start"] == json!([x, 0]))
            .unwrap()["width"]
            .clone()
    };
    assert_eq!(width_at(0), 300_000);
    assert_eq!(width_at(1_000_000), 250_000);
}
