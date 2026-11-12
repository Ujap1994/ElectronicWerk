//! The board methods through the engine API: design rules check (full and
//! by region), accepted findings, zone filling with undo, the design rules
//! and the rule syntax check.

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

/// A new project from a template; returns the engine and the board.
fn project(dir: &std::path::Path, template: &str) -> (Engine, String) {
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": dir.join("Board"), "name": "Board", "template": template}),
    );
    let board = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "pcb")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    (engine, board)
}

const MM: i64 = 1_000_000;

fn codes(report: &Value) -> Vec<String> {
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn drc_findings_regions_and_exclusions() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, board) = project(dir.path(), "demo");
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(report["counts"]["errors"].is_u64());
    let before = report["findings"].as_array().unwrap().len();

    // A via of no net in the middle of nowhere dangles.
    let via = json!({
        "type": "via", "id": "00000000-0000-4000-8000-0000000000aa",
        "at": [-200 * MM, -200 * MM], "diameter": 600_000, "drill": 300_000
    });
    call(
        &mut engine,
        "edit.apply",
        json!({"changes": [{"op": "insert", "doc": board, "object": via}]}),
    );
    let region = [-201 * MM, -201 * MM, -199 * MM, -199 * MM];
    let online = call(
        &mut engine,
        "pcb.drc",
        json!({"doc": board, "region": region}),
    );
    let found: Vec<&Value> = online["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["targets"][0]["id"] == "00000000-0000-4000-8000-0000000000aa")
        .collect();
    let found_codes: Vec<&str> = found.iter().map(|f| f["code"].as_str().unwrap()).collect();
    assert!(found_codes.contains(&"dangling-via"), "{found_codes:?}");
    assert!(found_codes.contains(&"outside-board"), "{found_codes:?}");
    assert_eq!(
        online["findings"].as_array().unwrap().len(),
        before + found.len()
    );

    // Accept the dangling via.
    let key = found.iter().find(|f| f["code"] == "dangling-via").unwrap()["key"].clone();
    call(
        &mut engine,
        "pcb.exclude",
        json!({"doc": board, "keys": [key]}),
    );
    let full = call(&mut engine, "pcb.drc", json!({"doc": board}));
    let accepted = full["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"] == key)
        .unwrap();
    assert_eq!(accepted["excluded"], true);
    assert_eq!(full["counts"]["excluded"], 1);
    call(&mut engine, "edit.undo", Value::Null);
    let again = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert_eq!(again["counts"]["excluded"], 0);
}

#[test]
fn zones_fill_and_unfill_with_undo() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, board) = project(dir.path(), "board-2l");
    let zone = json!({
        "type": "zone", "id": "00000000-0000-4000-8000-0000000000bb",
        "name": "GND", "layers": ["cu.bottom"],
        "outline": [[-100 * MM, -100 * MM], [100 * MM, -100 * MM], [100 * MM, 100 * MM], [-100 * MM, 100 * MM]]
    });
    call(
        &mut engine,
        "edit.apply",
        json!({"changes": [{"op": "insert", "doc": board, "object": zone}]}),
    );
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(codes(&report).contains(&"zone-unfilled".to_owned()));
    let filled = call(&mut engine, "pcb.fillZones", json!({"doc": board}));
    assert_eq!(filled["zones"], 1);
    assert!(filled["areas"].as_u64().unwrap() >= 1);
    let object = call(&mut engine, "document.get", json!({"doc": board}));
    let zones = object["zones"].as_array().unwrap();
    let filled_zone = zones.iter().find(|z| z["name"] == "GND").unwrap();
    // A zone without a net keeps its islands and fills the board inside
    // its edge clearance.
    assert_ne!(filled_zone["filled"].as_array().unwrap().len(), 0);
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(!codes(&report).contains(&"zone-unfilled".to_owned()));
    // Filling again changes nothing.
    let again = call(&mut engine, "pcb.fillZones", json!({"doc": board}));
    assert_eq!(again["zones"], 0);
    call(&mut engine, "pcb.unfillZones", json!({"doc": board}));
    call(&mut engine, "edit.undo", Value::Null);
    let object = call(&mut engine, "document.get", json!({"doc": board}));
    let zone = &object["zones"].as_array().unwrap()[0];
    assert_ne!(
        zone["filled"].as_array().unwrap().len(),
        0,
        "undo restores the fill"
    );
}

#[test]
fn design_rules_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, board) = project(dir.path(), "demo");
    let info = call(&mut engine, "pcb.rules", json!({"doc": board}));
    assert!(info["doc"].is_string());
    assert!(
        info["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "clearance" && c["severity"] == "error")
    );
    let default_nets = info["nets"]["Default"].as_array().unwrap().len();
    assert!(default_nets > 0);
    let all_nets: usize = info["nets"]
        .as_object()
        .unwrap()
        .values()
        .map(|nets| nets.as_array().unwrap().len())
        .sum();

    let ok = call(
        &mut engine,
        "pcb.validateRule",
        json!({"condition": "A.netclass == 'Power' && B.type == 'via'"}),
    );
    assert_eq!(ok["ok"], true);
    let bad = call(
        &mut engine,
        "pcb.validateRule",
        json!({"condition": "A.colour == 1"}),
    );
    assert_eq!(bad["ok"], false);
    assert_eq!(bad["position"], 2);

    let mut props = info["rules"].clone();
    props["custom"] = json!([{
        "name": "wide",
        "condition": "A.type == 'track'",
        "constraints": {"trackWidthMin": 5_000_000}
    }]);
    props["drc"] = json!({"severities": {"dangling-via": "ignore"}});
    props["assignments"] = json!([{"pattern": "*", "class": "Power"}]);
    call(&mut engine, "pcb.setRules", json!({"props": props}));
    let info = call(&mut engine, "pcb.rules", json!({"doc": board}));
    assert_eq!(info["rules"]["custom"][0]["name"], "wide");
    assert_eq!(info["nets"]["Default"].as_array().unwrap().len(), 0);
    assert_eq!(info["nets"]["Power"].as_array().unwrap().len(), all_nets);
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    let widths = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"] == "track-width")
        .count();
    assert!(widths > 0, "every track is narrower than 5 mm");
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["args"]["source"] == "rule wide")
    );
}
