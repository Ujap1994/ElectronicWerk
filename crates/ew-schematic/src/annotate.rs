//! Annotation: numbering designators (`R?` → `R1`) across the hierarchy.
//!
//! Symbols are numbered sheet by sheet in hierarchy order, by position on
//! each sheet. Units of a multi-unit part share one designator: a new unit
//! joins a part of the same symbol and value on the same sheet that does not
//! have this unit yet. Sheets used more than once get a designator per use.

use std::collections::{BTreeMap, BTreeSet};

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::project::Project;
use ew_model::schematic::PlacedSymbol;
use serde::Deserialize;

use crate::connectivity::{instances, natural_cmp, symbol_def};
use crate::edit::{Change, Edit, split_designator};

/// Which symbols to number.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Order {
    /// Top to bottom, then left to right (rows).
    #[default]
    Rows,
    /// Left to right, then top to bottom (columns).
    Columns,
}

/// Annotation settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AnnotateOptions {
    /// Only this sheet (all sheets if `None`).
    pub sheet: Option<Id>,
    /// Number every symbol again (otherwise only unannotated ones).
    pub reset: bool,
    /// Position order on each sheet.
    pub order: Order,
    /// Start each sheet at its number × 100 (sheet 2: R201, …).
    pub by_sheet: bool,
}

/// A designator and the units it has.
type Part = (String, BTreeSet<u16>);

struct Candidate {
    instance: usize,
    doc: Id,
    key: String,
    id: Id,
    symbol: String,
    value: String,
    unit: u16,
    units: usize,
    component: Option<Id>,
    prefix: String,
    x: i64,
    y: i64,
}

/// Numbers designators.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one pass: collect, sort, number, write"
)]
pub fn annotate(project: &Project, options: &AnnotateOptions) -> Edit {
    let (instances, _) = instances(project);
    let mut use_count: BTreeMap<Id, usize> = BTreeMap::new();
    for inst in &instances {
        *use_count.entry(inst.sheet).or_default() += 1;
    }
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut candidates: Vec<Candidate> = Vec::new();
    // Parts that may take more units: (instance, symbol, value) → parts.
    let mut open: BTreeMap<(usize, String, String), Vec<Part>> = BTreeMap::new();
    let mut by_component: BTreeMap<(usize, Id), String> = BTreeMap::new();
    for (index, inst) in instances.iter().enumerate() {
        let Some(Document::Schematic(sheet)) = project.document(inst.sheet) else {
            continue;
        };
        let in_scope = options.sheet.is_none_or(|s| s == inst.sheet);
        for symbol in &sheet.symbols {
            let designator = symbol.designator_in(&inst.key).to_owned();
            let def = symbol_def(project, &symbol.symbol);
            let units = def.map_or(1, |d| d.units.len());
            let unannotated = split_designator(&designator).is_none();
            if in_scope && (options.reset || unannotated) {
                let prefix = def.map_or_else(
                    || {
                        designator
                            .trim_end_matches([
                                '?', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
                            ])
                            .to_owned()
                    },
                    |d| {
                        if d.power {
                            "#PWR".to_owned()
                        } else if d.prefix.is_empty() {
                            "U".to_owned()
                        } else {
                            d.prefix.clone()
                        }
                    },
                );
                candidates.push(Candidate {
                    instance: index,
                    doc: inst.sheet,
                    key: inst.key.clone(),
                    id: symbol.id,
                    symbol: symbol.symbol.clone(),
                    value: symbol.value.clone(),
                    unit: symbol.unit,
                    units,
                    component: symbol.component,
                    prefix: if prefix.is_empty() {
                        "U".to_owned()
                    } else {
                        prefix
                    },
                    x: symbol.at.x,
                    y: symbol.at.y,
                });
            } else if !unannotated {
                used.insert(designator.clone());
                if units > 1 {
                    let parts = open
                        .entry((index, symbol.symbol.clone(), symbol.value.clone()))
                        .or_default();
                    match parts.iter_mut().find(|(d, _)| *d == designator) {
                        Some((_, taken)) => {
                            taken.insert(symbol.unit);
                        }
                        None => parts.push((designator.clone(), BTreeSet::from([symbol.unit]))),
                    }
                }
                if let Some(component) = symbol.component {
                    by_component.insert((index, component), designator);
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        let position = match options.order {
            Order::Rows => (-a.y, a.x).cmp(&(-b.y, b.x)),
            Order::Columns => (a.x, -a.y).cmp(&(b.x, -b.y)),
        };
        a.instance
            .cmp(&b.instance)
            .then(position)
            .then(a.unit.cmp(&b.unit))
    });
    // (doc, symbol) → instance key → designator.
    let mut assigned: BTreeMap<(Id, Id), BTreeMap<String, String>> = BTreeMap::new();
    for c in &candidates {
        let start = if options.by_sheet {
            u32::try_from(c.instance + 1).unwrap_or(1) * 100 + 1
        } else {
            1
        };
        let mut designator = c
            .component
            .and_then(|component| by_component.get(&(c.instance, component)).cloned());
        if designator.is_none() && c.units > 1 {
            let parts = open
                .entry((c.instance, c.symbol.clone(), c.value.clone()))
                .or_default();
            if let Some((d, taken)) = parts.iter_mut().find(|(_, taken)| !taken.contains(&c.unit)) {
                taken.insert(c.unit);
                designator = Some(d.clone());
            }
        }
        let designator = designator.unwrap_or_else(|| {
            let mut n = start;
            while used.contains(&format!("{}{n}", c.prefix)) {
                n += 1;
            }
            let new = format!("{}{n}", c.prefix);
            used.insert(new.clone());
            if c.units > 1 {
                open.entry((c.instance, c.symbol.clone(), c.value.clone()))
                    .or_default()
                    .push((new.clone(), BTreeSet::from([c.unit])));
            }
            new
        });
        if let Some(component) = c.component {
            by_component.insert((c.instance, component), designator.clone());
        }
        assigned
            .entry((c.doc, c.id))
            .or_default()
            .insert(c.key.clone(), designator);
    }
    let mut edit = Edit::default();
    let mut order: Vec<(Id, Id)> = assigned.keys().copied().collect();
    order.sort_by(|a, b| {
        let name = |key: &(Id, Id)| assigned[key].values().next().cloned().unwrap_or_default();
        natural_cmp(&name(a), &name(b))
    });
    for (doc, id) in order {
        let Some(Document::Schematic(sheet)) = project.document(doc) else {
            continue;
        };
        let Some(symbol) = sheet.symbols.get(id) else {
            continue;
        };
        let mut updated: PlacedSymbol = symbol.clone();
        let names = &assigned[&(doc, id)];
        if use_count.get(&doc).copied().unwrap_or(1) > 1 {
            for (key, designator) in names {
                updated.instances.insert(key.clone(), designator.clone());
            }
            // The plain designator is the one of the first use.
            if let Some(first) = instances.iter().find(|i| i.sheet == doc) {
                if let Some(d) = updated.instances.get(&first.key) {
                    updated.designator.clone_from(d);
                }
            }
        } else if let Some(designator) = names.values().next() {
            updated.designator.clone_from(designator);
        }
        if &updated != symbol {
            edit.changes.push(Change::Update {
                doc,
                object: Object::Symbol(updated),
            });
        }
    }
    edit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::tests::{apply, place, project, sheet};
    use ew_model::units::{Point, mm};

    #[test]
    fn numbers_unannotated_symbols_by_rows() {
        let (mut project, doc) = project();
        let a = place(&mut project, doc, Point::new(mm(20), mm(50)), false);
        let b = place(&mut project, doc, Point::new(mm(10), mm(50)), false);
        let c = place(&mut project, doc, Point::new(mm(10), mm(80)), false);
        let edit = annotate(&project, &AnnotateOptions::default());
        apply(&mut project, &edit);
        let name = |project: &Project, id| {
            sheet(project, doc)
                .symbols
                .get(id)
                .unwrap()
                .designator
                .clone()
        };
        // Top row first (y up), then left to right.
        assert_eq!(
            (name(&project, c), name(&project, b), name(&project, a)),
            ("R1".to_owned(), "R2".to_owned(), "R3".to_owned())
        );
        // Nothing left to do.
        assert_eq!(
            annotate(&project, &AnnotateOptions::default())
                .changes
                .len(),
            0
        );
        // Columns, numbered again.
        let edit = annotate(
            &project,
            &AnnotateOptions {
                reset: true,
                order: Order::Columns,
                ..AnnotateOptions::default()
            },
        );
        apply(&mut project, &edit);
        assert_eq!(
            (name(&project, c), name(&project, b), name(&project, a)),
            ("R1".to_owned(), "R2".to_owned(), "R3".to_owned())
        );
        let edit = annotate(
            &project,
            &AnnotateOptions {
                reset: true,
                by_sheet: true,
                ..AnnotateOptions::default()
            },
        );
        apply(&mut project, &edit);
        assert_eq!(name(&project, a), "R103");
    }

    #[test]
    fn keeps_taken_numbers() {
        let (mut project, doc) = project();
        place(&mut project, doc, Point::new(mm(10), mm(50)), true);
        let b = place(&mut project, doc, Point::new(mm(20), mm(50)), false);
        let edit = annotate(&project, &AnnotateOptions::default());
        apply(&mut project, &edit);
        assert_eq!(
            sheet(&project, doc).symbols.get(b).unwrap().designator,
            "R2"
        );
    }
}
