//! Widths of the standard 14 fonts for the printable ASCII characters,
//! used when a file names such a font without giving its widths.

/// Helvetica (also Arial and other sans-serif fonts), 0x20–0x7e.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// Helvetica-Bold, 0x20–0x7e.
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

/// Times-Roman (also other serif fonts), 0x20–0x7e.
const TIMES: [u16; 95] = [
    250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444, 921, 722, 667, 667, 722, 611,
    556, 722, 722, 333, 389, 722, 611, 889, 722, 722, 556, 722, 667, 556, 611, 722, 722, 944, 722,
    722, 611, 333, 278, 333, 469, 500, 333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500,
    278, 778, 500, 500, 500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541,
];

/// Width tables of a standard font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standard {
    /// Helvetica and other sans-serif fonts.
    Helvetica,
    /// Helvetica-Bold.
    HelveticaBold,
    /// Times and other serif fonts.
    Times,
    /// Courier and other monospaced fonts.
    Courier,
}

impl Standard {
    /// The table for a font name (`Helvetica-Bold`, `ArialMT`, `ABCDEF+Times`).
    #[must_use]
    pub fn for_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        if lower.contains("courier") || lower.contains("mono") {
            Self::Courier
        } else if lower.contains("times") || lower.contains("serif") && !lower.contains("sans") {
            Self::Times
        } else if lower.contains("bold") || lower.contains("black") || lower.contains("heavy") {
            Self::HelveticaBold
        } else {
            Self::Helvetica
        }
    }

    /// Width of a character in thousandths of the font size.
    #[must_use]
    pub fn width(self, c: char) -> f64 {
        let table = match self {
            Self::Courier => return 600.0,
            Self::Helvetica => &HELVETICA,
            Self::HelveticaBold => &HELVETICA_BOLD,
            Self::Times => &TIMES,
        };
        let code = c as u32;
        if (0x20..=0x7e).contains(&code) {
            f64::from(table[(code - 0x20) as usize])
        } else if c == '\u{a0}' {
            f64::from(table[0])
        } else {
            f64::from(table[('n' as u32 - 0x20) as usize])
        }
    }
}
