//! Gerber X2 files of a board: one file per layer plus a Gerber job file.

pub mod writer;

use ew_model::layer::{CopperLayer, Layer, Side};
use ew_model::pcb::{Pcb, StackupLayer};
use ew_model::units::to_mm;
use serde_json::{Value, json};

use crate::OutputFile;
use crate::art::BoardArt;

pub use writer::{Header, escape, write};

/// Options of the Gerber output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GerberOptions {
    /// The layers to plot; empty plots the fabrication layers
    /// ([`fabrication_layers`]).
    pub layers: Vec<Layer>,
    /// Protel file name extensions (`.gtl`, `.gbl`, …) instead of `.gbr`.
    pub protel_extensions: bool,
    /// Write X2 attributes. Without them the files are plain RS-274X (X1).
    pub attributes: bool,
    /// Also write a Gerber job file (`.gbrjob`).
    pub job_file: bool,
}

impl Default for GerberOptions {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            protel_extensions: true,
            attributes: true,
            job_file: true,
        }
    }
}

/// Where and when the files are made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// Application version.
    pub version: String,
    /// ISO 8601 creation time (empty: none, for reproducible files).
    pub created: String,
    /// Project name.
    pub project: String,
    /// Project GUID.
    pub guid: String,
    /// Project revision.
    pub revision: String,
    /// Start of every file name, for example the board name.
    pub stem: String,
}

/// The layers a board maker needs: copper, solder mask, silkscreen, solder
/// paste and the outline.
#[must_use]
pub fn fabrication_layers(pcb: &Pcb) -> Vec<Layer> {
    let mut layers: Vec<Layer> = pcb.stackup.copper_layers().map(Layer::Copper).collect();
    for side in [Side::Top, Side::Bottom] {
        layers.push(Layer::SolderMask(side));
        layers.push(Layer::Silkscreen(side));
        layers.push(Layer::Paste(side));
    }
    layers.push(Layer::Outline);
    layers
}

/// The `.FileFunction` and `.FilePolarity` of a layer.
#[must_use]
pub fn file_function(layer: Layer, stack: &[CopperLayer]) -> (String, Option<&'static str>) {
    let side = |side: Side| match side {
        Side::Top => "Top",
        Side::Bottom => "Bot",
    };
    match layer {
        Layer::Copper(copper) => {
            let index = stack.iter().position(|own| *own == copper).unwrap_or(0) + 1;
            let position = match copper {
                CopperLayer::Top => "Top",
                CopperLayer::Bottom => "Bot",
                CopperLayer::Inner(_) => "Inr",
            };
            (format!("Copper,L{index},{position}"), Some("Positive"))
        }
        Layer::SolderMask(s) => (format!("Soldermask,{}", side(s)), Some("Negative")),
        Layer::Silkscreen(s) => (format!("Legend,{}", side(s)), Some("Positive")),
        Layer::Paste(s) => (format!("Paste,{}", side(s)), Some("Positive")),
        Layer::Adhesive(s) => (format!("Glue,{}", side(s)), Some("Positive")),
        Layer::Fabrication(s) => (format!("AssemblyDrawing,{}", side(s)), None),
        Layer::Courtyard(_) => ("Other,Courtyard".to_owned(), None),
        Layer::Outline => ("Profile,NP".to_owned(), None),
        Layer::Mechanical(n) => (format!("Other,Mechanical{n}"), None),
        Layer::User(n) => (format!("Other,User{n}"), None),
    }
}

/// The file name part and extension of a layer.
#[must_use]
pub fn layer_file(layer: Layer, protel: bool) -> (String, String) {
    let side = |side: Side| match side {
        Side::Top => "Top",
        Side::Bottom => "Bottom",
    };
    let (name, ext) = match layer {
        Layer::Copper(CopperLayer::Top) => ("Copper_Top".to_owned(), "gtl".to_owned()),
        Layer::Copper(CopperLayer::Bottom) => ("Copper_Bottom".to_owned(), "gbl".to_owned()),
        Layer::Copper(CopperLayer::Inner(n)) => (format!("Copper_In{n}"), format!("g{n}")),
        Layer::SolderMask(s) => (
            format!("Soldermask_{}", side(s)),
            if s == Side::Top { "gts" } else { "gbs" }.to_owned(),
        ),
        Layer::Silkscreen(s) => (
            format!("Silkscreen_{}", side(s)),
            if s == Side::Top { "gto" } else { "gbo" }.to_owned(),
        ),
        Layer::Paste(s) => (
            format!("Paste_{}", side(s)),
            if s == Side::Top { "gtp" } else { "gbp" }.to_owned(),
        ),
        Layer::Adhesive(s) => (
            format!("Adhesive_{}", side(s)),
            if s == Side::Top { "gta" } else { "gba" }.to_owned(),
        ),
        Layer::Fabrication(s) => (format!("Fab_{}", side(s)), "gbr".to_owned()),
        Layer::Courtyard(s) => (format!("Courtyard_{}", side(s)), "gbr".to_owned()),
        Layer::Outline => ("Outline".to_owned(), "gm1".to_owned()),
        Layer::Mechanical(n) => (format!("Mechanical_{n}"), "gbr".to_owned()),
        Layer::User(n) => (format!("User_{n}"), "gbr".to_owned()),
    };
    (name, if protel { ext } else { "gbr".to_owned() })
}

/// Writes the Gerber files of a board.
#[must_use]
pub fn gerbers(art: &BoardArt<'_>, options: &GerberOptions, context: &Context) -> Vec<OutputFile> {
    let pcb = art.pcb();
    let layers = if options.layers.is_empty() {
        fabrication_layers(pcb)
    } else {
        options.layers.clone()
    };
    let mut files = Vec::new();
    let mut listed = Vec::new();
    for layer in layers {
        if let Layer::Copper(copper) = layer
            && !art.stack().contains(&copper)
        {
            continue;
        }
        let (function, polarity) = file_function(layer, art.stack());
        let header = Header {
            version: context.version.clone(),
            created: context.created.clone(),
            project: Some((
                context.project.clone(),
                context.guid.clone(),
                context.revision.clone(),
            )),
            function: function.clone(),
            polarity,
            attributes: options.attributes,
        };
        let content = write(&header, &art.features(layer));
        let (part, ext) = layer_file(layer, options.protel_extensions);
        let name = format!("{}-{part}.{ext}", context.stem);
        listed.push((name.clone(), function, polarity.unwrap_or("Positive")));
        files.push(OutputFile::text(name, content));
    }
    if options.job_file {
        let job = job_file(pcb, context, &listed);
        files.push(OutputFile::text(
            format!("{}-job.gbrjob", context.stem),
            serde_json::to_string_pretty(&job).unwrap_or_default() + "\n",
        ));
    }
    files
}

/// The Gerber job file: board size, layer count, stack-up and the list of
/// files with their functions.
fn job_file(pcb: &Pcb, context: &Context, files: &[(String, String, &str)]) -> Value {
    let size = pcb.outline_bounds().map_or_else(
        || json!({ "X": 0.0, "Y": 0.0 }),
        |r| json!({ "X": to_mm(r.width()), "Y": to_mm(r.height()) }),
    );
    let mut header = json!({
        "GenerationSoftware": {
            "Vendor": "ElectronicWerk",
            "Application": "ElectronicWerk",
            "Version": context.version,
        },
    });
    if !context.created.is_empty() {
        header["CreationDate"] = json!(context.created);
    }
    let mut stack = Vec::new();
    for layer in &pcb.stackup.layers {
        let side = |side: Side| if side == Side::Top { "Top" } else { "Bottom" };
        stack.push(match layer {
            StackupLayer::Silkscreen { side: s, color } => json!({
                "Type": "Legend",
                "Color": if color.is_empty() { "White" } else { color.as_str() },
                "Name": format!("{} Silk Screen", side(*s)),
            }),
            StackupLayer::SolderMask {
                side: s,
                thickness,
                color,
            } => json!({
                "Type": "SolderMask",
                "Color": if color.is_empty() { "Green" } else { color.as_str() },
                "Thickness": to_mm(*thickness),
                "Name": format!("{} Solder Mask", side(*s)),
            }),
            StackupLayer::Copper {
                layer, thickness, ..
            } => json!({
                "Type": "Copper",
                "Thickness": to_mm(*thickness),
                "Name": layer.to_string(),
            }),
            StackupLayer::Dielectric {
                thickness,
                material,
                kind,
                ..
            } => json!({
                "Type": "Dielectric",
                "Thickness": to_mm(*thickness),
                "Material": material,
                "Name": match kind {
                    ew_model::pcb::DielectricKind::Core => "Core",
                    ew_model::pcb::DielectricKind::Prepreg => "Prepreg",
                },
            }),
        });
    }
    let mut specs = json!({
        "ProjectId": {
            "Name": context.project,
            "GUID": context.guid,
            "Revision": context.revision,
        },
        "Size": size,
        "LayerNumber": pcb.stackup.copper_count(),
        "BoardThickness": to_mm(pcb.stackup.thickness()),
    });
    if !pcb.stackup.finish.is_empty() {
        specs["Finish"] = json!(pcb.stackup.finish);
    }
    json!({
        "Header": header,
        "GeneralSpecs": specs,
        "FilesAttributes": files
            .iter()
            .map(|(path, function, polarity)| json!({
                "Path": path,
                "FileFunction": function,
                "FilePolarity": polarity,
            }))
            .collect::<Vec<_>>(),
        "MaterialStackup": stack,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn functions_and_names_of_layers() {
        let stack = [CopperLayer::Top, CopperLayer::Inner(1), CopperLayer::Bottom];
        assert_eq!(
            file_function(Layer::Copper(CopperLayer::Inner(1)), &stack).0,
            "Copper,L2,Inr"
        );
        assert_eq!(
            file_function(Layer::Copper(CopperLayer::Bottom), &stack).0,
            "Copper,L3,Bot"
        );
        assert_eq!(
            file_function(Layer::SolderMask(Side::Bottom), &stack),
            ("Soldermask,Bot".to_owned(), Some("Negative"))
        );
        assert_eq!(
            layer_file(Layer::Copper(CopperLayer::Inner(2)), true),
            ("Copper_In2".to_owned(), "g2".to_owned())
        );
        assert_eq!(layer_file(Layer::Outline, false).1, "gbr");
    }
}
