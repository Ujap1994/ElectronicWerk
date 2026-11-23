//! The project documentation: one self-contained HTML file with the
//! project data, the schematic sheets and board views (as SVG images), the
//! layer stack-up, the bill of materials, the holes and the design
//! statistics. Open it in a browser, print it or save it as PDF.

use std::fmt::Write as _;

use ew_model::layer::{CopperLayer, Layer, Side};
use ew_model::pcb::{Pcb, StackupLayer};
use ew_model::project::Project;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::art::{ArtOptions, BoardArt};
use crate::bom::{self, BomOptions, Cell, escape_xml};
use crate::print::{self, AssemblyPrint, BoardPrint, ColorMode, Paper, SchematicPrint};

/// What the documentation contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::struct_excessive_bools, reason = "one switch per section")]
pub struct Sections {
    /// The schematic sheets.
    pub schematic: bool,
    /// Board views: top and bottom, copper and silkscreen.
    pub board: bool,
    /// Assembly drawings of both sides.
    pub assembly: bool,
    /// The layer stack-up.
    pub stackup: bool,
    /// The bill of materials.
    pub bom: bool,
    /// The holes by diameter.
    pub drills: bool,
    /// The design statistics.
    pub statistics: bool,
}

impl Default for Sections {
    fn default() -> Self {
        Self {
            schematic: true,
            board: true,
            assembly: true,
            stackup: true,
            bom: true,
            drills: true,
            statistics: true,
        }
    }
}

/// Everything the documentation needs besides the project.
pub struct Inputs<'a> {
    /// The board (`None`: a project without board).
    pub board: Option<&'a Pcb>,
    /// Expands text variables (with the document id).
    pub resolve: &'a dyn Fn(&str, Option<ew_model::id::Id>) -> String,
    /// Title block labels of the sheets.
    pub labels: ew_scene::TitleLabels,
    /// Application version.
    pub version: &'a str,
    /// ISO 8601 time (empty for reproducible files).
    pub created: &'a str,
}

/// An SVG page as an image.
fn image(svg: &str, alt: &str) -> String {
    format!(
        "<img class=\"page\" alt=\"{}\" src=\"data:image/svg+xml;base64,{}\">",
        escape_xml(alt),
        ew_scene::base64(svg.as_bytes())
    )
}

fn mm(nm: i64) -> String {
    #[allow(
        clippy::cast_precision_loss,
        reason = "lengths are shown with three decimals"
    )]
    let value = nm as f64 / 1e6;
    let text = format!("{value:.3}");
    format!("{} mm", text.trim_end_matches('0').trim_end_matches('.'))
}

fn number(value: &Value) -> String {
    match value {
        Value::Null => "–".to_owned(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => escape_xml(s),
        other => escape_xml(&other.to_string()),
    }
}

const STYLE: &str = "\
:root { color-scheme: light; --fg: #1d232b; --muted: #5d6875; --line: #d9dee5; --head: #eef2f6; --accent: #13795b; }
body { font: 14px/1.5 system-ui, -apple-system, 'Segoe UI', sans-serif; color: var(--fg); margin: 0 auto; max-width: 1100px; padding: 32px 24px; }
h1 { font-size: 28px; margin: 0; }
h2 { font-size: 20px; margin: 40px 0 12px; padding-bottom: 6px; border-bottom: 2px solid var(--accent); }
h3 { font-size: 16px; margin: 24px 0 8px; }
.meta { color: var(--muted); margin: 4px 0 24px; }
.meta span + span::before { content: ' · '; }
.description { white-space: pre-wrap; }
nav ol { columns: 2; padding-left: 20px; }
table { border-collapse: collapse; width: 100%; margin: 8px 0 16px; }
th, td { border: 1px solid var(--line); padding: 4px 8px; text-align: left; vertical-align: top; }
th { background: var(--head); }
td.n, th.n { text-align: right; font-variant-numeric: tabular-nums; }
figure { margin: 12px 0 24px; }
figcaption { color: var(--muted); font-size: 13px; margin-top: 4px; }
img.page { width: 100%; height: auto; border: 1px solid var(--line); background: #fff; }
.figures { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 8px; margin: 8px 0 16px; }
.figure { background: var(--head); border-radius: 6px; padding: 8px 12px; }
.figure b { display: block; font-size: 18px; }
.figure span { color: var(--muted); font-size: 12px; }
footer { color: var(--muted); font-size: 12px; margin-top: 48px; }
@media print { body { padding: 0; max-width: none; } h2 { break-before: page; } figure { break-inside: avoid; } }
";

fn figures(out: &mut String, items: &[(&str, String)]) {
    out.push_str("<div class=\"figures\">");
    for (label, value) in items {
        let _ = write!(
            out,
            "<div class=\"figure\"><b>{value}</b><span>{}</span></div>",
            escape_xml(label)
        );
    }
    out.push_str("</div>\n");
}

fn stackup_table(out: &mut String, pcb: &Pcb) {
    out.push_str(
        "<table><thead><tr><th>Layer</th><th>Material</th><th class=\"n\">Thickness</th></tr></thead><tbody>\n",
    );
    for layer in &pcb.stackup.layers {
        let (name, material) = match layer {
            StackupLayer::Silkscreen { side, color } => {
                (format!("Silkscreen {}", side_name(*side)), color.clone())
            }
            StackupLayer::SolderMask { side, color, .. } => {
                (format!("Solder mask {}", side_name(*side)), color.clone())
            }
            StackupLayer::Copper { layer, name, .. } => (
                if name.is_empty() {
                    copper_name(*layer)
                } else {
                    name.clone()
                },
                "Copper".to_owned(),
            ),
            StackupLayer::Dielectric { kind, material, .. } => {
                (format!("{kind:?}"), material.clone())
            }
        };
        let thickness = layer.thickness();
        let _ = writeln!(
            out,
            "<tr><td>{}</td><td>{}</td><td class=\"n\">{}</td></tr>",
            escape_xml(&name),
            escape_xml(&material),
            if thickness > 0 {
                mm(thickness)
            } else {
                String::new()
            }
        );
    }
    let _ = writeln!(
        out,
        "</tbody></table><p>Total thickness {}{}.</p>",
        mm(pcb.stackup.thickness()),
        if pcb.stackup.finish.is_empty() {
            String::new()
        } else {
            format!(", surface finish {}", escape_xml(&pcb.stackup.finish))
        }
    );
}

const fn side_name(side: Side) -> &'static str {
    match side {
        Side::Top => "top",
        Side::Bottom => "bottom",
    }
}

fn copper_name(layer: CopperLayer) -> String {
    match layer {
        CopperLayer::Top => "Top copper".to_owned(),
        CopperLayer::Bottom => "Bottom copper".to_owned(),
        CopperLayer::Inner(n) => format!("Inner copper {n}"),
    }
}

fn bom_table(out: &mut String, project: &Project, pcb: Option<&Pcb>) {
    let parts = bom::parts(project, pcb);
    let table = bom::build(&parts, &BomOptions::default());
    let _ = writeln!(
        out,
        "<p>{} lines, {} parts.</p><table><thead><tr>",
        table.rows.len(),
        table.parts
    );
    for header in &table.headers {
        let _ = write!(out, "<th>{}</th>", escape_xml(header));
    }
    out.push_str("</tr></thead><tbody>\n");
    for row in &table.rows {
        out.push_str("<tr>");
        for cell in row {
            match cell {
                Cell::Number(_) => {
                    let _ = write!(out, "<td class=\"n\">{}</td>", cell.text());
                }
                Cell::Text(text) => {
                    let _ = write!(out, "<td>{}</td>", escape_xml(text));
                }
            }
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody></table>\n");
}

/// Writes the documentation of a project.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one section after the other")]
pub fn html(project: &Project, inputs: &Inputs<'_>, sections: Sections) -> String {
    let name = escape_xml(&project.manifest.name);
    let stats = crate::statistics::statistics(project);
    let pcb = inputs.board;
    let mut toc: Vec<(&str, &str)> = vec![("overview", "Overview")];
    let has_schematic = project
        .documents
        .values()
        .any(|d| matches!(d, ew_model::document::Document::Schematic(_)));
    if sections.schematic && has_schematic {
        toc.push(("schematic", "Schematic"));
    }
    if pcb.is_some() {
        if sections.board {
            toc.push(("board", "Board"));
        }
        if sections.assembly {
            toc.push(("assembly", "Assembly drawings"));
        }
        if sections.stackup {
            toc.push(("stackup", "Layer stack-up"));
        }
    }
    if sections.bom {
        toc.push(("bom", "Bill of materials"));
    }
    if sections.drills && pcb.is_some() {
        toc.push(("drills", "Holes"));
    }
    if sections.statistics {
        toc.push(("statistics", "Design statistics"));
    }

    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{name} – Documentation</title>\n<style>\n{STYLE}</style>\n</head>\n<body>\n\
         <header>\n<h1>{name}</h1>\n<p class=\"meta\">"
    );
    if !project.manifest.revision.is_empty() {
        let _ = write!(
            out,
            "<span>Revision {}</span>",
            escape_xml(&project.manifest.revision)
        );
    }
    if !inputs.created.is_empty() {
        let _ = write!(out, "<span>{}</span>", escape_xml(inputs.created));
    }
    out.push_str("</p>\n");
    if !project.manifest.description.trim().is_empty() {
        let _ = writeln!(
            out,
            "<p class=\"description\">{}</p>",
            escape_xml(project.manifest.description.trim())
        );
    }
    out.push_str("</header>\n<nav><h2 id=\"contents\">Contents</h2><ol>\n");
    for (id, title) in &toc {
        let _ = writeln!(out, "<li><a href=\"#{id}\">{title}</a></li>");
    }
    out.push_str("</ol></nav>\n");

    // Overview.
    out.push_str("<h2 id=\"overview\">Overview</h2>\n");
    let schematic = &stats["schematic"];
    let bom_stats = &stats["bom"];
    let mut overview = vec![
        ("Schematic sheets", number(&schematic["sheets"])),
        ("Parts", number(&schematic["parts"])),
        ("Nets", number(&schematic["nets"])),
        ("BOM lines", number(&bom_stats["lines"])),
    ];
    if let Some(board) = stats["boards"].as_array().and_then(|b| b.first()) {
        if let Some(size) = board["size"].as_array() {
            overview.push((
                "Board size",
                format!("{} × {} mm", number(&size[0]), number(&size[1])),
            ));
        }
        overview.push(("Copper layers", number(&board["copperLayers"])));
        overview.push(("Footprints", number(&board["footprints"]["total"])));
        let completion = board["routing"]["completion"].as_f64().unwrap_or(0.0);
        overview.push(("Routed", format!("{:.1} %", completion * 100.0)));
    }
    figures(&mut out, &overview);

    // Schematic.
    if sections.schematic && has_schematic {
        out.push_str("<h2 id=\"schematic\">Schematic</h2>\n");
        let pages = print::schematic_pages(
            project,
            inputs.resolve,
            &inputs.labels,
            SchematicPrint {
                color: ColorMode::Color,
                paper: Paper::Fit,
            },
        );
        for (index, page) in pages.iter().enumerate() {
            let caption = format!("Sheet {}: {}", index + 1, page.name);
            let svg = crate::svg::write(page, &caption);
            let _ = writeln!(
                out,
                "<figure>{}<figcaption>{}</figcaption></figure>",
                image(&svg, &caption),
                escape_xml(&caption)
            );
        }
    }

    if let Some(pcb) = pcb {
        let resolve = |t: &str| (inputs.resolve)(t, Some(pcb.id));
        let art = BoardArt::new(
            pcb,
            ArtOptions {
                units: project.manifest.settings.units,
                ..ArtOptions::default()
            },
            &resolve,
        );
        let title = format!("{} – {}", project.manifest.name, pcb.name);
        if sections.board {
            out.push_str("<h2 id=\"board\">Board</h2>\n");
            for (side, mirror, layers) in [
                (
                    "Top side",
                    false,
                    vec![
                        Layer::Copper(CopperLayer::Top),
                        Layer::Silkscreen(Side::Top),
                    ],
                ),
                (
                    "Bottom side (seen from the bottom)",
                    true,
                    vec![
                        Layer::Copper(CopperLayer::Bottom),
                        Layer::Silkscreen(Side::Bottom),
                    ],
                ),
            ] {
                let pages = print::board_pages(
                    &art,
                    &BoardPrint {
                        layers,
                        page_per_layer: false,
                        mirror,
                        color: ColorMode::Color,
                        paper: Paper::Fit,
                        outline: true,
                    },
                    &title,
                );
                for page in &pages {
                    let svg = crate::svg::write(page, side);
                    let _ = writeln!(
                        out,
                        "<figure>{}<figcaption>{side}</figcaption></figure>",
                        image(&svg, side)
                    );
                }
            }
        }
        if sections.assembly {
            out.push_str("<h2 id=\"assembly\">Assembly drawings</h2>\n");
            let pages = print::assembly_pages(
                &art,
                &AssemblyPrint {
                    values: true,
                    ..AssemblyPrint::default()
                },
                &title,
            );
            for page in &pages {
                let svg = crate::svg::write(page, &page.name);
                let _ = writeln!(
                    out,
                    "<figure>{}<figcaption>{}</figcaption></figure>",
                    image(&svg, &page.name),
                    escape_xml(&page.name)
                );
            }
        }
        if sections.stackup {
            out.push_str("<h2 id=\"stackup\">Layer stack-up</h2>\n");
            stackup_table(&mut out, pcb);
        }
    }

    if sections.bom {
        out.push_str("<h2 id=\"bom\">Bill of materials</h2>\n");
        bom_table(&mut out, project, pcb);
    }

    if sections.drills && pcb.is_some() {
        out.push_str("<h2 id=\"drills\">Holes</h2>\n");
        for board in stats["boards"].as_array().into_iter().flatten() {
            out.push_str(
                "<table><thead><tr><th class=\"n\">Diameter</th><th class=\"n\">Plated</th><th class=\"n\">Non-plated</th></tr></thead><tbody>\n",
            );
            for drill in board["holes"]["drills"].as_array().into_iter().flatten() {
                let _ = writeln!(
                    out,
                    "<tr><td class=\"n\">{} mm</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
                    number(&drill["diameter"]),
                    number(&drill["plated"]),
                    number(&drill["nonPlated"])
                );
            }
            out.push_str("</tbody></table>\n");
        }
    }

    if sections.statistics {
        out.push_str("<h2 id=\"statistics\">Design statistics</h2>\n<h3>Schematic</h3>\n");
        figures(
            &mut out,
            &[
                ("Sheets", number(&schematic["sheets"])),
                ("Parts", number(&schematic["parts"])),
                ("Pins", number(&schematic["pins"])),
                ("Nets", number(&schematic["nets"])),
                ("Named nets", number(&schematic["namedNets"])),
                ("Nets with one pin", number(&schematic["singlePinNets"])),
            ],
        );
        out.push_str("<h3>Bill of materials</h3>\n");
        figures(
            &mut out,
            &[
                ("Lines", number(&bom_stats["lines"])),
                ("Parts to place", number(&bom_stats["parts"])),
                ("Not populated", number(&bom_stats["dnp"])),
                ("Without footprint", number(&bom_stats["withoutFootprint"])),
                (
                    "Without part number",
                    number(&bom_stats["withoutPartNumber"]),
                ),
            ],
        );
        for board in stats["boards"].as_array().into_iter().flatten() {
            let _ = writeln!(out, "<h3>Board {}</h3>", number(&board["board"]));
            let footprints = &board["footprints"];
            figures(
                &mut out,
                &[
                    ("Area (cm²)", number(&board["area"])),
                    ("Footprints", number(&footprints["total"])),
                    (
                        "Top / bottom",
                        format!(
                            "{} / {}",
                            number(&footprints["top"]),
                            number(&footprints["bottom"])
                        ),
                    ),
                    (
                        "SMD / through-hole",
                        format!(
                            "{} / {}",
                            number(&footprints["smd"]),
                            number(&footprints["throughHole"])
                        ),
                    ),
                    ("Pads", number(&board["pads"]["total"])),
                    ("Vias", number(&board["vias"])),
                    ("Track length (mm)", number(&board["tracks"]["length"])),
                    ("Thinnest track (mm)", number(&board["smallest"]["track"])),
                    ("Smallest hole (mm)", number(&board["smallest"]["hole"])),
                    (
                        "Unrouted connections",
                        number(&board["routing"]["unrouted"]),
                    ),
                ],
            );
        }
    }

    let _ = write!(
        out,
        "<footer>Generated by ElectronicWerk {}.</footer>\n</body>\n</html>\n",
        escape_xml(inputs.version)
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_the_demo_design() {
        let project = ew_synth::generate(ew_synth::DesignSize::S, ew_synth::DEFAULT_SEED);
        let pcb = project.documents.values().find_map(|d| match d {
            ew_model::document::Document::Pcb(pcb) => Some(pcb),
            _ => None,
        });
        let resolve = |text: &str, _: Option<ew_model::id::Id>| text.to_owned();
        let inputs = Inputs {
            board: pcb,
            resolve: &resolve,
            labels: ew_scene::TitleLabels::default(),
            version: "0.0.0-test",
            created: "",
        };
        // Every sheet resolves its own variables (`${SHEET_NAME}`).
        let asked = std::cell::RefCell::new(Vec::new());
        let recording = |text: &str, doc: Option<ew_model::id::Id>| {
            asked.borrow_mut().push((text.to_owned(), doc));
            text.to_owned()
        };
        let _ = print::schematic_pages(
            &project,
            &recording,
            &ew_scene::TitleLabels::default(),
            SchematicPrint::default(),
        );
        let asked = asked.into_inner();
        assert!(asked.iter().any(|(text, _)| text.contains("${SHEET_NAME}")));
        assert!(asked.iter().all(|(_, doc)| doc.is_some()));

        let all = html(&project, &inputs, Sections::default());
        for section in [
            "id=\"overview\"",
            "id=\"schematic\"",
            "id=\"board\"",
            "id=\"assembly\"",
            "id=\"stackup\"",
            "id=\"bom\"",
            "id=\"drills\"",
            "id=\"statistics\"",
        ] {
            assert!(all.contains(section), "{section}");
        }
        // Sheets, two board sides and two assembly sides as SVG images.
        assert!(all.matches("data:image/svg+xml;base64,").count() >= 5);
        assert!(all.contains("Generated by ElectronicWerk 0.0.0-test."));
        assert!(all.contains("<td class=\"n\">0.3 mm</td>"));
        // Reproducible: the same input gives the same file.
        assert_eq!(all, html(&project, &inputs, Sections::default()));

        let only_bom = html(
            &project,
            &inputs,
            Sections {
                schematic: false,
                board: false,
                assembly: false,
                stackup: false,
                drills: false,
                statistics: false,
                bom: true,
            },
        );
        assert!(!only_bom.contains("data:image/svg+xml"));
        assert!(only_bom.contains("id=\"bom\""));
        assert!(!only_bom.contains("id=\"statistics\""));
    }
}
