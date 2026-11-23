//! Rooms: placing parts into them, moving them with their parts, rooms
//! from schematic sheets and `inRoom` in custom rules.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
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
    let path = dir.path().join("Rooms");
    ew_format::store::create_project(&path, &ew_synth::generate(ew_synth::DesignSize::S, 13))
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

fn at_of(board: &Value, id: &Value) -> (i64, i64) {
    let fp = board["footprints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| &f["id"] == id)
        .unwrap();
    (fp["at"][0].as_i64().unwrap(), fp["at"][1].as_i64().unwrap())
}

#[test]
fn parts_go_into_a_room_and_move_with_it() {
    let (mut engine, doc, _dir) = open();
    // A room 40 × 30 mm to the right of the board.
    let corners = json!([
        [200_000_000, 0],
        [240_000_000, 0],
        [240_000_000, 30_000_000],
        [200_000_000, 30_000_000]
    ]);
    let room = call(
        &mut engine,
        "pcb.addRoom",
        json!({"doc": doc, "name": "Power", "outline": corners}),
    )["id"]
        .clone();
    let board = call(&mut engine, "document.get", json!({"doc": doc}));
    let ids: Vec<Value> = board["footprints"]
        .as_array()
        .unwrap()
        .iter()
        .take(4)
        .map(|f| f["id"].clone())
        .collect();
    let placed = call(
        &mut engine,
        "pcb.placeInRoom",
        json!({"doc": doc, "room": room, "ids": ids}),
    );
    assert_eq!(placed["placed"].as_array().unwrap().len(), 4, "{placed}");
    assert_eq!(placed["noRoom"], json!([]));
    let board = call(&mut engine, "document.get", json!({"doc": doc}));
    for id in &ids {
        let (x, y) = at_of(&board, id);
        assert!(
            (200_000_000..=240_000_000).contains(&x) && (0..=30_000_000).contains(&y),
            "{x} {y}"
        );
    }
    // The room's name is taken.
    let reply: Value = serde_json::from_str(&engine.dispatch(
        &json!({"method": "pcb.addRoom", "params": {"doc": doc, "name": "Power", "outline": corners}}).to_string(),
    ))
    .unwrap();
    assert_eq!(reply["error"]["code"], "conflict");

    // Moving the room moves its parts.
    let before = at_of(&board, &ids[0]);
    call(
        &mut engine,
        "pcb.edit",
        json!({"doc": doc, "op": "move", "ids": [room], "offset": [5_000_000, 0]}),
    );
    let moved = call(&mut engine, "document.get", json!({"doc": doc}));
    assert_eq!(at_of(&moved, &ids[0]), (before.0 + 5_000_000, before.1));

    // The inspector counts them.
    let info = call(&mut engine, "view.object", json!({"doc": doc, "id": room}));
    assert_eq!(info["type"], "room");
    assert_eq!(info["title"], "Power");

    // inRoom in a custom rule: wide tracks only inside the room.
    let rules = call(&mut engine, "pcb.rules", json!({"doc": doc}));
    let ok = call(
        &mut engine,
        "pcb.validateRule",
        json!({"condition": "A.inRoom('Power')"}),
    );
    assert_eq!(ok["ok"], true);
    let mut props = rules["rules"].clone();
    props["custom"] = json!([{
        "name": "power room",
        "condition": "A.inRoom('Nowhere')",
        "constraints": {"trackWidthMin": 50_000_000}
    }]);
    call(&mut engine, "pcb.setRules", json!({"props": props}));
    let report = call(&mut engine, "pcb.drc", json!({"doc": doc}));
    let wide = |report: &Value| {
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| {
                f["code"] == "track-width"
                    && f["message"].as_str().unwrap_or("").contains("power room")
            })
            .count()
    };
    assert_eq!(wide(&report), 0, "no room of that name");
    // A room over the whole board: the rule applies to its tracks.
    call(
        &mut engine,
        "pcb.addRoom",
        json!({"doc": doc, "name": "Everything", "outline": [[-1_000_000_000, -1_000_000_000], [1_000_000_000, -1_000_000_000], [1_000_000_000, 1_000_000_000], [-1_000_000_000, 1_000_000_000]]}),
    );
    let rules = call(&mut engine, "pcb.rules", json!({"doc": doc}));
    let mut props = rules["rules"].clone();
    props["custom"] = json!([{
        "name": "power room",
        "condition": "A.inRoom('Everything')",
        "constraints": {"trackWidthMin": 50_000_000}
    }]);
    call(&mut engine, "pcb.setRules", json!({"props": props}));
    let report = call(&mut engine, "pcb.drc", json!({"doc": doc}));
    assert!(wide(&report) > 0, "the tracks lie in the room");
}

#[test]
fn rooms_from_sheets_take_the_parts_off_the_board() {
    let (mut engine, doc, _dir) = open();
    let board = call(&mut engine, "document.get", json!({"doc": doc}));
    // Move every part far off the board, as after an update from the
    // schematic.
    let ids: Vec<Value> = board["footprints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].clone())
        .collect();
    call(
        &mut engine,
        "pcb.edit",
        json!({"doc": doc, "op": "move", "ids": ids, "offset": [0, -500_000_000]}),
    );
    let reply = call(&mut engine, "pcb.roomsFromSheets", json!({"doc": doc}));
    let rooms = reply["rooms"].as_array().unwrap();
    assert_ne!(rooms.len(), 0);
    let after = call(&mut engine, "document.get", json!({"doc": doc}));
    assert_eq!(after["rooms"].as_array().unwrap().len(), rooms.len());
    // Every placed part lies inside its room.
    for room in rooms {
        let outline = after["rooms"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == room["id"])
            .unwrap()["outline"]
            .clone();
        let xs: Vec<i64> = outline
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p[0].as_i64().unwrap())
            .collect();
        let ys: Vec<i64> = outline
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p[1].as_i64().unwrap())
            .collect();
        let (x0, x1) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
        let (y0, y1) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
        for designator in room["placed"].as_array().unwrap() {
            let fp = after["footprints"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| &f["designator"] == designator)
                .unwrap();
            let (x, y) = (fp["at"][0].as_i64().unwrap(), fp["at"][1].as_i64().unwrap());
            assert!(
                x > x0 && x < x1 && y > y0 && y < y1,
                "{designator} in {}",
                room["name"]
            );
        }
    }
    // Nothing left to do a second time: the parts are in their rooms.
    let again: Value =
        serde_json::from_str(&engine.dispatch(
            &json!({"method": "pcb.roomsFromSheets", "params": {"doc": doc}}).to_string(),
        ))
        .unwrap();
    assert!(
        again["error"]["message"]
            .as_str()
            .unwrap()
            .contains("in a room")
    );
}
