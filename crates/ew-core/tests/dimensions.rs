//! Dimensions on a board: added, described, turned and plotted.

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
    let path = dir.path().join("Dimensions");
    ew_format::store::create_project(&path, &ew_synth::generate(ew_synth::DesignSize::S, 3))
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

fn dimension(engine: &mut Engine, doc: &Value, id: &Value) -> Value {
    let board = call(engine, "document.get", json!({"doc": doc}));
    board["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| &d["id"] == id)
        .unwrap()
        .clone()
}

#[test]
fn a_dimension_is_drawn_described_and_turned() {
    let (mut engine, doc, _dir) = open();
    let added = call(
        &mut engine,
        "pcb.addDimension",
        json!({"doc": doc, "kind": "horizontal", "start": [0, 0], "end": [30_000_000, 2_000_000],
               "offset": 5_000_000}),
    );
    let id = added["id"].clone();
    let dim = dimension(&mut engine, &doc, &id);
    assert_eq!(
        (dim["layer"].as_str(), dim["kind"].as_str()),
        (Some("user.1"), Some("horizontal"))
    );

    // The inspector shows the measured value.
    let info = call(&mut engine, "view.object", json!({"doc": doc, "id": id}));
    assert_eq!(info["type"], "dimension");
    assert_eq!(info["title"], "30.00 mm");

    // The scene has it as one object.
    let scene = call(
        &mut engine,
        "view.scene",
        json!({"doc": doc, "encoding": "none"}),
    );
    assert!(
        scene["objectTypes"]
            .as_array()
            .unwrap()
            .contains(&json!("dimension"))
    );

    // Turned by 90° (counter-clockwise): it measures vertically, its line
    // lies on the left (a negative offset).
    call(
        &mut engine,
        "pcb.edit",
        json!({"doc": doc, "op": "rotate", "ids": [id], "angle": 90, "center": [0, 0]}),
    );
    let turned = dimension(&mut engine, &doc, &id);
    assert_eq!(turned["kind"], "vertical");
    assert_eq!(turned["offset"], -5_000_000);
    assert_eq!(turned["end"], json!([-2_000_000, 30_000_000]));
    let info = call(&mut engine, "view.object", json!({"doc": doc, "id": id}));
    assert_eq!(info["title"], "30.00 mm");

    // Locked dimensions stay where they are.
    call(
        &mut engine,
        "pcb.edit",
        json!({"doc": doc, "op": "lock", "ids": [id], "locked": true}),
    );
    call(
        &mut engine,
        "pcb.edit",
        json!({"doc": doc, "op": "move", "ids": [id], "offset": [1_000_000, 0]}),
    );
    assert_eq!(
        dimension(&mut engine, &doc, &id)["end"],
        json!([-2_000_000, 30_000_000])
    );

    // A leader needs a text.
    let reply: Value = serde_json::from_str(&engine.dispatch(
        &json!({"method": "pcb.addDimension", "params": {"doc": doc, "kind": "leader", "start": [0, 0], "end": [1, 1]}}).to_string(),
    ))
    .unwrap();
    assert!(reply["error"]["message"].as_str().unwrap().contains("text"));
}

#[test]
fn dimensions_are_plotted_on_their_layer() {
    let mut pcb = ew_model::pcb::Pcb::new(
        ew_model::id::Id::random(),
        "B",
        ew_model::pcb::Stackup::standard(2, 1_600_000),
    );
    let mut dim = ew_model::dimension::new_dimension(
        ew_model::id::Id::random(),
        ew_model::layer::Layer::User(1),
        ew_model::dimension::DimensionKind::Aligned,
        ew_model::units::Point::mm(0, 0),
        ew_model::units::Point::mm(20, 0),
        2_000_000,
    );
    dim.text = "{value} max".into();
    pcb.dimensions.insert(dim);
    let resolve = |text: &str| text.to_owned();
    let art = ew_output::art::BoardArt::new(&pcb, ew_output::art::ArtOptions::default(), &resolve);
    let features = art.features(ew_model::layer::Layer::User(1));
    // Two extension lines, the dimension line, four arrow wings, and the
    // strokes of the text.
    assert!(features.len() > 7, "{}", features.len());
    assert!(art.features(ew_model::layer::Layer::User(2)).is_empty());
}
