//! Property tests: random edit sequences with undo and redo.
//!
//! - Undoing everything returns exactly to the original project.
//! - Replaying the autosave journal reproduces exactly the same project and
//!   undo/redo history (this is what crash recovery relies on).

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    reason = "test helpers may panic and take owned JSON for brevity"
)]

use ew_core::Engine;
use proptest::prelude::*;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
enum Step {
    Insert(u8),
    Move(u8, i32),
    Delete(u8),
    Variable(u8),
    AddSheet,
    RemoveSheet,
    Undo,
    Redo,
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        4 => any::<u8>().prop_map(Step::Insert),
        2 => (any::<u8>(), -1000..1000_i32).prop_map(|(slot, dx)| Step::Move(slot, dx)),
        2 => any::<u8>().prop_map(Step::Delete),
        1 => any::<u8>().prop_map(Step::Variable),
        1 => Just(Step::AddSheet),
        1 => Just(Step::RemoveSheet),
        3 => Just(Step::Undo),
        2 => Just(Step::Redo),
    ]
}

fn call(engine: &mut Engine, method: &str, params: Value) -> Value {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    reply
}

fn object_id(slot: u8) -> String {
    format!("00000000-0000-4000-8000-{:012x}", u64::from(slot % 16) + 1)
}

/// Full state: every document and the manifest.
fn state(engine: &mut Engine) -> Value {
    let info = call(engine, "project.info", Value::Null)["result"].clone();
    let documents: Vec<Value> = info["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|document| {
            call(engine, "document.get", json!({"doc": document["id"]}))["result"].clone()
        })
        .collect();
    json!({
        "name": info["name"],
        "variables": info["variables"],
        "root": info["rootSchematic"],
        "documents": documents,
    })
}

fn run(engine: &mut Engine, root: &str, steps: &[Step]) {
    for step in steps {
        let _ = match step {
            Step::Insert(slot) => call(
                engine,
                "edit.apply",
                json!({"label": "insert", "changes": [{"op": "insert", "doc": root, "object": {"type": "junction", "id": object_id(*slot), "at": [i64::from(*slot) * 100, 0]}}]}),
            ),
            Step::Move(slot, dx) => call(
                engine,
                "edit.apply",
                json!({"label": "move", "changes": [{"op": "patch", "doc": root, "id": object_id(*slot), "fields": {"at": [dx, dx]}}]}),
            ),
            Step::Delete(slot) => call(
                engine,
                "edit.apply",
                json!({"label": "delete", "changes": [{"op": "delete", "doc": root, "id": object_id(*slot)}]}),
            ),
            Step::Variable(value) => call(
                engine,
                "project.setProperties",
                json!({"variables": {"V": value.to_string()}}),
            ),
            Step::AddSheet => call(
                engine,
                "document.add",
                json!({"kind": "schematic", "name": "Sheet", "parent": root}),
            ),
            Step::RemoveSheet => {
                let info = call(engine, "project.info", Value::Null)["result"].clone();
                let sheet = info["documents"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|document| document["kind"] == "schematic" && document["id"] != root)
                    .map(|document| document["id"].clone());
                match sheet {
                    Some(doc) => call(engine, "document.remove", json!({"doc": doc})),
                    None => Value::Null,
                }
            }
            Step::Undo => call(engine, "edit.undo", Value::Null),
            Step::Redo => call(engine, "edit.redo", Value::Null),
        };
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn undo_restores_and_journal_replays(steps in prop::collection::vec(step(), 1..40)) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p");
        let mut engine = Engine::new();
        let snapshot = call(&mut engine, "project.create", json!({"path": path, "name": "P", "template": "empty"}))["result"].clone();
        let root = snapshot["rootSchematic"].as_str().unwrap().to_owned();
        let original = state(&mut engine);

        run(&mut engine, &root, &steps);
        let edited = state(&mut engine);
        let history = call(&mut engine, "edit.history", Value::Null)["result"].clone();

        // Crash: drop the engine without saving and replay the journal.
        drop(engine);
        let mut recovered = Engine::new();
        let opened = call(&mut recovered, "project.open", json!({"path": path}))["result"].clone();
        if opened["recovery"].is_object() {
            let reply = call(&mut recovered, "project.recover", Value::Null);
            prop_assert!(reply.get("error").is_none(), "{reply}");
        }
        prop_assert_eq!(&state(&mut recovered), &edited);
        let replayed = call(&mut recovered, "edit.history", Value::Null)["result"].clone();
        prop_assert_eq!(&replayed["undo"], &history["undo"]);
        prop_assert_eq!(&replayed["redo"], &history["redo"]);

        // Undo everything: back to the original project.
        loop {
            let reply = call(&mut recovered, "edit.undo", Value::Null);
            if reply["result"]["applied"] != true {
                break;
            }
        }
        prop_assert_eq!(&state(&mut recovered), &original);
        prop_assert!(!call(&mut recovered, "project.info", Value::Null)["result"]["history"]["dirty"].as_bool().unwrap());
    }
}
