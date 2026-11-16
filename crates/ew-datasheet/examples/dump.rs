//! Prints the lines of a PDF file, for developing the recognition:
//! `cargo run -p ew-datasheet --example dump -- file.pdf [first-page [last-page]]`.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a developer tool"
)]

use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: dump <file.pdf> [first-page [last-page]]");
        return;
    };
    let first: usize = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1);
    let last: usize = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(first);
    let data = std::fs::read(path).unwrap_or_default();
    let started = Instant::now();
    match ew_datasheet::read(data) {
        Ok(sheet) => {
            println!(
                "{} pages in {:.0} ms, title {:?}, author {:?}, encrypted {}",
                sheet.pages.len(),
                started.elapsed().as_secs_f64() * 1000.0,
                sheet.title,
                sheet.author,
                sheet.encrypted
            );
            for mark in sheet.bookmarks.iter().take(60) {
                println!(
                    "{}- {} (p. {:?})",
                    "  ".repeat(mark.level),
                    mark.title,
                    mark.page
                );
            }
            for page in sheet
                .pages
                .iter()
                .filter(|p| p.number >= first && p.number <= last)
            {
                println!(
                    "=== page {} ({} x {}, {} rules{})",
                    page.number,
                    page.width,
                    page.height,
                    page.rules.len(),
                    if page.scanned { ", scanned" } else { "" }
                );
                for line in &page.lines {
                    let cells: Vec<String> = line
                        .words
                        .iter()
                        .map(|w| format!("{}@{:.0}", w.text, w.bbox[0]))
                        .collect();
                    println!(
                        "{:6.1} {:4.1} | {}",
                        line.bbox[1],
                        line.size,
                        cells.join(" ")
                    );
                }
            }
        }
        Err(e) => eprintln!("error: {e}"),
    }
}
