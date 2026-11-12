//! Where library items are used, and renaming them everywhere.

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::project::Project;
use serde::{Deserialize, Serialize};

/// The kind of a library item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LibraryKind {
    /// A symbol.
    Symbol,
    /// A footprint.
    Footprint,
    /// A component.
    Component,
}

/// How often an item is used.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Placed symbols (symbols) or placed symbols with this footprint
    /// (footprints).
    pub placed: usize,
    /// Footprints on boards.
    pub board: usize,
    /// Symbols that name it as their default footprint.
    pub symbols: usize,
    /// Components that use it.
    pub components: usize,
}

impl Usage {
    /// `true` if nothing uses the item.
    #[must_use]
    pub const fn is_unused(&self) -> bool {
        self.placed == 0 && self.board == 0 && self.symbols == 0 && self.components == 0
    }
}

/// Counts the uses of an item (a component is never referenced).
#[must_use]
pub fn usage(project: &Project, kind: LibraryKind, name: &str) -> Usage {
    let mut out = Usage::default();
    for document in project.documents.values() {
        match document {
            Document::Schematic(sheet) => {
                out.placed += sheet
                    .symbols
                    .iter()
                    .filter(|s| match kind {
                        LibraryKind::Symbol => s.symbol == name,
                        LibraryKind::Footprint => s.footprint.as_deref() == Some(name),
                        LibraryKind::Component => false,
                    })
                    .count();
            }
            Document::Pcb(pcb) if kind == LibraryKind::Footprint => {
                out.board += pcb
                    .footprints
                    .iter()
                    .filter(|f| f.footprint == name)
                    .count();
            }
            Document::Library(library) => {
                if kind == LibraryKind::Footprint {
                    out.symbols += library
                        .symbols
                        .iter()
                        .filter(|s| s.footprint.as_deref() == Some(name))
                        .count();
                }
                out.components += library
                    .components
                    .iter()
                    .filter(|c| match kind {
                        LibraryKind::Symbol => c.symbol == name,
                        LibraryKind::Footprint => c.footprint.as_deref() == Some(name),
                        LibraryKind::Component => false,
                    })
                    .count();
            }
            _ => {}
        }
    }
    out
}

/// The changed objects of a rename, per document.
pub type Renamed = Vec<(Id, Object)>;

/// Renames an item and every reference to it. Returns the updated objects
/// (the item itself first).
#[must_use]
pub fn rename(project: &Project, library: Id, kind: LibraryKind, old: &str, new: &str) -> Renamed {
    let mut out: Renamed = Vec::new();
    // The item itself.
    if let Some(Document::Library(lib)) = project.document(library) {
        match kind {
            LibraryKind::Symbol => {
                if let Some(def) = lib.symbol(old) {
                    let mut def = def.clone();
                    new.clone_into(&mut def.name);
                    out.push((library, Object::SymbolDef(def)));
                }
            }
            LibraryKind::Footprint => {
                if let Some(def) = lib.footprint(old) {
                    let mut def = def.clone();
                    new.clone_into(&mut def.name);
                    out.push((library, Object::FootprintDef(def)));
                }
            }
            LibraryKind::Component => {
                if let Some(def) = lib.component(old) {
                    let mut def = def.clone();
                    new.clone_into(&mut def.name);
                    out.push((library, Object::ComponentDef(def)));
                }
            }
        }
    }
    if kind == LibraryKind::Component {
        return out;
    }
    for (&doc, document) in &project.documents {
        match document {
            Document::Schematic(sheet) => {
                for symbol in &sheet.symbols {
                    let mut changed = symbol.clone();
                    match kind {
                        LibraryKind::Symbol if symbol.symbol == old => {
                            new.clone_into(&mut changed.symbol);
                        }
                        LibraryKind::Footprint if symbol.footprint.as_deref() == Some(old) => {
                            changed.footprint = Some(new.to_owned());
                        }
                        _ => continue,
                    }
                    out.push((doc, Object::Symbol(changed)));
                }
            }
            Document::Pcb(pcb) if kind == LibraryKind::Footprint => {
                for footprint in pcb.footprints.iter().filter(|f| f.footprint == old) {
                    let mut changed = footprint.clone();
                    new.clone_into(&mut changed.footprint);
                    out.push((doc, Object::Footprint(changed)));
                }
            }
            Document::Library(lib) => {
                if kind == LibraryKind::Footprint {
                    for def in lib
                        .symbols
                        .iter()
                        .filter(|s| s.footprint.as_deref() == Some(old))
                    {
                        // The renamed item itself is already in the list.
                        if out.iter().any(|(_, o)| o.id() == def.id) {
                            continue;
                        }
                        let mut changed = def.clone();
                        changed.footprint = Some(new.to_owned());
                        out.push((doc, Object::SymbolDef(changed)));
                    }
                }
                for component in &lib.components {
                    let mut changed = component.clone();
                    match kind {
                        LibraryKind::Symbol if component.symbol == old => {
                            new.clone_into(&mut changed.symbol);
                        }
                        LibraryKind::Footprint if component.footprint.as_deref() == Some(old) => {
                            changed.footprint = Some(new.to_owned());
                        }
                        _ => continue,
                    }
                    out.push((doc, Object::ComponentDef(changed)));
                }
            }
            _ => {}
        }
    }
    out
}
