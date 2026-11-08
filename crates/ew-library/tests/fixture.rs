//! Builds the synthetic library `tests/fixtures/kicad` (made by
//! `scripts/make-library-fixture.py`, meshes by `scripts/tessellate.ts`) and
//! checks the index, the duplicate groups, the search and the packs.

#![allow(clippy::unwrap_used, reason = "test helpers may panic")]

use std::path::{Path, PathBuf};

use ew_library::dedup::{Class, Decision};
use ew_library::index::Query;
use ew_library::{Kind, Library, Source, build, ingest, pack};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn source() -> Source {
    let mut source = ingest::detect_kicad(&fixture().join("kicad")).unwrap();
    "test".clone_into(&mut source.info.version);
    source.meshes = Some(fixture().join("meshes"));
    source
}

fn built(dir: &Path) -> ew_library::Built {
    build(
        dir,
        &[source()],
        Some("Synthetic test data\n"),
        &|_, _, _| {},
    )
    .unwrap()
}

fn names(library: &Library, group: &ew_library::index::GroupInfo) -> Vec<String> {
    let _ = library;
    group.members.iter().map(|m| m.name.clone()).collect()
}

fn groups(library: &Library, kind: Kind, class: Class) -> Vec<Vec<String>> {
    let page = library
        .groups(Some(kind), Some(class), None, 0, 100)
        .unwrap();
    page.groups.iter().map(|g| names(library, g)).collect()
}

#[test]
fn builds_the_fixture_with_every_duplicate_class() {
    let dir = tempfile::tempdir().unwrap();
    let result = built(dir.path());
    let library = &result.library;
    let status = library.status().unwrap();
    assert_eq!(status.counts["symbol"], 6);
    assert_eq!(status.counts["footprint"], 6);
    assert_eq!(status.counts["model"], 4);
    assert_eq!(status.meshes, 4);
    assert_eq!(status.sources[0].name, "KiCad");

    assert_eq!(
        groups(library, Kind::Symbol, Class::Exact),
        [["Demo:R", "Demo_Copy:R"]]
    );
    assert_eq!(
        groups(library, Kind::Symbol, Class::Alias),
        [["Demo:LM_A", "Demo:LM_B"]]
    );
    assert_eq!(
        groups(library, Kind::Symbol, Class::Near),
        [["Demo:LM_A", "Demo_Copy:LM_A"]]
    );
    assert_eq!(
        groups(library, Kind::Footprint, Class::Exact),
        [["Demo:R_0805", "Demo_Copy:R_0805"]]
    );
    assert_eq!(
        groups(library, Kind::Footprint, Class::Near),
        [[
            "Demo:C_0805",
            "Demo:R_0805",
            "Demo:R_0805_Wide",
            "Demo_Copy:R_0805"
        ]]
    );
    assert_eq!(
        groups(library, Kind::Model, Class::Exact),
        [["Demo:R_0805", "Demo:R_0805_Copy"]]
    );
    assert_eq!(
        groups(library, Kind::Model, Class::Near),
        [["Demo:R_0805", "Demo:R_0805_Copy", "Demo:R_0805_Red"]]
    );
    // Exact duplicates are merged; the copies are hidden.
    assert_eq!(status.visible["symbol"], 5);
    assert_eq!(status.visible["footprint"], 5);
    assert_eq!(status.visible["model"], 3);
    assert_eq!(status.pending, 3);
    assert!(dir.path().join("dedup-report.json").is_file());
    assert!(dir.path().join("LICENSE.md").is_file());
}

#[test]
fn searches_by_name_part_number_and_filters() {
    let dir = tempfile::tempdir().unwrap();
    let library = built(dir.path()).library;
    let search = |text: &str, kind: Option<Kind>, include_merged: bool| {
        let result = library
            .search(&Query {
                text: text.to_owned(),
                kind,
                include_merged,
                limit: 50,
                ..Query::default()
            })
            .unwrap();
        result
            .items
            .iter()
            .map(|item| format!("{}:{}", item.library, item.name))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        search("R_0805", Some(Kind::Footprint), false),
        ["Demo:R_0805", "Demo:R_0805_Wide"]
    );
    assert_eq!(
        search("R_0805", Some(Kind::Footprint), true),
        ["Demo:R_0805", "Demo_Copy:R_0805", "Demo:R_0805_Wide"]
    );
    assert_eq!(search("LMA-123", None, false), ["Demo:LM_A"]);
    assert_eq!(search("demo semi", Some(Kind::Symbol), false).len(), 2);
    assert_eq!(search("SOIC", None, false), ["Demo:SOIC-8", "Demo:SOIC-8"]);
    // Browsing without text lists everything visible, by library and name.
    let all = library
        .search(&Query {
            limit: 500,
            ..Query::default()
        })
        .unwrap();
    assert_eq!(all.total, 13);
    let smd = library
        .search(&Query {
            kind: Some(Kind::Footprint),
            mounting: Some("smd".to_owned()),
            limit: 50,
            ..Query::default()
        })
        .unwrap();
    assert_eq!(smd.total, 4);
    let facets = library.facets(Kind::Footprint).unwrap();
    assert_eq!(facets.len(), 1);
    assert_eq!(facets[0].count, 5);
}

#[test]
fn details_link_footprints_to_models() {
    let dir = tempfile::tempdir().unwrap();
    let library = built(dir.path()).library;
    let id = library
        .find(Kind::Footprint, "Demo:R_0805")
        .unwrap()
        .unwrap();
    let details = library.get(id).unwrap();
    assert_eq!(details.model.as_deref(), Some("Demo/R_0805"));
    let model = library.get(details.model_id.unwrap()).unwrap();
    assert!(model.mesh.is_some());
    assert!(model.source_file.unwrap().ends_with("R_0805.step"));
    assert_eq!(details.aliases, ["Demo_Copy:R_0805"]);
    let definition = details.definition.unwrap();
    assert_eq!(definition["pads"].as_array().unwrap().len(), 2);
    assert!(definition["id"].is_string(), "ids are restored");
    let symbol = library.find(Kind::Symbol, "Demo:LM_A").unwrap().unwrap();
    let details = library.get(symbol).unwrap();
    assert_eq!(details.mpn, "LMA-123");
    assert_eq!(details.manufacturer, "Demo Semi");
    assert_eq!(details.summary.pins, 5);
}

#[test]
fn decisions_survive_a_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    let library = built(dir.path()).library;
    let page = library
        .groups(Some(Kind::Footprint), Some(Class::Near), None, 0, 10)
        .unwrap();
    let group = &page.groups[0];
    let wide = group
        .members
        .iter()
        .find(|m| m.name == "Demo:C_0805")
        .unwrap()
        .id;
    let decided = library
        .decide(&group.key, Decision::Merge, Some(wide))
        .unwrap();
    assert_eq!(decided.decision, Decision::Merge);
    assert_eq!(decided.canonical, wide);
    assert_eq!(library.status().unwrap().visible["footprint"], 3);
    drop(library);
    // A rebuild applies the saved decision again.
    let library = built(dir.path()).library;
    let report = library.report().unwrap();
    let group = report.groups.iter().find(|g| g.key == decided.key).unwrap();
    assert_eq!(group.decision, Decision::Merge);
    assert_eq!(group.canonical, "Demo:C_0805");
    assert_eq!(library.status().unwrap().visible["footprint"], 3);
}

#[test]
fn the_report_is_reproducible() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    drop(built(a.path()));
    drop(built(b.path()));
    let read = |dir: &Path| std::fs::read_to_string(dir.join(pack::REPORT_JSON)).unwrap();
    assert_eq!(read(a.path()), read(b.path()));
    let markdown = std::fs::read_to_string(a.path().join(pack::REPORT_MARKDOWN)).unwrap();
    assert!(markdown.contains("Demo_Copy:R_0805"));
}

#[test]
fn packs_install_into_another_folder() {
    let dir = tempfile::tempdir().unwrap();
    drop(built(dir.path()));
    let out = tempfile::tempdir().unwrap();
    let manifest = pack::write_packs(dir.path(), out.path(), "lib-test", "Test", "1").unwrap();
    assert_eq!(manifest.packs.len(), 2, "index and one mesh pack");
    assert_eq!(manifest.counts["symbol"], 6);
    let target = tempfile::tempdir().unwrap();
    for file in &manifest.packs {
        let path = out.path().join(&file.file);
        assert_eq!(pack::sha256_file(&path).unwrap(), file.sha256);
        pack::install(&path, target.path()).unwrap();
    }
    let library = Library::open(target.path()).unwrap();
    assert_eq!(library.status().unwrap().counts["model"], 4);
    for (_, hash) in library.meshes().unwrap() {
        assert!(pack::mesh_path(target.path(), &hash).is_file());
    }
    // A damaged pack is refused and leaves the index alone.
    let broken = out.path().join("broken.zip");
    std::fs::write(&broken, b"not a zip").unwrap();
    assert!(pack::install(&broken, target.path()).is_err());
    assert!(Library::open(target.path()).is_ok());
}
