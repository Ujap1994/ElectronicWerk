//! Project libraries: symbol and footprint definitions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::collection::Collection;
use crate::id::Id;
use crate::pcb::{FootprintAttributes, FootprintText, Graphic, Model3d, Pad};
use crate::shape::{Fill, Geometry, Stroke};
use crate::text::TextStyle;
use crate::units::{Angle, Nm, Point};
use crate::{impl_keyed, is_default};

/// Electrical type of a pin (used by the ERC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PinKind {
    /// Input.
    Input,
    /// Output.
    Output,
    /// Bidirectional.
    Bidirectional,
    /// Tri-state output.
    TriState,
    /// Passive (resistors, capacitors, …).
    #[default]
    Passive,
    /// Free / internally unconnected.
    Free,
    /// Unspecified.
    Unspecified,
    /// Power input.
    PowerIn,
    /// Power output.
    PowerOut,
    /// Open collector.
    OpenCollector,
    /// Open emitter.
    OpenEmitter,
    /// Must not be connected.
    NoConnect,
}

/// Graphic style of a pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PinShape {
    /// A plain line.
    #[default]
    Line,
    /// Inversion bubble.
    Inverted,
    /// Clock triangle.
    Clock,
    /// Bubble and clock.
    InvertedClock,
    /// Active-low input.
    InputLow,
    /// Active-low clock.
    ClockLow,
    /// Active-low output.
    OutputLow,
    /// Falling-edge clock.
    FallingEdgeClock,
    /// Non-logic.
    NonLogic,
}

/// Direction a pin points to, from its connection point towards the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PinDirection {
    /// Body to the right of the connection point.
    #[default]
    Right,
    /// Body to the left.
    Left,
    /// Body above.
    Up,
    /// Body below.
    Down,
}

/// A symbol pin.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    /// Pin number (matches the footprint pad number).
    pub number: String,
    /// Pin name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Electrical type.
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: PinKind,
    /// Graphic style.
    #[serde(default, skip_serializing_if = "is_default")]
    pub shape: PinShape,
    /// Connection point (symbol coordinates).
    pub at: Point,
    /// Direction towards the body.
    #[serde(default, skip_serializing_if = "is_default")]
    pub direction: PinDirection,
    /// Pin length.
    pub length: Nm,
    /// Hidden pin.
    #[serde(default, skip_serializing_if = "is_default")]
    pub hidden: bool,
}

/// A graphic in a symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolGraphic {
    /// Geometry.
    pub geometry: Geometry,
    /// Outline.
    #[serde(default, skip_serializing_if = "is_default")]
    pub stroke: Stroke,
    /// Fill.
    #[serde(default, skip_serializing_if = "is_default")]
    pub fill: Fill,
}

/// A text in a symbol.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolText {
    /// Content.
    pub text: String,
    /// Anchor.
    pub at: Point,
    /// Rotation.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: Angle,
    /// Style.
    #[serde(default, skip_serializing_if = "is_default")]
    pub style: TextStyle,
}

/// One unit (gate) of a symbol.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SymbolUnit {
    /// Unit name, for example `A`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Pins.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<Pin>,
    /// Body graphics.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub graphics: Vec<SymbolGraphic>,
    /// Texts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<SymbolText>,
}

/// A schematic symbol definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolDef {
    /// Object id.
    pub id: Id,
    /// Name, unique within the library.
    pub name: String,
    /// Description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Search keywords.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub keywords: String,
    /// Designator prefix, for example `R` or `U`.
    pub prefix: String,
    /// Default value.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    /// Default footprint name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint: Option<String>,
    /// Datasheet link.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub datasheet: String,
    /// A power symbol (defines a global net, has no footprint).
    #[serde(default, skip_serializing_if = "is_default")]
    pub power: bool,
    /// Units (at least one).
    pub units: Vec<SymbolUnit>,
    /// Further default parameters.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

/// A footprint definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FootprintDef {
    /// Object id.
    pub id: Id,
    /// Name, unique within the library.
    pub name: String,
    /// Description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Search keywords.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub keywords: String,
    /// Pads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pads: Vec<Pad>,
    /// Graphics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphics: Vec<Graphic>,
    /// Texts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<FootprintText>,
    /// 3D models.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<Model3d>,
    /// Assembly attributes.
    #[serde(default, skip_serializing_if = "is_default")]
    pub attributes: FootprintAttributes,
}

impl_keyed!(SymbolDef, FootprintDef);

/// A library document (`.ewlib`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    /// Document id.
    pub id: Id,
    /// Library name.
    pub name: String,
    /// Description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Symbols.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub symbols: Collection<SymbolDef>,
    /// Footprints.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub footprints: Collection<FootprintDef>,
}

/// Editable library properties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryProps {
    /// Library name.
    pub name: String,
    /// Description.
    #[serde(default)]
    pub description: String,
}

impl Library {
    /// An empty library.
    #[must_use]
    pub fn new(id: Id, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            description: String::new(),
            symbols: Collection::new(),
            footprints: Collection::new(),
        }
    }

    /// The editable properties.
    #[must_use]
    pub fn props(&self) -> LibraryProps {
        LibraryProps {
            name: self.name.clone(),
            description: self.description.clone(),
        }
    }

    /// Replaces the editable properties.
    pub fn set_props(&mut self, props: LibraryProps) {
        self.name = props.name;
        self.description = props.description;
    }

    /// The symbol with this name.
    #[must_use]
    pub fn symbol(&self, name: &str) -> Option<&SymbolDef> {
        self.symbols.iter().find(|symbol| symbol.name == name)
    }

    /// The footprint with this name.
    #[must_use]
    pub fn footprint(&self, name: &str) -> Option<&FootprintDef> {
        self.footprints
            .iter()
            .find(|footprint| footprint.name == name)
    }
}
