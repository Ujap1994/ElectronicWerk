//! Text items and the monospace metrics shared with the renderer.

use ew_model::text::{HAlign, TextStyle, VAlign};
use serde::ser::SerializeTuple;
use serde::{Serialize, Serializer};

use crate::geom::{BBox, P};

/// Advance per character relative to the text size (cap height). The viewer
/// uses a monospace font with an advance of 0.6 em and a cap height of
/// 0.73 em.
pub const ADVANCE: f64 = 0.6 / 0.73;

/// Distance between baselines relative to the text size.
pub const LINE_PITCH: f64 = 1.6;

/// Flag: mirrored (text on the bottom side, seen from the top).
pub const MIRROR: u8 = 1;
/// Flag: bold.
pub const BOLD: u8 = 2;
/// Flag: italic.
pub const ITALIC: u8 = 4;

/// One text: anchor, size, rotation and alignment.
#[derive(Debug, Clone, PartialEq)]
pub struct TextItem {
    /// The text (may contain `\n`).
    pub text: String,
    /// Anchor point.
    pub at: P,
    /// Cap height.
    pub size: f64,
    /// Rotation in degrees, counter-clockwise.
    pub rotation: f64,
    /// −1 left, 0 centre, 1 right.
    pub h: i8,
    /// −1 bottom, 0 centre, 1 top.
    pub v: i8,
    /// [`MIRROR`], [`BOLD`], [`ITALIC`].
    pub flags: u8,
    /// Object index.
    pub obj: u32,
}

impl TextItem {
    /// A text with the alignment and flags of a model style.
    #[must_use]
    pub fn styled(text: String, at: P, rotation: f64, style: &TextStyle, obj: u32) -> Self {
        let mut flags = 0;
        if style.bold {
            flags |= BOLD;
        }
        if style.italic {
            flags |= ITALIC;
        }
        Self {
            text,
            at,
            size: style.size as f64,
            rotation,
            h: match style.h_align {
                HAlign::Left => -1,
                HAlign::Center => 0,
                HAlign::Right => 1,
            },
            v: match style.v_align {
                VAlign::Bottom => -1,
                VAlign::Center => 0,
                VAlign::Top => 1,
            },
            flags,
            obj,
        }
    }
}

impl Serialize for TextItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(9)?;
        tuple.serialize_element(&self.text)?;
        tuple.serialize_element(&self.at.x.round())?;
        tuple.serialize_element(&self.at.y.round())?;
        tuple.serialize_element(&self.size.round())?;
        tuple.serialize_element(&((self.rotation * 1000.0).round() / 1000.0))?;
        tuple.serialize_element(&self.h)?;
        tuple.serialize_element(&self.v)?;
        tuple.serialize_element(&self.flags)?;
        tuple.serialize_element(&self.obj)?;
        tuple.end()
    }
}

/// Width and height of a text block.
#[must_use]
pub fn extent(text: &str, size: f64) -> (f64, f64) {
    let mut lines = 0_usize;
    let mut longest = 0_usize;
    for line in text.split('\n') {
        lines += 1;
        longest = longest.max(line.chars().count());
    }
    let width = longest as f64 * ADVANCE * size;
    let height = size + (lines.max(1) - 1) as f64 * LINE_PITCH * size;
    (width, height)
}

/// The four corners of the text box, in scene coordinates.
#[must_use]
pub fn corners(item: &TextItem) -> [P; 4] {
    let (w, h) = extent(&item.text, item.size);
    let x0 = match item.h {
        i8::MIN..=-1 => 0.0,
        0 => -w / 2.0,
        _ => -w,
    };
    let y0 = match item.v {
        i8::MIN..=-1 => 0.0,
        0 => -h / 2.0,
        _ => -h,
    };
    let (x0, x1) = if item.flags & MIRROR == 0 {
        (x0, x0 + w)
    } else {
        (-x0 - w, -x0)
    };
    let angle = item.rotation.to_radians();
    [
        P::new(x0, y0),
        P::new(x1, y0),
        P::new(x1, y0 + h),
        P::new(x0, y0 + h),
    ]
    .map(|p| p.rotate(angle).add(item.at))
}

/// Bounding box of a text.
#[must_use]
pub fn text_bounds(item: &TextItem) -> BBox {
    corners(item).into_iter().fold(BBox::EMPTY, BBox::with)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(text: &str, h: i8, v: i8, rotation: f64, flags: u8) -> TextItem {
        TextItem {
            text: text.into(),
            at: P::new(100.0, 50.0),
            size: 10.0,
            rotation,
            h,
            v,
            flags,
            obj: 0,
        }
    }

    #[test]
    fn extent_counts_characters_and_lines() {
        let (w, h) = extent("ab\nabcd", 10.0);
        assert!((w - 4.0 * ADVANCE * 10.0).abs() < 1e-9);
        assert!((h - (10.0 + LINE_PITCH * 10.0)).abs() < 1e-9);
        assert!((extent("Ωµ", 1.0).0 - 2.0 * ADVANCE).abs() < 1e-9);
    }

    #[test]
    fn alignment_moves_the_box() {
        let b = text_bounds(&item("ab", -1, -1, 0.0, 0));
        assert!((b.min_x - 100.0).abs() < 1e-9 && (b.min_y - 50.0).abs() < 1e-9);
        let b = text_bounds(&item("ab", 1, 1, 0.0, 0));
        assert!((b.max_x - 100.0).abs() < 1e-9 && (b.max_y - 50.0).abs() < 1e-9);
        let b = text_bounds(&item("ab", 0, 0, 90.0, 0));
        assert!((b.center().x - 100.0).abs() < 1e-9 && (b.center().y - 50.0).abs() < 1e-9);
        assert!(b.max_y - b.min_y > b.max_x - b.min_x);
    }

    #[test]
    fn mirrored_left_aligned_text_extends_left() {
        let b = text_bounds(&item("abc", -1, -1, 0.0, MIRROR));
        assert!((b.max_x - 100.0).abs() < 1e-9);
    }

    #[test]
    fn serializes_as_compact_tuple() {
        let json = serde_json::to_string(&item("R1", 0, 0, 90.0, BOLD)).unwrap();
        assert_eq!(json, r#"["R1",100.0,50.0,10.0,90.0,0,0,2,0]"#);
    }
}
