//! Project libraries: symbol and footprint definitions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::collection::Collection;
use crate::id::Id;
use crate::pcb::{FootprintAttributes, FootprintText, Graphic, Model3d, Pad};
use crate::shape::{Fill, Geometry, Stroke};
use crate::text::TextStyle;
use crate::units::{Angle, Nm, Point, um};
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

impl PinKind {
    /// Every pin type, in the order of the ERC pin type matrix.
    pub const ALL: [Self; 12] = [
        Self::Input,
        Self::Output,
        Self::Bidirectional,
        Self::TriState,
        Self::Passive,
        Self::Free,
        Self::Unspecified,
        Self::PowerIn,
        Self::PowerOut,
        Self::OpenCollector,
        Self::OpenEmitter,
        Self::NoConnect,
    ];

    /// The name in files and settings (camelCase).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
            Self::Bidirectional => "bidirectional",
            Self::TriState => "triState",
            Self::Passive => "passive",
            Self::Free => "free",
            Self::Unspecified => "unspecified",
            Self::PowerIn => "powerIn",
            Self::PowerOut => "powerOut",
            Self::OpenCollector => "openCollector",
            Self::OpenEmitter => "openEmitter",
            Self::NoConnect => "noConnect",
        }
    }

    /// The position in [`PinKind::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(6)
    }

    /// Parses a name of [`PinKind::name`].
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }
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
    /// The alternate body (De Morgan equivalent, for example a NAND drawn
    /// as an OR with inverted inputs). Placed symbols choose it with
    /// `alternate`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternate: Option<SymbolBody>,
}

/// The pins, graphics and texts of an alternate body.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SymbolBody {
    /// Pins.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<Pin>,
    /// Graphics.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub graphics: Vec<SymbolGraphic>,
    /// Texts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<SymbolText>,
}

/// The pins, graphics and texts of one unit in one body style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyRef<'a> {
    /// Pins.
    pub pins: &'a [Pin],
    /// Graphics.
    pub graphics: &'a [SymbolGraphic],
    /// Texts.
    pub texts: &'a [SymbolText],
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
    /// How pin names and numbers are shown.
    #[serde(default, skip_serializing_if = "is_default")]
    pub pin_text: PinText,
}

impl SymbolDef {
    /// The unit with this (1-based) number, or the first unit.
    #[must_use]
    pub fn unit(&self, unit: u16) -> Option<&SymbolUnit> {
        self.units
            .get(usize::from(unit.max(1) - 1))
            .or_else(|| self.units.first())
    }

    /// The body of a unit: the alternate one if asked for and present,
    /// otherwise the normal one.
    #[must_use]
    pub fn body(&self, unit: u16, alternate: bool) -> Option<BodyRef<'_>> {
        let unit = self.unit(unit)?;
        Some(match (alternate, &unit.alternate) {
            (true, Some(body)) => BodyRef {
                pins: &body.pins,
                graphics: &body.graphics,
                texts: &body.texts,
            },
            _ => BodyRef {
                pins: &unit.pins,
                graphics: &unit.graphics,
                texts: &unit.texts,
            },
        })
    }

    /// `true` if a unit has an alternate body.
    #[must_use]
    pub fn has_alternate(&self) -> bool {
        self.units.iter().any(|unit| unit.alternate.is_some())
    }
}

/// How the pin names and numbers of a symbol are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PinText {
    /// Hide the pin numbers.
    #[serde(skip_serializing_if = "is_default")]
    pub hide_numbers: bool,
    /// Hide the pin names.
    #[serde(skip_serializing_if = "is_default")]
    pub hide_names: bool,
    /// Distance of the names from the pin end inside the body; 0 puts the
    /// names above the pin line (and the numbers below it).
    pub name_offset: Nm,
}

impl Default for PinText {
    fn default() -> Self {
        Self {
            hide_numbers: false,
            hide_names: false,
            name_offset: um(508),
        }
    }
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

/// A part number of a supplier.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SupplierPart {
    /// Supplier, for example `Mouser`.
    pub supplier: String,
    /// The supplier's part number.
    pub part_number: String,
    /// Product page.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
}

/// A library component: a symbol together with its footprint (and through
/// it the 3D model), value, parameters and ordering data. Placing a
/// component places its symbol with all of this filled in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDef {
    /// Object id.
    pub id: Id,
    /// Name, unique within the library (for example `RC0603FR-0710KL`).
    pub name: String,
    /// Description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Search keywords.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub keywords: String,
    /// Symbol name.
    pub symbol: String,
    /// Footprint name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint: Option<String>,
    /// Value shown on the schematic.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    /// Manufacturer.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub manufacturer: String,
    /// Manufacturer part number.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mpn: String,
    /// Datasheet link.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub datasheet: String,
    /// Further parameters (tolerance, voltage, …).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
    /// Ordering data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppliers: Vec<SupplierPart>,
}

impl_keyed!(SymbolDef, FootprintDef, ComponentDef);

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
    /// Components (symbol + footprint + parameters + ordering data).
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub components: Collection<ComponentDef>,
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
            components: Collection::new(),
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

    /// The component with this name.
    #[must_use]
    pub fn component(&self, name: &str) -> Option<&ComponentDef> {
        self.components
            .iter()
            .find(|component| component.name == name)
    }
}
