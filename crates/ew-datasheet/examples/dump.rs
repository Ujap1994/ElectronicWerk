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
            if std::env::var_os("DRAFT").is_some() {
                let draft = ew_datasheet::recognize::recognize_named(&sheet, path);
                println!(
                    "name {:?} manufacturer {:?} description {:?}",
                    draft.name, draft.manufacturer, draft.description
                );
                for p in &draft.packages {
                    let dims: Vec<String> = p
                        .dimensions
                        .iter()
                        .map(|(k, d)| {
                            format!(
                                "{k}={:.2}{}",
                                d.value().unwrap_or(0.0),
                                if d.source.is_some() { "*" } else { "" }
                            )
                        })
                        .collect();
                    println!(
                        "  package {:?} {:?} pins {} ep {} | {}",
                        p.name,
                        p.family,
                        p.pins,
                        p.exposed_pad,
                        dims.join(" ")
                    );
                }
                for pin in &draft.pins {
                    let numbers: Vec<String> = pin
                        .numbers
                        .iter()
                        .map(|(k, v)| format!("{k}:{}", v.join(",")))
                        .collect();
                    println!(
                        "  pin {:12} {:?} [{}] {}",
                        pin.name,
                        pin.kind,
                        numbers.join(" "),
                        pin.function.chars().take(40).collect::<String>()
                    );
                }
                for n in &draft.notes {
                    println!("  note: {n}");
                }
                return;
            }
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
                if std::env::var_os("TABLES").is_some() {
                    for table in ew_datasheet::table::ruled(page) {
                        println!(
                            "--- table {:?} ({} columns)",
                            table.bbox.map(f64::round),
                            table.columns
                        );
                        for row in &table.rows {
                            let cells: Vec<String> = row
                                .cells
                                .iter()
                                .map(|c| {
                                    format!(
                                        "[{}{}] {}",
                                        c.column,
                                        if c.span > 1 {
                                            format!("+{}", c.span - 1)
                                        } else {
                                            String::new()
                                        },
                                        c.text
                                    )
                                })
                                .collect();
                            println!("    {}", cells.join(" | "));
                        }
                    }
                    continue;
                }
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
