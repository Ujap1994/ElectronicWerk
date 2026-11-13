//! The stroke font of plotted outputs (Gerber, PDF, SVG).
//!
//! Plotters draw text with lines, so silkscreen and copper texts can be
//! manufactured and prints look the same everywhere. The font is our own
//! design: monospace, with the metrics of the viewer (`size` is the cap
//! height, every character advances [`ADVANCE`] × size, lines are
//! [`LINE_PITCH`] × size apart), so a text covers the same box in the viewer
//! and in every output.
//!
//! Glyphs are drawn on a grid where the cap height is 10 units, lower-case
//! letters are 7 units high, descenders reach −3 and the body spans x 1…7.
//! A glyph is a list of strokes separated by `|`; a stroke is a list of
//! points `x,y` and elliptical arcs `@cx,cy,rx,ry,from,to` (degrees,
//! counter-clockwise from `from` to `to`; `to < from` turns clockwise).

use std::collections::HashMap;
use std::sync::OnceLock;

pub use ew_scene::{ADVANCE, LINE_PITCH};

/// Grid units per text size (the cap height).
const UNITS: f64 = 10.0;

/// Horizontal shift that centres the 8-unit glyph body in the advance.
const CENTER: f64 = (ADVANCE * UNITS - 8.0) / 2.0;

/// Slant of italic text (x shift per unit of height).
const ITALIC_SHEAR: f64 = 0.2;

/// Largest angle step of arcs in glyphs, in degrees.
const ARC_STEP: f64 = 15.0;

/// The glyph used for characters the font does not have.
const MISSING: char = '?';

/// Stroke width relative to the size, when a text has none.
const THICKNESS: f64 = 0.125;
/// Stroke width relative to the size for bold text.
const BOLD_THICKNESS: f64 = 0.2;

/// Glyph definitions.
#[allow(clippy::too_many_lines, reason = "a table of glyphs")]
const fn definition(c: char) -> Option<&'static str> {
    Some(match c {
        ' ' => "",
        '!' => "4,10 4,3 | 4,0.2 4,0.6",
        '"' => "3,10 3,7.5 | 5,10 5,7.5",
        '#' => "2.8,10 2.2,0 | 5.8,10 5.2,0 | 1,6.5 7,6.5 | 1,3.5 7,3.5",
        '$' => "@4,6.6,2.7,1.8,25,270 @4,3,3,1.8,90,-155 | 4,10 4,0",
        '%' => "1,0 7,10 | @2.3,8,1.3,1.6,0,360 | @5.7,2,1.3,1.6,0,360",
        '&' => {
            "7,0 2.3,6.6 2.1,7.6 2.4,8.8 3.4,9.8 4.6,9.8 5.5,8.8 5.6,7.8 5,6.8 \
             1.9,4.6 1.1,3.2 1.2,1.6 2.2,0.4 3.6,0 5,0.4 6.2,1.5 7,3.5"
        }
        '\'' => "4,10 4,7.5",
        '(' => "@7,5,4,6.5,113,247",
        ')' => "@1,5,4,6.5,67,-67",
        '*' => "4,9 4,3 | 1.4,7.5 6.6,4.5 | 1.4,4.5 6.6,7.5",
        '+' => "4,8 4,2 | 1,5 7,5",
        ',' => "4.4,0.8 4.2,0 3,-2",
        '-' => "1.5,4 6.5,4",
        '.' => "4,0.2 4,0.6",
        '/' => "6.5,11 1.5,-1",
        '0' => "@4,5,3,5,0,360 | 6,8 2,2",
        '1' => "2,8 4.5,10 4.5,0 | 2,0 7,0",
        '2' => "@4,7,3,3,160,-30 1,0 7,0",
        '3' => "@4,7.5,2.8,2.5,150,-90 @4,2.5,3,2.5,90,-150",
        '4' => "5.5,0 5.5,10 1,3 7,3",
        '5' => "6.5,10 1.6,10 1.3,5.3 @4,3.2,3,3.2,135,-150",
        '6' => "@4,3,3,3,0,360 | @5.5,3,4.5,7,180,70",
        '7' => "1,10 7,10 3,0",
        '8' => "@4,7.5,2.6,2.5,0,360 | @4,2.5,3,2.5,0,360",
        '9' => "@4,7,3,3,0,360 | @2.5,7,4.5,7,0,-105",
        ':' => "4,6.6 4,7 | 4,0.2 4,0.6",
        ';' => "4,6.6 4,7 | 4.4,0.8 4.2,0 3,-2",
        '<' => "7,8.5 1,5 7,1.5",
        '=' => "1,6.5 7,6.5 | 1,3.5 7,3.5",
        '>' => "1,8.5 7,5 1,1.5",
        '?' => "@4,7.5,3,2.5,160,-90 4,3 | 4,0.2 4,0.6",
        '@' => "@4.3,5,1.4,2,0,360 | 5.7,7 5.7,3.6 6.3,3 7,3.6 7,5 @4,5,3,5,0,300",
        'A' => "1,0 4,10 7,0 | 2,3.33 6,3.33",
        'B' => "5,5 1,5 | 1,0 1,10 5,10 @5,7.5,1.8,2.5,90,-90 | 5,5 @5,2.5,2,2.5,90,-90 1,0",
        'C' => "@4,5,3,5,40,320",
        'D' => "1,0 1,10 3.5,10 @3.5,5,3.5,5,90,-90 1,0",
        'E' => "7,10 1,10 1,0 7,0 | 1,5 5.5,5",
        'F' => "7,10 1,10 1,0 | 1,5 5.5,5",
        'G' => "@4,5,3,5,45,360 4.5,5",
        'H' => "1,0 1,10 | 7,0 7,10 | 1,5 7,5",
        'I' => "2,10 6,10 | 4,10 4,0 | 2,0 6,0",
        'J' => "7,10 7,3 @4,3,3,3,0,-180",
        'K' => "1,0 1,10 | 7,10 1,3.5 | 3.3,5.9 7,0",
        'L' => "1,10 1,0 7,0",
        'M' => "1,0 1,10 4,4 7,10 7,0",
        'N' => "1,0 1,10 7,0 7,10",
        'O' => "@4,5,3,5,0,360",
        'P' => "1,0 1,10 5,10 @5,7.5,2,2.5,90,-90 1,5",
        'Q' => "@4,5,3,5,0,360 | 4.8,2.2 7,0",
        'R' => "1,0 1,10 5,10 @5,7.5,2,2.5,90,-90 1,5 | 4.5,5 7,0",
        'S' => "@4,7.5,2.8,2.5,20,270 @4,2.5,3,2.5,90,-160",
        'T' => "1,10 7,10 | 4,10 4,0",
        'U' => "1,10 1,3 @4,3,3,3,180,360 7,10",
        'V' => "1,10 4,0 7,10",
        'W' => "0.5,10 2.2,0 4,7 5.8,0 7.5,10",
        'X' => "1,10 7,0 | 1,0 7,10",
        'Y' => "1,10 4,5 7,10 | 4,5 4,0",
        'Z' => "1,10 7,10 1,0 7,0",
        '[' => "5.5,11 3,11 3,-1 5.5,-1",
        '\\' => "1.5,11 6.5,-1",
        ']' => "2.5,11 5,11 5,-1 2.5,-1",
        '^' => "2,7 4,10 6,7",
        '_' => "0.5,-1.5 7.5,-1.5",
        '`' => "3,10.5 4.5,8.5",
        'a' => "7,7 7,0 | @4,3.5,3,3.5,0,360",
        'b' => "1,10 1,0 | @4,3.5,3,3.5,0,360",
        'c' => "@4,3.5,3,3.5,40,320",
        'd' => "7,10 7,0 | @4,3.5,3,3.5,0,360",
        'e' => "1,3.5 7,3.5 @4,3.5,3,3.5,0,320",
        'f' => "4,0 4,8 @6,8,2,2,180,45 | 2,7 6.5,7",
        'g' => "@4,3.5,3,3.5,0,360 | 7,7 7,-1 @4,-1,3,2,0,-160",
        'h' => "1,10 1,0 | @4,4,3,3,180,0 7,0",
        'i' => "2.5,7 4,7 4,0 | 1.5,0 6.5,0 | 4,9.2 4,9.7",
        'j' => "3,7 5,7 5,-1 @2.5,-1,2.5,2,0,-150 | 5,9.2 5,9.7",
        'k' => "1,10 1,0 | 6.5,7 1,2.5 | 3,4.1 7,0",
        'l' => "2,10 4,10 4,1.5 @5.8,1.5,1.8,1.5,180,270 7,0",
        'm' => "1,0 1,7 | @2.5,5.5,1.5,1.5,180,0 4,0 | @5.5,5.5,1.5,1.5,180,0 7,0",
        'n' => "1,0 1,7 | @4,4,3,3,180,0 7,0",
        'o' => "@4,3.5,3,3.5,0,360",
        'p' => "1,7 1,-3 | @4,3.5,3,3.5,0,360",
        'q' => "7,7 7,-3 | @4,3.5,3,3.5,0,360",
        'r' => "1,0 1,7 | @4.5,3.5,3.5,3.5,180,55",
        's' => "@4,5.25,2.6,1.75,20,270 @4,1.75,3,1.75,90,-160",
        't' => "3.5,10 3.5,1.8 @5.3,1.8,1.8,1.8,180,280 | 1.5,7 6.5,7",
        'u' => "1,7 1,3 @4,3,3,3,180,360 | 7,7 7,0",
        'v' => "1,7 4,0 7,7",
        'w' => "0.5,7 2.2,0 4,5 5.8,0 7.5,7",
        'x' => "1,7 7,0 | 1,0 7,7",
        'y' => "1,7 4,0 | 7,7 2.7,-3 1.5,-3",
        'z' => "1,7 7,7 1,0 7,0",
        '{' => "5.5,11 4.6,10.6 4,9.6 4,6.2 3.4,5.2 2.5,5 3.4,4.8 4,3.8 4,0.4 4.6,-0.6 5.5,-1",
        '|' => "4,11 4,-1",
        '}' => "2.5,11 3.4,10.6 4,9.6 4,6.2 4.6,5.2 5.5,5 4.6,4.8 4,3.8 4,0.4 3.4,-0.6 2.5,-1",
        '~' => "1,4.5 2,5.5 3,5.5 5,4.5 6,4.5 7,5.5",
        '°' => "@4,8.5,1.2,1.2,0,360",
        '±' => "4,8 4,3 | 1,5.5 7,5.5 | 1,0.5 7,0.5",
        '²' => "@4,9.2,1.5,1.2,160,-30 2.5,6 5.5,6",
        '³' => "@4,9.4,1.4,0.9,150,-90 @4,7.3,1.5,1.2,90,-150",
        'µ' => "1,7 1,-3 | 1,3 @4,3,3,3,180,360 | 7,7 7,0",
        '·' => "4,4.8 4,5.2",
        '×' => "1.5,7.5 6.5,2.5 | 1.5,2.5 6.5,7.5",
        '÷' => "1,5 7,5 | 4,8 4,8.4 | 4,1.6 4,2",
        'ß' => {
            "1,0 1,8 1.5,9.4 2.6,10 3.8,10 4.9,9.4 5.4,8.4 5.2,7.2 4.2,6.1 3.4,5.8 \
             5.4,5.4 6.6,4.4 7,3 6.6,1.4 5.4,0.3 4,0 3,0.3"
        }
        'Ω' => "1,0 3,0 @4,5.8,3,4.2,240,-60 5,0 7,0",
        '€' => "@4.5,5,3,5,45,315 | 0.5,6 4.5,6 | 0.5,4 4.5,4",
        _ => return None,
    })
}

/// Base letter and accent of the composed characters.
const fn composed(c: char) -> Option<(char, &'static str)> {
    // Diaeresis above capitals and above lower-case letters.
    const CAP_UMLAUT: &str = "2.5,11.6 2.5,12.1 | 5.5,11.6 5.5,12.1";
    const UMLAUT: &str = "2.5,8.8 2.5,9.3 | 5.5,8.8 5.5,9.3";
    const CAP_ACUTE: &str = "3.5,11.2 5,12.6";
    const ACUTE: &str = "3.5,8.4 5,9.8";
    const CAP_GRAVE: &str = "3,12.6 4.5,11.2";
    const GRAVE: &str = "3,9.8 4.5,8.4";
    Some(match c {
        'Ä' => ('A', CAP_UMLAUT),
        'Ö' => ('O', CAP_UMLAUT),
        'Ü' => ('U', CAP_UMLAUT),
        'ä' => ('a', UMLAUT),
        'ö' => ('o', UMLAUT),
        'ü' => ('u', UMLAUT),
        'É' => ('E', CAP_ACUTE),
        'é' => ('e', ACUTE),
        'á' => ('a', ACUTE),
        'ó' => ('o', ACUTE),
        'í' => ('ı', ACUTE),
        'ú' => ('u', ACUTE),
        'È' => ('E', CAP_GRAVE),
        'è' => ('e', GRAVE),
        'à' => ('a', GRAVE),
        _ => return None,
    })
}

/// A parsed glyph: polylines in grid units.
type Glyph = Vec<Vec<[f64; 2]>>;

fn parse_number(text: &str) -> f64 {
    text.trim().parse().unwrap_or(0.0)
}

/// Parses one glyph definition.
fn parse(definition: &str) -> Glyph {
    let mut glyph = Vec::new();
    for stroke in definition.split('|') {
        let mut points: Vec<[f64; 2]> = Vec::new();
        for token in stroke.split_whitespace() {
            if let Some(arc) = token.strip_prefix('@') {
                let v: Vec<f64> = arc.split(',').map(parse_number).collect();
                let [cx, cy, rx, ry, from, to] = v[..] else {
                    continue;
                };
                let steps = ((to - from).abs() / ARC_STEP).ceil().max(1.0) as usize;
                for i in 0..=steps {
                    let a = (from + (to - from) * i as f64 / steps as f64).to_radians();
                    points.push([cx + rx * a.cos(), cy + ry * a.sin()]);
                }
            } else if let Some((x, y)) = token.split_once(',') {
                points.push([parse_number(x), parse_number(y)]);
            }
        }
        points.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9);
        if points.len() == 1 {
            // A dot: a very short stroke, drawn round by the pen.
            let [x, y] = points[0];
            points.push([x, y + 1e-3]);
        }
        if !points.is_empty() {
            glyph.push(points);
        }
    }
    glyph
}

fn glyphs() -> &'static HashMap<char, Glyph> {
    static GLYPHS: OnceLock<HashMap<char, Glyph>> = OnceLock::new();
    GLYPHS.get_or_init(|| {
        let mut map = HashMap::new();
        for code in 0x20_u32..0x3000 {
            let Some(c) = char::from_u32(code) else {
                continue;
            };
            if let Some(def) = definition(c) {
                map.insert(c, parse(def));
            }
        }
        // A dotless i for accented i.
        map.insert('ı', parse("2.5,7 4,7 4,0 | 1.5,0 6.5,0"));
        for code in 0x20_u32..0x3000 {
            let Some(c) = char::from_u32(code) else {
                continue;
            };
            if let Some((base, accent)) = composed(c)
                && let Some(base) = map.get(&base)
            {
                let mut glyph = base.clone();
                glyph.extend(parse(accent));
                map.insert(c, glyph);
            }
        }
        map
    })
}

/// `true` if the font has a glyph for the character.
#[must_use]
pub fn has_glyph(c: char) -> bool {
    glyphs().contains_key(&c)
}

/// A text to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSpec<'a> {
    /// The text; `\n` starts a new line.
    pub text: &'a str,
    /// Anchor point.
    pub at: [f64; 2],
    /// Cap height.
    pub size: f64,
    /// Rotation in degrees, counter-clockwise.
    pub rotation: f64,
    /// Horizontal alignment: −1 left, 0 centre, 1 right.
    pub h: i8,
    /// Vertical alignment: −1 bottom, 0 centre, 1 top.
    pub v: i8,
    /// Mirrored (read from the other side of the board).
    pub mirror: bool,
    /// Slanted.
    pub italic: bool,
}

/// Width and height of a text block (as in the viewer).
#[must_use]
pub fn extent(text: &str, size: f64) -> (f64, f64) {
    let mut lines = 0_usize;
    let mut longest = 0_usize;
    for line in text.split('\n') {
        lines += 1;
        longest = longest.max(line.chars().count());
    }
    (
        longest as f64 * ADVANCE * size,
        size + (lines.max(1) - 1) as f64 * LINE_PITCH * size,
    )
}

/// The stroke width of a text without one.
#[must_use]
pub fn default_thickness(size: f64, bold: bool) -> f64 {
    size * if bold { BOLD_THICKNESS } else { THICKNESS }
}

/// The strokes of a text as polylines in the coordinates of `at`.
#[must_use]
pub fn strokes(spec: &TextSpec<'_>) -> Vec<Vec<[f64; 2]>> {
    let table = glyphs();
    let size = spec.size;
    let (w, h) = extent(spec.text, size);
    let x0 = match spec.h {
        i8::MIN..=-1 => 0.0,
        0 => -w / 2.0,
        _ => -w,
    };
    let y0 = match spec.v {
        i8::MIN..=-1 => 0.0,
        0 => -h / 2.0,
        _ => -h,
    };
    let top = y0 + h;
    let scale = size / UNITS;
    let step = ADVANCE * size;
    let (sin, cos) = spec.rotation.to_radians().sin_cos();
    let mirror = if spec.mirror { -1.0 } else { 1.0 };
    let shear = if spec.italic { ITALIC_SHEAR } else { 0.0 };
    let place = |lx: f64, ly: f64| -> [f64; 2] {
        let mx = lx * mirror;
        [
            spec.at[0] + mx * cos - ly * sin,
            spec.at[1] + mx * sin + ly * cos,
        ]
    };
    let mut out = Vec::new();
    for (row, line) in spec.text.split('\n').enumerate() {
        let baseline = top - size - row as f64 * LINE_PITCH * size;
        for (column, c) in line.chars().enumerate() {
            let glyph = table
                .get(&c)
                .or_else(|| table.get(&MISSING))
                .map_or(&[][..], Vec::as_slice);
            let pen = x0 + column as f64 * step;
            for stroke in glyph {
                out.push(
                    stroke
                        .iter()
                        .map(|[gx, gy]| {
                            let ly = gy * scale;
                            let lx = pen + (gx + CENTER) * scale + shear * ly;
                            place(lx, baseline + ly)
                        })
                        .collect(),
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(text: &str) -> TextSpec<'_> {
        TextSpec {
            text,
            at: [0.0, 0.0],
            size: 10.0,
            rotation: 0.0,
            h: -1,
            v: -1,
            mirror: false,
            italic: false,
        }
    }

    fn bounds(lines: &[Vec<[f64; 2]>]) -> [f64; 4] {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in lines.iter().flatten() {
            b = [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ];
        }
        b
    }

    #[test]
    fn every_printable_ascii_character_has_a_glyph() {
        for code in 0x20_u8..0x7f {
            assert!(has_glyph(char::from(code)), "{}", char::from(code));
        }
        for c in "ÄÖÜäöüßµΩ°±éè".chars() {
            assert!(has_glyph(c), "{c}");
        }
    }

    #[test]
    fn capitals_fill_the_cap_height_and_stay_in_their_cell() {
        let b = bounds(&strokes(&spec("H")));
        assert!((b[1] - 0.0).abs() < 1e-9 && (b[3] - 10.0).abs() < 1e-9);
        assert!(b[0] > 0.0 && b[2] < ADVANCE * 10.0);
        let b = bounds(&strokes(&spec("HH")));
        assert!(b[2] > ADVANCE * 10.0 && b[2] < 2.0 * ADVANCE * 10.0);
    }

    #[test]
    fn alignment_rotation_and_mirroring() {
        let centred = TextSpec {
            h: 0,
            v: 0,
            ..spec("HHHH")
        };
        let b = bounds(&strokes(&centred));
        assert!((b[0] + b[2]).abs() < 1e-6, "{b:?}");
        assert!((b[1] + b[3]).abs() < 1e-6);
        let turned = TextSpec {
            rotation: 90.0,
            ..spec("HHHH")
        };
        let b = bounds(&strokes(&turned));
        assert!(b[3] > 30.0 && b[2] <= 1e-6, "{b:?}");
        let mirrored = TextSpec {
            mirror: true,
            ..spec("HHHH")
        };
        let b = bounds(&strokes(&mirrored));
        assert!(b[2] <= 1e-6 && b[0] < -30.0);
    }

    #[test]
    fn lines_go_down_and_unknown_characters_draw_a_question_mark() {
        let b = bounds(&strokes(&spec("H\nH")));
        assert!(b[1].abs() < 1e-6, "{b:?}");
        assert!((b[3] - (10.0 + LINE_PITCH * 10.0)).abs() < 1e-6, "{b:?}");
        assert_eq!(strokes(&spec("☃")).len(), strokes(&spec("?")).len());
        assert_eq!(strokes(&spec(" ")).len(), 0);
    }
}
