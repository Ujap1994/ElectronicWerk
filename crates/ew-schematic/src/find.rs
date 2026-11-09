//! Find and replace in schematics: designators, values, footprints,
//! parameters, labels, power ports, texts and sheet names.

use std::collections::BTreeSet;

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::project::Project;
use ew_model::units::Point;
use serde::{Deserialize, Serialize};

use crate::edit::{Change, Edit};

/// What to look for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FindOptions {
    /// Text to find.
    pub query: String,
    /// Upper and lower case must match.
    pub match_case: bool,
    /// Only whole words (not inside longer names).
    pub whole_word: bool,
    /// Only this sheet.
    pub sheet: Option<Id>,
}

/// One place where the text occurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    /// Schematic document.
    pub doc: Id,
    /// Object.
    pub id: Id,
    /// Field: `designator`, `value`, `footprint`, `property:<name>`,
    /// `label`, `power`, `text` or `sheet`.
    pub field: String,
    /// The whole text of the field.
    pub text: String,
    /// Where the object is.
    pub at: Point,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The byte ranges where `query` occurs in `text`.
fn matches(text: &str, options: &FindOptions) -> Vec<(usize, usize)> {
    let query = &options.query;
    if query.is_empty() {
        return Vec::new();
    }
    let (haystack, needle) = if options.match_case {
        (text.to_owned(), query.clone())
    } else {
        (text.to_lowercase(), query.to_lowercase())
    };
    // Lowercasing can change byte lengths; only search when it did not.
    if haystack.len() != text.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = haystack[from..].find(&needle) {
        let start = from + found;
        let end = start + needle.len();
        let before = text[..start].chars().next_back().is_some_and(is_word);
        let after = text[end..].chars().next().is_some_and(is_word);
        if !options.whole_word || (!before && !after) {
            out.push((start, end));
        }
        from = end.max(start + 1);
        while from < haystack.len() && !haystack.is_char_boundary(from) {
            from += 1;
        }
    }
    out
}

/// Every searchable field of an object: (field, text, position).
fn fields(object: &Object) -> Vec<(String, String, Point)> {
    match object {
        Object::Symbol(s) => {
            let mut out = vec![
                ("designator".to_owned(), s.designator.clone(), s.at),
                ("value".to_owned(), s.value.clone(), s.at),
            ];
            if let Some(footprint) = &s.footprint {
                out.push(("footprint".to_owned(), footprint.clone(), s.at));
            }
            for (key, value) in &s.properties {
                out.push((format!("property:{key}"), value.clone(), s.at));
            }
            out
        }
        Object::Label(l) => vec![("label".to_owned(), l.text.clone(), l.at)],
        Object::PowerPort(p) => vec![("power".to_owned(), p.net.clone(), p.at)],
        Object::Text(t) => vec![("text".to_owned(), t.text.clone(), t.at)],
        Object::Sheet(s) => vec![("sheet".to_owned(), s.name.clone(), s.at)],
        _ => Vec::new(),
    }
}

fn sheet_objects(project: &Project, options: &FindOptions) -> Vec<(Id, Object)> {
    let mut out = Vec::new();
    for entry in &project.manifest.documents {
        if options.sheet.is_some_and(|s| s != entry.id) {
            continue;
        }
        let Some(Document::Schematic(sheet)) = project.document(entry.id) else {
            continue;
        };
        let mut objects: Vec<Object> = Vec::new();
        objects.extend(sheet.symbols.iter().cloned().map(Object::Symbol));
        objects.extend(sheet.labels.iter().cloned().map(Object::Label));
        objects.extend(sheet.power_ports.iter().cloned().map(Object::PowerPort));
        objects.extend(sheet.texts.iter().cloned().map(Object::Text));
        objects.extend(sheet.sheets.iter().cloned().map(Object::Sheet));
        // Sheets top to bottom, left to right.
        objects.sort_by_key(|o| {
            let at = fields(o).first().map_or(Point::ORIGIN, |f| f.2);
            (-at.y, at.x)
        });
        out.extend(objects.into_iter().map(|o| (entry.id, o)));
    }
    out
}

/// Finds a text.
#[must_use]
pub fn find(project: &Project, options: &FindOptions) -> Vec<Hit> {
    let mut out = Vec::new();
    for (doc, object) in sheet_objects(project, options) {
        for (field, text, at) in fields(&object) {
            if !matches(&text, options).is_empty() {
                out.push(Hit {
                    doc,
                    id: object.id(),
                    field,
                    text,
                    at,
                });
            }
        }
    }
    out
}

fn replace_in(text: &str, options: &FindOptions, with: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for (start, end) in matches(text, options) {
        out.push_str(&text[last..start]);
        out.push_str(with);
        last = end;
    }
    out.push_str(&text[last..]);
    out
}

/// Replaces the text in every hit (or only in `only`, given as
/// `(doc, id, field)`). Empty labels, power ports and sheet names are left
/// alone.
#[must_use]
pub fn replace(
    project: &Project,
    options: &FindOptions,
    with: &str,
    only: Option<&[(Id, Id, String)]>,
) -> Edit {
    let only: Option<BTreeSet<(Id, Id, String)>> = only.map(|o| o.iter().cloned().collect());
    let mut edit = Edit::default();
    for (doc, object) in sheet_objects(project, options) {
        let mut changed = object.clone();
        let mut any = false;
        let wanted = |field: &str| {
            only.as_ref()
                .is_none_or(|o| o.contains(&(doc, object.id(), field.to_owned())))
        };
        let mut apply = |field: &str, text: &mut String, may_be_empty: bool| {
            if !wanted(field) {
                return;
            }
            let new = replace_in(text, options, with);
            if new != *text && (may_be_empty || !new.trim().is_empty()) {
                *text = new;
                any = true;
            }
        };
        match &mut changed {
            Object::Symbol(s) => {
                apply("designator", &mut s.designator, false);
                apply("value", &mut s.value, true);
                if let Some(footprint) = &mut s.footprint {
                    apply("footprint", footprint, true);
                }
                for (key, value) in &mut s.properties {
                    apply(&format!("property:{key}"), value, true);
                }
            }
            Object::Label(l) => apply("label", &mut l.text, false),
            Object::PowerPort(p) => apply("power", &mut p.net, false),
            Object::Text(t) => apply("text", &mut t.text, true),
            Object::Sheet(s) => apply("sheet", &mut s.name, false),
            _ => {}
        }
        if any {
            edit.select.push(changed.id());
            edit.changes.push(Change::Update {
                doc,
                object: changed,
            });
        }
    }
    edit
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(query: &str, match_case: bool, whole_word: bool) -> FindOptions {
        FindOptions {
            query: query.to_owned(),
            match_case,
            whole_word,
            sheet: None,
        }
    }

    #[test]
    fn matching_rules() {
        assert_eq!(matches("VCC_3V3", &options("vcc", false, false)), [(0, 3)]);
        assert_eq!(matches("VCC_3V3", &options("vcc", true, false)).len(), 0);
        assert_eq!(matches("VCC_3V3", &options("VCC", true, true)).len(), 0);
        assert_eq!(matches("SDA SDA1", &options("SDA", true, true)), [(0, 3)]);
        assert_eq!(replace_in("A+A", &options("a", false, false), "B"), "B+B");
        assert_eq!(
            replace_in("Grün", &options("grün", false, false), "Rot"),
            "Rot"
        );
        assert_eq!(matches("Grün", &options("grün", true, false)).len(), 0);
    }

    #[test]
    fn finds_and_replaces_labels_and_values() {
        use crate::edit::tests::{apply, place, project, sheet};
        use ew_model::units::{Angle, mm};
        let (mut project, doc) = project();
        place(&mut project, doc, Point::ORIGIN, true);
        let edit = crate::edit::add_item(
            &project,
            doc,
            &crate::edit::AddItem::Label {
                label: ew_model::schematic::LabelKind::Local,
                text: "SDA".to_owned(),
            },
            Point::new(mm(10), 0),
            Angle::ZERO,
        )
        .unwrap();
        apply(&mut project, &edit);
        let hits = find(&project, &options("10k", false, false));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].field, "value");
        let edit = replace(&project, &options("SDA", true, true), "I2C_SDA", None);
        apply(&mut project, &edit);
        assert_eq!(
            sheet(&project, doc).labels.iter().next().unwrap().text,
            "I2C_SDA"
        );
        // A label cannot become empty.
        assert_eq!(
            replace(&project, &options("I2C_SDA", true, false), "", None)
                .changes
                .len(),
            0
        );
    }
}
