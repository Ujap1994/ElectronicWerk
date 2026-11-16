//! Lengths through the engine API: `pcb.lengths` reports the skew of a
//! differential pair and the lengths of a length group, the DRC reports
//! both, `pcb.tuneLength` previews meanders on one track, and
//! `pcb.matchLengths` makes every net long enough in one step that keeps
//! the clearances.

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

#[allow(clippy::cast_possible_truncation, reason = "board coordinates")]
fn mm(x: f64, y: f64) -> Value {
    json!([(x * 1e6).round() as i64, (y * 1e6).round() as i64])
}

fn codes(engine: &mut Engine, board: &Value, code: &str) -> Vec<Value> {
    let report = call(engine, "pcb.drc", json!({"doc": board}));
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"] == code)
        .cloned()
        .collect()
}

#[test]
#[allow(clippy::too_many_lines, reason = "one scenario from rules to DRC")]
fn lengths_are_reported_checked_and_matched() {
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

    // A USB pair 2 mm apart in length, and a bus of two nets.
    let nets = [
        ("11111111-1111-4111-8111-111111111111", "USB_DP", 4.0, 60.0),
        ("22222222-2222-4222-8222-222222222222", "USB_DN", 4.5, 58.0),
        ("33333333-3333-4333-8333-333333333333", "D0", 9.0, 58.0),
        ("44444444-4444-4444-8444-444444444444", "D1", 11.0, 56.0),
    ];
    let changes: Vec<Value> = nets
        .iter()
        .map(|(id, name, ..)| {
            json!({"op": "insert", "doc": board, "object": {"type": "net", "id": id, "name": name}})
        })
        .collect();
    call(
        &mut engine,
        "edit.apply",
        json!({"label": "nets", "changes": changes}),
    );
    for (id, _, y, end) in nets {
        call(
            &mut engine,
            "pcb.addTracks",
            json!({
                "doc": board, "net": id, "width": 200_000,
                "segments": [{"layer": "cu.top", "points": [mm(46.0, y), mm(end, y)]}],
            }),
        );
    }

    // The rules: at most 0.1 mm skew for USB, D0 and D1 within 0.2 mm.
    let mut rules = call(&mut engine, "pcb.rules", json!({"doc": board}))["rules"].clone();
    let mut usb = rules["netClasses"][0].clone();
    usb["name"] = json!("USB");
    usb["maxSkew"] = json!(100_000);
    rules["netClasses"].as_array_mut().unwrap().push(usb);
    rules["assignments"] = json!([{"pattern": "USB_D?", "class": "USB"}]);
    rules["lengthGroups"] = json!([{"name": "Bus", "nets": "D0, D1", "tolerance": 200_000}]);
    call(&mut engine, "pcb.setRules", json!({"props": rules}));

    let lengths = call(&mut engine, "pcb.lengths", json!({"doc": board}));
    let pair = &lengths["pairs"][0];
    assert_eq!(pair["name"], "USB_D");
    assert!((pair["skew"].as_f64().unwrap() - 2e6).abs() < 1.0);
    assert_eq!(pair["maxSkew"], 100_000.0);
    assert_eq!(pair["ok"], false);
    let group = &lengths["groups"][0];
    assert_eq!(group["name"], "Bus");
    assert_eq!(group["nets"][0]["name"], "D0");
    assert_eq!(group["ok"], false);
    assert_eq!(codes(&mut engine, &board, "pair-skew").len(), 1);
    let short = codes(&mut engine, &board, "length-match");
    assert_eq!(short.len(), 1);
    assert_eq!(short[0]["args"]["net"], "D1");

    // Meanders on one track, as a preview: nothing changes yet.
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let dn = pcb["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["net"] == nets[1].0)
        .unwrap()["id"]
        .clone();
    let preview = call(
        &mut engine,
        "pcb.tuneLength",
        json!({"doc": board, "track": dn, "extra": 2_000_000.0}),
    );
    assert_eq!(preview["reached"], true, "{preview}");
    assert_eq!(preview["hidden"], json!([dn]));
    assert!(preview["tracks"].as_array().unwrap().len() > 8);
    assert!((preview["length"].as_f64().unwrap() - 14e6).abs() < 10.0);
    assert_eq!(codes(&mut engine, &board, "pair-skew").len(), 1);

    // Matching all at once: one step, the rules are kept.
    let matched = call(
        &mut engine,
        "pcb.matchLengths",
        json!({"doc": board, "apply": true}),
    );
    let names: Vec<&str> = matched["nets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["USB_DN", "D1"]);
    assert!(
        matched["nets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|n| n["reached"] == true),
        "{matched}"
    );
    let lengths = call(&mut engine, "pcb.lengths", json!({"doc": board}));
    assert_eq!(lengths["pairs"][0]["ok"], true, "{lengths}");
    assert!(lengths["pairs"][0]["skew"].as_f64().unwrap() < 1_000.0);
    assert_eq!(lengths["groups"][0]["ok"], true);
    assert_eq!(codes(&mut engine, &board, "pair-skew").len(), 0);
    assert_eq!(codes(&mut engine, &board, "length-match").len(), 0);
    let near: Vec<Value> = ["clearance", "short"]
        .iter()
        .flat_map(|code| codes(&mut engine, &board, code))
        .filter(|f| f["at"][0].as_i64().is_some_and(|x| x > 45 * MM))
        .collect();
    assert_eq!(near.len(), 0, "{near:#?}");

    // One undo takes all meanders back.
    call(&mut engine, "edit.undo", Value::Null);
    let lengths = call(&mut engine, "pcb.lengths", json!({"doc": board}));
    assert_eq!(lengths["pairs"][0]["ok"], false);
}
