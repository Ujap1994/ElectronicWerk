//! Printed circuit boards.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::collection::Collection;
use crate::id::Id;
use crate::layer::{CopperLayer, Layer, LayerSpec, Side};
use crate::shape::Geometry;
use crate::text::TextStyle;
use crate::units::{Angle, Nm, Point, Rect, Size, Vec3, um};
use crate::{default_true, impl_keyed, is_default};

/// Kind of dielectric layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DielectricKind {
    /// Rigid core laminate.
    #[default]
    Core,
    /// Prepreg (bonding layer).
    Prepreg,
}

/// One physical layer of the board stack-up.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StackupLayer {
    /// Silkscreen print.
    #[serde(rename_all = "camelCase")]
    Silkscreen {
        /// Board side.
        side: Side,
        /// Colour name, for example `white`.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        color: String,
    },
    /// Solder mask.
    #[serde(rename_all = "camelCase")]
    SolderMask {
        /// Board side.
        side: Side,
        /// Thickness.
        thickness: Nm,
        /// Colour name, for example `green`.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        color: String,
    },
    /// Copper.
    #[serde(rename_all = "camelCase")]
    Copper {
        /// Which copper layer.
        layer: CopperLayer,
        /// User name (empty: the default name is shown).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        name: String,
        /// Thickness.
        thickness: Nm,
    },
    /// Insulating material between copper layers.
    #[serde(rename_all = "camelCase")]
    Dielectric {
        /// Core or prepreg.
        kind: DielectricKind,
        /// Thickness.
        thickness: Nm,
        /// Material, for example `FR4`.
        material: String,
        /// Relative permittivity.
        epsilon_r: f64,
        /// Loss tangent.
        loss_tangent: f64,
    },
}

impl StackupLayer {
    /// Thickness of the layer (silkscreen counts as 0).
    #[must_use]
    pub const fn thickness(&self) -> Nm {
        match self {
            Self::Silkscreen { .. } => 0,
            Self::SolderMask { thickness, .. }
            | Self::Copper { thickness, .. }
            | Self::Dielectric { thickness, .. } => *thickness,
        }
    }
}

/// Standard copper thickness (35 µm, 1 oz/ft²).
pub const COPPER_1OZ: Nm = um(35);
/// Standard solder mask thickness.
pub const MASK_THICKNESS: Nm = um(10);
/// Standard board thickness (1.6 mm).
pub const STANDARD_THICKNESS: Nm = um(1_600);

/// The layer stack of a board, from top to bottom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stackup {
    /// Layers from top to bottom.
    pub layers: Vec<StackupLayer>,
    /// Surface finish, for example `HASL` or `ENIG`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub finish: String,
}

impl Stackup {
    /// A standard FR4 stack with `copper_layers` copper layers (at least 1)
    /// and the given total thickness.
    #[must_use]
    pub fn standard(copper_layers: u8, thickness: Nm) -> Self {
        let copper = CopperLayer::stack(copper_layers.max(1));
        let count = i64::try_from(copper.len()).unwrap_or(1);
        let dielectrics = (count - 1).max(1);
        let masks = if count > 1 { 2 } else { 1 };
        let fixed = count * COPPER_1OZ + masks * MASK_THICKNESS;
        let available = (thickness - fixed).max(dielectrics * um(50));
        let each = available / dielectrics;
        let first_extra = available - each * dielectrics;

        let mut layers = vec![
            StackupLayer::Silkscreen {
                side: Side::Top,
                color: "white".into(),
            },
            StackupLayer::SolderMask {
                side: Side::Top,
                thickness: MASK_THICKNESS,
                color: "green".into(),
            },
        ];
        for (index, layer) in copper.iter().enumerate() {
            layers.push(StackupLayer::Copper {
                layer: *layer,
                name: String::new(),
                thickness: COPPER_1OZ,
            });
            if index + 1 < copper.len() || copper.len() == 1 {
                let kind = if copper.len() <= 2 || index % 2 == 1 {
                    DielectricKind::Core
                } else {
                    DielectricKind::Prepreg
                };
                layers.push(StackupLayer::Dielectric {
                    kind,
                    thickness: each + if index == 0 { first_extra } else { 0 },
                    material: "FR4".into(),
                    epsilon_r: 4.5,
                    loss_tangent: 0.02,
                });
            }
        }
        if copper.len() > 1 {
            layers.extend([
                StackupLayer::SolderMask {
                    side: Side::Bottom,
                    thickness: MASK_THICKNESS,
                    color: "green".into(),
                },
                StackupLayer::Silkscreen {
                    side: Side::Bottom,
                    color: "white".into(),
                },
            ]);
        }
        Self {
            layers,
            finish: "HASL".into(),
        }
    }

    /// The copper layers from top to bottom.
    pub fn copper_layers(&self) -> impl Iterator<Item = CopperLayer> + '_ {
        self.layers.iter().filter_map(|layer| match layer {
            StackupLayer::Copper { layer, .. } => Some(*layer),
            _ => None,
        })
    }

    /// Number of copper layers.
    #[must_use]
    pub fn copper_count(&self) -> u8 {
        u8::try_from(self.copper_layers().count()).unwrap_or(u8::MAX)
    }

    /// Total thickness.
    #[must_use]
    pub fn thickness(&self) -> Nm {
        self.layers.iter().map(StackupLayer::thickness).sum()
    }

    /// `true` if `layer` exists in this stack-up (non-copper layers always
    /// exist).
    #[must_use]
    pub fn has_layer(&self, layer: Layer) -> bool {
        match layer {
            Layer::Copper(copper) => self.copper_layers().any(|own| own == copper),
            _ => true,
        }
    }
}

/// An electrical net on the board.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Net {
    /// Object id.
    pub id: Id,
    /// Net name.
    pub name: String,
    /// Net class name (empty: `Default`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub class: String,
}

/// Pad mounting type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PadKind {
    /// Surface-mount pad.
    #[default]
    Smd,
    /// Plated through-hole pad.
    ThroughHole,
    /// Non-plated hole (mounting hole).
    NpThroughHole,
    /// Edge connector contact (no paste).
    Connector,
}

/// Pad shape.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PadShape {
    /// Circle (diameter = width).
    Circle,
    /// Rectangle.
    Rect,
    /// Obround (stadium).
    Oval,
    /// Rectangle with rounded corners.
    RoundRect {
        /// Corner radius in thousandths of the shorter side (0–500).
        ratio: u16,
    },
    /// A trapezoid: the size plus a delta applied to opposite edges.
    Trapezoid {
        /// Edge delta.
        delta: Size,
    },
    /// A custom polygon relative to the pad centre.
    Custom {
        /// Outline.
        points: Vec<Point>,
    },
}

/// A drilled hole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Drill {
    /// Hole diameter (slot width for slots).
    pub diameter: Nm,
    /// Slot length along the pad's x axis, for oval holes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_length: Option<Nm>,
    /// Hole offset from the pad centre.
    #[serde(default, skip_serializing_if = "is_default")]
    pub offset: Point,
}

/// A footprint pad. Coordinates are relative to the footprint origin, as seen
/// from the top.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pad {
    /// Pad id.
    pub id: Id,
    /// Pad number (matches the symbol pin number).
    pub number: String,
    /// Net on the board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<Id>,
    /// Mounting type.
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: PadKind,
    /// Shape.
    pub shape: PadShape,
    /// Centre.
    pub at: Point,
    /// Rotation relative to the footprint.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: Angle,
    /// Size.
    pub size: Size,
    /// Hole, for through-hole pads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drill: Option<Drill>,
    /// Layers the pad exists on.
    pub layers: Vec<LayerSpec>,
    /// Clearance override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clearance: Option<Nm>,
    /// Solder mask expansion override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_margin: Option<Nm>,
    /// Solder paste margin override (negative shrinks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_margin: Option<Nm>,
}

/// A drawn graphic inside a footprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Graphic {
    /// Layer.
    pub layer: Layer,
    /// Geometry.
    pub geometry: Geometry,
    /// Line width.
    pub width: Nm,
    /// Filled (closed shapes only).
    #[serde(default, skip_serializing_if = "is_default")]
    pub filled: bool,
}

/// What a footprint text shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FootprintTextKind {
    /// The reference designator.
    #[default]
    Designator,
    /// The value.
    Value,
    /// Free text.
    User,
}

/// A text inside a footprint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FootprintText {
    /// What the text shows.
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: FootprintTextKind,
    /// Content for user texts.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Layer.
    pub layer: Layer,
    /// Anchor relative to the footprint origin.
    pub at: Point,
    /// Rotation relative to the footprint.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: Angle,
    /// Style.
    pub style: TextStyle,
    /// Shown.
    #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
    pub visible: bool,
}

/// Scale factors of a 3D model, serialized as `[x, y, z]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Scale3(pub [f64; 3]);

impl Default for Scale3 {
    fn default() -> Self {
        Self([1.0; 3])
    }
}

/// A 3D model attached to a footprint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model3d {
    /// Model file (STEP or WRL), relative to the project or a library.
    pub path: String,
    /// Offset from the footprint origin.
    #[serde(default, skip_serializing_if = "is_default")]
    pub offset: Vec3,
    /// Rotation about the x, y and z axes.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: [Angle; 3],
    /// Scale factors.
    #[serde(default, skip_serializing_if = "is_default")]
    pub scale: Scale3,
    /// Hidden models are kept but not shown.
    #[serde(default, skip_serializing_if = "is_default")]
    pub hidden: bool,
}

/// How a part is mounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mounting {
    /// Surface mount.
    #[default]
    Smd,
    /// Through hole.
    ThroughHole,
    /// Neither (fiducials, logos, mounting holes).
    Other,
}

/// Footprint assembly attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FootprintAttributes {
    /// Mounting type.
    #[serde(skip_serializing_if = "is_default")]
    pub mounting: Mounting,
    /// Leave out of the bill of materials.
    #[serde(skip_serializing_if = "is_default")]
    pub exclude_from_bom: bool,
    /// Leave out of pick-and-place files.
    #[serde(skip_serializing_if = "is_default")]
    pub exclude_from_placement: bool,
    /// Do not populate.
    #[serde(skip_serializing_if = "is_default")]
    pub dnp: bool,
}

/// A footprint placed on the board. It carries its own copy of the
/// geometry, so a later library change never alters the board silently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FootprintInstance {
    /// Object id.
    pub id: Id,
    /// The schematic component this footprint belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<Id>,
    /// Source footprint name in the project library.
    pub footprint: String,
    /// Reference designator.
    pub designator: String,
    /// Value.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    /// Origin position.
    pub at: Point,
    /// Rotation.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: Angle,
    /// Board side (bottom mirrors the footprint).
    #[serde(default, skip_serializing_if = "is_default")]
    pub side: Side,
    /// Locked against moving.
    #[serde(default, skip_serializing_if = "is_default")]
    pub locked: bool,
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
    /// Further parameters.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
    /// Assembly attributes.
    #[serde(default, skip_serializing_if = "is_default")]
    pub attributes: FootprintAttributes,
}

/// A copper track segment (straight, or an arc through `mid`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// Object id.
    pub id: Id,
    /// Copper layer.
    pub layer: CopperLayer,
    /// Net.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<Id>,
    /// Width.
    pub width: Nm,
    /// Start point.
    pub start: Point,
    /// End point.
    pub end: Point,
    /// A point on the arc, for arc tracks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mid: Option<Point>,
    /// Locked.
    #[serde(default, skip_serializing_if = "is_default")]
    pub locked: bool,
}

/// A via.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Via {
    /// Object id.
    pub id: Id,
    /// Centre.
    pub at: Point,
    /// Pad diameter.
    pub diameter: Nm,
    /// Drill diameter.
    pub drill: Nm,
    /// Net.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<Id>,
    /// First copper layer.
    #[serde(default = "top_copper", skip_serializing_if = "is_top_copper")]
    pub from: CopperLayer,
    /// Last copper layer.
    #[serde(default = "bottom_copper", skip_serializing_if = "is_bottom_copper")]
    pub to: CopperLayer,
    /// Laser-drilled microvia.
    #[serde(default, skip_serializing_if = "is_default")]
    pub micro: bool,
    /// Locked.
    #[serde(default, skip_serializing_if = "is_default")]
    pub locked: bool,
}

const fn top_copper() -> CopperLayer {
    CopperLayer::Top
}

const fn bottom_copper() -> CopperLayer {
    CopperLayer::Bottom
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_top_copper(layer: &CopperLayer) -> bool {
    *layer == CopperLayer::Top
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_bottom_copper(layer: &CopperLayer) -> bool {
    *layer == CopperLayer::Bottom
}

/// How pads connect to a copper pour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PadConnection {
    /// Thermal relief spokes.
    #[default]
    Thermal,
    /// Solid connection.
    Solid,
    /// Not connected.
    None,
}

/// When isolated pour islands are removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IslandRemoval {
    /// Always remove islands.
    #[default]
    Always,
    /// Keep islands.
    Never,
    /// Remove islands smaller than this area (nm²).
    BelowArea(u64),
}

/// Copper pour fill settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ZoneFill {
    /// Clearance to other nets.
    pub clearance: Nm,
    /// Minimum copper width.
    pub min_width: Nm,
    /// Pad connection style.
    #[serde(skip_serializing_if = "is_default")]
    pub pad_connection: PadConnection,
    /// Thermal relief gap.
    pub thermal_gap: Nm,
    /// Thermal spoke width.
    pub thermal_spoke_width: Nm,
    /// Hatched fill (line width, gap) instead of solid copper.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hatch: Option<Size>,
    /// Island removal.
    #[serde(skip_serializing_if = "is_default")]
    pub island_removal: IslandRemoval,
}

impl Default for ZoneFill {
    fn default() -> Self {
        Self {
            clearance: um(300),
            min_width: um(250),
            pad_connection: PadConnection::Thermal,
            thermal_gap: um(500),
            thermal_spoke_width: um(500),
            hatch: None,
            island_removal: IslandRemoval::Always,
        }
    }
}

/// What a keep-out area forbids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, as in every ECAD keep-out dialog"
)]
pub struct Keepout {
    /// No tracks.
    pub tracks: bool,
    /// No vias.
    pub vias: bool,
    /// No pads.
    pub pads: bool,
    /// No copper pours.
    pub pours: bool,
    /// No footprints.
    pub footprints: bool,
}

/// A copper pour or keep-out area.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    /// Object id.
    pub id: Id,
    /// Optional name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Net of a copper pour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<Id>,
    /// Copper layers.
    pub layers: Vec<CopperLayer>,
    /// Outline (closed polygon).
    pub outline: Vec<Point>,
    /// Cut-outs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<Point>>,
    /// Fill priority (higher fills first).
    #[serde(default, skip_serializing_if = "is_default")]
    pub priority: u32,
    /// Makes this zone a keep-out area instead of a pour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keepout: Option<Keepout>,
    /// Fill settings.
    #[serde(default)]
    pub fill: ZoneFill,
    /// The filled copper (computed by the zone filler or imported).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filled: Vec<FilledArea>,
}

/// One filled copper polygon of a zone. Holes are cut in with slits, so
/// every area is a single outline.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilledArea {
    /// The copper layer.
    pub layer: CopperLayer,
    /// The outline.
    pub points: Vec<Point>,
}

/// Free text on the board.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardText {
    /// Object id.
    pub id: Id,
    /// Layer.
    pub layer: Layer,
    /// Content (text variables allowed).
    pub text: String,
    /// Anchor.
    pub at: Point,
    /// Rotation.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: Angle,
    /// Style.
    pub style: TextStyle,
    /// Mirrored (normal for bottom-side texts).
    #[serde(default, skip_serializing_if = "is_default")]
    pub mirrored: bool,
}

/// A drawn graphic on the board (for example the outline).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardShape {
    /// Object id.
    pub id: Id,
    /// Layer.
    pub layer: Layer,
    /// Geometry.
    pub geometry: Geometry,
    /// Line width.
    pub width: Nm,
    /// Filled (closed shapes only).
    #[serde(default, skip_serializing_if = "is_default")]
    pub filled: bool,
}

impl_keyed!(
    Net,
    FootprintInstance,
    Track,
    Via,
    Zone,
    BoardText,
    BoardShape
);

/// A board (`.ewpcb`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pcb {
    /// Document id.
    pub id: Id,
    /// Board name.
    pub name: String,
    /// Layer stack-up.
    pub stackup: Stackup,
    /// Drill and placement origin.
    #[serde(default, skip_serializing_if = "is_default")]
    pub origin: Point,
    /// Nets.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub nets: Collection<Net>,
    /// Footprints.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub footprints: Collection<FootprintInstance>,
    /// Tracks.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub tracks: Collection<Track>,
    /// Vias.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub vias: Collection<Via>,
    /// Pours and keep-out areas.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub zones: Collection<Zone>,
    /// Texts.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub texts: Collection<BoardText>,
    /// Graphics, including the board outline.
    #[serde(default, skip_serializing_if = "Collection::is_empty")]
    pub shapes: Collection<BoardShape>,
}

/// Editable board properties (everything except the objects).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbProps {
    /// Board name.
    pub name: String,
    /// Stack-up.
    pub stackup: Stackup,
    /// Drill and placement origin.
    #[serde(default)]
    pub origin: Point,
}

impl Pcb {
    /// An empty board with the given stack-up.
    #[must_use]
    pub fn new(id: Id, name: impl Into<String>, stackup: Stackup) -> Self {
        Self {
            id,
            name: name.into(),
            stackup,
            origin: Point::ORIGIN,
            nets: Collection::new(),
            footprints: Collection::new(),
            tracks: Collection::new(),
            vias: Collection::new(),
            zones: Collection::new(),
            texts: Collection::new(),
            shapes: Collection::new(),
        }
    }

    /// The editable properties.
    #[must_use]
    pub fn props(&self) -> PcbProps {
        PcbProps {
            name: self.name.clone(),
            stackup: self.stackup.clone(),
            origin: self.origin,
        }
    }

    /// Replaces the editable properties.
    pub fn set_props(&mut self, props: PcbProps) {
        self.name = props.name;
        self.stackup = props.stackup;
        self.origin = props.origin;
    }

    /// Bounding box of the board outline, if one is drawn.
    #[must_use]
    pub fn outline_bounds(&self) -> Option<Rect> {
        self.shapes
            .iter()
            .filter(|shape| shape.layer == Layer::Outline)
            .filter_map(|shape| shape.geometry.bounds())
            .reduce(Rect::union)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::mm;

    #[test]
    fn standard_stackups_have_the_requested_thickness() {
        for count in [1_u8, 2, 4, 6, 8, 10] {
            let stackup = Stackup::standard(count, STANDARD_THICKNESS);
            assert_eq!(stackup.copper_count(), count);
            assert_eq!(stackup.thickness(), STANDARD_THICKNESS, "{count} layers");
            let json = serde_json::to_string(&stackup).unwrap();
            assert_eq!(serde_json::from_str::<Stackup>(&json).unwrap(), stackup);
        }
        let four = Stackup::standard(4, STANDARD_THICKNESS);
        let kinds: Vec<DielectricKind> = four
            .layers
            .iter()
            .filter_map(|layer| match layer {
                StackupLayer::Dielectric { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                DielectricKind::Prepreg,
                DielectricKind::Core,
                DielectricKind::Prepreg
            ]
        );
        assert!(four.has_layer(Layer::Copper(CopperLayer::Inner(2))));
        assert!(!four.has_layer(Layer::Copper(CopperLayer::Inner(3))));
    }

    #[test]
    fn via_defaults_are_omitted() {
        let via = Via {
            id: Id::from_u128(3),
            at: Point::mm(1, 1),
            diameter: um(600),
            drill: um(300),
            net: None,
            from: CopperLayer::Top,
            to: CopperLayer::Bottom,
            micro: false,
            locked: false,
        };
        let json = serde_json::to_string(&via).unwrap();
        assert!(!json.contains("from") && !json.contains("to"), "{json}");
        assert_eq!(serde_json::from_str::<Via>(&json).unwrap(), via);
    }

    #[test]
    fn outline_bounds() {
        let mut pcb = Pcb::new(
            Id::random(),
            "Board",
            Stackup::standard(2, STANDARD_THICKNESS),
        );
        assert!(pcb.outline_bounds().is_none());
        pcb.shapes.insert(BoardShape {
            id: Id::random(),
            layer: Layer::Outline,
            geometry: Geometry::Rect {
                start: Point::ORIGIN,
                end: Point::mm(100, 80),
            },
            width: um(100),
            filled: false,
        });
        pcb.shapes.insert(BoardShape {
            id: Id::random(),
            layer: Layer::Silkscreen(Side::Top),
            geometry: Geometry::Line {
                start: Point::mm(-50, 0),
                end: Point::mm(0, 0),
            },
            width: um(100),
            filled: false,
        });
        let bounds = pcb.outline_bounds().unwrap();
        assert_eq!((bounds.width(), bounds.height()), (mm(100), mm(80)));
    }

    #[test]
    fn pad_shapes_serialize_readably() {
        assert_eq!(serde_json::to_string(&PadShape::Rect).unwrap(), r#""rect""#);
        assert_eq!(
            serde_json::to_string(&PadShape::RoundRect { ratio: 250 }).unwrap(),
            r#"{"roundRect":{"ratio":250}}"#
        );
        assert_eq!(
            serde_json::to_string(&IslandRemoval::BelowArea(5)).unwrap(),
            r#"{"belowArea":5}"#
        );
    }
}
