//! Text styles.

use serde::{Deserialize, Serialize};

use crate::units::{Nm, um};

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HAlign {
    /// Anchor at the left edge.
    Left,
    /// Anchor at the centre.
    #[default]
    Center,
    /// Anchor at the right edge.
    Right,
}

/// Vertical text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VAlign {
    /// Anchor at the top.
    Top,
    /// Anchor at the middle.
    #[default]
    Center,
    /// Anchor at the bottom.
    Bottom,
}

/// Default schematic text height (1.27 mm, 50 mil).
pub const SCHEMATIC_TEXT_SIZE: Nm = um(1_270);
/// Default board text height (1 mm).
pub const BOARD_TEXT_SIZE: Nm = um(1_000);
/// Default board text stroke width (0.15 mm).
pub const BOARD_TEXT_THICKNESS: Nm = um(150);

/// Size and alignment of a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TextStyle {
    /// Character height.
    pub size: Nm,
    /// Stroke width; 0 means "derived from the size".
    #[serde(skip_serializing_if = "crate::is_default")]
    pub thickness: Nm,
    /// Bold.
    #[serde(skip_serializing_if = "crate::is_default")]
    pub bold: bool,
    /// Italic.
    #[serde(skip_serializing_if = "crate::is_default")]
    pub italic: bool,
    /// Horizontal alignment.
    #[serde(skip_serializing_if = "crate::is_default")]
    pub h_align: HAlign,
    /// Vertical alignment.
    #[serde(skip_serializing_if = "crate::is_default")]
    pub v_align: VAlign,
}

impl TextStyle {
    /// The default schematic text style.
    #[must_use]
    pub const fn schematic() -> Self {
        Self {
            size: SCHEMATIC_TEXT_SIZE,
            thickness: 0,
            bold: false,
            italic: false,
            h_align: HAlign::Center,
            v_align: VAlign::Center,
        }
    }

    /// The default board text style.
    #[must_use]
    pub const fn board() -> Self {
        Self {
            size: BOARD_TEXT_SIZE,
            thickness: BOARD_TEXT_THICKNESS,
            ..Self::schematic()
        }
    }

    /// Same style with another alignment.
    #[must_use]
    pub const fn aligned(self, h_align: HAlign, v_align: VAlign) -> Self {
        Self {
            h_align,
            v_align,
            ..self
        }
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::schematic()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_compact() {
        assert_eq!(
            serde_json::to_string(&TextStyle::default()).unwrap(),
            r#"{"size":1270000}"#
        );
        let board = TextStyle::board().aligned(HAlign::Left, VAlign::Bottom);
        let json = serde_json::to_string(&board).unwrap();
        assert_eq!(serde_json::from_str::<TextStyle>(&json).unwrap(), board);
        assert_eq!(
            serde_json::from_str::<TextStyle>("{}").unwrap(),
            TextStyle::default()
        );
    }
}
