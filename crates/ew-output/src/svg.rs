//! Writes a [`Page`] as SVG (one file per page). Sizes are in millimetres
//! so the drawing prints at its real scale.

use std::fmt::Write as _;

use crate::bom::escape_xml;
use crate::canvas::{Page, Seg};

fn mm(nm: f64) -> String {
    let text = format!("{:.4}", nm / 1e6);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" || text.is_empty() {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// Writes one page as an SVG document.
#[must_use]
pub fn write(page: &Page, title: &str) -> String {
    let (w, h) = (mm(page.width), mm(page.height));
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\n\
         <title>{}</title>\n",
        escape_xml(title)
    );
    let _ = writeln!(
        out,
        "<rect width=\"{w}\" height=\"{h}\" fill=\"{}\"/>",
        page.background
            .map_or_else(|| "#ffffff".to_owned(), super::canvas::Color::css)
    );
    // Model y points up, SVG y down: flip the page once.
    let _ = writeln!(
        out,
        "<g transform=\"matrix(1 0 0 -1 0 {h})\" stroke-linecap=\"round\" stroke-linejoin=\"round\" fill-rule=\"evenodd\">"
    );
    for item in &page.items {
        let mut d = String::new();
        for seg in &item.path.0 {
            match seg {
                Seg::Move(p) => {
                    let _ = write!(d, "M{} {}", mm(p.x), mm(p.y));
                }
                Seg::Line(p) => {
                    let _ = write!(d, "L{} {}", mm(p.x), mm(p.y));
                }
                Seg::Cubic(a, b, c) => {
                    let _ = write!(
                        d,
                        "C{} {} {} {} {} {}",
                        mm(a.x),
                        mm(a.y),
                        mm(b.x),
                        mm(b.y),
                        mm(c.x),
                        mm(c.y)
                    );
                }
                Seg::Close => d.push('Z'),
            }
        }
        let fill = item
            .paint
            .fill
            .map_or_else(|| "none".to_owned(), super::canvas::Color::css);
        let stroke = item.paint.stroke.map_or_else(String::new, |(c, width)| {
            format!(" stroke=\"{}\" stroke-width=\"{}\"", c.css(), mm(width))
        });
        let _ = writeln!(out, "<path d=\"{d}\" fill=\"{fill}\"{stroke}/>");
    }
    out.push_str("</g>\n</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{Color, Paint, Path};
    use ew_pcb::geom::V;

    #[test]
    fn writes_paths_in_millimetres() {
        let mut page = Page::new("p", 100e6, 50e6);
        page.add(
            Path::polyline(&[V::new(0.0, 0.0), V::new(1e6, 2.5e6)], false),
            Paint::stroke(Color::BLACK, 1e5),
        );
        let svg = write(&page, "A & B");
        assert!(svg.contains("width=\"100mm\" height=\"50mm\" viewBox=\"0 0 100 50\""));
        assert!(svg.contains("<title>A &amp; B</title>"));
        assert!(
            svg.contains("d=\"M0 0L1 2.5\" fill=\"none\" stroke=\"#000000\" stroke-width=\"0.1\"")
        );
        assert!(svg.contains("matrix(1 0 0 -1 0 50)"));
    }
}
