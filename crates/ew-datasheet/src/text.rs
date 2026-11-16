//! From placed characters to words and lines, and the page model the rest
//! of the crate and the app work with. Coordinates are points in display
//! space: origin at the top left of the visible page, y down.

use serde::{Deserialize, Serialize};

use crate::pdf::content::{PageContent, PlacedGlyph, Segment};

/// A rectangle `[x0, y0, x1, y1]` with `x0 <= x1`, `y0 <= y1`.
pub type Rect = [f64; 4];

/// A word: characters without a gap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    /// Text.
    pub text: String,
    /// Bounds.
    pub bbox: Rect,
    /// Font size.
    pub size: f64,
    /// Bold font.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
}

/// A line of words on one baseline, left to right.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// Words with single spaces.
    pub text: String,
    /// Bounds.
    pub bbox: Rect,
    /// Typical font size.
    pub size: f64,
    /// Reading direction: 0, 90, 180 or 270 degrees clockwise.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: u16,
    /// The words.
    pub words: Vec<Word>,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde skip predicate")]
const fn is_zero(value: &u16) -> bool {
    *value == 0
}

/// A horizontal or vertical rule `[x0, y0, x1, y1]`.
pub type Rule = [f64; 4];

/// One page of a datasheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    /// Page number (1-based).
    pub number: usize,
    /// Visible width (points).
    pub width: f64,
    /// Visible height (points).
    pub height: f64,
    /// Lines in reading order (top to bottom, then left to right).
    pub lines: Vec<Line>,
    /// Horizontal and vertical rules, merged.
    pub rules: Vec<Rule>,
    /// A scanned page: an image covers it and it has no text.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub scanned: bool,
}

struct Oriented<'a> {
    glyph: &'a PlacedGlyph,
    /// Baseline start and end in the rotated frame (text runs along +x).
    x0: f64,
    x1: f64,
    y: f64,
}

/// Rotation class of a glyph: 0, 1, 2 or 3 quarter turns clockwise.
fn quarter(angle: f64) -> u8 {
    let turns = (angle / std::f64::consts::FRAC_PI_2)
        .round()
        .rem_euclid(4.0);
    turns as u8
}

/// Into the frame where text of `q` quarter turns runs along +x.
fn to_frame(q: u8, x: f64, y: f64) -> (f64, f64) {
    match q {
        1 => (y, -x),
        2 => (-x, -y),
        3 => (-y, x),
        _ => (x, y),
    }
}

fn from_frame(q: u8, x: f64, y: f64) -> (f64, f64) {
    match q {
        1 => (-y, x),
        2 => (-x, -y),
        3 => (y, -x),
        _ => (x, y),
    }
}

fn rect_from_frame(q: u8, r: Rect) -> Rect {
    let (ax, ay) = from_frame(q, r[0], r[1]);
    let (bx, by) = from_frame(q, r[2], r[3]);
    [ax.min(bx), ay.min(by), ax.max(bx), ay.max(by)]
}

struct Building {
    text: String,
    x0: f64,
    x1: f64,
    top: f64,
    bottom: f64,
    baseline: f64,
    size: f64,
    bold: bool,
}

/// Builds the lines of a page.
#[must_use]
pub fn lines(content: &PageContent) -> Vec<Line> {
    let mut out = Vec::new();
    for q in 0..4u8 {
        let mut words = words_in(content, q);
        if words.is_empty() {
            continue;
        }
        // Lines: words whose boxes overlap vertically by half.
        words.sort_by(|a, b| {
            a.baseline
                .total_cmp(&b.baseline)
                .then(a.x0.total_cmp(&b.x0))
        });
        let mut groups: Vec<Vec<Building>> = Vec::new();
        for word in words {
            let joined = groups.iter_mut().rev().take(3).find(|g| {
                let reference = &g[0];
                let overlap = word.bottom.min(reference.bottom) - word.top.max(reference.top);
                let height = (word.bottom - word.top).min(reference.bottom - reference.top);
                overlap > 0.5 * height
            });
            match joined {
                Some(g) => g.push(word),
                None => groups.push(vec![word]),
            }
        }
        for mut group in groups {
            group.sort_by(|a, b| a.x0.total_cmp(&b.x0));
            // Join pieces that touch (sub- and superscripts, split runs).
            let mut merged: Vec<Building> = Vec::new();
            for w in group {
                if let Some(prev) = merged.last_mut()
                    && w.x0 - prev.x1 < 0.08 * prev.size.max(w.size)
                    && w.x0 >= prev.x0
                {
                    prev.text.push_str(&w.text);
                    prev.x1 = prev.x1.max(w.x1);
                    prev.top = prev.top.min(w.top);
                    prev.bottom = prev.bottom.max(w.bottom);
                    prev.bold &= w.bold;
                    continue;
                }
                merged.push(w);
            }
            let words: Vec<Word> = merged
                .iter()
                .map(|w| Word {
                    text: w.text.clone(),
                    bbox: rect_from_frame(q, [w.x0, w.top, w.x1, w.bottom]),
                    size: round(w.size),
                    bold: w.bold,
                })
                .collect();
            let mut sizes: Vec<f64> = merged.iter().map(|w| w.size).collect();
            sizes.sort_by(f64::total_cmp);
            let frame = [
                merged.iter().map(|w| w.x0).fold(f64::INFINITY, f64::min),
                merged.iter().map(|w| w.top).fold(f64::INFINITY, f64::min),
                merged
                    .iter()
                    .map(|w| w.x1)
                    .fold(f64::NEG_INFINITY, f64::max),
                merged
                    .iter()
                    .map(|w| w.bottom)
                    .fold(f64::NEG_INFINITY, f64::max),
            ];
            out.push(Line {
                text: words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
                bbox: rect_from_frame(q, frame),
                size: round(sizes[sizes.len() / 2]),
                angle: u16::from(q) * 90,
                words,
            });
        }
    }
    out.sort_by(|a, b| {
        a.angle
            .cmp(&b.angle)
            .then(a.bbox[1].total_cmp(&b.bbox[1]))
            .then(a.bbox[0].total_cmp(&b.bbox[0]))
    });
    out
}

fn round(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Words of one orientation, in the rotated frame.
fn words_in(content: &PageContent, q: u8) -> Vec<Building> {
    let glyphs: Vec<Oriented<'_>> = content
        .glyphs
        .iter()
        .filter(|g| quarter(g.angle) == q)
        .map(|g| {
            let (x0, y) = to_frame(q, g.x, g.y);
            let (x1, _) = to_frame(q, g.x + g.dx, g.y + g.dy);
            Oriented {
                glyph: g,
                x0: x0.min(x1),
                x1: x0.max(x1),
                y,
            }
        })
        .collect();
    let mut words: Vec<Building> = Vec::new();
    let mut previous: Option<&Oriented<'_>> = None;
    for g in &glyphs {
        let text = g.glyph.text.as_str();
        let blank = text.chars().all(char::is_whitespace);
        if let Some(p) = previous {
            // Overprinted for a bold look: the same character again.
            if p.glyph.text == text
                && (p.x0 - g.x0).abs() < 0.1 * g.glyph.size
                && (p.y - g.y).abs() < 0.1 * g.glyph.size
            {
                continue;
            }
        }
        let size = g.glyph.size;
        let continues = previous.is_some_and(|p| {
            let gap = g.x0 - p.x1;
            (p.y - g.y).abs() < 0.3 * size.min(p.glyph.size)
                && gap < 0.2 * size.max(p.glyph.size)
                && gap > -0.6 * size.max(p.glyph.size)
                && (p.glyph.size / size - 1.0).abs() < 0.25
        });
        if blank {
            previous = None;
            continue;
        }
        let bold = content
            .fonts
            .get(g.glyph.font)
            .is_some_and(|(_, bold)| *bold);
        let top = g.y - 0.8 * size;
        let bottom = g.y + 0.2 * size;
        match words.last_mut() {
            Some(w) if continues => {
                w.text.push_str(text);
                w.x1 = w.x1.max(g.x1);
                w.top = w.top.min(top);
                w.bottom = w.bottom.max(bottom);
                w.bold &= bold;
            }
            _ => words.push(Building {
                text: text.to_owned(),
                x0: g.x0,
                x1: g.x1.max(g.x0 + 0.1 * size),
                top,
                bottom,
                baseline: g.y,
                size,
                bold,
            }),
        }
        previous = Some(g);
    }
    for w in &mut words {
        w.text = w.text.trim().to_owned();
    }
    words.retain(|w| !w.text.is_empty());
    words
}

/// Merges rules on the same line that overlap or nearly touch.
#[must_use]
pub fn rules(segments: &[Segment]) -> Vec<Rule> {
    let mut horizontal: Vec<Rule> = Vec::new();
    let mut vertical: Vec<Rule> = Vec::new();
    for s in segments {
        if (s.y1 - s.y0).abs() < 0.5 {
            horizontal.push([s.x0, s.y0, s.x1, s.y0]);
        } else {
            vertical.push([s.x0, s.y0, s.x0, s.y1]);
        }
    }
    let merge = |list: &mut Vec<Rule>, horizontal: bool| -> Vec<Rule> {
        // Sort by the fixed coordinate, then along the line.
        let (fixed, from, to) = if horizontal { (1, 0, 2) } else { (0, 1, 3) };
        list.sort_by(|a, b| {
            a[fixed]
                .total_cmp(&b[fixed])
                .then(a[from].total_cmp(&b[from]))
        });
        let mut out: Vec<Rule> = Vec::new();
        for r in list.iter() {
            if let Some(last) = out.iter_mut().rev().take(8).find(|l| {
                (l[fixed] - r[fixed]).abs() < 1.5
                    && r[from] <= l[to] + 1.5
                    && r[to] >= l[from] - 1.5
            }) {
                last[from] = last[from].min(r[from]);
                last[to] = last[to].max(r[to]);
                continue;
            }
            out.push(*r);
        }
        out
    };
    let mut out = merge(&mut horizontal, true);
    out.extend(merge(&mut vertical, false));
    out.iter()
        .map(|r| [round(r[0]), round(r[1]), round(r[2]), round(r[3])])
        .collect()
}

/// Builds a page from interpreted content.
#[must_use]
pub fn page(number: usize, content: &PageContent) -> Page {
    let area = content.width * content.height;
    let covered = content.images.iter().any(|r| {
        let w = (r[2].min(content.width) - r[0].max(0.0)).max(0.0);
        let h = (r[3].min(content.height) - r[1].max(0.0)).max(0.0);
        w * h > 0.5 * area
    });
    let lines = lines(content);
    let characters: usize = lines.iter().map(|l| l.text.len()).sum();
    Page {
        number,
        width: round(content.width),
        height: round(content.height),
        scanned: covered && characters < 40,
        rules: rules(&content.segments),
        lines,
    }
}
