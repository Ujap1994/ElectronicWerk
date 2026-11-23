//! Design blocks: a sheet saved with its layout and placed again, in the
//! same project (parts numbered on) and in another one (library items
//! brought along), as one undo step.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "test code: one scenario after the other"
)]

use std::collections::BTreeSet;
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

fn open(dir: &Path, name: &str, seed: u64) -> (Engine, Value, Value) {
    let path = dir.join(name);
    ew_format::store::create_project(&path, &ew_synth::generate(ew_synth::DesignSize::S, seed))
        .unwrap();
    let mut engine = Engine::without_journal();
    call(&mut engine, "project.open", json!({"path": path}));
    let info = call(&mut engine, "project.info", Value::Null);
    let doc = |kind: &str| {
        info["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["kind"] == kind)
            .unwrap()["id"]
            .clone()
    };
    (engine, doc("schematic"), doc("pcb"))
}

fn ids(list: &Value) -> Vec<Value> {
    list.as_array()
        .map(|items| items.iter().map(|o| o["id"].clone()).collect())
        .unwrap_or_default()
}

fn count(board: &Value, key: &str) -> usize {
    board[key].as_array().map_or(0, Vec::len)
}

#[test]
fn a_block_is_saved_and_placed_with_its_layout() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("blocks");
    let (mut engine, sheet, pcb) = open(dir.path(), "Source", 13);
    let objects = call(&mut engine, "document.get", json!({"doc": sheet}));
    let mut all = Vec::new();
    for key in ["symbols", "wires", "labels", "powerPorts", "junctions"] {
        all.extend(ids(&objects[key]));
    }
    let saved = call(
        &mut engine,
        "block.save",
        json!({"doc": sheet, "ids": all, "name": "Input stage", "description": "The first sheet", "folder": folder}),
    );
    let parts = saved["parts"].as_u64().unwrap();
    assert!(parts > 1, "{saved}");
    assert_eq!(saved["layout"], true);
    assert_eq!(saved["footprints"].as_u64(), Some(parts), "{saved}");
    assert!(saved["tracks"].as_u64().unwrap() > 0, "{saved}");
    let listed = call(&mut engine, "block.list", json!({"folder": folder}));
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["name"], "Input stage");
    assert_eq!(listed[0]["description"], "The first sheet");

    // Again into the same project: every designator is taken.
    let before = call(&mut engine, "document.get", json!({"doc": pcb}));
    let placed = call(
        &mut engine,
        "block.place",
        json!({"name": "Input stage", "doc": sheet, "folder": folder}),
    );
    assert_eq!(placed["parts"].as_u64(), Some(parts));
    for (old, new) in placed["designators"].as_object().unwrap() {
        assert_ne!(old, new.as_str().unwrap(), "{placed}");
    }
    let board = &placed["board"];
    assert_eq!(board["footprints"].as_u64(), Some(parts), "{placed}");
    assert_eq!(board["tracks"], saved["tracks"], "{placed}");
    assert_eq!(board["vias"], saved["vias"], "{placed}");
    assert_eq!(board["skipped"], 0);
    let after = call(&mut engine, "document.get", json!({"doc": pcb}));
    assert_eq!(
        count(&after, "footprints"),
        count(&before, "footprints") + usize::try_from(parts).unwrap()
    );
    // The new tracks are on the nets of the new footprints.
    let members: BTreeSet<String> = board["select"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    let new_nets: BTreeSet<String> = after["footprints"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| members.contains(f["id"].as_str().unwrap()))
        .flat_map(|f| f["pads"].as_array().unwrap().iter())
        .filter_map(|p| p["net"].as_str().map(str::to_owned))
        .collect();
    let new_tracks: Vec<&Value> = after["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| members.contains(t["id"].as_str().unwrap()))
        .collect();
    assert_eq!(new_tracks.len() as u64, saved["tracks"].as_u64().unwrap());
    for track in new_tracks {
        assert!(new_nets.contains(track["net"].as_str().unwrap()), "{track}");
    }
    // One group named after the block.
    let group = after["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == board["group"])
        .unwrap();
    assert_eq!(group["name"], "Input stage");
    assert_eq!(group["members"].as_array().unwrap().len(), members.len());
    // One undo step.
    call(&mut engine, "edit.undo", Value::Null);
    let undone = call(&mut engine, "document.get", json!({"doc": pcb}));
    assert_eq!(count(&undone, "footprints"), count(&before, "footprints"));
    assert_eq!(count(&undone, "tracks"), count(&before, "tracks"));

    // Into another project, at given places.
    let (mut other, sheet, pcb) = open(dir.path(), "Target", 77);
    let before = call(&mut other, "document.get", json!({"doc": pcb}));
    let placed = call(
        &mut other,
        "block.place",
        json!({"name": "Input stage", "doc": sheet, "folder": folder,
               "at": [25_400_000, 50_800_000], "boardAt": [0, -50_000_000]}),
    );
    assert_eq!(placed["parts"].as_u64(), Some(parts), "{placed}");
    assert_eq!(placed["board"]["footprints"].as_u64(), Some(parts));
    let after = call(&mut other, "document.get", json!({"doc": pcb}));
    assert_eq!(
        count(&after, "footprints"),
        count(&before, "footprints") + usize::try_from(parts).unwrap()
    );
    // The top-left footprint is at boardAt.
    let placed_ids: BTreeSet<&str> = placed["board"]["select"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let positions: Vec<(i64, i64)> = after["footprints"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| placed_ids.contains(f["id"].as_str().unwrap()))
        .map(|f| (f["at"][0].as_i64().unwrap(), f["at"][1].as_i64().unwrap()))
        .collect();
    assert_eq!(positions.iter().map(|p| p.0).min(), Some(0));
    assert_eq!(positions.iter().map(|p| p.1).max(), Some(-50_000_000));

    // Deleting it empties the folder.
    call(
        &mut other,
        "block.delete",
        json!({"name": "Input stage", "folder": folder}),
    );
    assert_eq!(
        call(&mut other, "block.list", json!({"folder": folder})),
        json!([])
    );
}

#[test]
fn a_block_needs_a_part_and_a_name() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("blocks");
    let (mut engine, sheet, _) = open(dir.path(), "Empty", 5);
    let reply: Value = serde_json::from_str(&engine.dispatch(
        &json!({"method": "block.save", "params": {"doc": sheet, "ids": [], "name": "Nothing", "folder": folder}})
            .to_string(),
    ))
    .unwrap();
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("at least one part"),
        "{reply}"
    );
    let reply: Value = serde_json::from_str(&engine.dispatch(
        &json!({"method": "block.save", "params": {"doc": sheet, "ids": [], "name": "  ", "folder": folder}})
            .to_string(),
    ))
    .unwrap();
    assert!(
        reply["error"]["message"].as_str().unwrap().contains("name"),
        "{reply}"
    );
}
