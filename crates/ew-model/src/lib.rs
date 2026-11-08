//! ElectronicWerk data model.
//!
//! This crate defines every object a project can contain: the project
//! manifest, schematics, boards, libraries, design rules and output jobs. It
//! has no file or engine logic; `ew-format` reads and writes these types and
//! `ew-core` edits them through transactions.
//!
//! Conventions (see ADR-0002):
//!
//! - Lengths and coordinates are integer **nanometres** ([`units::Nm`]).
//! - The x axis points right and the **y axis points up**. Angles are
//!   counter-clockwise ([`units::Angle`]).
//! - Every editable object has a stable UUID ([`id::Id`]).
//! - Objects of one kind live in a [`collection::Collection`], which is
//!   always ordered by id, so saved files are deterministic.

pub mod collection;
pub mod document;
pub mod error;
pub mod id;
pub mod layer;
pub mod library;
pub mod object;
pub mod outputs;
pub mod pcb;
pub mod project;
pub mod rules;
pub mod schematic;
pub mod shape;
pub mod text;
pub mod units;
pub mod variables;

pub use collection::{Collection, Keyed};
pub use document::{DocProps, Document, DocumentKind};
pub use error::ModelError;
pub use id::Id;
pub use object::{Object, ObjectType};
pub use project::{DocumentRef, Manifest, Project, ProjectProps};
pub use units::{Angle, Nm, Point, Size};

/// Implements [`Keyed`] for structs with an `id: Id` field.
macro_rules! impl_keyed {
    ($($type:ty),+ $(,)?) => {
        $(impl crate::collection::Keyed for $type {
            fn id(&self) -> crate::id::Id {
                self.id
            }
        })+
    };
}
pub(crate) use impl_keyed;

/// Returns `true` if `value` equals its type's default (used to keep saved
/// files small by leaving out default values).
pub(crate) fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// Serde default helper that returns `true`.
pub(crate) const fn default_true() -> bool {
    true
}
