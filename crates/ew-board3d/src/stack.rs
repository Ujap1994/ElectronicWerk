//! Heights and colours from the stack-up.
//!
//! The body spans the dielectrics and the inner copper layers: from z = 0
//! (bottom face) to [`Levels::body_top`]. The outer copper lies on the faces,
//! the solder mask on the copper, the silkscreen on the mask. Parts sit on
//! the outer copper: their models' z = 0 is the copper surface.

use ew_model::layer::{CopperLayer, Side};
use ew_model::pcb::{Stackup, StackupLayer};
use serde::Serialize;

use crate::mm;

/// Gap between drawn surfaces that would otherwise lie in one plane (5 µm).
pub const SURFACE_GAP: f64 = 0.005;

/// Heights in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Levels {
    /// Top face of the body (the bottom face is at 0).
    pub body_top: f64,
    /// Thickness of the top copper.
    pub top_copper: f64,
    /// Thickness of the bottom copper (0 on single-sided boards).
    pub bottom_copper: f64,
    /// Thickness of the top solder mask (0 without one).
    pub top_mask: f64,
    /// Thickness of the bottom solder mask (0 without one).
    pub bottom_mask: f64,
    /// Number of copper layers.
    pub copper_layers: u8,
}

impl Levels {
    /// The heights of a stack-up.
    #[must_use]
    pub fn of(stackup: &Stackup) -> Self {
        let copper: Vec<(usize, CopperLayer, f64)> = stackup
            .layers
            .iter()
            .enumerate()
            .filter_map(|(index, layer)| match layer {
                StackupLayer::Copper {
                    layer, thickness, ..
                } => Some((index, *layer, mm(*thickness as f64))),
                _ => None,
            })
            .collect();
        let mask = |side: Side| {
            stackup
                .layers
                .iter()
                .find_map(|layer| match layer {
                    StackupLayer::SolderMask {
                        side: own,
                        thickness,
                        ..
                    } if *own == side => Some(mm(*thickness as f64)),
                    _ => None,
                })
                .unwrap_or(0.0)
        };
        let (first, last) = match (copper.first(), copper.last()) {
            (Some(first), Some(last)) => (first.0, last.0),
            _ => (0, 0),
        };
        let body: f64 = if copper.len() >= 2 {
            stackup.layers[first + 1..last]
                .iter()
                .map(|layer| mm(layer.thickness() as f64))
                .sum()
        } else {
            // Single-sided: the dielectrics below the copper.
            stackup.layers[first.min(stackup.layers.len())..]
                .iter()
                .filter(|layer| matches!(layer, StackupLayer::Dielectric { .. }))
                .map(|layer| mm(layer.thickness() as f64))
                .sum()
        };
        let body_top = if body > 0.0 { body } else { 1.6 };
        let top_copper = copper.first().map_or(0.0, |c| c.2);
        let bottom_copper = if copper.len() >= 2 {
            copper.last().map_or(0.0, |c| c.2)
        } else {
            0.0
        };
        Self {
            body_top,
            top_copper,
            bottom_copper,
            top_mask: mask(Side::Top),
            bottom_mask: if copper.len() >= 2 {
                mask(Side::Bottom)
            } else {
                0.0
            },
            copper_layers: u8::try_from(copper.len()).unwrap_or(u8::MAX),
        }
    }

    /// The surface the parts of a side stand on (their models' z = 0).
    #[must_use]
    pub fn mount(&self, side: Side) -> f64 {
        match side {
            Side::Top => self.body_top + self.top_copper,
            Side::Bottom => -self.bottom_copper,
        }
    }

    /// Height of the outer copper surface of a side.
    #[must_use]
    pub fn copper(&self, side: Side) -> f64 {
        self.mount(side)
    }

    /// Height of the mask surface of a side (over the copper).
    #[must_use]
    pub fn mask(&self, side: Side) -> f64 {
        match side {
            Side::Top => self.copper(side) + self.top_mask.max(SURFACE_GAP),
            Side::Bottom => self.copper(side) - self.bottom_mask.max(SURFACE_GAP),
        }
    }

    /// Height of the silkscreen surface of a side.
    #[must_use]
    pub fn silk(&self, side: Side) -> f64 {
        self.mask(side) + side_sign(side) * SURFACE_GAP
    }

    /// Height of the paste surface of a side.
    #[must_use]
    pub fn paste(&self, side: Side) -> f64 {
        self.silk(side) + side_sign(side) * SURFACE_GAP
    }

    /// Height of inner copper layer `index` (0 = top) of the stack: its
    /// middle, spread evenly through the body.
    #[must_use]
    pub fn inner(&self, index: usize) -> f64 {
        let count = f64::from(self.copper_layers.max(2) - 1);
        self.body_top * (1.0 - index as f64 / count)
    }

    /// Total height from the bottom mask to the top mask.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.mask(Side::Top) - self.mask(Side::Bottom)
    }
}

/// +1 for the top side, −1 for the bottom side.
#[must_use]
pub const fn side_sign(side: Side) -> f64 {
    match side {
        Side::Top => 1.0,
        Side::Bottom => -1.0,
    }
}

/// An RGB colour (0–1).
pub type Rgb = [f32; 3];

/// Colours of the board materials.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Colors {
    /// Laminate (the body's edges and holes).
    pub body: Rgb,
    /// Top solder mask.
    pub top_mask: Rgb,
    /// Bottom solder mask.
    pub bottom_mask: Rgb,
    /// Top silkscreen.
    pub top_silk: Rgb,
    /// Bottom silkscreen.
    pub bottom_silk: Rgb,
    /// Exposed copper (the surface finish).
    pub copper: Rgb,
    /// Solder paste.
    pub paste: Rgb,
    /// Surface finish name (`HASL`, `ENIG`, …).
    pub finish: String,
}

/// The colour of a mask or silkscreen colour name (KiCad's names and a few
/// more); `None` for unknown names.
#[must_use]
pub fn named_color(name: &str) -> Option<Rgb> {
    let lower = name.trim().to_ascii_lowercase();
    if let Some(hex) = lower.strip_prefix('#')
        && hex.len() == 6
    {
        let value = u32::from_str_radix(hex, 16).ok()?;
        return Some([
            ((value >> 16) & 0xff) as f32 / 255.0,
            ((value >> 8) & 0xff) as f32 / 255.0,
            (value & 0xff) as f32 / 255.0,
        ]);
    }
    Some(match lower.as_str() {
        "green" => [0.08, 0.36, 0.17],
        "light green" => [0.36, 0.62, 0.36],
        "saturated green" => [0.05, 0.42, 0.1],
        "red" => [0.62, 0.08, 0.08],
        "light red" => [0.82, 0.32, 0.3],
        "red/orange" => [0.82, 0.3, 0.1],
        "blue" => [0.06, 0.18, 0.5],
        "light blue" => [0.3, 0.5, 0.8],
        "blue/green" => [0.0, 0.4, 0.4],
        "black" => [0.06, 0.06, 0.07],
        "white" => [0.92, 0.92, 0.9],
        "purple" => [0.3, 0.1, 0.42],
        "light purple" => [0.55, 0.38, 0.68],
        "yellow" => [0.85, 0.75, 0.15],
        "matte black" => [0.1, 0.1, 0.1],
        "grey" | "gray" => [0.5, 0.5, 0.5],
        _ => return None,
    })
}

const DEFAULT_MASK: Rgb = [0.08, 0.36, 0.17];
const DEFAULT_SILK: Rgb = [0.92, 0.92, 0.9];

/// The colours of a stack-up.
#[must_use]
pub fn colors(stackup: &Stackup) -> Colors {
    let mut out = Colors {
        body: [0.66, 0.6, 0.4],
        top_mask: DEFAULT_MASK,
        bottom_mask: DEFAULT_MASK,
        top_silk: DEFAULT_SILK,
        bottom_silk: DEFAULT_SILK,
        copper: [0.8, 0.8, 0.82],
        paste: [0.62, 0.62, 0.64],
        finish: stackup.finish.clone(),
    };
    for layer in &stackup.layers {
        match layer {
            StackupLayer::SolderMask { side, color, .. } => {
                if let Some(rgb) = named_color(color) {
                    match side {
                        Side::Top => out.top_mask = rgb,
                        Side::Bottom => out.bottom_mask = rgb,
                    }
                }
            }
            StackupLayer::Silkscreen { side, color } => {
                if let Some(rgb) = named_color(color) {
                    match side {
                        Side::Top => out.top_silk = rgb,
                        Side::Bottom => out.bottom_silk = rgb,
                    }
                }
            }
            _ => {}
        }
    }
    let finish = stackup.finish.to_ascii_uppercase();
    out.copper = if finish.contains("ENIG") || finish.contains("ENEPIG") || finish.contains("GOLD")
    {
        [0.86, 0.68, 0.32]
    } else if finish.contains("OSP") || finish == "NONE" || finish.is_empty() {
        [0.78, 0.48, 0.28]
    } else {
        // HASL, immersion tin and silver.
        [0.8, 0.8, 0.82]
    };
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::pcb::STANDARD_THICKNESS;

    #[test]
    fn levels_of_a_standard_board() {
        let stack = Stackup::standard(2, STANDARD_THICKNESS);
        let levels = Levels::of(&stack);
        assert!((levels.top_copper - 0.035).abs() < 1e-9);
        // 1.6 mm minus two coppers and two masks.
        assert!((levels.body_top - (1.6 - 0.07 - 0.02)).abs() < 1e-6);
        assert!((levels.mount(Side::Top) - (levels.body_top + 0.035)).abs() < 1e-9);
        assert!((levels.mount(Side::Bottom) + 0.035).abs() < 1e-9);
        assert!((levels.total() - 1.6).abs() < 1e-6);
        assert!(levels.silk(Side::Top) > levels.mask(Side::Top));
        assert!(levels.silk(Side::Bottom) < levels.mask(Side::Bottom));

        let single = Levels::of(&Stackup::standard(1, STANDARD_THICKNESS));
        assert_eq!(single.copper_layers, 1);
        assert!(single.bottom_copper.abs() < 1e-12);
        assert!(single.body_top > 1.0);
    }

    #[test]
    fn colours_follow_the_stackup() {
        let mut stack = Stackup::standard(2, STANDARD_THICKNESS);
        stack.finish = "ENIG".into();
        for layer in &mut stack.layers {
            if let StackupLayer::SolderMask { color, .. } = layer {
                *color = "Blue".into();
            }
        }
        let colors = colors(&stack);
        assert_eq!(colors.top_mask, named_color("blue").unwrap());
        assert!(colors.copper[0] > colors.copper[2], "gold");
        assert_eq!(named_color("#ff0000"), Some([1.0, 0.0, 0.0]));
        assert_eq!(named_color("unknown"), None);
    }
}
