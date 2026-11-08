//! Editable objects of every document kind, as one enum.
//!
//! Generic edits (insert, update, delete) work on [`Object`] values. In JSON
//! the object type is given by a `type` field, for example
//! `{"type": "wire", "id": "…", "start": [0, 0], "end": [1000000, 0]}`.

use serde::{Deserialize, Serialize};

use crate::collection::Keyed;
use crate::id::Id;
use crate::library::{FootprintDef, Library, SymbolDef};
use crate::pcb::{BoardShape, BoardText, FootprintInstance, Net, Pcb, Track, Via, Zone};
use crate::schematic::{
    Bus, BusEntry, Junction, Label, NoConnect, PlacedSymbol, PowerPort, Schematic, Shape,
    SheetSymbol, Text, Wire,
};

macro_rules! objects {
    ($( $variant:ident($type:ty) = $name:literal ),+ $(,)?) => {
        /// Any editable object.
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "type", rename_all = "camelCase")]
        #[allow(
            clippy::large_enum_variant,
            reason = "objects are moved through edits one at a time"
        )]
        pub enum Object {
            $(
                #[doc = concat!("A `", $name, "` object.")]
                $variant($type),
            )+
        }

        /// The type of an [`Object`].
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub enum ObjectType {
            $(
                #[doc = concat!("`", $name, "`")]
                $variant,
            )+
        }

        impl ObjectType {
            /// Every object type.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Name used in JSON.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }
        }

        impl Object {
            /// The object id.
            #[must_use]
            pub fn id(&self) -> Id {
                match self {
                    $(Self::$variant(object) => object.id(),)+
                }
            }

            /// The object type.
            #[must_use]
            pub const fn object_type(&self) -> ObjectType {
                match self {
                    $(Self::$variant(_) => ObjectType::$variant,)+
                }
            }
        }
    };
}

objects! {
    Symbol(PlacedSymbol) = "symbol",
    Wire(Wire) = "wire",
    Bus(Bus) = "bus",
    BusEntry(BusEntry) = "busEntry",
    Junction(Junction) = "junction",
    Label(Label) = "label",
    PowerPort(PowerPort) = "powerPort",
    NoConnect(NoConnect) = "noConnect",
    Sheet(SheetSymbol) = "sheet",
    Text(Text) = "text",
    Shape(Shape) = "shape",
    Net(Net) = "net",
    Footprint(FootprintInstance) = "footprint",
    Track(Track) = "track",
    Via(Via) = "via",
    Zone(Zone) = "zone",
    BoardText(BoardText) = "boardText",
    BoardShape(BoardShape) = "boardShape",
    SymbolDef(SymbolDef) = "symbolDef",
    FootprintDef(FootprintDef) = "footprintDef",
}

impl std::str::FromStr for ObjectType {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.name() == text)
            .ok_or_else(|| format!("unknown object type `{text}`"))
    }
}

/// Generates the collection accessors a document needs for generic edits.
macro_rules! object_store {
    ($doc:ty { $($field:ident => $variant:ident),+ $(,)? }) => {
        impl $doc {
            pub(crate) fn get_object(&self, id: Id) -> Option<Object> {
                $(
                    if let Some(object) = self.$field.get(id) {
                        return Some(Object::$variant(object.clone()));
                    }
                )+
                None
            }

            pub(crate) fn type_of(&self, id: Id) -> Option<ObjectType> {
                $(
                    if self.$field.contains(id) {
                        return Some(ObjectType::$variant);
                    }
                )+
                None
            }

            /// Inserts or replaces; reports the object type if it does not
            /// belong in this document.
            pub(crate) fn put_object(&mut self, object: Object) -> Result<Option<Object>, ObjectType> {
                match object {
                    $(Object::$variant(object) => Ok(self.$field.insert(object).map(Object::$variant)),)+
                    #[allow(unreachable_patterns, reason = "generated for every document kind")]
                    other => Err(other.object_type()),
                }
            }

            pub(crate) fn take_object(&mut self, id: Id) -> Option<Object> {
                $(
                    if let Some(object) = self.$field.remove(id) {
                        return Some(Object::$variant(object));
                    }
                )+
                None
            }

            pub(crate) fn count_objects(&self) -> usize {
                0 $(+ self.$field.len())+
            }

            pub(crate) fn visit_ids(&self, visit: &mut dyn FnMut(Id, ObjectType)) {
                $(
                    for id in self.$field.ids() {
                        visit(id, ObjectType::$variant);
                    }
                )+
            }
        }
    };
}

object_store!(Schematic {
    symbols => Symbol,
    wires => Wire,
    buses => Bus,
    bus_entries => BusEntry,
    junctions => Junction,
    labels => Label,
    power_ports => PowerPort,
    no_connects => NoConnect,
    sheets => Sheet,
    texts => Text,
    shapes => Shape,
});

object_store!(Pcb {
    nets => Net,
    footprints => Footprint,
    tracks => Track,
    vias => Via,
    zones => Zone,
    texts => BoardText,
    shapes => BoardShape,
});

object_store!(Library {
    symbols => SymbolDef,
    footprints => FootprintDef,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::CopperLayer;
    use crate::units::{Point, um};

    #[test]
    fn objects_are_tagged_with_their_type() {
        let track = Object::Track(Track {
            id: Id::from_u128(1),
            layer: CopperLayer::Top,
            net: None,
            width: um(250),
            start: Point::ORIGIN,
            end: Point::mm(1, 0),
            mid: None,
            locked: false,
        });
        let json = serde_json::to_value(&track).unwrap();
        assert_eq!(json["type"], "track");
        assert_eq!(json["layer"], "cu.top");
        assert_eq!(serde_json::from_value::<Object>(json).unwrap(), track);
        assert_eq!(track.object_type().name(), "track");
    }

    #[test]
    fn object_type_names_are_unique_and_parse() {
        let mut names: Vec<&str> = ObjectType::ALL.iter().map(|kind| kind.name()).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
        for kind in ObjectType::ALL {
            assert_eq!(kind.name().parse::<ObjectType>().unwrap(), *kind);
            assert_eq!(serde_json::to_value(kind).unwrap(), kind.name());
        }
        assert!("nope".parse::<ObjectType>().is_err());
    }
}
