//! Assembly variants: kept in the project, applied to the bill of materials
//! preview and to the outputs of a job made for a variant.

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

/// The text of the BOM preview cells, row by row.
fn rows(bom: &Value) -> Vec<String> {
    bom["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .map(|c| c.as_str().map_or_else(|| c.to_string(), str::to_owned))
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect()
}

#[test]
#[allow(clippy::too_many_lines, reason = "one scenario")]
fn a_variant_changes_the_bill_of_materials_and_the_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": dir.path().join("Var"), "name": "Var", "template": "demo"}),
    );
    let board = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "pcb")
        .unwrap()["id"]
        .clone();
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let mut designators: Vec<String> = pcb["footprints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["designator"].as_str().unwrap().to_owned())
        .filter(|d| d.starts_with('R') || d.starts_with('C'))
        .collect();
    designators.sort();
    let (first, second) = (designators[0].clone(), designators[1].clone());

    // A variant without the first part and with another value for the second.
    call(
        &mut engine,
        "project.setProperties",
        json!({"variants": [{
            "name": "Lite", "description": "Without the first part",
            "parts": {
                first.clone(): {"fitted": false},
                second.clone(): {"value": "4k7", "properties": {"MPN": "XYZ-47"}},
            },
        }]}),
    );
    let info = call(&mut engine, "project.info", json!({}));
    assert_eq!(info["variants"][0]["name"], "Lite");

    let options = json!({
        "columns": [{"field": "designators"}, {"field": "value"}, {"field": "dnp"}, {"field": "MPN"}],
        "groupBy": [], "includeDnp": true,
    });
    let designed = rows(&call(
        &mut engine,
        "outputs.bom",
        json!({"options": options}),
    ));
    let lite = rows(&call(
        &mut engine,
        "outputs.bom",
        json!({"options": options, "variant": "Lite"}),
    ));
    let row_of = |rows: &[String], designator: &str| {
        rows.iter()
            .find(|r| r.split('|').any(|c| c == designator))
            .cloned()
            .unwrap()
    };
    assert!(!row_of(&designed, &first).contains("DNP"));
    assert!(row_of(&lite, &first).contains("DNP"), "{lite:?}");
    assert!(row_of(&lite, &second).contains("4k7"));
    assert!(row_of(&lite, &second).contains("XYZ-47"));

    // A job for the variant: its placement file leaves the first part out.
    let job = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "outputJob")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    call(
        &mut engine,
        "document.setProperties",
        json!({"doc": job, "props": {
            "variant": "Lite",
            "outputs": [
                {"id": "00000000-0000-4000-8000-0000000000b1", "name": "BOM", "kind": "bom"},
                {"id": "00000000-0000-4000-8000-0000000000b2", "name": "Placement", "kind": "pickAndPlace"},
            ],
        }}),
    );
    let report = call(&mut engine, "outputs.run", json!({"job": job}));
    assert!(
        report["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["status"] == "done"),
        "{report}"
    );
    let directory = std::path::PathBuf::from(report["directory"].as_str().unwrap());
    let placement: String = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .filter(|p| {
            std::path::Path::new(p)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("csv"))
        })
        .filter_map(|p| std::fs::read_to_string(directory.join(p)).ok())
        .collect();
    assert!(placement.contains(&second), "{placement}");
    let placed_first = placement.lines().any(|l| {
        l.split([',', ';', '\t', ' '])
            .any(|c| c.trim_matches('"') == first)
            && !l.contains("DNP")
    });
    assert!(!placed_first, "{placement}");

    // A job for a variant the project does not have fails clearly.
    call(
        &mut engine,
        "document.setProperties",
        json!({"doc": job, "props": {"variant": "Nope"}}),
    );
    let report = call(&mut engine, "outputs.run", json!({"job": job}));
    assert_eq!(report["results"][0]["status"], "failed");
}
