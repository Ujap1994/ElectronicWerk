//! End-to-end tests of the engine API: projects, edits, undo/redo, saving,
//! the autosave journal and crash recovery.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    reason = "test helpers may panic and take owned JSON for brevity"
)]

use std::fs;
use std::path::Path;

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

fn call_err(engine: &mut Engine, method: &str, params: Value) -> String {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    reply["error"]["code"].as_str().unwrap_or("none").to_owned()
}

fn create(engine: &mut Engine, path: &Path, template: &str) -> Value {
    call(
        engine,
        "project.create",
        json!({"path": path, "name": "Demo", "template": template}),
    )
}

fn root_doc(snapshot: &Value) -> String {
    snapshot["rootSchematic"].as_str().unwrap().to_owned()
}

fn add_junctions(engine: &mut Engine, doc: &str, count: usize) {
    for index in 0..count {
        call(
            engine,
            "edit.apply",
            json!({
                "label": format!("junction-{index}"),
                "changes": [{"op": "insert", "doc": doc, "object": {"type": "junction", "at": [index * 1000, 0]}}]
            }),
        );
    }
}

#[test]
fn create_edit_save_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("demo");
    let mut engine = Engine::new();
    let snapshot = create(&mut engine, &path, "board-2l");
    assert_eq!(snapshot["name"], "Demo");
    assert_eq!(snapshot["documents"].as_array().unwrap().len(), 5);
    assert!(!snapshot["history"]["dirty"].as_bool().unwrap());
    let events = engine.take_events();
    assert_eq!(events[0]["action"], "opened");

    let history = call(
        &mut engine,
        "project.setProperties",
        json!({"revision": "B", "variables": {"AUTHOR": "Lab"}}),
    );
    assert!(history["dirty"].as_bool().unwrap());
    assert_eq!(history["undoLabel"], "set-project-properties");
    let resolved = call(
        &mut engine,
        "project.resolveText",
        json!({"text": "${PROJECT_NAME} rev ${REVISION} by ${AUTHOR} on ${SHEET_NAME}", "doc": root_doc(&snapshot)}),
    );
    assert_eq!(resolved["text"], "Demo rev B by Lab on Root");

    let added = call(
        &mut engine,
        "document.add",
        json!({"kind": "schematic", "name": "Power Supply", "parent": root_doc(&snapshot)}),
    );
    assert_eq!(added["path"], "schematics/power-supply.ewsch");
    let info = call(&mut engine, "project.info", Value::Null);
    let root = info["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| document["id"] == info["rootSchematic"])
        .unwrap()
        .clone();
    assert_eq!(root["stats"]["children"][0], added["id"]);
    assert!(root["dirty"].as_bool().unwrap());

    let saved = call(&mut engine, "project.save", Value::Null);
    assert!(!saved["history"]["dirty"].as_bool().unwrap());
    assert!(saved["written"].as_array().unwrap().len() >= 3);
    assert!(path.join("schematics/power-supply.ewsch").is_file());
    assert!(!path.join(".ewerk/journal.jsonl").exists());

    // Saving again writes nothing.
    let again = call(&mut engine, "project.save", Value::Null);
    assert_eq!(again["written"], json!([]));

    call(&mut engine, "project.close", Value::Null);
    let reopened = call(&mut engine, "project.open", json!({"path": path}));
    assert_eq!(reopened["revision"], "B");
    assert_eq!(reopened["variables"]["AUTHOR"], "Lab");
    assert_eq!(reopened["documents"].as_array().unwrap().len(), 6);
    let report = call(&mut engine, "project.validate", Value::Null);
    assert_eq!(report["errors"], 0, "{report}");
}

#[test]
fn undo_redo_and_dirty_tracking() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::new();
    let snapshot = create(&mut engine, &dir.path().join("p"), "empty");
    let doc = root_doc(&snapshot);
    add_junctions(&mut engine, &doc, 3);
    call(&mut engine, "project.save", Value::Null);

    let undo = call(&mut engine, "edit.undo", Value::Null);
    assert_eq!(undo["label"], "junction-2");
    assert!(undo["history"]["dirty"].as_bool().unwrap());
    let redo = call(&mut engine, "edit.redo", Value::Null);
    assert!(
        !redo["history"]["dirty"].as_bool().unwrap(),
        "back at the saved state"
    );

    let history = call(&mut engine, "edit.history", Value::Null);
    assert_eq!(
        history["undo"],
        json!(["junction-2", "junction-1", "junction-0"])
    );
    for _ in 0..3 {
        call(&mut engine, "edit.undo", Value::Null);
    }
    let nothing = call(&mut engine, "edit.undo", Value::Null);
    assert_eq!(nothing["applied"], false);
    let document = call(&mut engine, "document.get", json!({"doc": doc}));
    assert!(document.get("junctions").is_none());
    let events = engine.take_events();
    assert!(
        events
            .iter()
            .any(|event| event["changeSet"]["cause"] == "undo")
    );

    assert_eq!(
        call_err(&mut engine, "project.close", Value::Null),
        "unsavedChanges"
    );
    call(&mut engine, "project.close", json!({"discard": true}));
    assert!(
        dir.path()
            .join("p/.ewerk/journal.discarded.jsonl")
            .is_file()
    );
}

#[test]
fn transactions_are_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::new();
    let snapshot = create(&mut engine, &dir.path().join("p"), "empty");
    let doc = root_doc(&snapshot);
    let id = "00000000-0000-4000-8000-000000000001";
    let error = call_err(
        &mut engine,
        "edit.apply",
        json!({"changes": [
            {"op": "insert", "doc": doc, "object": {"type": "junction", "id": id, "at": [0, 0]}},
            {"op": "insert", "doc": doc, "object": {"type": "junction", "id": id, "at": [1, 1]}}
        ]}),
    );
    assert_eq!(error, "conflict");
    let document = call(&mut engine, "document.get", json!({"doc": doc}));
    assert!(
        document.get("junctions").is_none(),
        "the first insert was rolled back"
    );
    assert!(
        !call(&mut engine, "project.info", Value::Null)["history"]["dirty"]
            .as_bool()
            .unwrap()
    );

    // Patch, update and delete.
    call(
        &mut engine,
        "edit.apply",
        json!({"changes": [{"op": "insert", "doc": doc, "object": {"type": "wire", "id": id, "start": [0, 0], "end": [10, 0]}}]}),
    );
    call(
        &mut engine,
        "edit.apply",
        json!({"changes": [{"op": "patch", "doc": doc, "id": id, "fields": {"end": [20, 5]}}]}),
    );
    let document = call(&mut engine, "document.get", json!({"doc": doc}));
    assert_eq!(document["wires"][0]["end"], json!([20, 5]));
    assert_eq!(
        call_err(
            &mut engine,
            "edit.apply",
            json!({"changes": [{"op": "patch", "doc": doc, "id": id, "fields": {"type": "bus"}}]})
        ),
        "invalidParams"
    );
    call(
        &mut engine,
        "edit.apply",
        json!({"changes": [{"op": "delete", "doc": doc, "id": id}]}),
    );
    assert_eq!(
        call_err(
            &mut engine,
            "edit.apply",
            json!({"changes": [{"op": "delete", "doc": doc, "id": id}]})
        ),
        "notFound"
    );
}

#[test]
fn documents_can_be_renamed_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p");
    let mut engine = Engine::new();
    let snapshot = create(&mut engine, &path, "board-2l");
    let root = root_doc(&snapshot);
    let child = call(
        &mut engine,
        "document.add",
        json!({"kind": "schematic", "name": "Child", "parent": root}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    call(&mut engine, "project.save", Value::Null);
    call(
        &mut engine,
        "document.rename",
        json!({"doc": child, "name": "Interface"}),
    );
    assert_eq!(
        call_err(
            &mut engine,
            "document.rename",
            json!({"doc": child, "name": "  "})
        ),
        "invalidParams"
    );
    call(&mut engine, "document.remove", json!({"doc": child}));
    let info = call(&mut engine, "project.info", Value::Null);
    assert_eq!(info["documents"].as_array().unwrap().len(), 5);
    let root_document = call(&mut engine, "document.get", json!({"doc": root}));
    assert!(
        root_document.get("sheets").is_none(),
        "the sheet symbol went with it"
    );
    call(&mut engine, "project.save", Value::Null);
    assert!(!path.join("schematics/child.ewsch").exists());

    // Undo brings the document and its sheet symbol back; saving restores the file.
    call(&mut engine, "edit.undo", Value::Null);
    call(&mut engine, "project.save", Value::Null);
    assert!(path.join("schematics/child.ewsch").exists());
    let props = call(&mut engine, "document.properties", json!({"doc": child}));
    assert_eq!(props["name"], "Interface");
    assert_eq!(props["docKind"], "schematic");

    // Removing the root sheet clears the root reference.
    call(&mut engine, "document.remove", json!({"doc": root}));
    assert!(call(&mut engine, "project.info", Value::Null)["rootSchematic"].is_null());
}

#[test]
fn crash_recovery_restores_work_and_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p");
    let expected;
    let expected_history;
    {
        let mut engine = Engine::new();
        let snapshot = create(&mut engine, &path, "empty");
        let doc = root_doc(&snapshot);
        add_junctions(&mut engine, &doc, 4);
        call(&mut engine, "edit.undo", Value::Null);
        call(&mut engine, "edit.undo", Value::Null);
        call(&mut engine, "edit.redo", Value::Null);
        call(
            &mut engine,
            "project.setProperties",
            json!({"revision": "X"}),
        );
        expected = call(&mut engine, "document.get", json!({"doc": doc}));
        expected_history = call(&mut engine, "edit.history", Value::Null);
        // The engine "crashes": it is dropped without saving or closing.
    }
    assert!(path.join(".ewerk/journal.jsonl").is_file());

    let mut engine = Engine::new();
    let probe = call(&mut engine, "project.probe", json!({"path": path}));
    assert_eq!(probe["hasRecovery"], true);
    let opened = call(&mut engine, "project.open", json!({"path": path}));
    assert_eq!(opened["recovery"]["commits"], 5);
    assert_eq!(opened["recovery"]["matchesFiles"], true);
    assert_eq!(opened["revision"], "A");
    assert_eq!(
        call_err(
            &mut engine,
            "project.setProperties",
            json!({"revision": "Y"})
        ),
        "recoveryPending"
    );
    assert_eq!(
        call_err(&mut engine, "project.save", Value::Null),
        "recoveryPending"
    );

    let recovered = call(&mut engine, "project.recover", Value::Null);
    assert_eq!(recovered["revision"], "X");
    assert!(recovered["recovery"].is_null());
    assert!(recovered["history"]["dirty"].as_bool().unwrap());
    let doc = root_doc(&recovered);
    assert_eq!(
        call(&mut engine, "document.get", json!({"doc": doc})),
        expected
    );
    let history = call(&mut engine, "edit.history", Value::Null);
    assert_eq!(history["undo"], expected_history["undo"]);
    assert_eq!(history["redo"], expected_history["redo"]);

    // Work continues in the same journal; a second crash recovers both parts.
    add_junctions(&mut engine, &doc, 1);
    let expected = call(&mut engine, "document.get", json!({"doc": doc}));
    drop(engine);
    let mut engine = Engine::new();
    call(&mut engine, "project.open", json!({"path": path}));
    call(&mut engine, "project.recover", Value::Null);
    assert_eq!(
        call(&mut engine, "document.get", json!({"doc": doc})),
        expected
    );
    call(&mut engine, "project.save", Value::Null);
    assert!(!path.join(".ewerk/journal.jsonl").exists());
}

#[test]
fn recovery_can_be_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p");
    {
        let mut engine = Engine::new();
        let snapshot = create(&mut engine, &path, "empty");
        add_junctions(&mut engine, &root_doc(&snapshot), 2);
    }
    let mut engine = Engine::new();
    call(&mut engine, "project.open", json!({"path": path}));
    let snapshot = call(&mut engine, "project.discardRecovery", Value::Null);
    assert!(snapshot["recovery"].is_null());
    assert_eq!(
        call_err(&mut engine, "project.recover", Value::Null),
        "conflict"
    );
    assert!(path.join(".ewerk/journal.discarded.jsonl").is_file());
    call(&mut engine, "project.close", Value::Null);
    let probe = call(&mut engine, "project.probe", json!({"path": path}));
    assert_eq!(probe["hasRecovery"], false);
}

#[test]
fn recovery_that_no_longer_fits_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p");
    let doc;
    {
        let mut engine = Engine::new();
        let snapshot = create(&mut engine, &path, "empty");
        doc = root_doc(&snapshot);
        call(
            &mut engine,
            "edit.apply",
            json!({"changes": [{"op": "insert", "doc": doc, "object": {"type": "junction", "id": "00000000-0000-4000-8000-0000000000aa", "at": [0, 0]}}]}),
        );
    }
    // Someone else adds the same object to the saved file in the meantime.
    {
        let mut engine = Engine::without_journal();
        call(&mut engine, "project.open", json!({"path": path}));
        call(
            &mut engine,
            "edit.apply",
            json!({"changes": [{"op": "insert", "doc": doc, "object": {"type": "junction", "id": "00000000-0000-4000-8000-0000000000aa", "at": [5, 5]}}]}),
        );
        call(&mut engine, "project.save", Value::Null);
    }
    let mut engine = Engine::new();
    let opened = call(&mut engine, "project.open", json!({"path": path}));
    assert_eq!(opened["recovery"]["matchesFiles"], false);
    assert_eq!(
        call_err(&mut engine, "project.recover", Value::Null),
        "conflict"
    );
    // Nothing changed and the decision is still pending.
    let info = call(&mut engine, "project.info", Value::Null);
    assert!(info["recovery"].is_object());
    let document = call(&mut engine, "document.get", json!({"doc": doc}));
    assert_eq!(document["junctions"][0]["at"], json!([5, 5]));
}

#[test]
fn save_as_copy_and_archive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("original");
    let mut engine = Engine::new();
    create(&mut engine, &path, "arduino-shield");
    fs::write(path.join("NOTES.md"), "notes").unwrap();
    call(
        &mut engine,
        "project.setProperties",
        json!({"description": "changed"}),
    );

    let copy = call(
        &mut engine,
        "project.saveCopy",
        json!({"path": dir.path().join("copy")}),
    );
    assert!(
        copy["manifestPath"]
            .as_str()
            .unwrap()
            .ends_with("Demo.ewproj")
    );
    assert!(dir.path().join("copy/NOTES.md").is_file());
    assert!(
        call(&mut engine, "project.info", Value::Null)["history"]["dirty"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(
        call_err(
            &mut engine,
            "project.saveCopy",
            json!({"path": path.join("inner")})
        ),
        "invalidParams"
    );

    let moved = call(
        &mut engine,
        "project.saveAs",
        json!({"path": dir.path().join("moved")}),
    );
    assert!(moved["path"].as_str().unwrap().ends_with("moved"));
    assert!(!moved["history"]["dirty"].as_bool().unwrap());
    assert_eq!(
        call_err(
            &mut engine,
            "project.saveAs",
            json!({"path": dir.path().join("copy")})
        ),
        "alreadyExists"
    );

    let archive = call(
        &mut engine,
        "project.archive",
        json!({"path": dir.path().join("demo.zip")}),
    );
    assert!(archive["files"].as_u64().unwrap() >= 7);
    let files = call(&mut engine, "project.files", Value::Null);
    assert!(
        files
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "NOTES.md")
    );
}

#[test]
fn only_one_project_at_a_time() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::new();
    create(&mut engine, &dir.path().join("a"), "empty");
    assert_eq!(
        call_err(
            &mut engine,
            "project.create",
            json!({"path": dir.path().join("b"), "name": "B"})
        ),
        "projectOpen"
    );
    assert_eq!(
        call_err(
            &mut engine,
            "project.open",
            json!({"path": dir.path().join("a")})
        ),
        "projectOpen"
    );
    call(&mut engine, "project.close", Value::Null);
    assert_eq!(
        call_err(
            &mut engine,
            "project.create",
            json!({"path": dir.path().join("a"), "name": "A"})
        ),
        "alreadyExists"
    );
}

#[test]
fn templates_are_listed() {
    let mut engine = Engine::new();
    let templates = call(&mut engine, "project.templates", Value::Null);
    let ids: Vec<&str> = templates
        .as_array()
        .unwrap()
        .iter()
        .map(|template| template["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec!["empty", "board-2l", "board-4l", "arduino-shield", "demo"]
    );
}

#[test]
fn view_scene_and_object() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::new();
    let snapshot = create(&mut engine, &dir.path().join("Viewer"), "board-2l");
    let docs = snapshot["documents"].as_array().unwrap();
    let pcb = docs.iter().find(|d| d["kind"] == "pcb").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let sheet = root_doc(&snapshot);

    // JSON clients get the buffer as base64 (or not at all).
    let meta = call(
        &mut engine,
        "view.scene",
        json!({"doc": pcb, "encoding": "none"}),
    );
    assert_eq!(meta["kind"], "pcb");
    assert!(meta["data"].is_null());
    let keys: Vec<&str> = meta["layers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["key"].as_str().unwrap())
        .collect();
    assert_eq!(&keys[..2], ["cu.top", "cu.bottom"]);
    assert_eq!(meta["home"], json!([0, 0, 100_000_000, 100_000_000]));
    let encoded = call(&mut engine, "view.scene", json!({"doc": pcb}));
    let bytes = meta["stats"]["bytes"].as_u64().unwrap();
    assert_eq!(
        encoded["data"].as_str().unwrap().len() as u64,
        bytes.div_ceil(3) * 4
    );

    // The desktop app gets the raw buffer.
    let request =
        json!({"method": "view.scene", "params": {"doc": sheet, "labels": {"file": "Datei"}}});
    let (reply, data) = engine.dispatch_binary(&request.to_string());
    let reply: Value = serde_json::from_str(&reply).unwrap();
    let data = data.unwrap();
    assert_eq!(reply["result"]["kind"], "schematic");
    assert_eq!(
        reply["result"]["stats"]["bytes"].as_u64().unwrap(),
        data.len() as u64
    );
    let frame = reply["result"]["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["key"] == "sch.frame")
        .unwrap()
        .clone();
    assert!(
        frame["texts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t[0].as_str().unwrap().starts_with("Datei: "))
    );
    let (other, data) = engine.dispatch_binary(r#"{"method":"engine.ping"}"#);
    assert!(data.is_none());
    assert!(other.contains("pong"));

    // Objects are described for the inspector.
    let outline = call(&mut engine, "document.get", json!({"doc": pcb}))["shapes"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let info = call(
        &mut engine,
        "view.object",
        json!({"doc": pcb, "id": outline}),
    );
    assert_eq!(info["type"], "boardShape");
    assert!(
        info["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["key"] == "layer" && r["value"] == "outline")
    );
    assert_eq!(
        call_err(&mut engine, "view.object", json!({"doc": pcb, "id": sheet})),
        "notFound"
    );
    let library = docs.iter().find(|d| d["kind"] == "library").unwrap()["id"].clone();
    assert_eq!(
        call_err(&mut engine, "view.scene", json!({"doc": library})),
        "invalidParams"
    );
}
