//! Assembly variants in outputs: a job made for a variant sees the project
//! as the variant builds it. Footprints and schematic symbols get the
//! variant's fitting, value and fields, the bill of materials too, and the
//! text variable `${VARIANT}` holds the variant's name.

use ew_model::document::Document;
use ew_model::project::{Project, Variant};

use crate::bom::Part;

/// The text variable with the name of the variant.
pub const VARIABLE: &str = "VARIANT";

/// The project as the variant builds it.
#[must_use]
pub fn apply(project: &Project, variant: &Variant) -> Project {
    let mut out = project.clone();
    out.manifest
        .variables
        .insert(VARIABLE.to_owned(), variant.name.clone());
    for document in out.documents.values_mut() {
        match document {
            Document::Pcb(pcb) => variant.apply(pcb),
            Document::Schematic(sheet) => {
                for symbol in &mut sheet.symbols {
                    // Symbols of sheets used more than once have one
                    // designator per use; their parts change in the bill of
                    // materials and on the board only.
                    if symbol.instances.len() > 1 {
                        continue;
                    }
                    let designator = symbol
                        .instances
                        .values()
                        .next()
                        .cloned()
                        .unwrap_or_else(|| symbol.designator.clone());
                    let Some(part) = variant.parts.get(&designator) else {
                        continue;
                    };
                    if let Some(fitted) = part.fitted {
                        symbol.flags.dnp = !fitted;
                    }
                    if let Some(value) = &part.value {
                        symbol.value.clone_from(value);
                    }
                    for (key, value) in &part.properties {
                        symbol.properties.insert(key.clone(), value.clone());
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The variant of a project made by [`apply`], if any.
#[must_use]
pub fn active(project: &Project) -> Option<&Variant> {
    project
        .manifest
        .variables
        .get(VARIABLE)
        .and_then(|name| project.variant(name))
}

/// The parts of the bill of materials as the variant has them.
pub fn apply_parts(parts: &mut [Part], variant: &Variant) {
    for part in parts.iter_mut() {
        let Some(change) = variant.parts.get(&part.designator) else {
            continue;
        };
        if let Some(fitted) = change.fitted {
            part.dnp = !fitted;
        }
        if let Some(value) = &change.value {
            part.value.clone_from(value);
        }
        for (key, value) in &change.properties {
            part.properties.insert(key.clone(), value.clone());
        }
    }
}
