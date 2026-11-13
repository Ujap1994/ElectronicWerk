//! Downloads of online part libraries through the engine API: the preview
//! of a SnapEDA-like and an Ultra Librarian-like ZIP (`tests/fixtures/parts`,
//! made by `scripts/make-parts-fixture.py`), the duplicate check against the
//! base library, the import with the 3D models, and archives that try to
//! write outside their folder.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "test helpers"
)]

use std::io::Write;
use std::path::PathBuf;

use ew_core::Engine;
use serde_json::{Value, json};

fn parts(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parts")
        .join(name)
}

fn library_fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ew-library/tests/fixtures")
}

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

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn previews_read_both_layouts() {
    let mut engine = Engine::without_journal();
    let library = tempfile::tempdir().unwrap();
    // An empty library folder: no base library, no duplicates.
    call(&mut engine, "library.open", json!({"dir": library.path()}));

    let snap = call(
        &mut engine,
        "import.archivePreview",
        json!({"path": parts("NE555-DEMO.zip")}),
    );
    assert_eq!(snap["provider"], "SnapEDA");
    assert_eq!(snap["nickname"], "SnapEDA");
    assert_eq!(names(&snap["symbols"]), ["SnapEDA:NE555-DEMO"]);
    assert_eq!(names(&snap["footprints"]), ["SnapEDA:SOIC127P600X175-8N"]);
    // The symbol points at the downloaded footprint by its new name.
    assert_eq!(
        snap["symbols"][0]["footprint"],
        "SnapEDA:SOIC127P600X175-8N"
    );
    // One footprint, one model: the model is attached.
    assert_eq!(
        snap["footprints"][0]["models"][0],
        "${KIPRJMOD}/3d/SnapEDA/NE555-DEMO.step"
    );
    assert_eq!(snap["models"][0]["used"], true);
    assert_eq!(snap["symbols"][0]["pins"], 8);
    assert!(snap["footprints"][0].get("duplicate").is_none());
    assert_eq!(snap["problems"], json!([]));

    let ul = call(
        &mut engine,
        "import.archivePreview",
        json!({"path": parts("ul_OPAMP-DEMO.zip")}),
    );
    assert_eq!(ul["provider"], "UltraLibrarian");
    assert_eq!(names(&ul["symbols"]), ["UltraLibrarian:OPAMP-DEMO"]);
    assert_eq!(names(&ul["footprints"]), ["UltraLibrarian:DBV0005A"]);
    assert_eq!(ul["symbols"][0]["footprint"], "UltraLibrarian:DBV0005A");
    // The footprint names its model by file name; it is found in `STEP/`.
    assert_eq!(
        ul["footprints"][0]["models"][0],
        "${KIPRJMOD}/3d/UltraLibrarian/DBV0005A.step"
    );
    // The KiCad 5 library next to the KiCad 6 files is not a problem.
    assert_eq!(ul["problems"], json!([]));

    // A nickname of the user's choice.
    let named = call(
        &mut engine,
        "import.archivePreview",
        json!({"path": parts("ul_OPAMP-DEMO.zip"), "nickname": "My parts"}),
    );
    assert_eq!(names(&named["symbols"]), ["My_parts:OPAMP-DEMO"]);
    assert_eq!(named["modelFolder"], "3d/My_parts");
}

#[test]
fn duplicates_are_found_in_the_base_library() {
    let mut engine = Engine::without_journal();
    let library = tempfile::tempdir().unwrap();
    call(&mut engine, "library.open", json!({"dir": library.path()}));
    call(
        &mut engine,
        "library.build",
        json!({"sources": [{
            "kicad": library_fixtures().join("kicad"),
            "meshes": library_fixtures().join("meshes"),
            "version": "test"
        }], "wait": true}),
    );
    let snap = call(
        &mut engine,
        "import.archivePreview",
        json!({"path": parts("NE555-DEMO.zip")}),
    );
    // The footprint is Demo:SOIC-8 under another name.
    let duplicate = &snap["footprints"][0]["duplicate"];
    assert_eq!(duplicate["name"], "Demo:SOIC-8");
    assert_eq!(duplicate["class"], "exact");
    // The new SOT-23-5 has no duplicate.
    let ul = call(
        &mut engine,
        "import.archivePreview",
        json!({"path": parts("ul_OPAMP-DEMO.zip")}),
    );
    assert!(ul["footprints"][0].get("duplicate").is_none());

    // Import with the base library's footprint instead of the download's.
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("Parts");
    call(
        &mut engine,
        "project.create",
        json!({"path": project, "name": "Parts", "template": "empty"}),
    );
    let result = call(
        &mut engine,
        "import.archive",
        json!({
            "path": parts("NE555-DEMO.zip"),
            "useLibrary": ["SnapEDA:SOIC127P600X175-8N"]
        }),
    );
    assert_eq!(result["symbols"], 1);
    assert_eq!(result["footprints"], 1);
    assert_eq!(result["replaced"], 1);
    // The replaced footprint's model is not needed.
    assert_eq!(result["models"], 0);
    assert!(!project.join("3d/SnapEDA").exists());
    let info = call(&mut engine, "project.info", json!(null));
    let library_doc = info["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "library")
        .unwrap()["id"]
        .clone();
    let list = call(&mut engine, "libedit.list", json!({"doc": library_doc}));
    let text = list.to_string();
    assert!(text.contains("SnapEDA:NE555-DEMO"), "{text}");
    assert!(text.contains("Demo:SOIC-8"), "{text}");
    assert!(!text.contains("SOIC127P600X175-8N"), "{text}");
    let symbol = call(
        &mut engine,
        "libedit.item",
        json!({"doc": library_doc, "kind": "symbol", "name": "SnapEDA:NE555-DEMO"}),
    );
    assert_eq!(symbol["def"]["footprint"], "Demo:SOIC-8", "{symbol}");
}

#[test]
fn imports_with_models_and_undo() {
    let mut engine = Engine::without_journal();
    let library = tempfile::tempdir().unwrap();
    call(&mut engine, "library.open", json!({"dir": library.path()}));
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("Parts");
    call(
        &mut engine,
        "project.create",
        json!({"path": project, "name": "Parts", "template": "empty"}),
    );
    let result = call(
        &mut engine,
        "import.archive",
        json!({"path": parts("ul_OPAMP-DEMO.zip")}),
    );
    assert_eq!(result["symbols"], 1);
    assert_eq!(result["footprints"], 1);
    assert_eq!(result["models"], 1);
    let model = project.join("3d/UltraLibrarian/DBV0005A.step");
    assert!(
        std::fs::read_to_string(&model)
            .unwrap()
            .starts_with("ISO-10303-21")
    );
    // The 3D view finds the model in the project folder.
    let found = call(
        &mut engine,
        "board3d.resolve",
        json!({"path": "${KIPRJMOD}/3d/UltraLibrarian/DBV0005A.step"}),
    );
    assert_eq!(found["format"], "step");

    // Importing again skips what is there.
    let again = call(
        &mut engine,
        "import.archive",
        json!({"path": parts("ul_OPAMP-DEMO.zip")}),
    );
    assert_eq!(again["symbols"], 0);
    assert_eq!(again["skipped"], 2);

    // One undo step removes the import.
    let history = call(&mut engine, "edit.history", json!(null));
    assert_eq!(history["state"]["undoLabel"], "import-download");
    call(&mut engine, "edit.undo", json!(null));
    let snap = call(
        &mut engine,
        "import.archivePreview",
        json!({"path": parts("ul_OPAMP-DEMO.zip")}),
    );
    assert_eq!(snap["symbols"][0]["exists"], false);
}

#[test]
fn archives_cannot_write_outside_their_folder() {
    let dir = tempfile::tempdir().unwrap();
    let evil = dir.path().join("evil.zip");
    {
        let file = std::fs::File::create(&evil).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("../../outside.kicad_sym", options).unwrap();
        zip.write_all(b"(kicad_symbol_lib (version 20241209))")
            .unwrap();
        zip.finish().unwrap();
    }
    let mut engine = Engine::without_journal();
    let preview = call(&mut engine, "import.archivePreview", json!({"path": evil}));
    assert!(
        !dir.path()
            .parent()
            .unwrap()
            .join("outside.kicad_sym")
            .exists()
    );
    let problems = preview["problems"].to_string();
    assert!(problems.contains("no KiCad"), "{problems}");

    let text = dir.path().join("notes.txt");
    std::fs::write(&text, "hello").unwrap();
    let message = error(&mut engine, "import.archivePreview", json!({"path": text}));
    assert!(message.contains("ZIP"), "{message}");
}
