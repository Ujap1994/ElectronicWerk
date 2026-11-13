//! The 3D view and the 3D exports through the engine API: the scene of the
//! demo board, the parts for live updates, STEP/GLB files and a 3D output
//! in an output job.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
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

fn doc_of(snapshot: &Value, kind: &str) -> String {
    snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == kind)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn the_demo_board_in_3d() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": dir.path().join("Demo"), "name": "Demo", "template": "demo"}),
    );
    let board = doc_of(&snapshot, "pcb");

    // The scene: meshes in a binary buffer (base64 for JSON clients).
    let scene = call(&mut engine, "board3d.scene", json!({"doc": board}));
    let keys: Vec<&str> = scene["meshes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["key"].as_str().unwrap())
        .collect();
    assert!(keys.contains(&"body"), "{keys:?}");
    assert!(keys.contains(&"copper:top"), "{keys:?}");
    assert!(keys.contains(&"mask:top"), "{keys:?}");
    assert!(scene["data"].as_str().is_some_and(|d| !d.is_empty()));
    let parts = scene["parts"].as_array().unwrap();
    assert!(!parts.is_empty());
    // Every part has a placement matrix; models are listed with where they
    // were found.
    assert!(
        parts
            .iter()
            .all(|p| p["matrix"].as_array().unwrap().len() == 16)
    );
    assert!(scene["models"].is_array());
    assert!(scene["board"]["levels"]["bodyTop"].as_f64().unwrap() > 1.0);

    // The fast path for live updates.
    let quick = call(&mut engine, "board3d.parts", json!({"doc": board}));
    assert_eq!(quick["parts"].as_array().unwrap().len(), parts.len());

    // Exports.
    for (format, magic) in [("step", b"ISO-10303-21".as_slice()), ("glb", b"glTF")] {
        let path = dir.path().join(format!("demo.{format}"));
        let report = call(
            &mut engine,
            "board3d.export",
            json!({"doc": board, "path": path, "format": format, "origin": "center"}),
        );
        assert!(report["bytes"].as_u64().unwrap() > 1000, "{report}");
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(magic), "{format}");
    }
    let relative: Value = serde_json::from_str(
        &engine.dispatch(
            &json!({"method": "board3d.export", "params": {"doc": board, "path": "x.step"}})
                .to_string(),
        ),
    )
    .unwrap();
    assert!(
        relative.get("error").is_some(),
        "relative paths are refused"
    );

    // A 3D output in the template's output job.
    let job = doc_of(&snapshot, "outputJob");
    let document = call(&mut engine, "document.get", json!({"doc": job}));
    let mut outputs = document["outputs"].as_array().cloned().unwrap_or_default();
    outputs.push(json!({
        "id": "00000000-0000-4000-8000-000000000301",
        "name": "3D model",
        "kind": "step",
        "options": {"format": "step"}
    }));
    call(
        &mut engine,
        "document.setProperties",
        json!({"doc": job, "props": {"outputs": outputs}}),
    );
    let report = call(
        &mut engine,
        "outputs.run",
        json!({"job": job, "outputs": ["00000000-0000-4000-8000-000000000301"]}),
    );
    let result = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "step")
        .unwrap();
    assert_eq!(result["status"], "done", "{result}");
    let file = result["files"][0].as_str().unwrap();
    assert!(file.starts_with("3d/") && file.ends_with(".step"), "{file}");
    let directory = report["directory"].as_str().unwrap();
    let text = std::fs::read_to_string(std::path::Path::new(directory).join(file)).unwrap();
    assert!(text.contains("MANIFOLD_SOLID_BREP"));
}
