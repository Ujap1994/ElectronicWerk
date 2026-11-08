//! Project validation: structural and referential checks.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::layer::Layer;
use ew_model::object::ObjectType;
use ew_model::pcb::Pcb;
use ew_model::project::Project;
use ew_model::rules::DEFAULT_NET_CLASS;
use ew_model::schematic::Schematic;
use ew_model::units::Point;
use ew_model::variables;
use serde::Serialize;

use crate::codec;
use crate::error::FormatError;
use crate::store::{self, STATE_DIR};

/// How serious an issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// The project is broken or will produce wrong results.
    Error,
    /// Probably a mistake.
    Warning,
    /// Worth knowing.
    Info,
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// Severity.
    pub severity: Severity,
    /// Stable machine-readable code, for example `unknown-net`.
    pub code: &'static str,
    /// Description (English).
    pub message: String,
    /// Project-relative file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Object concerned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<Id>,
}

/// Validation result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Findings, errors first.
    pub issues: Vec<Issue>,
    /// Number of errors.
    pub errors: usize,
    /// Number of warnings.
    pub warnings: usize,
    /// Number of documents checked.
    pub documents: usize,
    /// Number of objects checked.
    pub objects: usize,
}

impl Report {
    fn finish(mut issues: Vec<Issue>, documents: usize, objects: usize) -> Self {
        issues.sort_by(|a, b| {
            (a.severity, &a.path, a.code, &a.message)
                .cmp(&(b.severity, &b.path, b.code, &b.message))
        });
        let errors = issues
            .iter()
            .filter(|issue| issue.severity == Severity::Error)
            .count();
        let warnings = issues
            .iter()
            .filter(|issue| issue.severity == Severity::Warning)
            .count();
        Self {
            issues,
            errors,
            warnings,
            documents,
            objects,
        }
    }

    /// `true` if there are no errors.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.errors == 0
    }
}

struct Checker<'a> {
    project: &'a Project,
    issues: Vec<Issue>,
    path: Option<String>,
}

impl Checker<'_> {
    fn push(
        &mut self,
        severity: Severity,
        code: &'static str,
        object: Option<Id>,
        message: String,
    ) {
        self.issues.push(Issue {
            severity,
            code,
            message,
            path: self.path.clone(),
            object,
        });
    }

    fn point(&mut self, object: Id, point: Point) {
        if !point.in_range() {
            self.push(
                Severity::Error,
                "coordinate-out-of-range",
                Some(object),
                format!("object {object} has a coordinate outside ±10 m"),
            );
        }
    }
}

/// Checks a project in memory.
#[must_use]
pub fn validate_project(project: &Project) -> Report {
    let mut checker = Checker {
        project,
        issues: Vec::new(),
        path: None,
    };
    check_project(&mut checker);
    let objects = project.object_count();
    Report::finish(checker.issues, project.documents.len(), objects)
}

fn check_project(checker: &mut Checker<'_>) {
    check_variables(checker);
    check_ids(checker);
    check_root(checker);
    check_documents(checker);
}

fn check_variables(checker: &mut Checker<'_>) {
    for name in checker.project.manifest.variables.keys() {
        if !variables::is_valid_name(name) {
            checker.push(
                Severity::Warning,
                "invalid-variable-name",
                None,
                format!("variable name `{name}` is not valid (use letters, digits and _)"),
            );
        } else if variables::BUILTIN_VARIABLES.contains(&name.as_str()) {
            checker.push(
                Severity::Warning,
                "builtin-variable-shadowed",
                None,
                format!("variable `{name}` is built in; the project value is ignored"),
            );
        }
    }
}

/// Every id must be unique across the whole project.
fn check_ids(checker: &mut Checker<'_>) {
    let project = checker.project;
    let manifest = &project.manifest;
    let mut seen: BTreeMap<Id, (String, ObjectType)> = BTreeMap::new();
    let mut duplicates: Vec<(Id, String)> = Vec::new();
    for entry in &manifest.documents {
        if let Some(document) = project.document(entry.id) {
            document.visit_ids(&mut |id, object_type| {
                if seen.insert(id, (entry.path.clone(), object_type)).is_some()
                    || project.documents.contains_key(&id)
                {
                    duplicates.push((id, entry.path.clone()));
                }
            });
        }
    }
    for (id, path) in duplicates {
        checker.path = Some(path);
        checker.push(
            Severity::Error,
            "duplicate-id",
            Some(id),
            format!("id {id} is used by more than one object"),
        );
    }
    checker.path = None;
}

fn check_root(checker: &mut Checker<'_>) {
    let project = checker.project;
    let schematics: BTreeSet<Id> = project
        .documents_of(DocumentKind::Schematic)
        .map(|entry| entry.id)
        .collect();
    match project.manifest.root_schematic {
        Some(root) if !schematics.contains(&root) => checker.push(
            Severity::Error,
            "root-schematic-missing",
            Some(root),
            format!("the top sheet {root} is not a schematic of this project"),
        ),
        None if !schematics.is_empty() => checker.push(
            Severity::Warning,
            "no-root-schematic",
            None,
            "the project has schematics but no top sheet".to_owned(),
        ),
        _ => {}
    }

    check_hierarchy(checker, &schematics);
}

fn check_documents(checker: &mut Checker<'_>) {
    let project = checker.project;
    let symbols: BTreeSet<&str> = libraries(project)
        .flat_map(|library| library.symbols.iter().map(|symbol| symbol.name.as_str()))
        .collect();
    let footprints: BTreeSet<&str> = libraries(project)
        .flat_map(|library| {
            library
                .footprints
                .iter()
                .map(|footprint| footprint.name.as_str())
        })
        .collect();
    let net_classes: Option<BTreeSet<&str>> =
        project
            .documents
            .values()
            .find_map(|document| match document {
                Document::Rules(rules) => Some(
                    rules
                        .net_classes
                        .iter()
                        .map(|class| class.name.as_str())
                        .collect(),
                ),
                _ => None,
            });

    for entry in &project.manifest.documents {
        let Some(document) = project.document(entry.id) else {
            continue;
        };
        checker.path = Some(entry.path.clone());
        match document {
            Document::Schematic(sheet) => check_schematic(checker, sheet, &symbols, &footprints),
            Document::Pcb(pcb) => check_pcb(checker, pcb, net_classes.as_ref()),
            Document::Library(_) => {}
            Document::Rules(rules) => {
                let mut names = BTreeSet::new();
                for class in &rules.net_classes {
                    if !names.insert(class.name.as_str()) {
                        checker.push(
                            Severity::Error,
                            "duplicate-net-class",
                            None,
                            format!("net class `{}` is defined twice", class.name),
                        );
                    }
                    if class.track_width <= 0 || class.clearance < 0 || class.via_drill <= 0 {
                        checker.push(
                            Severity::Error,
                            "invalid-net-class",
                            None,
                            format!("net class `{}` has a zero or negative size", class.name),
                        );
                    }
                }
                if !names.contains(DEFAULT_NET_CLASS) {
                    checker.push(
                        Severity::Warning,
                        "missing-default-net-class",
                        None,
                        format!("there is no `{DEFAULT_NET_CLASS}` net class"),
                    );
                }
            }
            Document::OutputJob(job) => {
                for output in &job.outputs {
                    if let Some(source) = output.source
                        && project.document(source).is_none()
                    {
                        checker.push(
                            Severity::Error,
                            "output-source-missing",
                            Some(output.id),
                            format!("output `{}` refers to a missing document", output.name),
                        );
                    }
                }
            }
        }
    }
    checker.path = None;
}

fn libraries(project: &Project) -> impl Iterator<Item = &ew_model::library::Library> {
    project
        .documents
        .values()
        .filter_map(|document| match document {
            Document::Library(library) => Some(library),
            _ => None,
        })
}

/// Depth-first search; records the first node found on a cycle.
fn visit_sheets(
    node: Id,
    children: &BTreeMap<Id, Vec<Id>>,
    state: &mut BTreeMap<Id, u8>,
    cycle: &mut Option<Id>,
) {
    state.insert(node, 1);
    for &child in children.get(&node).map_or(&[][..], Vec::as_slice) {
        match state.get(&child) {
            Some(1) => {
                cycle.get_or_insert(child);
            }
            Some(_) => {}
            None => visit_sheets(child, children, state, cycle),
        }
    }
    state.insert(node, 2);
}

fn check_hierarchy(checker: &mut Checker<'_>, schematics: &BTreeSet<Id>) {
    let project = checker.project;
    let mut children: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    let mut referenced = BTreeSet::new();
    for id in schematics {
        if let Some(Document::Schematic(sheet)) = project.document(*id) {
            checker.path = project.reference(*id).map(|entry| entry.path.clone());
            for symbol in &sheet.sheets {
                if !schematics.contains(&symbol.schematic) {
                    checker.push(
                        Severity::Error,
                        "sheet-target-missing",
                        Some(symbol.id),
                        format!(
                            "sheet `{}` refers to a schematic that does not exist",
                            symbol.name
                        ),
                    );
                }
                referenced.insert(symbol.schematic);
                children.entry(*id).or_default().push(symbol.schematic);
            }
        }
    }
    checker.path = None;

    // Depth-first search for cycles.
    let mut state = BTreeMap::new();
    let mut cycle = None;
    for id in schematics {
        if !state.contains_key(id) {
            visit_sheets(*id, &children, &mut state, &mut cycle);
        }
    }
    if let Some(id) = cycle {
        checker.push(
            Severity::Error,
            "sheet-cycle",
            Some(id),
            "the sheet hierarchy contains a cycle (a sheet includes itself)".to_owned(),
        );
    }

    let root = project.manifest.root_schematic;
    for id in schematics {
        if Some(*id) != root && !referenced.contains(id) {
            checker.path = project.reference(*id).map(|entry| entry.path.clone());
            checker.push(
                Severity::Info,
                "unplaced-sheet",
                Some(*id),
                "this sheet is not placed in the hierarchy (it is treated as a flat extra sheet)"
                    .to_owned(),
            );
        }
    }
    checker.path = None;
}

fn check_schematic(
    checker: &mut Checker<'_>,
    sheet: &Schematic,
    symbols: &BTreeSet<&str>,
    footprints: &BTreeSet<&str>,
) {
    let mut designators: BTreeMap<&str, Option<Id>> = BTreeMap::new();
    for symbol in &sheet.symbols {
        checker.point(symbol.id, symbol.at);
        if !symbols.contains(symbol.symbol.as_str()) {
            checker.push(
                Severity::Warning,
                "symbol-not-in-library",
                Some(symbol.id),
                format!(
                    "{} uses symbol `{}`, which is not in a project library",
                    symbol.designator, symbol.symbol
                ),
            );
        }
        if let Some(footprint) = &symbol.footprint
            && !footprints.contains(footprint.as_str())
        {
            checker.push(
                Severity::Warning,
                "footprint-not-in-library",
                Some(symbol.id),
                format!(
                    "{} uses footprint `{footprint}`, which is not in a project library",
                    symbol.designator
                ),
            );
        }
        if let Some(previous) = designators.insert(&symbol.designator, symbol.component)
            && (previous.is_none() || previous != symbol.component)
            && !symbol.designator.ends_with('?')
        {
            checker.push(
                Severity::Warning,
                "duplicate-designator",
                Some(symbol.id),
                format!(
                    "designator {} is used by more than one part",
                    symbol.designator
                ),
            );
        }
    }
    for wire in &sheet.wires {
        checker.point(wire.id, wire.start);
        checker.point(wire.id, wire.end);
        if wire.start == wire.end {
            checker.push(
                Severity::Warning,
                "zero-length-wire",
                Some(wire.id),
                format!("wire {} has zero length", wire.id),
            );
        }
    }
    for label in &sheet.labels {
        checker.point(label.id, label.at);
        if label.text.trim().is_empty() {
            checker.push(
                Severity::Error,
                "empty-label",
                Some(label.id),
                "a net label has no text".to_owned(),
            );
        }
    }
}

fn check_pcb(checker: &mut Checker<'_>, pcb: &Pcb, net_classes: Option<&BTreeSet<&str>>) {
    let stackup = &pcb.stackup;
    if stackup.copper_count() == 0 {
        checker.push(
            Severity::Error,
            "stackup-no-copper",
            None,
            "the stack-up has no copper layer".to_owned(),
        );
    }
    if pcb.outline_bounds().is_none() {
        checker.push(
            Severity::Warning,
            "no-board-outline",
            None,
            "the board has no outline (draw it on the `outline` layer)".to_owned(),
        );
    }
    for net in &pcb.nets {
        if let Some(classes) = net_classes
            && !net.class.is_empty()
            && !classes.contains(net.class.as_str())
        {
            checker.push(
                Severity::Warning,
                "unknown-net-class",
                Some(net.id),
                format!(
                    "net `{}` uses the unknown net class `{}`",
                    net.name, net.class
                ),
            );
        }
    }
    check_pcb_objects(checker, pcb);
}

fn check_pcb_objects(checker: &mut Checker<'_>, pcb: &Pcb) {
    let stackup = &pcb.stackup;
    let unknown_net = |checker: &mut Checker<'_>, object: Id, net: Option<Id>| {
        if let Some(net) = net
            && !pcb.nets.contains(net)
        {
            checker.push(
                Severity::Error,
                "unknown-net",
                Some(object),
                format!("object {object} refers to the unknown net {net}"),
            );
        }
    };
    let missing_layer = |checker: &mut Checker<'_>, object: Id, layer: Layer| {
        if !stackup.has_layer(layer) {
            checker.push(
                Severity::Error,
                "layer-not-in-stackup",
                Some(object),
                format!("object {object} is on {layer}, which the stack-up does not have"),
            );
        }
    };
    for track in &pcb.tracks {
        checker.point(track.id, track.start);
        checker.point(track.id, track.end);
        unknown_net(checker, track.id, track.net);
        missing_layer(checker, track.id, Layer::Copper(track.layer));
        if track.width <= 0 {
            checker.push(
                Severity::Error,
                "invalid-width",
                Some(track.id),
                format!("track {} has no width", track.id),
            );
        }
    }
    for via in &pcb.vias {
        checker.point(via.id, via.at);
        unknown_net(checker, via.id, via.net);
        missing_layer(checker, via.id, Layer::Copper(via.from));
        missing_layer(checker, via.id, Layer::Copper(via.to));
        if via.drill <= 0 || via.drill >= via.diameter {
            checker.push(
                Severity::Error,
                "invalid-via",
                Some(via.id),
                format!("via {} needs a drill smaller than its diameter", via.id),
            );
        }
    }
    for zone in &pcb.zones {
        unknown_net(checker, zone.id, zone.net);
        for layer in &zone.layers {
            missing_layer(checker, zone.id, Layer::Copper(*layer));
        }
        if zone.outline.len() < 3 {
            checker.push(
                Severity::Error,
                "zone-outline-too-small",
                Some(zone.id),
                format!("zone {} needs at least three outline points", zone.id),
            );
        }
        for point in &zone.outline {
            checker.point(zone.id, *point);
        }
    }
    for footprint in &pcb.footprints {
        checker.point(footprint.id, footprint.at);
        for pad in &footprint.pads {
            unknown_net(checker, footprint.id, pad.net);
        }
    }
    for shape in &pcb.shapes {
        missing_layer(checker, shape.id, shape.layer);
    }
    for text in &pcb.texts {
        missing_layer(checker, text.id, text.layer);
    }
}

/// Validates a project folder. Unlike loading, this keeps going after a
/// broken file and reports every problem it finds.
///
/// # Errors
///
/// Fails only if the manifest itself cannot be found or read.
pub fn validate_dir(path: &Path) -> Result<Report, FormatError> {
    let manifest_path = store::find_manifest(path)?;
    let root = manifest_path.parent().unwrap_or(Path::new("."));
    let manifest_file = manifest_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = fs::read_to_string(&manifest_path)
        .map_err(|error| FormatError::io(&manifest_path, error))?;
    let mut issues = Vec::new();
    let file_issue = |path: &str, code: &'static str, message: String| Issue {
        severity: Severity::Error,
        code,
        message,
        path: Some(path.to_owned()),
        object: None,
    };
    let manifest = match codec::decode_manifest(&manifest_path, &text) {
        Ok(decoded) => decoded.value,
        Err(error) => {
            issues.push(file_issue(&manifest_file, error.code(), error.to_string()));
            return Ok(Report::finish(issues, 0, 0));
        }
    };

    let mut project = Project {
        manifest: manifest.clone(),
        documents: BTreeMap::new(),
    };
    let mut seen_paths = BTreeSet::new();
    let mut seen_ids = BTreeSet::new();
    for entry in &manifest.documents {
        if let Err(message) = store::check_document_path(&entry.path, entry.kind) {
            issues.push(file_issue(&manifest_file, "invalid-document-path", message));
            continue;
        }
        if !seen_paths.insert(entry.path.to_lowercase()) || !seen_ids.insert(entry.id) {
            issues.push(file_issue(
                &manifest_file,
                "duplicate-document",
                format!("`{}` is listed more than once", entry.path),
            ));
            continue;
        }
        let file = store::resolve(root, &entry.path);
        let result = fs::read_to_string(&file)
            .map_err(|error| FormatError::io(&file, error))
            .and_then(|text| codec::decode_document(&file, entry.kind, &text));
        match result {
            Ok(decoded) if decoded.value.id() == entry.id => {
                project.documents.insert(entry.id, decoded.value);
            }
            Ok(_) => issues.push(file_issue(
                &entry.path,
                "document-id-mismatch",
                "the document id does not match the project file".to_owned(),
            )),
            Err(error) => issues.push(file_issue(
                &entry.path,
                "unreadable-document",
                error.to_string(),
            )),
        }
    }
    // Keep only loaded documents in the list so the semantic checks agree.
    project
        .manifest
        .documents
        .retain(|entry| project.documents.contains_key(&entry.id));

    let report = validate_project(&project);
    issues.extend(report.issues);

    // Native files that the manifest does not list.
    let listed: BTreeSet<String> = manifest
        .documents
        .iter()
        .map(|entry| entry.path.to_lowercase())
        .collect();
    if let Ok(files) = store::list_files(root, &manifest_file, &project, &[]) {
        for file in files {
            let extension = Path::new(&file.path)
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or_default();
            if DocumentKind::from_extension(extension).is_some()
                && !listed.contains(&file.path.to_lowercase())
                && !file.path.starts_with(STATE_DIR)
            {
                issues.push(Issue {
                    severity: Severity::Warning,
                    code: "unlisted-file",
                    message: format!("`{}` is not part of the project", file.path),
                    path: Some(file.path),
                    object: None,
                });
            }
        }
    }
    Ok(Report::finish(
        issues,
        project.documents.len(),
        project.object_count(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::document::Document;
    use ew_model::layer::CopperLayer;
    use ew_model::object::Object;
    use ew_model::pcb::{Stackup, Track, Via};
    use ew_model::project::DocumentRef;
    use ew_model::schematic::SheetSymbol;
    use ew_model::units::{Size, um};

    fn add(project: &mut Project, kind: DocumentKind, name: &str) -> Id {
        let id = Id::random();
        let path = store::unique_document_path(project, kind, name);
        project
            .add_document(
                DocumentRef { id, kind, path },
                usize::MAX,
                Document::new(kind, id, name),
            )
            .unwrap();
        id
    }

    fn codes(report: &Report) -> Vec<&'static str> {
        report.issues.iter().map(|issue| issue.code).collect()
    }

    #[test]
    fn empty_project_is_valid() {
        let project = Project::new(Id::random(), "Empty");
        let report = validate_project(&project);
        assert!(report.is_valid());
        assert_eq!(report.issues, Vec::new());
    }

    #[test]
    fn reports_board_problems() {
        let mut project = Project::new(Id::random(), "Bad");
        let pcb = add(&mut project, DocumentKind::Pcb, "Board");
        if let Some(Document::Pcb(board)) = project.document_mut(pcb) {
            board.stackup = Stackup::standard(2, um(1_600));
        }
        let document = project.document_mut(pcb).unwrap();
        document
            .insert_object(Object::Track(Track {
                id: Id::random(),
                layer: CopperLayer::Inner(1),
                net: Some(Id::random()),
                width: 0,
                start: Point::ORIGIN,
                end: Point::new(0, 20_000_000_000),
                mid: None,
                locked: false,
            }))
            .unwrap();
        document
            .insert_object(Object::Via(Via {
                id: Id::random(),
                at: Point::ORIGIN,
                diameter: um(300),
                drill: um(300),
                net: None,
                from: CopperLayer::Top,
                to: CopperLayer::Bottom,
                micro: false,
                locked: false,
            }))
            .unwrap();
        let report = validate_project(&project);
        let found = codes(&report);
        for code in [
            "unknown-net",
            "layer-not-in-stackup",
            "invalid-width",
            "coordinate-out-of-range",
            "invalid-via",
            "no-board-outline",
        ] {
            assert!(found.contains(&code), "{code} missing in {found:?}");
        }
        assert!(!report.is_valid());
        assert_eq!(report.issues[0].severity, Severity::Error);
    }

    #[test]
    fn reports_hierarchy_problems() {
        let mut project = Project::new(Id::random(), "Sheets");
        let root = add(&mut project, DocumentKind::Schematic, "Root");
        let child = add(&mut project, DocumentKind::Schematic, "Child");
        let loose = add(&mut project, DocumentKind::Schematic, "Loose");
        project.manifest.root_schematic = Some(root);
        let sheet = |target: Id| {
            Object::Sheet(SheetSymbol {
                id: Id::random(),
                name: "S".into(),
                schematic: target,
                at: Point::ORIGIN,
                size: Size::mm(10, 10),
                pins: Vec::new(),
            })
        };
        project
            .document_mut(root)
            .unwrap()
            .insert_object(sheet(child))
            .unwrap();
        project
            .document_mut(child)
            .unwrap()
            .insert_object(sheet(root))
            .unwrap();
        project
            .document_mut(child)
            .unwrap()
            .insert_object(sheet(Id::random()))
            .unwrap();
        let report = validate_project(&project);
        let found = codes(&report);
        assert!(found.contains(&"sheet-cycle"), "{found:?}");
        assert!(found.contains(&"sheet-target-missing"), "{found:?}");
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "unplaced-sheet" && issue.object == Some(loose))
        );
    }

    #[test]
    fn reports_variables_and_missing_root() {
        let mut project = Project::new(Id::random(), "Vars");
        add(&mut project, DocumentKind::Schematic, "Root");
        project
            .manifest
            .variables
            .insert("bad name".into(), "x".into());
        project
            .manifest
            .variables
            .insert("REVISION".into(), "x".into());
        let found = codes(&validate_project(&project));
        assert!(found.contains(&"invalid-variable-name"));
        assert!(found.contains(&"builtin-variable-shadowed"));
        assert!(found.contains(&"no-root-schematic"));
    }

    #[test]
    fn validates_folders_leniently() {
        let dir = tempfile::tempdir().unwrap();
        let mut project = Project::new(Id::random(), "Folder");
        let root = add(&mut project, DocumentKind::Schematic, "Root");
        add(&mut project, DocumentKind::Rules, "Rules");
        project.manifest.root_schematic = Some(root);
        store::create_project(dir.path(), &project).unwrap();
        assert!(validate_dir(dir.path()).unwrap().is_valid());

        fs::write(dir.path().join("rules/rules.ewrules"), "{ broken").unwrap();
        fs::write(dir.path().join("schematics/stray.ewsch"), "{}").unwrap();
        let report = validate_dir(dir.path()).unwrap();
        let found = codes(&report);
        assert!(found.contains(&"unreadable-document"), "{found:?}");
        assert!(found.contains(&"unlisted-file"), "{found:?}");
        assert_eq!(report.documents, 1);

        fs::write(dir.path().join("Folder.ewproj"), "nope").unwrap();
        let report = validate_dir(dir.path()).unwrap();
        assert_eq!(report.errors, 1);
    }
}
