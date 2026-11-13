//! The Gerber viewer: generated (or any other) Gerber and drill files as a
//! scene for the 2D viewer, one layer per file, coloured by what the file
//! is for.

use std::collections::BTreeSet;

use ew_model::id::Id;
use ew_pcb::geom::{Core, Shape, V};
use ew_scene::{Header, LayerDef, ObjType, P, Scene, SceneBuilder};

use crate::read::Image;

/// Chord tolerance of curves turned into polygons (nm).
const TOLERANCE: f64 = 1_000.0;

/// One file to show.
#[derive(Debug, Clone)]
pub struct ViewerFile {
    /// File name.
    pub name: String,
    /// The parsed file.
    pub image: Image,
}

/// The X2 file function of a file, or one guessed from its name (Protel
/// extensions and KiCad-style names).
#[must_use]
pub fn function_of(file: &ViewerFile) -> Option<String> {
    if let Some(function) = &file.image.function {
        return Some(function.clone());
    }
    let lower = file.name.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    let guess = match ext {
        "gtl" => "Copper,L1,Top",
        "gbl" => "Copper,L99,Bot",
        "gts" => "Soldermask,Top",
        "gbs" => "Soldermask,Bot",
        "gto" => "Legend,Top",
        "gbo" => "Legend,Bot",
        "gtp" => "Paste,Top",
        "gbp" => "Paste,Bot",
        "gko" | "gm1" | "gml" => "Profile,NP",
        "drl" | "xln" | "exc" | "txt" if lower.contains("npth") => "NonPlated",
        "drl" | "xln" | "exc" => "Plated",
        e if e.starts_with('g') && e[1..].parse::<u32>().is_ok() => {
            return Some(format!(
                "Copper,L{},Inr",
                e[1..].parse::<u32>().unwrap_or(0) + 1
            ));
        }
        _ => return None,
    };
    Some(guess.to_owned())
}

/// The scene layer key, group, side and draw order of a file function.
fn layer_style(
    function: Option<&str>,
    inner: &mut u32,
) -> (String, &'static str, Option<&'static str>, i32) {
    let Some(function) = function else {
        return ("user.1".to_owned(), "user", None, 700);
    };
    let parts: Vec<&str> = function.split(',').collect();
    let side = |s: Option<&&str>| match s.copied() {
        Some("Top") => Some("top"),
        Some("Bot") => Some("bottom"),
        _ => None,
    };
    match parts[0] {
        "Copper" => match parts.get(2).copied() {
            Some("Top") => ("cu.top".to_owned(), "copper", Some("top"), 300),
            Some("Bot") => ("cu.bottom".to_owned(), "copper", Some("bottom"), 100),
            _ => {
                *inner += 1;
                (
                    format!("cu.in{inner}"),
                    "copper",
                    None,
                    200 - i32::try_from(*inner).unwrap_or(99),
                )
            }
        },
        "Soldermask" => {
            let s = side(parts.get(1));
            (
                format!("mask.{}", s.unwrap_or("top")),
                "solderMask",
                s,
                if s == Some("bottom") { 3 } else { 401 },
            )
        }
        "Legend" => {
            let s = side(parts.get(1));
            (
                format!("silk.{}", s.unwrap_or("top")),
                "silkscreen",
                s,
                if s == Some("bottom") { 6 } else { 404 },
            )
        }
        "Paste" => {
            let s = side(parts.get(1));
            (
                format!("paste.{}", s.unwrap_or("top")),
                "paste",
                s,
                if s == Some("bottom") { 2 } else { 402 },
            )
        }
        "Glue" => {
            let s = side(parts.get(1));
            (
                format!("adhesive.{}", s.unwrap_or("top")),
                "adhesive",
                s,
                if s == Some("bottom") { 1 } else { 403 },
            )
        }
        "AssemblyDrawing" => {
            let s = side(parts.get(1));
            (
                format!("fab.{}", s.unwrap_or("top")),
                "fabrication",
                s,
                if s == Some("bottom") { 5 } else { 405 },
            )
        }
        "Profile" => ("outline".to_owned(), "outline", None, 500),
        "Plated" | "NonPlated" | "MixedPlating" => ("holes".to_owned(), "holes", None, 1000),
        _ => ("user.1".to_owned(), "user", None, 700),
    }
}

/// Polygons of the dark shapes minus the clear ones, in drawing order.
fn resolve_polarity(image: &Image) -> Vec<Vec<Vec<V>>> {
    let mut result: Vec<Vec<Vec<V>>> = Vec::new();
    let mut i = 0;
    let objects = &image.objects;
    while i < objects.len() {
        let dark = objects[i].dark;
        let mut group: Vec<Vec<Vec<V>>> = Vec::new();
        while i < objects.len() && objects[i].dark == dark {
            group.extend(objects[i].shape.outlines(0.0, TOLERANCE));
            i += 1;
        }
        if dark {
            group.extend(result);
            result = ew_pcb::poly::union(&group);
        } else {
            result = ew_pcb::poly::difference(&result, &group);
        }
    }
    result
}

/// Builds the scene of a set of files.
#[must_use]
pub fn scene(files: &[ViewerFile], doc: Id, name: &str) -> Scene {
    let mut bounds = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for file in files {
        if let Some(b) = file.image.bounds() {
            bounds = [
                bounds[0].min(b[0]),
                bounds[1].min(b[1]),
                bounds[2].max(b[2]),
                bounds[3].max(b[3]),
            ];
        }
    }
    if bounds[0] > bounds[2] {
        bounds = [0.0, 0.0, 1e6, 1e6];
    }
    let origin = P::new(
        f64::midpoint(bounds[0], bounds[2]).round(),
        f64::midpoint(bounds[1], bounds[3]).round(),
    );
    let local = |v: V| P::new(v.x - origin.x, v.y - origin.y);
    let mut b = SceneBuilder::new(origin);
    let mut used = BTreeSet::new();
    let mut inner = 0;
    for (index, file) in files.iter().enumerate() {
        let function = function_of(file);
        let (mut key, group, side, z) = layer_style(function.as_deref(), &mut inner);
        if !used.insert(key.clone()) {
            key = format!("{key}#{index}");
            used.insert(key.clone());
        }
        let layer = b.layer(LayerDef {
            key,
            group,
            side,
            name: Some(file.name.clone()),
            z,
            visible: true,
        });
        let obj = b.object(
            ObjType::BoardShape,
            Id::derived(&[doc.to_string().as_str(), file.name.as_str()]),
            None,
            None,
        );
        if file.image.has_clear() {
            for polygon in resolve_polarity(&file.image) {
                let Some((outline, holes)) = polygon.split_first() else {
                    continue;
                };
                let outline: Vec<P> = outline.iter().map(|p| local(*p)).collect();
                let holes: Vec<Vec<P>> = holes
                    .iter()
                    .map(|h| h.iter().map(|p| local(*p)).collect())
                    .collect();
                b.polygon(layer, obj, &outline, &holes, 0);
            }
            continue;
        }
        for object in &file.image.objects {
            draw(&mut b, layer, obj, &object.shape, &local);
        }
    }
    b.finish(Header {
        doc,
        kind: "pcb",
        name: name.to_owned(),
        page: None,
        home: Some([
            bounds[0].floor() as i64,
            bounds[1].floor() as i64,
            bounds[2].ceil() as i64,
            bounds[3].ceil() as i64,
        ]),
        grid: 100_000,
    })
}

fn draw(b: &mut SceneBuilder, layer: usize, obj: u32, shape: &Shape, local: &dyn Fn(V) -> P) {
    match &shape.core {
        Core::Point(c) => b.circle(layer, obj, local(*c), shape.radius, 0.0),
        Core::Segment(a, c) => b.segment(layer, obj, local(*a), local(*c), shape.radius * 2.0),
        Core::Poly(poly) => {
            if shape.radius < 0.5 {
                let outline: Vec<P> = poly.outline().iter().map(|p| local(*p)).collect();
                let holes: Vec<Vec<P>> = poly
                    .holes()
                    .iter()
                    .map(|h| h.iter().map(|p| local(*p)).collect())
                    .collect();
                b.polygon(layer, obj, &outline, &holes, 0);
            } else {
                for polygon in shape.outlines(0.0, TOLERANCE) {
                    let Some((outline, holes)) = polygon.split_first() else {
                        continue;
                    };
                    let outline: Vec<P> = outline.iter().map(|p| local(*p)).collect();
                    let holes: Vec<Vec<P>> = holes
                        .iter()
                        .map(|h| h.iter().map(|p| local(*p)).collect())
                        .collect();
                    b.polygon(layer, obj, &outline, &holes, 0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::{Object, gerber};

    #[test]
    fn functions_from_names() {
        let file = |name: &str| ViewerFile {
            name: name.into(),
            image: Image::default(),
        };
        assert_eq!(
            function_of(&file("a.GTL")).as_deref(),
            Some("Copper,L1,Top")
        );
        assert_eq!(function_of(&file("a.g2")).as_deref(), Some("Copper,L3,Inr"));
        assert_eq!(
            function_of(&file("a-NPTH.drl")).as_deref(),
            Some("NonPlated")
        );
        assert_eq!(function_of(&file("readme.txt")), None);
    }

    #[test]
    fn clear_polarity_cuts_holes() {
        let image = Image {
            function: Some("Copper,L1,Top".into()),
            objects: vec![
                Object {
                    dark: true,
                    shape: Shape::disc(V::new(0.0, 0.0), 1e6),
                },
                Object {
                    dark: false,
                    shape: Shape::disc(V::new(0.0, 0.0), 5e5),
                },
            ],
            warnings: Vec::new(),
        };
        let polygons = resolve_polarity(&image);
        assert_eq!(polygons.len(), 1);
        assert_eq!(polygons[0].len(), 2, "outline and hole");
        let text = "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1*%\nD10*\nX0Y0D03*\nM02*\n";
        let scene = scene(
            &[
                ViewerFile {
                    name: "top.gtl".into(),
                    image: gerber(text),
                },
                ViewerFile {
                    name: "x.gbr".into(),
                    image,
                },
            ],
            Id::random(),
            "gerbers",
        );
        assert_eq!(scene.meta.layers.len(), 2);
        assert_eq!(scene.meta.layers[0].key, "cu.top");
        assert_eq!(scene.meta.layers[1].key, "cu.top#1");
        assert_eq!(scene.meta.layers[0].circles.count, 1);
        assert!(scene.meta.layers[1].mesh.index_count > 0);
    }
}
