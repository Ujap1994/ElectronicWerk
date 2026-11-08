//! Board layers.
//!
//! Layers are written as short strings in files, for example `cu.top`,
//! `cu.in1`, `cu.bottom`, `silk.top`, `mask.bottom` or `outline`.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Board side.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    /// Top (component) side.
    #[default]
    Top,
    /// Bottom (solder) side.
    Bottom,
}

impl Side {
    /// The opposite side.
    #[must_use]
    pub const fn flipped(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }

    /// `top` or `bottom`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

/// Maximum number of inner copper layers (64 copper layers in total).
pub const MAX_INNER_LAYERS: u8 = 62;

/// Maximum index of numbered mechanical and user layers.
pub const MAX_NUMBERED_LAYER: u8 = 32;

/// A copper layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CopperLayer {
    /// Top copper.
    Top,
    /// Inner copper layer `1..=62`, counted from the top.
    Inner(u8),
    /// Bottom copper.
    Bottom,
}

impl CopperLayer {
    /// The copper layers of a board with `count` layers, from top to bottom.
    /// A 1-layer board only has [`CopperLayer::Top`].
    #[must_use]
    pub fn stack(count: u8) -> Vec<Self> {
        match count {
            0 => Vec::new(),
            1 => vec![Self::Top],
            n => {
                let inner = (n - 2).min(MAX_INNER_LAYERS);
                let mut layers = Vec::with_capacity(usize::from(inner) + 2);
                layers.push(Self::Top);
                layers.extend((1..=inner).map(Self::Inner));
                layers.push(Self::Bottom);
                layers
            }
        }
    }

    /// The same layer seen from the other side (top ↔ bottom, inner layers
    /// mirrored within a board of `count` layers).
    #[must_use]
    pub const fn flipped(self, count: u8) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Inner(n) => {
                let inner = count.saturating_sub(2);
                if n >= 1 && n <= inner {
                    Self::Inner(inner + 1 - n)
                } else {
                    Self::Inner(n)
                }
            }
        }
    }
}

impl fmt::Display for CopperLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Top => f.write_str("cu.top"),
            Self::Bottom => f.write_str("cu.bottom"),
            Self::Inner(n) => write!(f, "cu.in{n}"),
        }
    }
}

/// Error for an unknown layer name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown layer `{0}`")]
pub struct ParseLayerError(String);

impl ParseLayerError {
    fn new(text: &str) -> Self {
        Self(text.chars().take(32).collect())
    }
}

fn parse_numbered(rest: &str, max: u8) -> Option<u8> {
    if rest.starts_with('0') {
        return None;
    }
    rest.parse::<u8>().ok().filter(|n| (1..=max).contains(n))
}

impl FromStr for CopperLayer {
    type Err = ParseLayerError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "cu.top" => Ok(Self::Top),
            "cu.bottom" => Ok(Self::Bottom),
            _ => text
                .strip_prefix("cu.in")
                .and_then(|rest| parse_numbered(rest, MAX_INNER_LAYERS))
                .map(Self::Inner)
                .ok_or_else(|| ParseLayerError::new(text)),
        }
    }
}

/// Any board layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Layer {
    /// Copper.
    Copper(CopperLayer),
    /// Silkscreen (legend).
    Silkscreen(Side),
    /// Solder mask (openings).
    SolderMask(Side),
    /// Solder paste (stencil openings).
    Paste(Side),
    /// Adhesive (glue dots).
    Adhesive(Side),
    /// Fabrication drawing (assembly outlines, values).
    Fabrication(Side),
    /// Component courtyard (keep-out area of a part).
    Courtyard(Side),
    /// Board outline (edge cuts).
    Outline,
    /// Mechanical layer `1..=32`.
    Mechanical(u8),
    /// User drawing layer `1..=32`.
    User(u8),
}

impl Layer {
    /// Top copper.
    pub const TOP_COPPER: Self = Self::Copper(CopperLayer::Top);
    /// Bottom copper.
    pub const BOTTOM_COPPER: Self = Self::Copper(CopperLayer::Bottom);

    /// The side of a sided layer (outer copper counts as sided).
    #[must_use]
    pub const fn side(self) -> Option<Side> {
        match self {
            Self::Copper(CopperLayer::Top) => Some(Side::Top),
            Self::Copper(CopperLayer::Bottom) => Some(Side::Bottom),
            Self::Silkscreen(side)
            | Self::SolderMask(side)
            | Self::Paste(side)
            | Self::Adhesive(side)
            | Self::Fabrication(side)
            | Self::Courtyard(side) => Some(side),
            Self::Copper(CopperLayer::Inner(_))
            | Self::Outline
            | Self::Mechanical(_)
            | Self::User(_) => None,
        }
    }

    /// `true` for copper layers.
    #[must_use]
    pub const fn is_copper(self) -> bool {
        matches!(self, Self::Copper(_))
    }

    /// The same layer on the other board side (used for parts on the
    /// bottom). Layers without a side stay unchanged.
    #[must_use]
    pub const fn flipped(self, copper_count: u8) -> Self {
        match self {
            Self::Copper(layer) => Self::Copper(layer.flipped(copper_count)),
            Self::Silkscreen(side) => Self::Silkscreen(side.flipped()),
            Self::SolderMask(side) => Self::SolderMask(side.flipped()),
            Self::Paste(side) => Self::Paste(side.flipped()),
            Self::Adhesive(side) => Self::Adhesive(side.flipped()),
            Self::Fabrication(side) => Self::Fabrication(side.flipped()),
            Self::Courtyard(side) => Self::Courtyard(side.flipped()),
            other => other,
        }
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sided = |f: &mut fmt::Formatter<'_>, prefix: &str, side: Side| {
            write!(f, "{prefix}.{}", side.as_str())
        };
        match *self {
            Self::Copper(layer) => fmt::Display::fmt(&layer, f),
            Self::Silkscreen(side) => sided(f, "silk", side),
            Self::SolderMask(side) => sided(f, "mask", side),
            Self::Paste(side) => sided(f, "paste", side),
            Self::Adhesive(side) => sided(f, "adhesive", side),
            Self::Fabrication(side) => sided(f, "fab", side),
            Self::Courtyard(side) => sided(f, "courtyard", side),
            Self::Outline => f.write_str("outline"),
            Self::Mechanical(n) => write!(f, "mech.{n}"),
            Self::User(n) => write!(f, "user.{n}"),
        }
    }
}

impl FromStr for Layer {
    type Err = ParseLayerError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.starts_with("cu.") {
            return text.parse().map(Self::Copper);
        }
        if text == "outline" {
            return Ok(Self::Outline);
        }
        let error = || ParseLayerError::new(text);
        let (prefix, rest) = text.split_once('.').ok_or_else(error)?;
        let side = match rest {
            "top" => Some(Side::Top),
            "bottom" => Some(Side::Bottom),
            _ => None,
        };
        let layer = match (prefix, side) {
            ("silk", Some(side)) => Self::Silkscreen(side),
            ("mask", Some(side)) => Self::SolderMask(side),
            ("paste", Some(side)) => Self::Paste(side),
            ("adhesive", Some(side)) => Self::Adhesive(side),
            ("fab", Some(side)) => Self::Fabrication(side),
            ("courtyard", Some(side)) => Self::Courtyard(side),
            ("mech", None) => {
                Self::Mechanical(parse_numbered(rest, MAX_NUMBERED_LAYER).ok_or_else(error)?)
            }
            ("user", None) => {
                Self::User(parse_numbered(rest, MAX_NUMBERED_LAYER).ok_or_else(error)?)
            }
            _ => return Err(error()),
        };
        Ok(layer)
    }
}

/// A layer selection on a pad: one layer or a whole layer group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LayerSpec {
    /// One specific layer.
    Layer(Layer),
    /// Every copper layer (`cu.*`), for through-hole pads.
    AllCopper,
    /// Solder mask on both sides (`mask.*`).
    AllMask,
    /// Solder paste on both sides (`paste.*`).
    AllPaste,
}

impl LayerSpec {
    /// `true` if this selection includes `layer`.
    #[must_use]
    pub fn contains(self, layer: Layer) -> bool {
        match self {
            Self::Layer(own) => own == layer,
            Self::AllCopper => layer.is_copper(),
            Self::AllMask => matches!(layer, Layer::SolderMask(_)),
            Self::AllPaste => matches!(layer, Layer::Paste(_)),
        }
    }
}

impl fmt::Display for LayerSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layer(layer) => fmt::Display::fmt(layer, f),
            Self::AllCopper => f.write_str("cu.*"),
            Self::AllMask => f.write_str("mask.*"),
            Self::AllPaste => f.write_str("paste.*"),
        }
    }
}

impl FromStr for LayerSpec {
    type Err = ParseLayerError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "cu.*" => Ok(Self::AllCopper),
            "mask.*" => Ok(Self::AllMask),
            "paste.*" => Ok(Self::AllPaste),
            _ => text.parse().map(Self::Layer),
        }
    }
}

/// Implements string-based serde for a type with `Display` and `FromStr`.
macro_rules! string_serde {
    ($type:ty, $expecting:literal) => {
        impl Serialize for $type {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct StrVisitor;

                impl Visitor<'_> for StrVisitor {
                    type Value = $type;

                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str($expecting)
                    }

                    fn visit_str<E: de::Error>(self, value: &str) -> Result<$type, E> {
                        value.parse().map_err(E::custom)
                    }
                }

                deserializer.deserialize_str(StrVisitor)
            }
        }
    };
}

string_serde!(CopperLayer, "a copper layer such as `cu.top`");
string_serde!(Layer, "a layer such as `silk.top`");
string_serde!(LayerSpec, "a layer or a layer group such as `cu.*`");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copper_stack() {
        assert_eq!(CopperLayer::stack(0), Vec::<CopperLayer>::new());
        assert_eq!(CopperLayer::stack(1), vec![CopperLayer::Top]);
        assert_eq!(
            CopperLayer::stack(2),
            vec![CopperLayer::Top, CopperLayer::Bottom]
        );
        let four = CopperLayer::stack(4);
        assert_eq!(
            four,
            vec![
                CopperLayer::Top,
                CopperLayer::Inner(1),
                CopperLayer::Inner(2),
                CopperLayer::Bottom
            ]
        );
        assert_eq!(CopperLayer::stack(64).len(), 64);
        assert_eq!(CopperLayer::stack(200).len(), 64);
        let mut sorted = four.clone();
        sorted.sort();
        assert_eq!(sorted, four);
    }

    #[test]
    fn every_layer_round_trips_through_its_name() {
        let mut layers: Vec<Layer> = CopperLayer::stack(64)
            .into_iter()
            .map(Layer::Copper)
            .collect();
        for side in [Side::Top, Side::Bottom] {
            layers.extend([
                Layer::Silkscreen(side),
                Layer::SolderMask(side),
                Layer::Paste(side),
                Layer::Adhesive(side),
                Layer::Fabrication(side),
                Layer::Courtyard(side),
            ]);
        }
        layers.extend([
            Layer::Outline,
            Layer::Mechanical(1),
            Layer::Mechanical(32),
            Layer::User(7),
        ]);
        for layer in layers {
            let text = layer.to_string();
            assert_eq!(text.parse::<Layer>().unwrap(), layer, "{text}");
            let json = serde_json::to_string(&layer).unwrap();
            assert_eq!(serde_json::from_str::<Layer>(&json).unwrap(), layer);
        }
    }

    #[test]
    fn rejects_invalid_names() {
        for text in [
            "",
            "cu",
            "cu.in0",
            "cu.in63",
            "cu.in01",
            "silk",
            "silk.left",
            "mech.0",
            "mech.33",
            "user.x",
            "outline.top",
            "copper.top",
        ] {
            assert!(text.parse::<Layer>().is_err(), "{text}");
        }
        assert!(serde_json::from_str::<Layer>("1").is_err());
    }

    #[test]
    fn flipping() {
        assert_eq!(Layer::TOP_COPPER.flipped(4), Layer::BOTTOM_COPPER);
        assert_eq!(
            Layer::Copper(CopperLayer::Inner(1)).flipped(6),
            Layer::Copper(CopperLayer::Inner(4))
        );
        assert_eq!(
            Layer::Silkscreen(Side::Top).flipped(2),
            Layer::Silkscreen(Side::Bottom)
        );
        assert_eq!(Layer::Outline.flipped(2), Layer::Outline);
        assert_eq!(Layer::Courtyard(Side::Bottom).side(), Some(Side::Bottom));
        assert_eq!(Layer::Copper(CopperLayer::Inner(2)).side(), None);
    }

    #[test]
    fn layer_specs() {
        assert_eq!("cu.*".parse::<LayerSpec>().unwrap(), LayerSpec::AllCopper);
        assert_eq!(
            "paste.top".parse::<LayerSpec>().unwrap(),
            LayerSpec::Layer(Layer::Paste(Side::Top))
        );
        assert!(LayerSpec::AllCopper.contains(Layer::Copper(CopperLayer::Inner(3))));
        assert!(LayerSpec::AllMask.contains(Layer::SolderMask(Side::Bottom)));
        assert!(!LayerSpec::AllPaste.contains(Layer::TOP_COPPER));
        for spec in ["cu.*", "mask.*", "paste.*", "silk.bottom"] {
            assert_eq!(spec.parse::<LayerSpec>().unwrap().to_string(), spec);
        }
    }
}
