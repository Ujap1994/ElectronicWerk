//! Schematic sheets.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::collection::Collection;
use crate::id::Id;
use crate::shape::{Fill, Geometry, Stroke};
use crate::text::TextStyle;
use crate::units::{Angle, Nm, Point, Size, mm, um};
use crate::{default_true, impl_keyed, is_default};

/// Paper format of a sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageFormat {
    /// ISO A5 (210 × 148 mm).
    A5,
    /// ISO A4 (297 × 210 mm).
    #[default]
    A4,
    /// ISO A3 (420 × 297 mm).
    A3,
    /// ISO A2 (594 × 420 mm).
    A2,
    /// ISO A1 (841 × 594 mm).
    A1,
    /// ISO A0 (1189 × 841 mm).
    A0,
    /// US Letter (11 × 8.5 in).
    Letter,
    /// US Legal (14 × 8.5 in).
    Legal,
    /// US Tabloid / ANSI B (17 × 11 in).
    Tabloid,
    /// Custom size ([`Page::size`]).
    Custom,
}

impl PageFormat {
    /// Every format in display order.
    pub const ALL: [Self; 10] = [
        Self::A5,
        Self::A4,
        Self::A3,
        Self::A2,
        Self::A1,
        Self::A0,
        Self::Letter,
        Self::Legal,
        Self::Tabloid,
        Self::Custom,
    ];

    /// Landscape size, or `None` for [`PageFormat::Custom`].
    #[must_use]
    pub const fn landscape_size(self) -> Option<Size> {
        let (w, h) = match self {
            Self::A5 => (um(210_000), um(148_000)),
            Self::A4 => (um(297_000), um(210_000)),
            Self::A3 => (um(420_000), um(297_000)),
            Self::A2 => (um(594_000), um(420_000)),
            Self::A1 => (um(841_000), um(594_000)),
            Self::A0 => (um(1_189_000), um(841_000)),
            Self::Letter => (um(279_400), um(215_900)),
            Self::Legal => (um(355_600), um(215_900)),
            Self::Tabloid => (um(431_800), um(279_400)),
            Self::Custom => return None,
        };
        Some(Size::new(w, h))
    }
}

/// Page orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    /// Wider than tall.
    #[default]
    Landscape,
    /// Taller than wide.
    Portrait,
}

/// Sheet paper setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Page {
    /// Paper format.
    pub format: PageFormat,
    /// Orientation (ignored for custom sizes).
    #[serde(skip_serializing_if = "is_default")]
    pub orientation: Orientation,
    /// Size for [`PageFormat::Custom`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
}

impl Page {
    /// Default size of a custom page (A4 landscape).
    pub const DEFAULT_CUSTOM: Size = Size::new(um(297_000), um(210_000));

    /// Width and height of the page.
    #[must_use]
    pub fn dimensions(&self) -> Size {
        match self.format.landscape_size() {
            None => self.size.unwrap_or(Self::DEFAULT_CUSTOM),
            Some(size) => match self.orientation {
                Orientation::Landscape => size,
                Orientation::Portrait => Size::new(size.h, size.w),
            },
        }
    }
}

/// Title block fields. Text variables such as `${PROJECT_NAME}` are allowed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TitleBlock {
    /// Sheet title.
    pub title: String,
    /// Date.
    pub date: String,
    /// Revision.
    pub revision: String,
    /// Company or author.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub company: String,
    /// Free comment lines.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<String>,
}

impl Default for TitleBlock {
    fn default() -> Self {
        Self {
            title: "${SHEET_NAME}".to_owned(),
            date: "${CURRENT_DATE}".to_owned(),
            revision: "${REVISION}".to_owned(),
            company: String::new(),
            comments: Vec::new(),
        }
    }
}

/// Flags that keep a component out of some outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ComponentFlags {
    /// Do not populate (the part stays in the design but is not assembled).
    #[serde(skip_serializing_if = "is_default")]
    pub dnp: bool,
    /// Leave out of the bill of materials.
    #[serde(skip_serializing_if = "is_default")]
    pub exclude_from_bom: bool,
    /// Do not transfer to the board.
    #[serde(skip_serializing_if = "is_default")]
    pub exclude_from_board: bool,
}

/// Where and how one field of a placed symbol is drawn.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldLabel {
    /// Field name: `designator`, `value`, `footprint` or a property key.
    pub name: String,
    /// Text anchor (absolute sheet coordinates).
    pub at: Point,
    /// Text rotation.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: Angle,
    /// Text style.
    #[serde(default, skip_serializing_if = "is_default")]
    pub style: TextStyle,
    /// Whether the field is shown.
    #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
    pub visible: bool,
}

/// A symbol (one unit of a component) placed on a sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacedSymbol {
    /// Object id.
    pub id: Id,
    /// Symbol definition: a symbol name in the project library.
    pub symbol: String,
    /// Unit number (1-based) for multi-unit parts.
    #[serde(default = "first_unit", skip_serializing_if = "is_first_unit")]
    pub unit: u16,
    /// Anchor position.
    pub at: Point,
    /// Rotation (normally a multiple of 90°).
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: Angle,
    /// Mirrored left-to-right before rotating.
    #[serde(default, skip_serializing_if = "is_default")]
    pub mirror: bool,
    /// Reference designator, for example `R1`.
    pub designator: String,
    /// Value, for example `10k`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    /// Footprint name in the project library.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint: Option<String>,
    /// Component this unit belongs to. All units of a part share it, and
    /// the matching board footprint refers to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<Id>,
    /// Further parameters (manufacturer part number, tolerance, …).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
    /// Placement of the visible fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldLabel>,
    /// Output flags.
    #[serde(default, skip_serializing_if = "is_default")]
    pub flags: ComponentFlags,
    /// Designators per sheet instance path, for sheets used more than once.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub instances: BTreeMap<String, String>,
}

const fn first_unit() -> u16 {
    1
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
const fn is_first_unit(unit: &u16) -> bool {
    *unit == 1
}

/// A wire segment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wire {
    /// Object id.
    pub id: Id,
    /// Start point.
    pub start: Point,
    /// End point.
    pub end: Point,
}

/// A bus segment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bus {
    /// Object id.
    pub id: Id,
    /// Start point.
    pub start: Point,
    /// End point.
    pub end: Point,
}

/// A diagonal entry from a bus to a wire.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusEntry {
    /// Object id.
    pub id: Id,
    /// Point on the bus.
    pub at: Point,
    /// Offset to the wire end.
    pub size: Point,
}

/// A junction dot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Junction {
    /// Object id.
    pub id: Id,
    /// Position.
    pub at: Point,
}

/// A no-connect flag on a pin.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoConnect {
    /// Object id.
    pub id: Id,
    /// Position.
    pub at: Point,
}

/// Scope of a net label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LabelKind {
    /// Connects within one sheet.
    #[default]
    Local,
    /// Connects across all sheets.
    Global,
    /// Connects to a sheet pin of the parent sheet.
    Hierarchical,
}

/// Signal direction shown on global labels, hierarchical labels and sheet
/// pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortShape {
    /// No direction.
    #[default]
    Passive,
    /// Input.
    Input,
    /// Output.
    Output,
    /// Bidirectional.
    Bidirectional,
    /// Tri-state.
    TriState,
}

/// A net label.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    /// Object id.
    pub id: Id,
    /// Scope.
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: LabelKind,
    /// Net name.
    pub text: String,
    /// Connection point.
    pub at: Point,
    /// Rotation.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: Angle,
    /// Direction marker.
    #[serde(default, skip_serializing_if = "is_default")]
    pub shape: PortShape,
    /// Text style.
    #[serde(default, skip_serializing_if = "is_default")]
    pub style: TextStyle,
}

/// Symbol drawn for a power port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PowerStyle {
    /// A bar (supply).
    #[default]
    Bar,
    /// An arrow.
    Arrow,
    /// A circle.
    Circle,
    /// A wave (AC).
    Wave,
    /// Power ground.
    Ground,
    /// Signal ground.
    SignalGround,
    /// Earth.
    Earth,
}

/// A power port: connects to the global net with its name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerPort {
    /// Object id.
    pub id: Id,
    /// Net name, for example `GND` or `+3V3`.
    pub net: String,
    /// Connection point.
    pub at: Point,
    /// Rotation.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: Angle,
    /// Symbol style.
    #[serde(default, skip_serializing_if = "is_default")]
    pub style: PowerStyle,
}

/// Edge of a sheet symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SheetSide {
    /// Left edge.
    #[default]
    Left,
    /// Right edge.
    Right,
    /// Top edge.
    Top,
    /// Bottom edge.
    Bottom,
}

/// A pin on a sheet symbol, matching a hierarchical label in the child.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetPin {
    /// Pin id.
    pub id: Id,
    /// Name (equals the hierarchical label text).
    pub name: String,
    /// Direction marker.
    #[serde(default, skip_serializing_if = "is_default")]
    pub shape: PortShape,
    /// Edge the pin sits on.
    pub side: SheetSide,
    /// Distance from the top (left/right edges) or left (top/bottom edges)
    /// corner.
    pub offset: Nm,
}

/// A hierarchical sheet symbol: places a child schematic on this sheet.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetSymbol {
    /// Object id (also the sheet instance id in instance paths).
    pub id: Id,
    /// Instance name shown above the symbol.
    pub name: String,
    /// The child schematic document.
    pub schematic: Id,
    /// Top-left corner.
    pub at: Point,
    /// Size of the box.
    pub size: Size,
    /// Pins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<SheetPin>,
}

/// Free text on a sheet.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    /// Object id.
    pub id: Id,
    /// Content (text variables allowed).
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

/// A drawn graphic on a sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shape {
    /// Object id.
    pub id: Id,
    /// Geometry.
    pub geometry: Geometry,
    /// Outline.
    #[serde(default, skip_serializing_if = "is_default")]
    pub stroke: Stroke,
    /// Fill.
    #[serde(default, skip_serializing_if = "is_default")]
    pub fill: Fill,
}

impl_keyed!(
    PlacedSymbol,
    Wire,
    Bus,
    BusEntry,
    Junction,
    NoConnect,
    Label,
    PowerPort,
    SheetSymbol,
    Text,
    Shape
);

/// A schematic sheet (`.ewsch`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schematic {
    /// Document id (same as in the project manifest).
    pub id: Id,
    /// Sheet name.
    pub name: String,
    /// Paper setup.
    #[serde(default)]
    pub page: Page,
    /// Title block.
    #[serde(default)]
    pub title_block: TitleBlock,
    /// Placed symbols.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub symbols: Collection<PlacedSymbol>,
    /// Wires.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub wires: Collection<Wire>,
    /// Buses.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub buses: Collection<Bus>,
    /// Bus entries.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub bus_entries: Collection<BusEntry>,
    /// Junctions.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub junctions: Collection<Junction>,
    /// Net labels.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub labels: Collection<Label>,
    /// Power ports.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub power_ports: Collection<PowerPort>,
    /// No-connect flags.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub no_connects: Collection<NoConnect>,
    /// Child sheet symbols.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub sheets: Collection<SheetSymbol>,
    /// Free texts.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub texts: Collection<Text>,
    /// Graphics.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub shapes: Collection<Shape>,
}

/// Editable schematic properties (everything except the objects).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchematicProps {
    /// Sheet name.
    pub name: String,
    /// Paper setup.
    pub page: Page,
    /// Title block.
    pub title_block: TitleBlock,
}

impl Schematic {
    /// An empty sheet with default page and title block.
    #[must_use]
    pub fn new(id: Id, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            page: Page::default(),
            title_block: TitleBlock::default(),
            symbols: Collection::new(),
            wires: Collection::new(),
            buses: Collection::new(),
            bus_entries: Collection::new(),
            junctions: Collection::new(),
            labels: Collection::new(),
            power_ports: Collection::new(),
            no_connects: Collection::new(),
            sheets: Collection::new(),
            texts: Collection::new(),
            shapes: Collection::new(),
        }
    }

    /// The editable properties.
    #[must_use]
    pub fn props(&self) -> SchematicProps {
        SchematicProps {
            name: self.name.clone(),
            page: self.page,
            title_block: self.title_block.clone(),
        }
    }

    /// Replaces the editable properties.
    pub fn set_props(&mut self, props: SchematicProps) {
        self.name = props.name;
        self.page = props.page;
        self.title_block = props.title_block;
    }

    /// Ids of the child schematics placed on this sheet.
    pub fn child_schematics(&self) -> impl Iterator<Item = Id> + '_ {
        self.sheets.iter().map(|sheet| sheet.schematic)
    }
}

/// Default size of a new sheet symbol.
pub const DEFAULT_SHEET_SYMBOL_SIZE: Size = Size::new(mm(25), mm(15));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_dimensions() {
        let a4 = Page::default();
        assert_eq!(a4.dimensions(), Size::new(um(297_000), um(210_000)));
        let portrait = Page {
            format: PageFormat::A3,
            orientation: Orientation::Portrait,
            size: None,
        };
        assert_eq!(portrait.dimensions(), Size::new(um(297_000), um(420_000)));
        let custom = Page {
            format: PageFormat::Custom,
            orientation: Orientation::Portrait,
            size: Some(Size::mm(100, 50)),
        };
        assert_eq!(custom.dimensions(), Size::mm(100, 50));
        assert!(
            PageFormat::ALL.iter().all(|format| {
                *format == PageFormat::Custom || format.landscape_size().is_some()
            })
        );
    }

    #[test]
    fn empty_sheet_is_compact_and_round_trips() {
        let sheet = Schematic::new(Id::from_u128(1), "Root");
        let json = serde_json::to_string(&sheet).unwrap();
        assert!(!json.contains("symbols"), "{json}");
        let back: Schematic = serde_json::from_str(&json).unwrap();
        assert_eq!(back, sheet);
    }

    #[test]
    fn placed_symbol_defaults_are_omitted() {
        let symbol = PlacedSymbol {
            id: Id::from_u128(9),
            symbol: "R".into(),
            unit: 1,
            at: Point::mm(10, 20),
            rotation: Angle::ZERO,
            mirror: false,
            designator: "R1".into(),
            value: "10k".into(),
            footprint: None,
            component: None,
            properties: BTreeMap::new(),
            fields: vec![FieldLabel {
                name: "designator".into(),
                at: Point::mm(10, 22),
                rotation: Angle::ZERO,
                style: TextStyle::default(),
                visible: true,
            }],
            flags: ComponentFlags::default(),
            instances: BTreeMap::new(),
        };
        let json = serde_json::to_string(&symbol).unwrap();
        for absent in ["unit", "rotation", "mirror", "flags", "visible", "style"] {
            assert!(!json.contains(absent), "{absent} in {json}");
        }
        assert_eq!(serde_json::from_str::<PlacedSymbol>(&json).unwrap(), symbol);
        let mut hidden = symbol;
        hidden.fields[0].visible = false;
        hidden.unit = 2;
        let json = serde_json::to_string(&hidden).unwrap();
        assert!(json.contains(r#""visible":false"#) && json.contains(r#""unit":2"#));
        assert_eq!(serde_json::from_str::<PlacedSymbol>(&json).unwrap(), hidden);
    }

    #[test]
    fn props_round_trip() {
        let mut sheet = Schematic::new(Id::random(), "A");
        let mut props = sheet.props();
        props.name = "B".into();
        props.page.format = PageFormat::A3;
        sheet.set_props(props.clone());
        assert_eq!(sheet.props(), props);
    }
}
