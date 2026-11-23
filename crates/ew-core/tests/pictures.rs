//! Pictures and QR codes on a board (`pcb.addImage`, `pcb.addQrCode`).

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    clippy::cast_precision_loss,
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

fn open() -> (Engine, Value, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Pictures");
    ew_format::store::create_project(&path, &ew_synth::generate(ew_synth::DesignSize::S, 7))
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
    (engine, doc, dir)
}

/// Area (nm²) and bounds of filled polygons.
fn measure(shapes: &[&Value]) -> (f64, [i64; 4]) {
    let mut area = 0.0;
    let mut bounds = [i64::MAX, i64::MAX, i64::MIN, i64::MIN];
    for shape in shapes {
        let points: Vec<(i64, i64)> = shape["geometry"]["polygon"]["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p[0].as_i64().unwrap(), p[1].as_i64().unwrap()))
            .collect();
        let n = points.len();
        let twice: i128 = (0..n)
            .map(|i| {
                let (a, b) = (points[i], points[(i + 1) % n]);
                i128::from(a.0) * i128::from(b.1) - i128::from(b.0) * i128::from(a.1)
            })
            .sum();
        area += twice as f64 / 2.0;
        for (x, y) in points {
            bounds = [
                bounds[0].min(x),
                bounds[1].min(y),
                bounds[2].max(x),
                bounds[3].max(y),
            ];
        }
    }
    (area, bounds)
}

#[test]
fn a_picture_becomes_filled_polygons() {
    let (mut engine, doc, _dir) = open();
    let reply = call(
        &mut engine,
        "pcb.addImage",
        json!({
            "doc": doc,
            "layer": "silk.top",
            "rows": ["#####", "#...#", "#####", "....#"],
            "pixel": 500_000,
            "center": [10_000_000, 10_000_000],
        }),
    );
    // One region (a ring with a tail): one polygon, no group.
    assert_eq!(reply["shapes"], 1);
    assert_eq!(reply["group"], Value::Null);
    let board = call(&mut engine, "document.get", json!({"doc": doc}));
    let id = &reply["select"][0];
    let shape = board["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| &s["id"] == id)
        .unwrap();
    assert_eq!(shape["filled"], true);
    assert_eq!(shape["layer"], "silk.top");
    let (area, bounds) = measure(&[shape]);
    // 13 set pixels of 0.5 mm; counter-clockwise outline.
    assert!((area - 13.0 * 0.25e12).abs() < 1.0, "{area}");
    assert_eq!(bounds, [8_750_000, 9_000_000, 11_250_000, 11_000_000]);

    // Inverted: the 7 clear pixels (the hole and the corner) are drawn.
    let inverted = call(
        &mut engine,
        "pcb.addImage",
        json!({"doc": doc, "layer": "silk.top", "rows": ["#####", "#...#", "#####", "....#"],
               "pixel": 500_000, "center": [0, 0], "invert": true}),
    );
    assert_eq!(inverted["shapes"], 2);
    assert!(inverted["group"].is_string());
}

#[test]
fn a_qr_code_is_one_undo_step() {
    let (mut engine, doc, _dir) = open();
    let before = call(&mut engine, "document.get", json!({"doc": doc}));
    let count = |board: &Value| board["shapes"].as_array().map_or(0, Vec::len);
    let reply = call(
        &mut engine,
        "pcb.addQrCode",
        json!({
            "doc": doc,
            "layer": "silk.top",
            "text": "https://example.com",
            "size": 14_500_000,
            "center": [0, 0],
            "invert": true,
            "quiet": 2,
        }),
    );
    let made = reply["shapes"].as_u64().unwrap();
    assert!(made > 1);
    // Version 2 (25 modules) and two on each side: 29 modules of 0.5 mm.
    assert_eq!(
        (reply["width"].as_i64(), reply["height"].as_i64()),
        (Some(14_500_000), Some(14_500_000))
    );
    let board = call(&mut engine, "document.get", json!({"doc": doc}));
    assert_eq!(
        count(&board),
        count(&before) + usize::try_from(made).unwrap()
    );
    let ids: Vec<&Value> = reply["select"].as_array().unwrap().iter().collect();
    let shapes: Vec<&Value> = board["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| ids.contains(&&s["id"]))
        .collect();
    let (_, bounds) = measure(&shapes);
    // Inverted with a quiet zone: the drawing fills the whole square.
    assert_eq!(bounds, [-7_250_000, -7_250_000, 7_250_000, 7_250_000]);
    let groups = board["groups"].as_array().unwrap();
    assert!(
        groups
            .iter()
            .any(|g| g["members"].as_array().unwrap().len() == ids.len())
    );

    call(&mut engine, "edit.undo", Value::Null);
    let undone = call(&mut engine, "document.get", json!({"doc": doc}));
    assert_eq!(count(&undone), count(&before));
    assert!(undone.get("groups").is_none());

    let reply: Value = serde_json::from_str(&engine.dispatch(
        &json!({"method": "pcb.addQrCode", "params": {"doc": doc, "layer": "silk.top", "text": "x".repeat(4000), "size": 1, "center": [0, 0]}}).to_string(),
    ))
    .unwrap();
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("too long")
    );
}
