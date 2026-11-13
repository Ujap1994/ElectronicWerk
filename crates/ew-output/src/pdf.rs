//! A small PDF writer for [`Doc`] pages: compressed vector content, one
//! bookmark per page and invisible text (Courier, render mode 3) laid over
//! the stroke-font texts so that prints can be searched and copied.

use std::fmt::Write as _;
use std::io::Write as _;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::canvas::{Doc, Page, Paint, Seg};
use crate::font::ADVANCE;

/// Nanometres per PDF point (1/72 inch).
const NM_PER_PT: f64 = 25_400_000.0 / 72.0;

/// Advance of Courier glyphs relative to the font size.
const COURIER_ADVANCE: f64 = 0.6;

fn pt(nm: f64) -> String {
    let v = nm / NM_PER_PT;
    let text = format!("{v:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" || text.is_empty() {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

fn component(value: u8) -> String {
    let text = format!("{:.3}", f64::from(value) / 255.0);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// A PDF text string: Latin-1 characters, escaped; others become `?`.
fn pdf_string(text: &str) -> String {
    let mut out = String::from("(");
    for c in text.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) >= 0x20 && (c as u32) < 0x7f => out.push(c),
            c if (c as u32) >= 0xa0 && (c as u32) <= 0xff => {
                let _ = write!(out, "\\{:03o}", c as u32);
            }
            _ => out.push('?'),
        }
    }
    out.push(')');
    out
}

/// A text string for the document information and bookmarks (UTF-16).
fn utf16_string(text: &str) -> String {
    let mut out = String::from("<FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(out, "{unit:04X}");
    }
    out.push('>');
    out
}

fn content(page: &Page) -> String {
    let mut out = String::new();
    out.push_str("1 J 1 j\n");
    if let Some(bg) = page.background {
        let _ = writeln!(
            out,
            "{} {} {} rg 0 0 {} {} re f",
            component(bg.r),
            component(bg.g),
            component(bg.b),
            pt(page.width),
            pt(page.height)
        );
    }
    for item in &page.items {
        for seg in &item.path.0 {
            match seg {
                Seg::Move(p) => {
                    let _ = writeln!(out, "{} {} m", pt(p.x), pt(p.y));
                }
                Seg::Line(p) => {
                    let _ = writeln!(out, "{} {} l", pt(p.x), pt(p.y));
                }
                Seg::Cubic(a, b, c) => {
                    let _ = writeln!(
                        out,
                        "{} {} {} {} {} {} c",
                        pt(a.x),
                        pt(a.y),
                        pt(b.x),
                        pt(b.y),
                        pt(c.x),
                        pt(c.y)
                    );
                }
                Seg::Close => out.push_str("h\n"),
            }
        }
        let Paint { fill, stroke } = item.paint;
        if let Some(color) = fill {
            let _ = write!(
                out,
                "{} {} {} rg ",
                component(color.r),
                component(color.g),
                component(color.b)
            );
        }
        if let Some((color, width)) = stroke {
            let _ = write!(
                out,
                "{} {} {} RG {} w ",
                component(color.r),
                component(color.g),
                component(color.b),
                pt(width.max(0.0))
            );
        }
        out.push_str(match (fill.is_some(), stroke.is_some()) {
            (true, true) => "B*\n",
            (true, false) => "f*\n",
            (false, true) => "S\n",
            (false, false) => "n\n",
        });
    }
    if !page.labels.is_empty() {
        out.push_str("BT 3 Tr\n");
        for label in &page.labels {
            let size = ADVANCE * label.size / COURIER_ADVANCE;
            let (sin, cos) = label.rotation.to_radians().sin_cos();
            let m = if label.mirror { -1.0 } else { 1.0 };
            let _ = writeln!(
                out,
                "/F1 {} Tf {:.5} {:.5} {:.5} {:.5} {} {} Tm {} Tj",
                pt(size),
                cos * m,
                sin * m,
                -sin,
                cos,
                pt(label.at.x),
                pt(label.at.y),
                pdf_string(&label.text)
            );
        }
        out.push_str("ET\n");
    }
    out
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    let _ = encoder.write_all(data);
    encoder.finish().unwrap_or_default()
}

/// `D:YYYYMMDDHHmmSS` from `YYYY-MM-DDTHH:MM:SS…`.
fn pdf_date(iso: &str) -> Option<String> {
    let digits: String = iso.chars().filter(char::is_ascii_digit).take(14).collect();
    (digits.len() == 14).then(|| format!("D:{digits}Z"))
}

/// Writes a document as PDF.
#[must_use]
pub fn write(doc: &Doc) -> Vec<u8> {
    // Objects: 1 catalog, 2 pages, 3 font, 4 info, 5 outlines, then per page
    // the page, its content and its bookmark.
    let pages = doc.pages.len();
    let first = 6;
    let page_id = |i: usize| first + 3 * i;
    let mut objects: Vec<Vec<u8>> = Vec::new();
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", page_id(i))).collect();
    objects
        .push(b"<< /Type /Catalog /Pages 2 0 R /Outlines 5 0 R /PageMode /UseOutlines >>".to_vec());
    objects.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {pages} >>",
            kids.join(" ")
        )
        .into_bytes(),
    );
    objects.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>".to_vec(),
    );
    let mut info = format!(
        "<< /Title {} /Producer {} /Creator {}",
        utf16_string(&doc.title),
        utf16_string(&format!("ElectronicWerk {}", doc.version)),
        utf16_string("ElectronicWerk")
    );
    if let Some(date) = pdf_date(&doc.created) {
        let _ = write!(info, " /CreationDate ({date})");
    }
    info.push_str(" >>");
    objects.push(info.into_bytes());
    if pages == 0 {
        objects.push(b"<< /Type /Outlines /Count 0 >>".to_vec());
    } else {
        objects.push(
            format!(
                "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {pages} >>",
                page_id(0) + 2,
                page_id(pages - 1) + 2
            )
            .into_bytes(),
        );
    }
    for (i, page) in doc.pages.iter().enumerate() {
        let id = page_id(i);
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                pt(page.width),
                pt(page.height),
                id + 1
            )
            .into_bytes(),
        );
        let stream = deflate(content(page).as_bytes());
        let mut object = format!(
            "<< /Length {} /Filter /FlateDecode >>\nstream\n",
            stream.len()
        )
        .into_bytes();
        object.extend_from_slice(&stream);
        object.extend_from_slice(b"\nendstream");
        objects.push(object);
        let mut bookmark = format!(
            "<< /Title {} /Parent 5 0 R /Dest [{id} 0 R /Fit]",
            utf16_string(if page.name.is_empty() {
                &doc.title
            } else {
                &page.name
            })
        );
        if i > 0 {
            let _ = write!(bookmark, " /Prev {} 0 R", page_id(i - 1) + 2);
        }
        if i + 1 < pages {
            let _ = write!(bookmark, " /Next {} 0 R", page_id(i + 1) + 2);
        }
        bookmark.push_str(" >>");
        objects.push(bookmark.into_bytes());
    }
    let mut out: Vec<u8> = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(object);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for offset in offsets {
        let _ = writeln!(table, "{offset:010} 00000 n ");
    }
    let _ = write!(
        table,
        "trailer\n<< /Size {} /Root 1 0 R /Info 4 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    );
    out.extend_from_slice(table.as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{Color, Path};
    use crate::font::TextSpec;
    use ew_pcb::geom::V;

    #[test]
    fn writes_pages_bookmarks_and_text() {
        let mut page = Page::new("Sheet 1", 297e6, 210e6);
        page.add(
            Path::polyline(&[V::new(0.0, 0.0), V::new(1e7, 1e7)], false),
            Paint::stroke(Color::BLACK, 1e5),
        );
        page.add(
            Path::circle(V::new(5e7, 5e7), 1e7),
            Paint::fill(Color::rgb(200, 52, 52)),
        );
        page.text(
            &TextSpec {
                text: "R1 (10k)",
                at: [1e7, 1e7],
                size: 2e6,
                rotation: 0.0,
                h: -1,
                v: -1,
                mirror: false,
                italic: false,
            },
            Color::BLACK,
            2e5,
        );
        let doc = Doc {
            title: "Demo µ".into(),
            version: "1.0".into(),
            created: "2026-11-13T07:00:00".into(),
            pages: vec![page.clone(), Page::new("Sheet 2", 297e6, 210e6)],
        };
        let bytes = write(&doc);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.7"));
        assert!(text.contains("/Count 2"));
        assert!(text.contains("/MediaBox [0 0 841.89 595.276]"), "{text}");
        assert!(text.contains("/CreationDate (D:20261113070000Z)"));
        assert!(text.trim_end().ends_with("%%EOF"));
        let content = content(&page);
        assert!(content.contains("0.784 0.204 0.204 rg f*"), "{content}");
        assert!(content.contains("(R1 \\(10k\\)) Tj"));
        // Every xref offset points at its object.
        let xref = bytes.windows(5).rposition(|w| w == b"xref\n").unwrap();
        let table = String::from_utf8_lossy(&bytes[xref..]).into_owned();
        for (i, line) in table.lines().skip(3).take(9).enumerate() {
            let offset: usize = line[..10].parse().unwrap();
            let expected = format!("{} 0 obj", i + 1);
            assert!(bytes[offset..].starts_with(expected.as_bytes()), "{i}");
        }
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(pdf_string("a(b)\\"), "(a\\(b\\)\\\\)");
        assert_eq!(pdf_string("µ☃"), "(\\265?)");
        assert_eq!(utf16_string("A"), "<FEFF0041>");
    }
}
