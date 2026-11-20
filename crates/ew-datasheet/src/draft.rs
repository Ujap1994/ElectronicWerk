//! The part draft: everything read from a datasheet that makes a part —
//! the part's names, its pins and its packages with their dimensions. Each
//! value keeps where it was read, so the user can check it against the
//! datasheet. The same draft comes from the local recognition, from an AI
//! agent through the engine, or from the user's corrections.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::text::Rect;

/// Where a value was read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// Page number (1-based).
    pub page: usize,
    /// Bounds on the page (points, top-left origin), when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bbox: Option<Rect>,
}

impl Source {
    /// A source on a page.
    #[must_use]
    pub const fn at(page: usize, bbox: Rect) -> Self {
        Self {
            page,
            bbox: Some(bbox),
        }
    }
}

/// The electrical type of a pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PinType {
    /// Input.
    Input,
    /// Output.
    Output,
    /// Input and output.
    Bidirectional,
    /// Tri-state output.
    TriState,
    /// Passive (resistor ends, analog pins, exposed pads).
    Passive,
    /// Power input (supply and ground pins).
    PowerIn,
    /// Power output (regulator outputs).
    PowerOut,
    /// Open collector or open drain.
    OpenCollector,
    /// Not connected inside.
    NotConnected,
    /// Not known.
    #[default]
    Unspecified,
}

/// A pin of the part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinDraft {
    /// Name (`VCC`, `PA0`, `OUT1`).
    pub name: String,
    /// Pad numbers per package name; one pin can have several pads
    /// (`GND` on 6 and 7). A package without an entry does not have the
    /// pin.
    pub numbers: BTreeMap<String, Vec<String>>,
    /// Electrical type.
    #[serde(default)]
    pub kind: PinType,
    /// Function from the datasheet.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub function: String,
    /// Where it was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

/// The family of a package: decides the footprint and the 3D body.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum Family {
    /// Gull-wing leads on two sides (SOIC, SOP, SSOP, TSSOP, MSOP).
    Sop,
    /// Gull-wing leads on four sides (QFP, LQFP, TQFP).
    Qfp,
    /// No leads, terminals on four sides (QFN, VQFN, WQFN).
    Qfn,
    /// No leads, terminals on two sides (DFN, SON, WSON).
    Dfn,
    /// Ball grid array.
    Bga,
    /// Small outline transistor with gull-wing leads (SOT-23, SC-70,
    /// SOT-363, SC-74).
    Sot23,
    /// SOT-223: three leads and a tab.
    Sot223,
    /// SOT-89: flat leads under the body and a tab.
    Sot89,
    /// TO-252 (DPAK) and TO-263 (D²PAK): leads on one side, a tab on the
    /// other.
    Dpak,
    /// Through-hole dual in-line (DIP, PDIP).
    Dip,
    /// Through-hole transistor with a tab (TO-220, TO-247).
    To220,
    /// Through-hole transistor without a tab (TO-92).
    To92,
    /// Two-terminal chip (0402 … 2512, SOD-323 as a chip).
    Chip,
    /// Pin header or another connector in a row.
    Header,
    /// Not known.
    #[default]
    Other,
}

/// One dimension of a package (millimetres).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dimension {
    /// Smallest value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Nominal value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nom: Option<f64>,
    /// Largest value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Where it was read; `None` for values of the standard package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

impl Dimension {
    /// A dimension from a nominal value.
    #[must_use]
    pub const fn nominal(value: f64) -> Self {
        Self {
            min: None,
            nom: Some(value),
            max: None,
            source: None,
        }
    }

    /// A dimension from a range.
    #[must_use]
    pub const fn range(min: f64, max: f64) -> Self {
        Self {
            min: Some(min),
            nom: None,
            max: Some(max),
            source: None,
        }
    }

    /// The nominal value, or the middle of the range.
    #[must_use]
    pub fn value(&self) -> Option<f64> {
        self.nom.or(match (self.min, self.max) {
            (Some(a), Some(b)) => Some(f64::midpoint(a, b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        })
    }

    /// The smallest value (or the nominal one).
    #[must_use]
    pub fn least(&self) -> Option<f64> {
        self.min.or_else(|| self.value())
    }

    /// The largest value (or the nominal one).
    #[must_use]
    pub fn most(&self) -> Option<f64> {
        self.max.or_else(|| self.value())
    }
}

/// A package the part comes in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDraft {
    /// Name as the datasheet writes it (`SOIC-8`, `SOT25`, `D (SOIC)`); the
    /// key of the pin numbers.
    pub name: String,
    /// Family.
    #[serde(default)]
    pub family: Family,
    /// Number of terminals (without an exposed pad).
    #[serde(default)]
    pub pins: u32,
    /// Exposed (thermal) pad under the body.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exposed_pad: bool,
    /// Dimensions by JEDEC symbol (`A`, `A1`, `b`, `c`, `D`, `E`, `E1`,
    /// `E2`, `D2`, `e`, `L`), in millimetres.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dimensions: BTreeMap<String, Dimension>,
    /// Where the package was named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// Where its dimensions are (a drawing or a table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawing: Option<Source>,
}

/// A note for the user: a code the app translates, the English text and
/// the values of the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    /// Code (`pinout`, `scanned`, `pad-no-pin` …).
    pub code: String,
    /// English text.
    pub message: String,
    /// Values for the translated text.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
}

impl Note {
    /// A note with values.
    #[must_use]
    pub fn new(code: &str, message: impl Into<String>, args: &[(&str, String)]) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
            args: args
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
        }
    }
}

/// Everything known about a part.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    /// Part name (`EW321`, `LM358`).
    pub name: String,
    /// Manufacturer.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub manufacturer: String,
    /// Manufacturer part number(s) as ordered.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mpn: String,
    /// Short description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Link or path of the datasheet.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub datasheet: String,
    /// Packages.
    #[serde(default)]
    pub packages: Vec<PackageDraft>,
    /// Pins.
    #[serde(default)]
    pub pins: Vec<PinDraft>,
    /// Where the name was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// Notes of the recognition for the user (pages without text, tables
    /// that could not be read).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
}

impl Draft {
    /// The pins a package has, with their pad numbers, in pad order.
    #[must_use]
    pub fn pins_of(&self, package: &str) -> Vec<(&PinDraft, &[String])> {
        let mut out: Vec<(&PinDraft, &[String])> = self
            .pins
            .iter()
            .filter_map(|p| p.numbers.get(package).map(|n| (p, n.as_slice())))
            .filter(|(_, n)| !n.is_empty())
            .collect();
        out.sort_by(|a, b| pad_order(&a.1[0], &b.1[0]));
        out
    }
}

/// Orders pad numbers naturally (`2` before `10`, `A2` before `A10`, rows
/// of balls by letter).
#[must_use]
pub fn pad_order(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| {
        let letters: String = s.chars().take_while(|c| !c.is_ascii_digit()).collect();
        let digits: String = s
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(char::is_ascii_digit)
            .collect();
        (
            letters.len(),
            letters,
            digits.parse::<u64>().unwrap_or(u64::MAX),
        )
    };
    split(a).cmp(&split(b)).then_with(|| a.cmp(b))
}
