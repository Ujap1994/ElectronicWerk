//! Synthetic datasheets of invented parts, written as PDF files the way
//! manufacturers write them: a title page with a pinout drawing, a ruled
//! pin table with a column per package, electrical characteristics,
//! package outlines with dimension tables (ruled with millimetres and
//! inches, or aligned text) and ordering information. They are made for
//! the tests and the documentation of the part generator; no real
//! datasheet is copied.
//!
//! * `EW7021` — a dual comparator in SOIC-8, MSOP-8 and DFN-8 (3 × 3 mm
//!   with an exposed pad). Standard fonts, plain file.
//! * `EW4410` — an I/O expander in QFN-32 (5 × 5 mm) and LQFP-32 with a
//!   pin table without rules over two pages. A composite font, compressed
//!   object streams and AES encryption, as many datasheets are.

#![allow(
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    reason = "small drawing counts; drawing code named as in geometry"
)]

use std::fmt::Write as _;

use ew_datasheet::pdf::metrics::Standard;
use ew_datasheet::pdf::object::Object;
use ew_datasheet::pdf::write::{Encryption, WriteOptions, Writer, dict, name, numbers, reference};

const WIDTH: f64 = 612.0;
const HEIGHT: f64 = 792.0;
const MARGIN: f64 = 54.0;

/// A font of the synthetic datasheets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Face {
    Regular,
    Bold,
    Symbol,
}

/// One page being drawn (coordinates from the top left, y down).
struct Canvas {
    pages: Vec<String>,
    page: String,
    composite: bool,
    part: &'static str,
}

fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x80 => out.push(c),
            c => {
                // WinAnsi for the Latin-1 signs (µ, ±, °, ×).
                let code = match c {
                    '–' => 0x96,
                    '—' => 0x97,
                    '•' => 0x95,
                    c if (c as u32) <= 0xff => c as u32,
                    _ => u32::from(b'?'),
                };
                let _ = write!(out, "\\{code:03o}");
            }
        }
    }
    out
}

fn width_of(text: &str, size: f64, face: Face) -> f64 {
    let metrics = if face == Face::Bold {
        Standard::HelveticaBold
    } else {
        Standard::Helvetica
    };
    text.chars().map(|c| metrics.width(c)).sum::<f64>() / 1000.0 * size
}

impl Canvas {
    fn new(composite: bool, part: &'static str) -> Self {
        Self {
            pages: Vec::new(),
            page: String::new(),
            composite,
            part,
        }
    }

    fn new_page(&mut self, title: &str) {
        if !self.page.is_empty() {
            let page = std::mem::take(&mut self.page);
            self.pages.push(page);
        }
        let number = self.pages.len() + 1;
        // Header and footer of every page.
        self.fill_rect(0.0, 0.0, WIDTH, 34.0, 0.16);
        self.text_colored(MARGIN, 22.0, 13.0, Face::Bold, self.part, 1.0);
        let w = width_of(title, 9.0, Face::Regular);
        self.text_colored(WIDTH - MARGIN - w, 21.0, 9.0, Face::Regular, title, 1.0);
        self.line(MARGIN, HEIGHT - 40.0, WIDTH - MARGIN, HEIGHT - 40.0, 0.5);
        self.text(MARGIN, HEIGHT - 28.0, 7.5, Face::Regular, "Example Semiconductors — synthetic datasheet for ElectronicWerk tests and documentation. Not a real part.");
        let footer = format!("{number}");
        self.text(
            WIDTH - MARGIN - width_of(&footer, 8.0, Face::Regular),
            HEIGHT - 28.0,
            8.0,
            Face::Regular,
            &footer,
        );
    }

    fn finish(mut self) -> Vec<String> {
        if !self.page.is_empty() {
            self.pages.push(self.page);
        }
        self.pages
    }

    fn text(&mut self, x: f64, y: f64, size: f64, face: Face, text: &str) {
        if text == "θ" {
            // The Greek letter from the Symbol font, as datasheets do.
            self.text_colored(x, y, size, Face::Symbol, "q", 0.0);
        } else {
            self.text_colored(x, y, size, face, text, 0.0);
        }
    }

    fn text_colored(&mut self, x: f64, y: f64, size: f64, face: Face, text: &str, gray: f64) {
        let font = match face {
            Face::Regular => "F1",
            Face::Bold => "F2",
            Face::Symbol => "F3",
        };
        let shown = if self.composite && face != Face::Symbol {
            let mut hex = String::from("<");
            for c in text.chars() {
                let _ = write!(hex, "{:04X}", c as u32);
            }
            hex.push('>');
            hex
        } else {
            format!("({})", escape(text))
        };
        let font = if self.composite && face != Face::Symbol {
            if face == Face::Bold { "F5" } else { "F4" }
        } else {
            font
        };
        let _ = writeln!(
            self.page,
            "BT {gray} g /{font} {size} Tf {x:.2} {:.2} Td {shown} Tj ET 0 g",
            HEIGHT - y
        );
    }

    fn text_center(&mut self, cx: f64, y: f64, size: f64, face: Face, text: &str) {
        let w = width_of(text, size, face);
        self.text(cx - w / 2.0, y, size, face, text);
    }

    fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, width: f64) {
        let _ = writeln!(
            self.page,
            "{width} w {x0:.2} {:.2} m {x1:.2} {:.2} l S",
            HEIGHT - y0,
            HEIGHT - y1
        );
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, width: f64) {
        let _ = writeln!(
            self.page,
            "{width} w {x:.2} {:.2} {w:.2} {h:.2} re S",
            HEIGHT - y - h
        );
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, gray: f64) {
        let _ = writeln!(
            self.page,
            "{gray} g {x:.2} {:.2} {w:.2} {h:.2} re f 0 g",
            HEIGHT - y - h
        );
    }

    fn circle(&mut self, cx: f64, cy: f64, r: f64) {
        let k = 0.5523 * r;
        let y = HEIGHT - cy;
        let _ = writeln!(
            self.page,
            "{:.2} {y:.2} m {:.2} {:.2} {:.2} {:.2} {cx:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {y:.2} c {:.2} {:.2} {:.2} {:.2} {cx:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {y:.2} c f",
            cx + r,
            cx + r,
            y + k,
            cx + k,
            y + r,
            y + r,
            cx - k,
            y + r,
            cx - r,
            y + k,
            cx - r,
            cx - r,
            y - k,
            cx - k,
            y - r,
            y - r,
            cx + k,
            y - r,
            cx + r,
            y - k,
            cx + r
        );
    }

    fn heading(&mut self, y: f64, text: &str) -> f64 {
        self.text(MARGIN, y, 13.0, Face::Bold, text);
        y + 20.0
    }

    fn paragraph(&mut self, mut y: f64, width: f64, size: f64, text: &str) -> f64 {
        let mut line = String::new();
        for word in text.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if width_of(&candidate, size, Face::Regular) > width && !line.is_empty() {
                self.text(MARGIN, y, size, Face::Regular, &line);
                y += size * 1.35;
                word.clone_into(&mut line);
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            self.text(MARGIN, y, size, Face::Regular, &line);
            y += size * 1.35;
        }
        y
    }

    /// A ruled table; a cell of `""` joins the cell to its left in header
    /// rows (`span`), `None` widths come from the text.
    fn ruled_table(
        &mut self,
        x: f64,
        y: f64,
        widths: &[f64],
        header: &[Vec<(&str, usize)>],
        rows: &[Vec<String>],
    ) -> f64 {
        let row_height = 15.0;
        let total: f64 = widths.iter().sum();
        let mut top = y;
        let starts: Vec<f64> = widths
            .iter()
            .scan(x, |acc, w| {
                let s = *acc;
                *acc += w;
                Some(s)
            })
            .collect();
        // Header rows with spans, shaded.
        for row in header {
            self.fill_rect(x, top, total, row_height, 0.88);
            let mut column = 0;
            for (text, span) in row {
                let left = starts[column];
                let right = starts[column] + widths[column..column + span].iter().sum::<f64>();
                if !text.is_empty() {
                    self.text_center(
                        f64::midpoint(left, right),
                        top + 10.5,
                        8.0,
                        Face::Bold,
                        text,
                    );
                }
                self.line(left, top, left, top + row_height, 0.5);
                column += span;
            }
            self.line(x + total, top, x + total, top + row_height, 0.5);
            // A rule under the spanning cells only where they end.
            self.line(x, top, x + total, top, 0.5);
            top += row_height;
        }
        for row in rows {
            let lines: Vec<Vec<String>> = row
                .iter()
                .zip(widths)
                .map(|(text, w)| wrap(text, w - 8.0, 8.0))
                .collect();
            let count = lines.iter().map(Vec::len).max().unwrap_or(1);
            let height = row_height + (count as f64 - 1.0) * 10.0;
            self.line(x, top, x + total, top, 0.5);
            for (i, cell) in lines.iter().enumerate() {
                for (k, l) in cell.iter().enumerate() {
                    self.text(
                        starts[i] + 4.0,
                        top + 10.5 + k as f64 * 10.0,
                        8.0,
                        Face::Regular,
                        l,
                    );
                }
                self.line(starts[i], top, starts[i], top + height, 0.5);
            }
            self.line(x + total, top, x + total, top + height, 0.5);
            top += height;
        }
        self.line(x, top, x + total, top, 0.5);
        top
    }

    /// Lines of text aligned in columns, without rules.
    fn text_table(
        &mut self,
        x: f64,
        y: f64,
        starts: &[f64],
        rows: &[Vec<String>],
        bold_first: bool,
    ) -> f64 {
        let mut top = y;
        for (r, row) in rows.iter().enumerate() {
            for (i, cell) in row.iter().enumerate() {
                let face = if r == 0 && bold_first {
                    Face::Bold
                } else {
                    Face::Regular
                };
                self.text(x + starts[i], top, 8.0, face, cell);
            }
            top += 12.0;
        }
        top
    }
}

fn wrap(text: &str, width: f64, size: f64) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if width_of(&candidate, size, Face::Regular) > width && !line.is_empty() {
            out.push(std::mem::take(&mut line));
            word.clone_into(&mut line);
        } else {
            line = candidate;
        }
    }
    out.push(line);
    out
}

/// A two-row pinout drawing (SOIC/MSOP top view).
fn pinout(c: &mut Canvas, cx: f64, top: f64, names: &[&str]) {
    let half = names.len() / 2;
    let pitch = 18.0;
    let body_w = 70.0;
    let body_h = pitch * half as f64 + 8.0;
    let left = cx - body_w / 2.0;
    c.rect(left, top, body_w, body_h, 1.0);
    c.circle(left + 9.0, top + 9.0, 2.2);
    for i in 0..half {
        let y = top + 4.0 + pitch * (i as f64 + 0.5);
        // Left: 1 … N/2 down; right: N … N/2+1 down.
        c.line(left - 14.0, y, left, y, 1.5);
        c.line(left + body_w, y, left + body_w + 14.0, y, 1.5);
        let n_left = format!("{}", i + 1);
        let n_right = format!("{}", names.len() - i);
        c.text(left + 5.0, y + 3.0, 8.0, Face::Regular, &n_left);
        c.text(
            left + body_w - 5.0 - width_of(&n_right, 8.0, Face::Regular),
            y + 3.0,
            8.0,
            Face::Regular,
            &n_right,
        );
        let name_left = names[i];
        c.text(
            left - 18.0 - width_of(name_left, 8.0, Face::Regular),
            y + 3.0,
            8.0,
            Face::Regular,
            name_left,
        );
        c.text(
            left + body_w + 18.0,
            y + 3.0,
            8.0,
            Face::Regular,
            names[names.len() - 1 - i],
        );
    }
}

/// A dimension line with arrows and a label.
fn dimension(c: &mut Canvas, x0: f64, y0: f64, x1: f64, y1: f64, label: &str) {
    c.line(x0, y0, x1, y1, 0.4);
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = dx.hypot(dy).max(1.0);
    let (ux, uy) = (dx / len, dy / len);
    for (px, py, s) in [(x0, y0, 1.0), (x1, y1, -1.0)] {
        let (ax, ay) = (px + s * ux * 4.0, py + s * uy * 4.0);
        c.line(px, py, ax - uy * 1.6, ay + ux * 1.6, 0.4);
        c.line(px, py, ax + uy * 1.6, ay - ux * 1.6, 0.4);
    }
    let (mx, my) = (f64::midpoint(x0, x1), f64::midpoint(y0, y1));
    if dy.abs() > dx.abs() {
        c.text(mx + 4.0, my + 3.0, 8.0, Face::Regular, label);
    } else {
        c.text_center(mx, my - 3.0, 8.0, Face::Regular, label);
    }
}

/// A gull-wing package outline: top view and side view with dimensions.
fn soic_outline(c: &mut Canvas, x: f64, y: f64) {
    let s = 22.0; // points per millimetre
    let (d, e1, e, pitch, b) = (4.9 * s, 3.9 * s, 6.0 * s, 1.27 * s, 0.41 * s);
    let top = y + 20.0;
    let body_left = x + (e - e1) / 2.0;
    c.rect(body_left, top, e1, d, 1.0);
    c.circle(body_left + 10.0, top + 10.0, 3.0);
    for i in 0..4 {
        let cy = top + d / 2.0 - 1.5 * pitch + f64::from(i) * pitch;
        c.rect(x, cy - b / 2.0, (e - e1) / 2.0, b, 0.6);
        c.rect(body_left + e1, cy - b / 2.0, (e - e1) / 2.0, b, 0.6);
    }
    dimension(c, x, top - 10.0, x + e, top - 10.0, "E");
    dimension(
        c,
        body_left,
        top + d + 12.0,
        body_left + e1,
        top + d + 12.0,
        "E1",
    );
    dimension(c, x + e + 12.0, top, x + e + 12.0, top + d, "D");
    let p0 = top + d / 2.0 - 1.5 * pitch;
    dimension(c, x - 12.0, p0, x - 12.0, p0 + pitch, "e");
    c.text(x - 28.0, top + d + 30.0, 8.0, Face::Bold, "TOP VIEW");
    // Side view.
    let sx = x + e + 60.0;
    let base = top + 70.0;
    let (a, a1, l) = (1.55 * s, 0.17 * s, 0.84 * s);
    c.rect(sx + 10.0, base - a, e1, a - a1, 1.0);
    c.line(sx + 10.0 - l, base, sx + 10.0, base - a / 2.0, 0.8);
    c.line(
        sx + 10.0 + e1,
        base - a / 2.0,
        sx + 10.0 + e1 + l,
        base,
        0.8,
    );
    c.line(sx - 20.0, base, sx + e1 + 40.0, base, 0.3);
    dimension(c, sx + e1 + 34.0, base - a, sx + e1 + 34.0, base, "A");
    dimension(c, sx + 10.0 - l, base + 10.0, sx + 10.0, base + 10.0, "L");
    c.text(sx + 10.0, base + 30.0, 8.0, Face::Bold, "SIDE VIEW");
}

fn fonts(writer: &mut Writer, composite: bool) -> Vec<(&'static str, Object)> {
    let simple = |writer: &mut Writer, base: &str| {
        reference(writer.add(Object::Dict(dict(vec![
            ("Type", name("Font")),
            ("Subtype", name("Type1")),
            ("BaseFont", name(base)),
            ("Encoding", name("WinAnsiEncoding")),
        ]))))
    };
    let mut out = vec![
        ("F1", simple(writer, "Helvetica")),
        ("F2", simple(writer, "Helvetica-Bold")),
        (
            "F3",
            reference(writer.add(Object::Dict(dict(vec![
                ("Type", name("Font")),
                ("Subtype", name("Type1")),
                ("BaseFont", name("Symbol")),
            ])))),
        ),
    ];
    if composite {
        // CIDs are the Unicode values; ToUnicode maps them back.
        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n/CMapName /EW-UCS def\n1 begincodespacerange <0000> <FFFF> endcodespacerange\n",
        );
        cmap.push_str("2 beginbfrange <0020> <007E> <0020> <00A0> <00FF> <00A0> endbfrange\n");
        cmap.push_str(
            "4 beginbfchar <2013> <2013> <2014> <2014> <2022> <2022> <03A9> <03A9> endbfchar\n",
        );
        cmap.push_str("endcmap CMapName currentdict /CMap defineresource pop end end\n");
        for (key, base, metrics) in [
            ("F4", "ABCDEF+Helvetica", Standard::Helvetica),
            ("F5", "ABCDEF+Helvetica-Bold", Standard::HelveticaBold),
        ] {
            let to_unicode = writer.stream(dict(vec![]), cmap.as_bytes());
            let widths: Vec<f64> = (0x20u32..=0x7e)
                .map(|c| metrics.width(char::from_u32(c).unwrap_or(' ')))
                .collect();
            let descendant = writer.add(Object::Dict(dict(vec![
                ("Type", name("Font")),
                ("Subtype", name("CIDFontType2")),
                ("BaseFont", name(base)),
                ("DW", Object::Int(556)),
                (
                    "W",
                    Object::Array(vec![Object::Int(0x20), numbers(&widths)]),
                ),
                (
                    "CIDSystemInfo",
                    Object::Dict(dict(vec![
                        ("Registry", Object::String(b"Adobe".to_vec())),
                        ("Ordering", Object::String(b"Identity".to_vec())),
                        ("Supplement", Object::Int(0)),
                    ])),
                ),
            ])));
            out.push((
                key,
                reference(writer.add(Object::Dict(dict(vec![
                    ("Type", name("Font")),
                    ("Subtype", name("Type0")),
                    ("BaseFont", name(base)),
                    ("Encoding", name("Identity-H")),
                    (
                        "DescendantFonts",
                        Object::Array(vec![reference(descendant)]),
                    ),
                    ("ToUnicode", reference(to_unicode)),
                ])))),
            ));
        }
    }
    out
}

fn write_pdf(
    pages: &[String],
    options: WriteOptions,
    composite: bool,
    title: &str,
    subject: &str,
    bookmarks: &[(&str, usize)],
) -> Vec<u8> {
    let mut writer = Writer::new(options);
    let pages_root = writer.reserve();
    let font_list = fonts(&mut writer, composite);
    let resources = Object::Dict(dict(vec![(
        "Font",
        Object::Dict(
            font_list
                .iter()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.clone()))
                .collect(),
        ),
    )]));
    let mut page_refs = Vec::new();
    for content in pages {
        let contents = writer.stream(dict(vec![]), content.as_bytes());
        page_refs.push(writer.add(Object::Dict(dict(vec![
            ("Type", name("Page")),
            ("Parent", reference(pages_root)),
            ("Contents", reference(contents)),
            ("Resources", resources.clone()),
        ]))));
    }
    writer.set(
        pages_root,
        Object::Dict(dict(vec![
            ("Type", name("Pages")),
            (
                "Kids",
                Object::Array(page_refs.iter().map(|p| reference(*p)).collect()),
            ),
            (
                "Count",
                Object::Int(i64::try_from(page_refs.len()).unwrap_or(0)),
            ),
            ("MediaBox", numbers(&[0.0, 0.0, WIDTH, HEIGHT])),
        ])),
    );
    // Bookmarks, one level.
    let outlines = writer.reserve();
    let items: Vec<u32> = bookmarks.iter().map(|_| writer.reserve()).collect();
    for (i, (title, page)) in bookmarks.iter().enumerate() {
        let mut entries = vec![
            ("Title", Object::String(title.as_bytes().to_vec())),
            ("Parent", reference(outlines)),
            (
                "Dest",
                Object::Array(vec![
                    reference(page_refs[page.saturating_sub(1).min(page_refs.len() - 1)]),
                    name("Fit"),
                ]),
            ),
        ];
        if i + 1 < items.len() {
            entries.push(("Next", reference(items[i + 1])));
        }
        if i > 0 {
            entries.push(("Prev", reference(items[i - 1])));
        }
        writer.set(items[i], Object::Dict(dict(entries)));
    }
    let mut outline = vec![
        ("Type", name("Outlines")),
        (
            "Count",
            Object::Int(i64::try_from(items.len()).unwrap_or(0)),
        ),
    ];
    if let (Some(first), Some(last)) = (items.first(), items.last()) {
        outline.push(("First", reference(*first)));
        outline.push(("Last", reference(*last)));
    }
    writer.set(outlines, Object::Dict(dict(outline)));
    let catalog = writer.add(Object::Dict(dict(vec![
        ("Type", name("Catalog")),
        ("Pages", reference(pages_root)),
        ("Outlines", reference(outlines)),
        ("PageMode", name("UseOutlines")),
    ])));
    let info = writer.add(Object::Dict(dict(vec![
        ("Title", Object::String(title.as_bytes().to_vec())),
        ("Author", Object::String(b"Example Semiconductors".to_vec())),
        ("Subject", Object::String(subject.as_bytes().to_vec())),
        (
            "Creator",
            Object::String(b"ElectronicWerk synthetic datasheet".to_vec()),
        ),
    ])));
    writer.finish(catalog, Some(info))
}

fn s(text: &str) -> String {
    text.to_owned()
}

/// The `EW7021` dual comparator (SOIC-8, MSOP-8, DFN-8 with exposed pad).
#[must_use]
#[allow(clippy::too_many_lines, reason = "the pages of one datasheet")]
pub fn ew7021() -> Vec<u8> {
    let mut c = Canvas::new(false, "EW7021");
    // Page 1: title, features, description, pinout.
    c.new_page("Dual Low-Power Comparator");
    c.text(MARGIN, 70.0, 24.0, Face::Bold, "EW7021");
    c.text(
        MARGIN,
        92.0,
        14.0,
        Face::Regular,
        "Dual Low-Power Comparator, 1.8 V to 5.5 V",
    );
    let mut y = c.heading(126.0, "1 Features");
    for feature in [
        "• Supply voltage 1.8 V to 5.5 V",
        "• Supply current 6 µA per comparator",
        "• Propagation delay 450 ns",
        "• Push-pull outputs, rail-to-rail inputs",
        "• Operating temperature –40 °C to 125 °C",
        "• SOIC-8, MSOP-8 and DFN-8 (3 mm × 3 mm) packages",
    ] {
        c.text(MARGIN + 6.0, y, 9.5, Face::Regular, feature);
        y += 14.0;
    }
    let y = c.heading(y + 12.0, "2 Description");
    let y = c.paragraph(
        y,
        270.0,
        9.5,
        "The EW7021 is a dual comparator with very low supply current for battery powered systems. Both comparators have push-pull outputs and inputs that reach both supply rails. The device is offered in an 8-pin SOIC, an 8-pin MSOP and an 8-pin DFN package with an exposed thermal pad.",
    );
    let _ = y;
    c.text(360.0, 130.0, 11.0, Face::Bold, "Pinout");
    pinout(
        &mut c,
        450.0,
        150.0,
        &["OUT1", "IN1-", "IN1+", "GND", "IN2+", "IN2-", "OUT2", "VCC"],
    );
    c.text_center(
        450.0,
        248.0,
        8.0,
        Face::Regular,
        "SOIC-8, MSOP-8 (Top View)",
    );
    c.text(360.0, 290.0, 11.0, Face::Bold, "Device Information");
    c.ruled_table(
        360.0,
        300.0,
        &[70.0, 60.0, 70.0],
        &[vec![("PART NUMBER", 1), ("PACKAGE", 1), ("BODY SIZE", 1)]],
        &[
            vec![s("EW7021D"), s("SOIC-8"), s("4.90 × 3.90")],
            vec![s("EW7021DGK"), s("MSOP-8"), s("3.00 × 3.00")],
            vec![s("EW7021DRB"), s("DFN-8"), s("3.00 × 3.00")],
        ],
    );
    // Page 2: pin table.
    c.new_page("Pin Configuration and Functions");
    let y = c.heading(70.0, "3 Pin Configuration and Functions");
    c.text(MARGIN, y, 9.0, Face::Bold, "Table 3-1. Pin Functions");
    let rows: Vec<Vec<String>> = [
        ("OUT1", "1", "1", "O", "Output of comparator 1"),
        ("IN1-", "2", "2", "I", "Inverting input of comparator 1"),
        ("IN1+", "3", "3", "I", "Non-inverting input of comparator 1"),
        ("GND", "4", "4", "G", "Ground"),
        ("IN2+", "5", "5", "I", "Non-inverting input of comparator 2"),
        ("IN2-", "6", "6", "I", "Inverting input of comparator 2"),
        ("OUT2", "7", "7", "O", "Output of comparator 2"),
        (
            "VCC",
            "8",
            "8",
            "P",
            "Supply voltage, 1.8 V to 5.5 V. Decouple with 100 nF to GND close to the pin.",
        ),
        (
            "EP",
            "—",
            "9",
            "—",
            "Exposed thermal pad of the DFN package. Connect to GND or leave floating.",
        ),
    ]
    .iter()
    .map(|(a, b, cc, d, e)| vec![s(a), s(b), s(cc), s(d), s(e)])
    .collect();
    c.ruled_table(
        MARGIN,
        y + 8.0,
        &[60.0, 90.0, 60.0, 40.0, 254.0],
        &[
            vec![("PIN", 3), ("TYPE", 1), ("DESCRIPTION", 1)],
            vec![
                ("NAME", 1),
                ("SOIC-8, MSOP-8", 1),
                ("DFN-8", 1),
                ("", 1),
                ("", 1),
            ],
        ],
        &rows,
    );
    c.text(
        MARGIN,
        330.0,
        8.0,
        Face::Regular,
        "I = input, O = output, P = power, G = ground.",
    );
    // Page 3: electrical characteristics (not a pin table).
    c.new_page("Specifications");
    let y = c.heading(70.0, "4 Electrical Characteristics");
    c.text(
        MARGIN,
        y,
        9.0,
        Face::Regular,
        "VCC = 3.3 V, TA = 25 °C (unless otherwise noted)",
    );
    let spec: Vec<Vec<String>> = [
        ("VCC", "Supply voltage", "", "1.8", "", "5.5", "V"),
        (
            "ICC",
            "Supply current",
            "per comparator",
            "",
            "6",
            "10",
            "µA",
        ),
        ("VOS", "Input offset voltage", "", "", "1", "5", "mV"),
        (
            "tPD",
            "Propagation delay",
            "100 mV overdrive",
            "",
            "450",
            "",
            "ns",
        ),
        (
            "VOH",
            "Output high voltage",
            "IOUT = –2 mA",
            "3.1",
            "3.2",
            "",
            "V",
        ),
    ]
    .iter()
    .map(|(a, b, cc, d, e, f, g)| vec![s(a), s(b), s(cc), s(d), s(e), s(f), s(g)])
    .collect();
    c.ruled_table(
        MARGIN,
        y + 10.0,
        &[44.0, 120.0, 120.0, 44.0, 44.0, 44.0, 44.0],
        &[vec![
            ("SYMBOL", 1),
            ("PARAMETER", 1),
            ("CONDITIONS", 1),
            ("MIN", 1),
            ("TYP", 1),
            ("MAX", 1),
            ("UNIT", 1),
        ]],
        &spec,
    );
    // Page 4: SOIC-8 outline with a ruled table in mm and inches.
    c.new_page("Package Information");
    let y = c.heading(70.0, "5 Mechanical Data");
    c.text(
        MARGIN,
        y,
        10.0,
        Face::Bold,
        "5.1 SOIC-8 Package Outline (D)",
    );
    soic_outline(&mut c, MARGIN + 40.0, y + 20.0);
    let dims: Vec<Vec<String>> = [
        ("A", "1.35", "", "1.75", ".053", "", ".069"),
        ("A1", "0.10", "", "0.25", ".004", "", ".010"),
        ("b", "0.31", "0.41", "0.51", ".012", ".016", ".020"),
        ("c", "0.17", "", "0.25", ".007", "", ".010"),
        ("D", "4.80", "4.90", "5.00", ".189", ".193", ".197"),
        ("E", "5.80", "6.00", "6.20", ".228", ".236", ".244"),
        ("E1", "3.80", "3.90", "4.00", ".150", ".154", ".157"),
        ("e", "", "1.27 BSC", "", "", ".050 BSC", ""),
        ("L", "0.40", "", "1.27", ".016", "", ".050"),
        ("θ", "0°", "", "8°", "0°", "", "8°"),
    ]
    .iter()
    .map(|(a, b, cc, d, e, f, g)| vec![s(a), s(b), s(cc), s(d), s(e), s(f), s(g)])
    .collect();
    c.ruled_table(
        MARGIN + 40.0,
        y + 230.0,
        &[50.0, 50.0, 50.0, 50.0, 50.0, 50.0, 50.0],
        &[
            vec![("SYMBOL", 1), ("MILLIMETERS", 3), ("INCHES", 3)],
            vec![
                ("", 1),
                ("MIN", 1),
                ("NOM", 1),
                ("MAX", 1),
                ("MIN", 1),
                ("NOM", 1),
                ("MAX", 1),
            ],
        ],
        &dims,
    );
    // Page 5: DFN-8 with an aligned text table; MSOP without a table.
    c.new_page("Package Information");
    let y = c.heading(70.0, "5.2 DFN-8 (3 mm × 3 mm) Package Outline (DRB)");
    c.rect(MARGIN + 30.0, y + 20.0, 120.0, 120.0, 1.0);
    c.rect(MARGIN + 30.0 + 27.0, y + 20.0 + 12.4, 66.0, 95.2, 0.6);
    c.text(MARGIN + 30.0, y + 160.0, 8.0, Face::Bold, "BOTTOM VIEW");
    let table: Vec<Vec<String>> = [
        ("Symbol", "Min", "Nom", "Max"),
        ("A", "0.70", "0.75", "0.80"),
        ("A1", "0.00", "0.02", "0.05"),
        ("b", "0.25", "0.30", "0.35"),
        ("D", "2.90", "3.00", "3.10"),
        ("D2", "2.23", "2.38", "2.48"),
        ("E", "2.90", "3.00", "3.10"),
        ("E2", "1.55", "1.65", "1.75"),
        ("e", "", "0.65 BSC", ""),
        ("L", "0.30", "0.40", "0.50"),
    ]
    .iter()
    .map(|(a, b, cc, d)| vec![s(a), s(b), s(cc), s(d)])
    .collect();
    let bottom = c.text_table(320.0, y + 30.0, &[0.0, 60.0, 110.0, 170.0], &table, true);
    c.text(
        320.0,
        bottom + 8.0,
        8.0,
        Face::Regular,
        "All dimensions in millimeters.",
    );
    let y = c.heading(y + 220.0, "5.3 MSOP-8 Package (DGK)");
    c.paragraph(
        y,
        480.0,
        9.5,
        "The MSOP-8 package follows the JEDEC MO-187 outline (3.00 mm × 3.00 mm body, 0.65 mm pitch).",
    );
    // Page 6: ordering information.
    c.new_page("Ordering Information");
    let y = c.heading(70.0, "6 Ordering Information");
    c.ruled_table(
        MARGIN,
        y + 6.0,
        &[100.0, 70.0, 60.0, 50.0, 80.0],
        &[vec![
            ("ORDERABLE PART", 1),
            ("PACKAGE", 1),
            ("PINS", 1),
            ("QTY", 1),
            ("MARKING", 1),
        ]],
        &[
            vec![s("EW7021DR"), s("SOIC"), s("8"), s("2500"), s("7021")],
            vec![s("EW7021DGKR"), s("MSOP"), s("8"), s("2500"), s("7021")],
            vec![s("EW7021DRBR"), s("DFN"), s("8"), s("3000"), s("7021")],
        ],
    );
    let pages = c.finish();
    write_pdf(
        &pages,
        WriteOptions::default(),
        false,
        "EW7021 Dual Low-Power Comparator",
        "Data Sheet",
        &[
            ("1 Features", 1),
            ("3 Pin Configuration and Functions", 2),
            ("4 Electrical Characteristics", 3),
            ("5 Mechanical Data", 4),
            ("6 Ordering Information", 6),
        ],
    )
}

#[allow(clippy::too_many_lines, reason = "the pages of one datasheet")]
/// The `EW4410` I/O expander (QFN-32 5 × 5 mm, LQFP-32), with a pin table
/// without rules over two pages; written with a composite font,
/// compressed object streams and AES encryption.
#[must_use]
pub fn ew4410() -> Vec<u8> {
    let mut c = Canvas::new(true, "EW4410");
    c.new_page("16-Bit I/O Expander");
    c.text(MARGIN, 70.0, 24.0, Face::Bold, "EW4410");
    c.text(
        MARGIN,
        92.0,
        14.0,
        Face::Regular,
        "16-Bit I/O Expander with I²C Interface",
    );
    c.paragraph(
        120.0,
        480.0,
        9.5,
        "The EW4410 adds sixteen general purpose input/output pins to an I2C bus. It comes in a QFN-32 package (5 mm × 5 mm) with an exposed pad and in an LQFP-32 package (7 mm × 7 mm).",
    );
    let mut pins: Vec<(String, String, String, String, String)> = Vec::new();
    let mut add = |name: &str, qfn: &str, lqfp: &str, kind: &str, function: &str| {
        pins.push((s(name), s(qfn), s(lqfp), s(kind), s(function)));
    };
    for i in 0..8 {
        add(
            &format!("PA{i}"),
            &format!("{}", i + 1),
            &format!("{}", i + 1),
            "I/O",
            &format!("Port A, bit {i}"),
        );
    }
    add("VDD", "9", "9", "P", "Supply voltage");
    add("SCL", "10", "10", "I", "I2C clock");
    add("SDA", "11", "11", "I/O", "I2C data");
    add("INT", "12", "12", "OD", "Interrupt output, open drain");
    add("RESET", "13", "13", "I", "Reset, active low");
    add("A0", "14", "14", "I", "Address input 0");
    add("A1", "15", "15", "I", "Address input 1");
    add(
        "GND",
        "16, 33",
        "16",
        "G",
        "Ground; pad 33 is the exposed pad of the QFN package",
    );
    for i in 0..8 {
        add(
            &format!("PB{i}"),
            &format!("{}", 17 + i),
            &format!("{}", 17 + i),
            "I/O",
            &format!("Port B, bit {i}"),
        );
    }
    for i in 0..8 {
        add(
            "NC",
            &format!("{}", 25 + i),
            &format!("{}", 25 + i),
            "—",
            "Not connected",
        );
    }
    let header = vec![
        s("Pin Name"),
        s("QFN-32"),
        s("LQFP-32"),
        s("Type"),
        s("Function"),
    ];
    let starts = [0.0, 80.0, 140.0, 200.0, 250.0];
    let first: Vec<Vec<String>> = std::iter::once(header.clone())
        .chain(pins[..18].iter().map(|p| {
            vec![
                p.0.clone(),
                p.1.clone(),
                p.2.clone(),
                p.3.clone(),
                p.4.clone(),
            ]
        }))
        .collect();
    let y = c.heading(200.0, "Pin Functions");
    c.text_table(MARGIN, y + 6.0, &starts, &first, true);
    c.new_page("Pin Functions (continued)");
    let y = c.heading(70.0, "Pin Functions (continued)");
    let rest: Vec<Vec<String>> = std::iter::once(header)
        .chain(pins[18..].iter().map(|p| {
            vec![
                p.0.clone(),
                p.1.clone(),
                p.2.clone(),
                p.3.clone(),
                p.4.clone(),
            ]
        }))
        .collect();
    c.text_table(MARGIN, y + 6.0, &starts, &rest, true);
    let pages = c.finish();
    write_pdf(
        &pages,
        WriteOptions {
            compress: true,
            object_streams: true,
            encryption: Some(Encryption::Aes128),
        },
        true,
        "EW4410 16-Bit I/O Expander",
        "16-bit I/O expander with I2C interface",
        &[("Pin Functions", 1)],
    )
}
