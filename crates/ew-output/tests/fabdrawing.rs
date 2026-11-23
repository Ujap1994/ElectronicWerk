//! The fabrication drawing of a synthetic board.

#![allow(clippy::unwrap_used, clippy::cast_precision_loss, reason = "test code")]

use ew_model::document::Document;
use ew_model::pcb::Pcb;
use ew_output::art::{ArtOptions, BoardArt};
use ew_output::fabdrawing::{Labels, Options, fabrication_doc, generated_notes};

fn board() -> Pcb {
    let project = ew_synth::generate(ew_synth::DesignSize::S, 11);
    project
        .documents
        .values()
        .find_map(|doc| match doc {
            Document::Pcb(pcb) => Some(pcb.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn the_drawing_has_tables_notes_and_the_overall_size() {
    let mut pcb = board();
    pcb.stackup.finish = "ENIG".to_owned();
    let resolve = |text: &str| text.to_owned();
    let art = BoardArt::new(&pcb, ArtOptions::default(), &resolve);
    let options = Options {
        notes: vec!["Electrical test 100 %.".to_owned()],
        ..Options::default()
    };
    let doc = fabrication_doc(&art, &Labels::default(), &options, "0.0.0", "");
    assert_eq!(doc.pages.len(), 1);
    let page = &doc.pages[0];
    let texts: Vec<&str> = page.labels.iter().map(|l| l.text.as_str()).collect();
    let has = |part: &str| texts.iter().any(|t| t.contains(part));
    assert!(has("Fabrication drawing"), "{texts:?}");
    assert!(has("Scale 1:1"));
    assert!(has("Drills") && has("Diameter"));
    assert!(has("Layer stack-up") && has("Total"));
    assert!(has("Surface finish: ENIG."));
    assert!(has("IPC-6012"));
    assert!(has("Electrical test 100 %."));
    // The overall width of the board as a dimension.
    let r = pcb.outline_bounds().unwrap();
    let width = format!("{:.2} mm", (r.max.x - r.min.x) as f64 / 1e6);
    assert!(has(&width), "{width} in {texts:?}");
    // A PDF comes out.
    let pdf = ew_output::pdf::write(&doc);
    assert!(pdf.starts_with(b"%PDF"));
    // EW_FAB_PDF=<file> keeps the drawing for a look.
    if let Ok(path) = std::env::var("EW_FAB_PDF") {
        std::fs::write(path, &pdf).unwrap();
    }
}

#[test]
fn generated_notes_follow_the_board() {
    let mut pcb = board();
    let all = generated_notes(&pcb, 3);
    assert!(all.iter().any(|n| n.contains("class 3")));
    assert!(all.iter().any(|n| n.starts_with("Board: ")));
    let without = generated_notes(&pcb, 0);
    assert!(!without.iter().any(|n| n.contains("IPC")));
    pcb.stackup.finish.clear();
    assert!(
        !generated_notes(&pcb, 2)
            .iter()
            .any(|n| n.contains("Surface finish"))
    );
}
