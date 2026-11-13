//! Output jobs through the engine API: presets, creating a job, running it
//! into the project folder, the BOM preview and the Gerber viewer scene.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "test helpers"
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

fn error(engine: &mut Engine, method: &str, params: Value) -> Value {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    reply["error"].clone()
}

#[test]
fn jobs_run_into_the_project_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": dir.path().join("Demo"), "name": "Demo", "template": "demo"}),
    );
    let job = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "outputJob")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let presets = call(&mut engine, "outputs.presets", json!({}));
    let ids: Vec<&str> = presets
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "generic",
            "jlcpcb",
            "pcbway",
            "aisler",
            "eurocircuits",
            "oshpark"
        ]
    );

    // The template's job is the generic preset; run it.
    let report = call(&mut engine, "outputs.run", json!({"job": job}));
    for result in report["results"].as_array().unwrap() {
        assert_eq!(result["status"], "done", "{result}");
    }
    let directory = std::path::PathBuf::from(report["directory"].as_str().unwrap());
    assert!(directory.starts_with(dir.path()));
    let paths: Vec<&str> = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    for path in &paths {
        assert!(directory.join(path).is_file(), "{path}");
    }
    assert!(paths.iter().any(|p| p.ends_with("-fabrication.zip")));

    // The Gerber viewer shows the generated files.
    let files = call(&mut engine, "outputs.files", json!({"job": job}));
    let list: Vec<String> = files["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap().to_owned())
        .collect();
    assert!(list.len() >= 10, "{list:?}");
    let scene = call(
        &mut engine,
        "gerber.scene",
        json!({"paths": list, "name": "Demo", "encoding": "none"}),
    );
    assert!(scene["layers"].as_array().unwrap().len() >= 10);
    assert!(
        scene["layers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["key"] == "cu.top")
    );
    assert_eq!(
        scene["problems"].as_array().unwrap().len(),
        0,
        "{}",
        scene["problems"]
    );

    // A second job from a preset, with undo.
    let created = call(
        &mut engine,
        "outputs.create",
        json!({"name": "JLCPCB", "preset": "jlcpcb"}),
    );
    let second = created["id"].as_str().unwrap().to_owned();
    let doc = call(&mut engine, "document.get", json!({"doc": second}));
    assert_eq!(doc["outputs"].as_array().unwrap().len(), 5);
    call(
        &mut engine,
        "outputs.applyPreset",
        json!({"job": second, "preset": "oshpark"}),
    );
    let doc = call(&mut engine, "document.get", json!({"doc": second}));
    assert_eq!(doc["outputs"].as_array().unwrap().len(), 3);
    call(&mut engine, "edit.undo", json!({}));
    let doc = call(&mut engine, "document.get", json!({"doc": second}));
    assert_eq!(doc["outputs"].as_array().unwrap().len(), 5);
    let bad = error(
        &mut engine,
        "outputs.applyPreset",
        json!({"job": second, "preset": "nope"}),
    );
    assert_eq!(bad["code"], "invalidParams");

    // The BOM preview with fields to choose from.
    let bom = call(&mut engine, "outputs.bom", json!({}));
    assert!(bom["rows"].as_array().unwrap().len() > 2);
    assert!(bom["fields"].as_array().unwrap().iter().any(|f| f == "MPN"));
    assert_eq!(bom["headers"][0], "Item");
}
