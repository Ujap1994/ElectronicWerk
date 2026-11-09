//! Project templates for the new-project wizard and `ewerk project new`.

use std::collections::BTreeMap;

use ew_format::store;
use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::layer::{Layer, LayerSpec};
use ew_model::library::{FootprintDef, Library};
use ew_model::outputs::OutputJob;
use ew_model::pcb::{
    BoardShape, Drill, FootprintAttributes, FootprintInstance, FootprintText, FootprintTextKind,
    Graphic, Mounting, Pad, PadKind, PadShape, Pcb, Stackup,
};
use ew_model::project::{DocumentRef, Project, Units};
use ew_model::rules::Rules;
use ew_model::schematic::Schematic;
use ew_model::shape::Geometry;
use ew_model::text::TextStyle;
use ew_model::units::{Angle, Nm, Point, Size, mm, um};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;

/// Board settings of a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardOptions {
    /// Board width.
    pub width: Nm,
    /// Board height.
    pub height: Nm,
    /// Number of copper layers.
    pub layers: u8,
    /// Board thickness.
    pub thickness: Nm,
}

/// A template as shown in the wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    /// Template id.
    pub id: &'static str,
    /// Default board settings, or `None` for templates without a board.
    pub board: Option<BoardOptions>,
    /// The board size cannot be changed (it is given by a standard).
    pub fixed_size: bool,
    /// The template contains example content.
    pub example: bool,
}

/// Arduino Uno R3 shield size (2.7 × 2.1 in).
const SHIELD: Size = Size::new(um(68_580), um(53_340));
/// Mounting holes of an Arduino Uno R3 shield.
const SHIELD_HOLES: [Point; 4] = [
    Point::new(um(13_970), um(2_540)),
    Point::new(um(15_240), um(50_800)),
    Point::new(um(66_040), um(7_620)),
    Point::new(um(66_040), um(35_560)),
];

const fn board(width: Nm, height: Nm, layers: u8) -> BoardOptions {
    BoardOptions {
        width,
        height,
        layers,
        thickness: ew_model::pcb::STANDARD_THICKNESS,
    }
}

/// Every template, in wizard order.
pub const TEMPLATES: [TemplateInfo; 5] = [
    TemplateInfo {
        id: "empty",
        board: None,
        fixed_size: false,
        example: false,
    },
    TemplateInfo {
        id: "board-2l",
        board: Some(board(mm(100), mm(100), 2)),
        fixed_size: false,
        example: false,
    },
    TemplateInfo {
        id: "board-4l",
        board: Some(board(mm(100), mm(100), 4)),
        fixed_size: false,
        example: false,
    },
    TemplateInfo {
        id: "arduino-shield",
        board: Some(board(SHIELD.w, SHIELD.h, 2)),
        fixed_size: true,
        example: false,
    },
    TemplateInfo {
        id: "demo",
        board: None,
        fixed_size: true,
        example: true,
    },
];

/// Names for the documents a template creates (the UI passes them in the
/// user's language). Missing names fall back to English.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct DocumentNames {
    /// Top schematic sheet.
    pub root_schematic: Option<String>,
    /// Project library.
    pub library: Option<String>,
    /// Design rules.
    pub rules: Option<String>,
    /// Output job.
    pub output_job: Option<String>,
    /// Output names by output kind (`gerber`, `bom`, …).
    pub outputs: BTreeMap<String, String>,
}

/// Replaces `target` with a valid, trimmed `name`; keeps it otherwise.
fn rename(target: &mut String, name: Option<&String>) {
    if let Some(name) = name
        && crate::change::check_name(name).is_ok()
    {
        name.trim().clone_into(target);
    }
}

/// Options for a new project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProject {
    /// Project name.
    pub name: String,
    /// Template id.
    pub template: String,
    /// Board settings (template defaults if `None`).
    pub board: Option<BoardOptions>,
    /// Display units.
    pub units: Units,
    /// Names of the created documents.
    pub names: DocumentNames,
}

/// Renames the outputs of a job to the given names (by output kind).
fn name_outputs(job: &mut OutputJob, names: &BTreeMap<String, String>) {
    for output in &mut job.outputs {
        let kind = serde_json::to_value(output.kind)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        rename(&mut output.name, names.get(&kind));
    }
}

/// Applies localized names to the documents of a generated project.
fn rename_documents(project: &mut Project, names: &DocumentNames) {
    for document in project.documents.values_mut() {
        match document {
            Document::Schematic(sheet) if Some(sheet.id) == project.manifest.root_schematic => {
                rename(&mut sheet.name, names.root_schematic.as_ref());
            }
            Document::Library(library) => rename(&mut library.name, names.library.as_ref()),
            Document::Rules(rules) => rename(&mut rules.name, names.rules.as_ref()),
            Document::OutputJob(job) => {
                rename(&mut job.name, names.output_job.as_ref());
                name_outputs(job, &names.outputs);
            }
            Document::Pcb(pcb) => project.manifest.name.clone_into(&mut pcb.name),
            Document::Schematic(_) => {}
        }
    }
}

/// Applies localized names to the library, rules and output job of an
/// imported project (sheets and the board keep their KiCad names).
pub(crate) fn name_imported_documents(project: &mut Project, names: &DocumentNames) {
    for document in project.documents.values_mut() {
        match document {
            Document::Library(library) => rename(&mut library.name, names.library.as_ref()),
            Document::Rules(rules) => rename(&mut rules.name, names.rules.as_ref()),
            Document::OutputJob(job) => {
                rename(&mut job.name, names.output_job.as_ref());
                name_outputs(job, &names.outputs);
            }
            Document::Pcb(_) | Document::Schematic(_) => {}
        }
    }
}

/// Builds a project from a template.
///
/// # Errors
///
/// Fails for unknown templates or invalid board settings.
pub fn build(options: &NewProject) -> Result<Project, EngineError> {
    crate::change::check_name(&options.name)?;
    let template = TEMPLATES
        .iter()
        .find(|template| template.id == options.template)
        .ok_or_else(|| {
            EngineError::invalid_params(format!("unknown template `{}`", options.template))
        })?;
    let name = options.name.trim();

    if template.id == "demo" {
        let mut project = ew_synth::generate(ew_synth::DesignSize::S, ew_synth::DEFAULT_SEED);
        project.manifest.id = Id::random();
        name.clone_into(&mut project.manifest.name);
        project.manifest.description = String::new();
        project.manifest.settings.units = options.units;
        rename_documents(&mut project, &options.names);
        return Ok(project);
    }

    let mut project = Project::new(Id::random(), name);
    "A".clone_into(&mut project.manifest.revision);
    project.manifest.settings.units = options.units;

    let root = Id::random();
    add(
        &mut project,
        Document::Schematic(Schematic::new(root, "Root")),
        "schematics/root.ewsch",
    );
    project.manifest.root_schematic = Some(root);

    let mut library = Library::new(Id::random(), "Project library");
    // Common parts, so a first schematic works without the base library.
    for def in ew_schematic::starter::starter_symbols() {
        library.symbols.insert(def);
    }
    let mut pcb_id = None;
    if let Some(defaults) = template.board {
        let mut settings = options.board.unwrap_or(defaults);
        if template.fixed_size {
            settings.width = defaults.width;
            settings.height = defaults.height;
        }
        check_board(&settings)?;
        let pcb = board_document(name, &settings, template.id, &mut library);
        pcb_id = Some(pcb.id);
        let path = store::unique_document_path(&project, DocumentKind::Pcb, name);
        add(&mut project, Document::Pcb(pcb), &path);
    }
    add(
        &mut project,
        Document::Library(library),
        "library/project.ewlib",
    );
    add(
        &mut project,
        Document::Rules(Rules::new(Id::random(), "Design rules")),
        "rules/design.ewrules",
    );
    let mut job = OutputJob::fabrication(Id::random(), "Fabrication", Id::random);
    if pcb_id.is_none() {
        // Without a board only the schematic outputs make sense.
        job.outputs.retain(|output| {
            matches!(
                output.kind,
                ew_model::outputs::OutputKind::Pdf | ew_model::outputs::OutputKind::Bom
            )
        });
    }
    add(
        &mut project,
        Document::OutputJob(job),
        "outputs/fabrication.ewout",
    );
    rename_documents(&mut project, &options.names);
    Ok(project)
}

fn add(project: &mut Project, document: Document, path: &str) {
    let reference = DocumentRef {
        id: document.id(),
        kind: document.kind(),
        path: path.to_owned(),
    };
    // Template paths are fixed and unique, so this cannot fail.
    let _ = project.add_document(reference, usize::MAX, document);
}

fn check_board(settings: &BoardOptions) -> Result<(), EngineError> {
    if !(mm(5)..=mm(2_000)).contains(&settings.width)
        || !(mm(5)..=mm(2_000)).contains(&settings.height)
    {
        return Err(EngineError::invalid_params(
            "the board must be between 5 mm and 2000 mm wide and high",
        ));
    }
    if !matches!(settings.layers, 1 | 2 | 4 | 6 | 8 | 10 | 12 | 14 | 16) {
        return Err(EngineError::invalid_params(
            "the layer count must be 1, 2 or an even number up to 16",
        ));
    }
    if !(um(200)..=mm(10)).contains(&settings.thickness) {
        return Err(EngineError::invalid_params(
            "the board thickness must be between 0.2 mm and 10 mm",
        ));
    }
    Ok(())
}

fn board_document(
    name: &str,
    settings: &BoardOptions,
    template: &str,
    library: &mut Library,
) -> Pcb {
    let mut pcb = Pcb::new(
        Id::random(),
        name,
        Stackup::standard(settings.layers, settings.thickness),
    );
    pcb.shapes.insert(BoardShape {
        id: Id::random(),
        layer: Layer::Outline,
        geometry: Geometry::Rect {
            start: Point::ORIGIN,
            end: Point::new(settings.width, settings.height),
        },
        width: um(100),
        filled: false,
    });
    if template == "arduino-shield" {
        let hole = mounting_hole();
        for (index, at) in SHIELD_HOLES.iter().enumerate() {
            let mut pads = hole.pads.clone();
            for pad in &mut pads {
                pad.id = Id::random();
            }
            pcb.footprints.insert(FootprintInstance {
                id: Id::random(),
                component: None,
                footprint: hole.name.clone(),
                designator: format!("H{}", index + 1),
                value: String::new(),
                at: *at,
                rotation: Angle::ZERO,
                side: ew_model::layer::Side::Top,
                locked: true,
                pads,
                graphics: hole.graphics.clone(),
                texts: hole.texts.clone(),
                models: Vec::new(),
                properties: BTreeMap::new(),
                attributes: hole.attributes,
            });
        }
        library.footprints.insert(hole);
    }
    pcb
}

/// A 3.2 mm mounting hole (for M3 screws).
fn mounting_hole() -> FootprintDef {
    FootprintDef {
        id: Id::random(),
        name: "MountingHole_3.2mm".to_owned(),
        description: "Mounting hole for M3 screws, 3.2 mm, not plated".to_owned(),
        keywords: "mounting hole M3".to_owned(),
        pads: vec![Pad {
            id: Id::random(),
            number: String::new(),
            net: None,
            kind: PadKind::NpThroughHole,
            shape: PadShape::Circle,
            at: Point::ORIGIN,
            rotation: Angle::ZERO,
            size: Size::new(um(3_200), um(3_200)),
            drill: Some(Drill {
                diameter: um(3_200),
                slot_length: None,
                offset: Point::ORIGIN,
            }),
            layers: vec![LayerSpec::AllCopper, LayerSpec::AllMask],
            clearance: None,
            mask_margin: None,
            paste_margin: None,
        }],
        graphics: vec![Graphic {
            layer: Layer::Courtyard(ew_model::layer::Side::Top),
            geometry: Geometry::Circle {
                center: Point::ORIGIN,
                radius: um(3_450),
            },
            width: um(50),
            filled: false,
        }],
        texts: vec![FootprintText {
            kind: FootprintTextKind::Designator,
            text: String::new(),
            layer: Layer::Fabrication(ew_model::layer::Side::Top),
            at: Point::new(0, um(4_500)),
            rotation: Angle::ZERO,
            style: TextStyle::board(),
            visible: true,
        }],
        models: Vec::new(),
        attributes: FootprintAttributes {
            mounting: Mounting::Other,
            exclude_from_bom: true,
            exclude_from_placement: true,
            dnp: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(template: &str) -> NewProject {
        NewProject {
            name: "My Board".into(),
            template: template.into(),
            board: None,
            units: Units::Mm,
            names: DocumentNames::default(),
        }
    }

    #[test]
    fn documents_get_localized_names() {
        for template in ["board-2l", "demo"] {
            let mut options = new(template);
            options.names = DocumentNames {
                root_schematic: Some("Hauptblatt".into()),
                library: Some("Projektbibliothek".into()),
                rules: Some("Designregeln".into()),
                output_job: Some("Fertigung".into()),
                outputs: BTreeMap::from([("bom".to_owned(), "Stückliste".to_owned())]),
            };
            let project = build(&options).unwrap();
            let mut names: Vec<&str> = project.documents.values().map(Document::name).collect();
            names.sort_unstable();
            assert!(names.contains(&"Hauptblatt"), "{names:?}");
            assert!(names.contains(&"Projektbibliothek"));
            assert!(names.contains(&"Designregeln"));
            assert!(names.contains(&"Fertigung"));
            assert!(
                names.contains(&"My Board"),
                "the board is named after the project"
            );
            let job = project
                .documents
                .values()
                .find_map(|document| match document {
                    Document::OutputJob(job) => Some(job),
                    _ => None,
                })
                .unwrap();
            assert!(job.outputs.iter().any(|output| output.name == "Stückliste"));
        }
        // Invalid names fall back to English.
        let mut options = new("empty");
        options.names.library = Some("  ".into());
        let project = build(&options).unwrap();
        assert!(
            project
                .documents
                .values()
                .any(|document| document.name() == "Project library")
        );
    }

    #[test]
    fn every_template_builds_a_valid_project() {
        for template in TEMPLATES {
            let project = build(&new(template.id)).unwrap();
            let report = ew_format::validate::validate_project(&project);
            assert!(report.is_valid(), "{}: {:?}", template.id, report.issues);
            let has_board = project.documents_of(DocumentKind::Pcb).count() == 1;
            assert_eq!(
                has_board,
                template.board.is_some() || template.example,
                "{}",
                template.id
            );
            assert_eq!(project.manifest.name, "My Board");
        }
    }

    #[test]
    fn board_settings_apply_and_are_checked() {
        let mut options = new("board-4l");
        options.board = Some(BoardOptions {
            width: mm(50),
            height: mm(30),
            layers: 6,
            thickness: um(1_000),
        });
        let project = build(&options).unwrap();
        let pcb = project
            .documents
            .values()
            .find_map(|document| match document {
                Document::Pcb(pcb) => Some(pcb),
                _ => None,
            })
            .unwrap();
        assert_eq!(pcb.stackup.copper_count(), 6);
        assert_eq!(pcb.stackup.thickness(), um(1_000));
        let bounds = pcb.outline_bounds().unwrap();
        assert_eq!((bounds.width(), bounds.height()), (mm(50), mm(30)));

        options.board = Some(BoardOptions {
            layers: 3,
            ..options.board.unwrap()
        });
        assert!(build(&options).is_err());
        assert!(build(&new("nope")).is_err());
        let mut unnamed = new("empty");
        unnamed.name = " ".into();
        assert!(build(&unnamed).is_err());
    }

    #[test]
    fn shield_has_fixed_size_and_holes() {
        let mut options = new("arduino-shield");
        options.board = Some(BoardOptions {
            width: mm(10),
            height: mm(10),
            layers: 2,
            thickness: um(1_600),
        });
        let project = build(&options).unwrap();
        let pcb = project
            .documents
            .values()
            .find_map(|document| match document {
                Document::Pcb(pcb) => Some(pcb),
                _ => None,
            })
            .unwrap();
        assert_eq!(pcb.footprints.len(), 4);
        assert_eq!(pcb.outline_bounds().unwrap().width(), SHIELD.w);
    }
}
