//! A system of several boards (`.ewsys`, M18): the boards (of this project
//! or of other projects), how they connect — connectors that mate directly
//! (stacked boards, board-to-board connectors) and cables — and where they
//! sit in the system's 3D assembly.
//!
//! A system holds no objects of its own: the whole document is its
//! properties, so every edit is one undo step of `document.setProperties`.

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::units::{Angle, Nm, Vec3};

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

const fn one() -> u32 {
    1
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde skip predicate")]
const fn is_one(value: &u32) -> bool {
    *value == 1
}

/// Where a board of a system comes from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum BoardSource {
    /// A board document of this project.
    Local {
        /// The board's document id.
        doc: Id,
    },
    /// A board of another ElectronicWerk project (also reference boards
    /// imported from open design files).
    Project {
        /// The project file or folder, relative to this project's folder
        /// (`../Carrier/Carrier.ewproj`) or absolute.
        path: String,
        /// The board's name or document id (empty: the first board).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        board: String,
    },
}

/// A board of the system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemBoard {
    /// Id within the system.
    pub id: Id,
    /// Name in the system (`Carrier`, `Shield`).
    pub name: String,
    /// The board.
    pub source: BoardSource,
    /// How many of it the system has (for the bill of materials).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub quantity: u32,
    /// Where the board's origin goes in the system (nm); z is the height
    /// of the board's bottom face.
    #[serde(default, skip_serializing_if = "is_default")]
    pub position: Vec3,
    /// Rotation about x, y and z (applied in that order).
    #[serde(default, skip_serializing_if = "is_default")]
    pub rotation: [Angle; 3],
    /// Not shown in the 3D assembly.
    #[serde(default, skip_serializing_if = "is_default")]
    pub hidden: bool,
}

/// A connector of a board in the system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorRef {
    /// The system board.
    pub board: Id,
    /// The connector's designator on that board.
    pub designator: String,
}

/// How the pins of two mating connectors pair up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pairing {
    /// Pin numbers match (1 to 1, 2 to 2 …): headers and sockets, most
    /// board-to-board pairs.
    #[default]
    Number,
    /// Pin numbers of two-row connectors swap rows (1 to 2, 3 to 4 …), as
    /// when one connector is turned over.
    Mirrored,
    /// Pins mate where they lie on each other in the 3D assembly.
    Position,
}

/// Two connectors that mate directly: a board stacked on another, or a
/// board-to-board connector pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    /// Id within the system.
    pub id: Id,
    /// One connector.
    pub a: ConnectorRef,
    /// The connector it mates with.
    pub b: ConnectorRef,
    /// How the pins pair up.
    #[serde(default, skip_serializing_if = "is_default")]
    pub pairing: Pairing,
}

/// One end of a cable: a connector on a board, or a free end.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CableEnd {
    /// Id within the cable.
    pub id: Id,
    /// Label on the drawing (`X1`).
    pub label: String,
    /// The connector it plugs into (none: a free end, a switch, a sensor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector: Option<ConnectorRef>,
    /// The cable's own connector (housing), for the bill of materials
    /// (`JST PHR-4`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub housing: String,
}

/// One end of a wire: a cable end and a pin of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireEnd {
    /// The cable end.
    pub end: Id,
    /// The pin number.
    pub pin: String,
}

/// A wire of a cable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wire {
    /// Id within the cable.
    pub id: Id,
    /// Where it starts.
    pub from: WireEnd,
    /// Where it ends.
    pub to: WireEnd,
    /// The signal it carries (empty: from the boards).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signal: String,
    /// Colour (`red`, `RD`, `#d00`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color: String,
    /// Cross-section (`AWG 24`, `0.25 mm²`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gauge: String,
    /// Length (nm); `None`: the cable's length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<Nm>,
}

/// A cable or harness between boards.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cable {
    /// Id within the system.
    pub id: Id,
    /// Name (`W1`, `Display cable`).
    pub name: String,
    /// Cable type or part number (`Ribbon 10-way`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub part: String,
    /// Length of the wires (nm).
    #[serde(default, skip_serializing_if = "is_default")]
    pub length: Nm,
    /// Ends.
    #[serde(default)]
    pub ends: Vec<CableEnd>,
    /// Wires.
    #[serde(default)]
    pub wires: Vec<Wire>,
}

/// A system document (`.ewsys`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct System {
    /// Document id.
    pub id: Id,
    /// Name.
    pub name: String,
    /// Boards.
    #[serde(default)]
    pub boards: Vec<SystemBoard>,
    /// Connectors that mate directly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
    /// Cables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cables: Vec<Cable>,
    /// Signal names that count as the same across boards
    /// (`["SDA", "I2C_SDA"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<Vec<String>>,
    /// Accepted findings of the system check (their keys).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclusions: Vec<String>,
}

/// Editable system properties (the whole document except its id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemProps {
    /// Name.
    pub name: String,
    /// Boards.
    #[serde(default)]
    pub boards: Vec<SystemBoard>,
    /// Links.
    #[serde(default)]
    pub links: Vec<Link>,
    /// Cables.
    #[serde(default)]
    pub cables: Vec<Cable>,
    /// Aliases.
    #[serde(default)]
    pub aliases: Vec<Vec<String>>,
    /// Accepted findings.
    #[serde(default)]
    pub exclusions: Vec<String>,
}

impl System {
    /// An empty system.
    #[must_use]
    pub fn new(id: Id, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            boards: Vec::new(),
            links: Vec::new(),
            cables: Vec::new(),
            aliases: Vec::new(),
            exclusions: Vec::new(),
        }
    }

    /// The editable properties.
    #[must_use]
    pub fn props(&self) -> SystemProps {
        SystemProps {
            name: self.name.clone(),
            boards: self.boards.clone(),
            links: self.links.clone(),
            cables: self.cables.clone(),
            aliases: self.aliases.clone(),
            exclusions: self.exclusions.clone(),
        }
    }

    /// Replaces the editable properties.
    pub fn set_props(&mut self, props: SystemProps) {
        self.name = props.name;
        self.boards = props.boards;
        self.links = props.links;
        self.cables = props.cables;
        self.aliases = props.aliases;
        self.exclusions = props.exclusions;
    }

    /// A board of the system by id.
    #[must_use]
    pub fn board(&self, id: Id) -> Option<&SystemBoard> {
        self.boards.iter().find(|b| b.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_with_short_json() {
        let carrier = Id::from_u128(1);
        let shield = Id::from_u128(2);
        let mut system = System::new(Id::from_u128(9), "Station");
        system.boards = vec![
            SystemBoard {
                id: carrier,
                name: "Carrier".to_owned(),
                source: BoardSource::Local {
                    doc: Id::from_u128(10),
                },
                quantity: 1,
                position: Vec3::default(),
                rotation: [Angle::ZERO; 3],
                hidden: false,
            },
            SystemBoard {
                id: shield,
                name: "Shield".to_owned(),
                source: BoardSource::Project {
                    path: "../Shield/Shield.ewproj".to_owned(),
                    board: String::new(),
                },
                quantity: 2,
                position: Vec3 {
                    x: 0,
                    y: 0,
                    z: 10_000_000,
                },
                rotation: [Angle::ZERO; 3],
                hidden: false,
            },
        ];
        system.links = vec![Link {
            id: Id::from_u128(3),
            a: ConnectorRef {
                board: carrier,
                designator: "J1".to_owned(),
            },
            b: ConnectorRef {
                board: shield,
                designator: "P1".to_owned(),
            },
            pairing: Pairing::Number,
        }];
        let json = serde_json::to_value(&system).unwrap_or_default();
        assert_eq!(json["boards"][0].get("quantity"), None);
        assert_eq!(json["boards"][1]["quantity"], 2);
        assert_eq!(json["boards"][1]["source"]["kind"], "project");
        assert_eq!(json["links"][0].get("pairing"), None);
        assert_eq!(json.get("cables"), None);
        let back: System =
            serde_json::from_value(json).unwrap_or_else(|_| System::new(Id::from_u128(0), ""));
        assert_eq!(back, system);
        let mut copy = System::new(system.id, "x");
        copy.set_props(system.props());
        assert_eq!(copy, system);
    }
}
