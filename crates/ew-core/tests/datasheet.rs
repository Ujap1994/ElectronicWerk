//! The part generator through the engine: a synthetic datasheet read,
//! recognised, previewed with its 3D model, generated into a project
//! library and found again by the duplicate check.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    reason = "test helpers may panic and take owned JSON for brevity"
)]

use ew_core::Engine;
use serde_json::{Value, json};

fn reply(engine: &mut Engine, method: &str, params: Value) -> Value {
    serde_json::from_str(&engine.dispatch(&json!({"method": method, "params": params}).to_string()))
        .unwrap()
}

fn call(engine: &mut Engine, method: &str, params: Value) -> Value {
    let reply = reply(engine, method, params);
    assert!(reply.get("error").is_none(), "{method}: {reply}");
    reply["result"].clone()
}

#[test]
#[allow(clippy::too_many_lines, reason = "one scenario")]
fn generates_a_part_from_a_datasheet() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("ew7021.pdf");
    std::fs::write(&pdf, ew_synth::datasheet::ew7021()).unwrap();
    let mut engine = Engine::without_journal();

    // Reading needs no project.
    let sheet = call(&mut engine, "datasheet.read", json!({"path": pdf}));
    assert_eq!(sheet["pages"].as_array().unwrap().len(), 6);
    assert_eq!(sheet["title"], "EW7021 Dual Low-Power Comparator");
    let page = call(
        &mut engine,
        "datasheet.page",
        json!({"path": pdf, "page": 2}),
    );
    assert!(
        page["lines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["text"] == "3 Pin Configuration and Functions")
    );
    let text = call(
        &mut engine,
        "datasheet.text",
        json!({"path": pdf, "from": 2, "to": 2}),
    );
    assert!(
        text["pages"][0]["text"]
            .as_str()
            .unwrap()
            .contains("OUT1 1 1 O")
    );
    let draft = call(&mut engine, "datasheet.extract", json!({"path": pdf}));
    assert_eq!(draft["name"], "EW7021");
    assert_eq!(draft["datasheet"], pdf.to_string_lossy().as_ref());

    // Preview with the 3D model.
    let preview = call(
        &mut engine,
        "part.preview",
        json!({"draft": draft, "package": "SOIC-8", "model": true}),
    );
    assert_eq!(preview["footprint"]["name"], "SOIC-8_3.9x4.9mm_P1.27mm");
    assert_eq!(
        preview["symbol"]["units"][0]["pins"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    assert!(
        preview["step"]
            .as_str()
            .unwrap()
            .contains("MANIFOLD_SOLID_BREP")
    );
    let errors: Vec<&Value> = preview["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["finding"]["severity"] == "error")
        .collect();
    assert!(errors.is_empty(), "{errors:?}");

    // Generating needs a project.
    let failed = reply(
        &mut engine,
        "part.generate",
        json!({"draft": draft, "package": "SOIC-8"}),
    );
    assert!(failed.get("error").is_some());
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": dir.path().join("Parts"), "name": "Parts", "template": "empty"}),
    );
    let library = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "library")
        .map(|d| d["id"].clone());
    let library = library.unwrap_or_else(|| {
        call(
            &mut engine,
            "document.add",
            json!({"kind": "library", "name": "Parts"}),
        )["id"]
            .clone()
    });
    let none = call(
        &mut engine,
        "part.duplicates",
        json!({"draft": draft, "package": "SOIC-8"}),
    );
    assert_eq!(none["matches"], json!([]));
    let made = call(
        &mut engine,
        "part.generate",
        json!({"draft": draft, "package": "SOIC-8", "doc": library}),
    );
    assert_eq!(made["component"], "EW7021_SOIC-8");
    assert_eq!(made["model"], "models/SOIC-8_3.9x4.9mm_P1.27mm.step");
    let model = dir
        .path()
        .join("Parts")
        .join("models/SOIC-8_3.9x4.9mm_P1.27mm.step");
    assert!(
        std::fs::read_to_string(&model)
            .unwrap()
            .contains("ElectronicWerk package model")
    );
    let lib = call(&mut engine, "document.get", json!({"doc": library}));
    let component = lib["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "EW7021_SOIC-8")
        .unwrap()
        .clone();
    assert_eq!(component["manufacturer"], "Example Semiconductors");
    assert_eq!(component["footprint"], "SOIC-8_3.9x4.9mm_P1.27mm");
    let footprint = lib["footprints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "SOIC-8_3.9x4.9mm_P1.27mm")
        .unwrap()
        .clone();
    assert_eq!(
        footprint["models"][0]["path"],
        "models/SOIC-8_3.9x4.9mm_P1.27mm.step"
    );

    // Now the duplicate check finds it; another package is only related.
    let found = call(
        &mut engine,
        "part.duplicates",
        json!({"draft": draft, "package": "SOIC-8"}),
    );
    assert_eq!(found["matches"][0]["name"], "EW7021_SOIC-8");
    let other = call(
        &mut engine,
        "part.duplicates",
        json!({"draft": draft, "package": "MSOP-8"}),
    );
    assert_eq!(other["matches"], json!([]));
    assert_eq!(other["related"][0]["name"], "EW7021_SOIC-8");

    // A second generation gets free names; one undo step removes it.
    let again = call(
        &mut engine,
        "part.generate",
        json!({"draft": draft, "package": "SOIC-8", "doc": library, "model": false}),
    );
    assert_eq!(again["component"], "EW7021_SOIC-8_2");
    assert_eq!(again["footprint"], "SOIC-8_3.9x4.9mm_P1.27mm_2");
    assert!(again["model"].is_null());
    let history = call(&mut engine, "edit.undo", json!({}));
    assert!(history.is_object());
    let lib = call(&mut engine, "document.get", json!({"doc": library}));
    assert_eq!(lib["components"].as_array().unwrap().len(), 1);
}
