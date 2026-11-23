//! The 3D assembly and the enclosure check through the engine API: a box
//! enclosure made for the demo board, bodies moved into the board and its
//! parts, the findings in the design rules check, a STEP file added from
//! outside the project.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
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

fn error(engine: &mut Engine, method: &str, params: Value) -> String {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    reply["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
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

fn codes(findings: &Value) -> Vec<String> {
    findings
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"].as_str().unwrap().starts_with("mechanical-"))
        .map(|f| f["code"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn checks_the_board_in_its_enclosure() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Demo");
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": root, "name": "Demo", "template": "demo"}),
    );
    let board = doc_of(&snapshot, "pcb");

    // A box made to fit: no findings.
    let made = call(&mut engine, "mechanical.enclosure", json!({"doc": board}));
    let ids: Vec<String> = made["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(root.join("mechanical/enclosure-bottom.step").is_file());
    assert!(root.join("mechanical/enclosure-lid.step").is_file());
    let list = call(&mut engine, "mechanical.list", json!({"doc": board}));
    let bodies = list["bodies"].as_array().unwrap();
    assert_eq!(bodies.len(), 2);
    assert_eq!(bodies[0]["exists"], true);
    assert_eq!(bodies[0]["path"], "mechanical/enclosure-bottom.step");
    assert_eq!(bodies[1]["kind"], "lid");
    assert_eq!(bodies[1]["opacity"], 40);
    assert_eq!(list["clearance"], 0.5);
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    assert_eq!(checked["errors"], 0, "{checked}");
    assert_eq!(checked["warnings"], 0, "{checked}");
    assert_eq!(checked["unchecked"], json!([]));
    assert_eq!(checked["tessellate"], json!([]));

    // The STEP export can take the enclosure along.
    let step = dir.path().join("boxed.step");
    let report = call(
        &mut engine,
        "board3d.export",
        json!({"doc": board, "path": step, "mechanical": true}),
    );
    let text = std::fs::read_to_string(&step).unwrap();
    assert!(text.contains("'Enclosure bottom'") && text.contains("'Enclosure lid'"));
    let without = call(
        &mut engine,
        "board3d.export",
        json!({"doc": board, "path": dir.path().join("bare.step")}),
    );
    assert_eq!(
        report["placed"].as_u64().unwrap(),
        without["placed"].as_u64().unwrap() + 2
    );

    // The lid 6 mm lower cuts into the tallest parts (and the board).
    let lid = &bodies[1];
    let z = lid["position"][2].as_i64().unwrap();
    let mut position = lid["position"].clone();
    position[2] = json!(z - 6_000_000);
    call(
        &mut engine,
        "mechanical.update",
        json!({"doc": board, "id": ids[1], "position": position}),
    );
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    assert!(checked["errors"].as_u64().unwrap() >= 1, "{checked}");
    let collision = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "mechanical-collision" && f["targets"][1].is_object())
        .unwrap()
        .clone();
    assert_eq!(collision["targets"][0]["id"], ids[1].as_str());
    assert!(collision["targets"][1]["id"].is_string());
    assert_eq!(collision["args"]["body"], "Enclosure lid");
    assert!(collision["point"][2].as_f64().unwrap() > 1.0);
    assert!(
        collision["message"]
            .as_str()
            .unwrap()
            .ends_with("collides with Enclosure lid")
    );

    // The design rules check reports them too.
    let drc = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(codes(&drc["findings"]).contains(&"mechanical-collision".to_owned()));

    // Undo puts the lid back.
    call(&mut engine, "edit.undo", Value::Null);
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    assert_eq!(checked["errors"], 0, "{checked}");

    // The bottom shell 1 mm to the right: a wall cuts into the board.
    let bottom = &bodies[0];
    let mut position = bottom["position"].clone();
    position[0] = json!(position[0].as_i64().unwrap() + 1_000_000);
    call(
        &mut engine,
        "mechanical.update",
        json!({"doc": board, "id": ids[0], "position": position}),
    );
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    let board_hit = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"].as_str().unwrap().ends_with("/board"))
        .unwrap()
        .clone();
    assert_eq!(board_hit["code"], "mechanical-collision");
    assert_eq!(
        board_hit["message"],
        "The board collides with Enclosure bottom"
    );

    // 0.3 mm instead: a clearance finding (minimum 0.5 mm).
    position[0] = json!(bottom["position"][0].as_i64().unwrap() + 300_000);
    call(
        &mut engine,
        "mechanical.update",
        json!({"doc": board, "id": ids[0], "position": position}),
    );
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    let board_hit = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"].as_str().unwrap().ends_with("/board"))
        .unwrap()
        .clone();
    assert_eq!(board_hit["code"], "mechanical-clearance");
    assert_eq!(board_hit["severity"], "warning");
    assert_eq!(board_hit["args"]["distance"], "0.2 mm");
    // A smaller minimum clears it.
    let checked = call(
        &mut engine,
        "mechanical.check",
        json!({"doc": board, "clearance": 0.1}),
    );
    assert_eq!(checked["warnings"], 0, "{checked}");

    // Bodies that are not checked, or hidden and not checked.
    call(
        &mut engine,
        "mechanical.update",
        json!({"doc": board, "id": ids[0], "check": false, "opacity": 30}),
    );
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    assert_eq!(checked["findings"], json!([]));
    assert!(
        error(
            &mut engine,
            "mechanical.update",
            json!({"doc": board, "id": ids[0], "opacity": 0})
        )
        .contains("opacity")
    );

    // A STEP file from outside the project is copied in.
    let outside = dir.path().join("Front panel.step");
    std::fs::copy(root.join("mechanical/enclosure-lid.step"), &outside).unwrap();
    let added = call(
        &mut engine,
        "mechanical.add",
        json!({"doc": board, "file": outside}),
    );
    assert_eq!(added["path"], "mechanical/Front panel.step");
    assert!(root.join("mechanical/Front panel.step").is_file());
    let list = call(&mut engine, "mechanical.list", json!({"doc": board}));
    let panel = list["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == added["id"])
        .unwrap()
        .clone();
    assert_eq!(panel["kind"], "panel");
    assert_eq!(panel["name"], "Front panel");
    assert!(
        error(
            &mut engine,
            "mechanical.add",
            json!({"doc": board, "file": dir.path().join("missing.step")})
        )
        .contains("missing.step")
    );
    let not_step = dir.path().join("notes.txt");
    std::fs::write(&not_step, "hello").unwrap();
    assert!(
        error(
            &mut engine,
            "mechanical.add",
            json!({"doc": board, "file": not_step})
        )
        .contains("STEP")
    );

    // Removing keeps the files; saving and opening again keeps the bodies.
    call(
        &mut engine,
        "mechanical.remove",
        json!({"doc": board, "ids": [added["id"]]}),
    );
    assert!(root.join("mechanical/Front panel.step").is_file());
    call(&mut engine, "project.save", Value::Null);
    let mut later = Engine::without_journal();
    call(&mut later, "project.open", json!({"path": root}));
    let list = call(&mut later, "mechanical.list", json!({"doc": board}));
    assert_eq!(list["bodies"].as_array().unwrap().len(), 2);
    assert_eq!(list["bodies"][0]["check"], false);
    assert_eq!(list["bodies"][0]["opacity"], 30);
}

/// A mounting hole footprint (an unplated 3.2 mm hole) at `x`, `y` mm.
fn mounting_hole(designator: &str, x: i64, y: i64) -> Value {
    use ew_model::id::Id;
    use ew_model::layer::{Layer, LayerSpec, Side};
    use ew_model::pcb::{Drill, FootprintAttributes, FootprintInstance, Pad, PadKind, PadShape};
    use ew_model::units::{Angle, Point, Size};
    let pad = Pad {
        id: Id::random(),
        number: String::new(),
        net: None,
        kind: PadKind::NpThroughHole,
        shape: PadShape::Circle,
        at: Point::ORIGIN,
        rotation: Angle::ZERO,
        size: Size {
            w: 3_200_000,
            h: 3_200_000,
        },
        drill: Some(Drill {
            diameter: 3_200_000,
            slot_length: None,
            offset: Point::ORIGIN,
        }),
        layers: vec![
            LayerSpec::AllCopper,
            LayerSpec::Layer(Layer::SolderMask(Side::Top)),
        ],
        clearance: None,
        mask_margin: None,
        paste_margin: None,
        stack: Vec::new(),
    };
    let fp = FootprintInstance {
        id: Id::random(),
        component: None,
        footprint: "MountingHole_3.2mm_M3".to_owned(),
        designator: designator.to_owned(),
        value: "MountingHole".to_owned(),
        at: Point::mm(x, y),
        rotation: Angle::ZERO,
        side: Side::Top,
        locked: false,
        pads: vec![pad],
        graphics: Vec::new(),
        texts: Vec::new(),
        models: Vec::new(),
        properties: std::collections::BTreeMap::new(),
        attributes: FootprintAttributes::default(),
    };
    let mut value = serde_json::to_value(&fp).unwrap();
    value["type"] = json!("footprint");
    value
}

#[test]
fn helpers_from_the_enclosure() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Holes");
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": root, "name": "Holes", "template": "board-2l",
               "board": {"width": 60_000_000, "height": 40_000_000, "layers": 2, "thickness": 1_600_000}}),
    );
    let board = doc_of(&snapshot, "pcb");
    let outline = call(&mut engine, "design.board", Value::Null)["outline"].clone();
    // Four mounting holes 4 mm inside the corners.
    let [x0, y0, x1, y1] = [
        outline["min"][0].as_f64().unwrap(),
        outline["min"][1].as_f64().unwrap(),
        outline["max"][0].as_f64().unwrap(),
        outline["max"][1].as_f64().unwrap(),
    ];
    #[allow(clippy::cast_possible_truncation, reason = "test positions")]
    let corners = [
        ((x0 + 4.0) as i64, (y0 + 4.0) as i64),
        ((x1 - 4.0) as i64, (y0 + 4.0) as i64),
        ((x1 - 4.0) as i64, (y1 - 4.0) as i64),
        ((x0 + 4.0) as i64, (y1 - 4.0) as i64),
    ];
    let changes: Vec<Value> = corners
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            json!({"op": "insert", "doc": board, "object": mounting_hole(&format!("H{}", i + 1), *x, *y)})
        })
        .collect();
    call(&mut engine, "edit.apply", json!({"changes": changes}));

    // The box gets a boss under every hole: nothing to report.
    let made = call(&mut engine, "mechanical.enclosure", json!({"doc": board}));
    assert_eq!(made["spec"]["bosses"].as_array().unwrap().len(), 4);
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    assert_eq!(checked["findings"], json!([]), "{checked}");

    // Moved 0.4 mm: the holes are off their bosses.
    let ids: Vec<Value> = made["ids"].as_array().unwrap().clone();
    let list = call(&mut engine, "mechanical.list", json!({"doc": board}));
    let bottom = list["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == ids[0])
        .unwrap()
        .clone();
    let mut position = bottom["position"].clone();
    position[1] = json!(position[1].as_i64().unwrap() + 400_000);
    call(
        &mut engine,
        "mechanical.update",
        json!({"doc": board, "id": ids[0], "position": position}),
    );
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    let offsets: Vec<&Value> = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"] == "mechanical-mounting" && f["args"]["variant"] == "offset")
        .collect();
    assert_eq!(offsets.len(), 4, "{checked}");
    assert_eq!(offsets[0]["args"]["distance"], "0.4 mm");
    call(&mut engine, "edit.undo", Value::Null);

    // A hole the enclosure has no boss for.
    #[allow(clippy::cast_possible_truncation, reason = "test positions")]
    let (mx, my) = (f64::midpoint(x0, x1) as i64, f64::midpoint(y0, y1) as i64);
    call(
        &mut engine,
        "edit.apply",
        json!({"changes": [{"op": "insert", "doc": board, "object": mounting_hole("H5", mx, my)}]}),
    );
    let checked = call(&mut engine, "mechanical.check", json!({"doc": board}));
    assert!(
        checked["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["args"]["variant"] == "hole"),
        "{checked}"
    );

    // The cross-section at the board plane: the walls around a room.
    let section = call(&mut engine, "mechanical.section", json!({"doc": board}));
    let polygons = section["polygons"].as_array().unwrap();
    assert_eq!(polygons.len(), 1, "{section}");
    assert_eq!(
        polygons[0].as_array().unwrap().len(),
        2,
        "outline and the room"
    );

    // The board outline from the room: the same 60 × 40 mm (0.5 mm walls
    // gap minus 0.5 mm clearance).
    let preview = call(
        &mut engine,
        "mechanical.outline",
        json!({"doc": board, "apply": false}),
    );
    let points = preview["outline"].as_array().unwrap();
    assert_eq!(points.len(), 4, "{preview}");
    let xs: Vec<f64> = points.iter().map(|p| p[0].as_f64().unwrap()).collect();
    let ys: Vec<f64> = points.iter().map(|p| p[1].as_f64().unwrap()).collect();
    let span = |v: &[f64]| {
        v.iter().copied().fold(f64::MIN, f64::max) - v.iter().copied().fold(f64::MAX, f64::min)
    };
    assert!((span(&xs) - (x1 - x0)).abs() < 0.01, "{preview}");
    assert!((span(&ys) - (y1 - y0)).abs() < 0.01, "{preview}");
    let applied = call(
        &mut engine,
        "mechanical.outline",
        json!({"doc": board, "clearance": 1.0}),
    );
    assert_eq!(applied["replaced"], 1);
    let after = call(&mut engine, "design.board", Value::Null)["outline"].clone();
    assert!(
        ((after["max"][0].as_f64().unwrap() - after["min"][0].as_f64().unwrap()) - (x1 - x0 - 1.0))
            .abs()
            < 0.01,
        "{after}"
    );
    call(&mut engine, "edit.undo", Value::Null);

    // Height areas under the lid and above the floor.
    let areas = call(
        &mut engine,
        "mechanical.areas",
        json!({"doc": board, "step": 2.0}),
    );
    let heights = areas["heights"].as_array().unwrap();
    assert!(heights.iter().any(|h| h["side"] == "top"), "{areas}");
    assert!(heights.iter().any(|h| h["side"] == "bottom"), "{areas}");
    // Again: the old areas are replaced.
    let again = call(
        &mut engine,
        "mechanical.areas",
        json!({"doc": board, "step": 2.0}),
    );
    assert!(again["removed"].as_u64().unwrap() > 0);
}
