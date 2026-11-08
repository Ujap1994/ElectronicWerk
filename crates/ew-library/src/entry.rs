//! Library entries: what the index stores about a symbol, a footprint or a
//! 3D model, independent of the source it came from.

use ew_model::layer::Layer;
use ew_model::library::{FootprintDef, SymbolDef};
use ew_model::pcb::Mounting;
use ew_model::shape::Geometry;
use serde::{Deserialize, Serialize};

use crate::fingerprint::{self, FootprintPrint, SymbolPrint};
use crate::mesh::MeshPrint;

/// The kind of a library entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// A schematic symbol.
    Symbol,
    /// A footprint.
    Footprint,
    /// A 3D model.
    Model,
}

impl Kind {
    /// All kinds, in index order.
    pub const ALL: [Self; 3] = [Self::Symbol, Self::Footprint, Self::Model];

    /// The number stored in the index.
    #[must_use]
    pub const fn code(self) -> i64 {
        match self {
            Self::Symbol => 0,
            Self::Footprint => 1,
            Self::Model => 2,
        }
    }

    /// The kind with the given index number.
    #[must_use]
    pub const fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(Self::Symbol),
            1 => Some(Self::Footprint),
            2 => Some(Self::Model),
            _ => None,
        }
    }

    /// `symbol`, `footprint` or `model`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Symbol => "symbol",
            Self::Footprint => "footprint",
            Self::Model => "model",
        }
    }
}

/// The fingerprint of an entry.
#[derive(Debug, Clone)]
pub enum Print {
    /// A symbol.
    Symbol(SymbolPrint),
    /// A footprint.
    Footprint(FootprintPrint),
    /// A 3D model with its mesh.
    Model(MeshPrint),
    /// A 3D model without a mesh (not tessellated yet).
    None,
}

/// One symbol, footprint or 3D model.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Kind.
    pub kind: Kind,
    /// Position of the source in the build (0 = highest priority).
    pub source: usize,
    /// Library name (`Device`, `Resistor_SMD`).
    pub library: String,
    /// Name inside the library (`R`, `R_0805_2012Metric`).
    pub name: String,
    /// Description.
    pub description: String,
    /// Search keywords.
    pub keywords: String,
    /// Default value (symbols).
    pub value: String,
    /// Package: the default footprint of a symbol, the name of a footprint
    /// or model.
    pub package: String,
    /// Manufacturer part number.
    pub mpn: String,
    /// Manufacturer.
    pub manufacturer: String,
    /// Datasheet link.
    pub datasheet: String,
    /// Other parameters as `key: value` lines.
    pub params: String,
    /// Pins (symbols) or pads (footprints).
    pub pins: u32,
    /// Units (symbols).
    pub units: u32,
    /// `smd`, `tht` or `other` (footprints).
    pub mounting: Option<String>,
    /// Size in mm: width, depth, height (footprints: courtyard; models:
    /// bounding box).
    pub size: Option<[f64; 3]>,
    /// The 3D model of a footprint, as a model key (`Resistor_SMD/R_0805`).
    pub model: Option<String>,
    /// The source file of a model, relative to the model folder.
    pub file: Option<String>,
    /// The mesh of a model (content hash of its GLB file).
    pub mesh: Option<String>,
    /// The full definition as JSON (symbols and footprints).
    pub data: Option<String>,
    /// Fingerprint.
    pub print: Print,
}

impl Entry {
    /// `library:name`.
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}:{}", self.library, self.name)
    }
}

/// A definition as JSON without its ids (they are random-looking and make
/// up a fifth of the compressed size); [`restore_ids`] derives them again.
fn compact<T: Serialize>(def: &T) -> Option<String> {
    let mut value = serde_json::to_value(def).ok()?;
    if let Some(object) = value.as_object_mut() {
        object.remove("id");
        for pad in object
            .get_mut("pads")
            .and_then(serde_json::Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if let Some(pad) = pad.as_object_mut() {
                pad.remove("id");
            }
        }
    }
    serde_json::to_string(&value).ok()
}

/// Puts stable ids into a definition stored by [`compact`]: derived from the
/// kind and the full name, so they are the same every time.
pub fn restore_ids(kind: Kind, full_name: &str, value: &mut serde_json::Value) {
    use ew_kicad::common::derived_id;
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let id = derived_id(&["library", kind.as_str(), full_name]);
    object.insert("id".to_owned(), serde_json::json!(id));
    if let Some(pads) = object
        .get_mut("pads")
        .and_then(serde_json::Value::as_array_mut)
    {
        for (index, pad) in pads.iter_mut().enumerate() {
            if let Some(pad) = pad.as_object_mut() {
                let id = derived_id(&[
                    "library",
                    kind.as_str(),
                    full_name,
                    "pad",
                    &index.to_string(),
                ]);
                pad.insert("id".to_owned(), serde_json::json!(id));
            }
        }
    }
}

/// Splits `library:name`.
#[must_use]
pub fn split_name(full: &str) -> (&str, &str) {
    full.split_once(':').unwrap_or(("", full))
}

/// The model key of a model path: `${KICAD10_3DMODEL_DIR}/Resistor_SMD.3dshapes/R_0805.wrl`
/// → `Resistor_SMD/R_0805` (the extension does not matter: KiCad finds a
/// STEP model for a WRL path and the other way round).
#[must_use]
pub fn model_key(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let mut parts = path.rsplit('/');
    let file = parts.next()?;
    let folder = parts.next()?;
    let library = folder.strip_suffix(".3dshapes").unwrap_or(folder);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    (!stem.is_empty() && !library.is_empty()).then(|| format!("{library}/{stem}"))
}

const MPN_KEYS: [&str; 6] = [
    "mpn",
    "manufacturer_part_number",
    "manufacturer part number",
    "mfr. part #",
    "part number",
    "mfr_pn",
];
const MANUFACTURER_KEYS: [&str; 3] = ["manufacturer", "mfr", "mf"];
const FILTERS: &str = "ki_fp_filters";

/// The entry of a symbol definition (`library:name`).
#[must_use]
pub fn symbol_entry(def: &SymbolDef, source: usize) -> Entry {
    let (library, name) = split_name(&def.name);
    let mut mpn = String::new();
    let mut manufacturer = String::new();
    let mut params = Vec::new();
    for (key, value) in &def.properties {
        let lower = key.to_lowercase();
        if MPN_KEYS.contains(&lower.as_str()) {
            value.clone_into(&mut mpn);
        } else if MANUFACTURER_KEYS.contains(&lower.as_str()) {
            value.clone_into(&mut manufacturer);
        } else if key == FILTERS {
            params.push(format!("Footprint filters: {value}"));
        } else if !value.is_empty() {
            params.push(format!("{key}: {value}"));
        }
    }
    let mut numbers: Vec<&str> = def
        .units
        .iter()
        .flat_map(|unit| unit.pins.iter().map(|pin| pin.number.as_str()))
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    let package = def
        .footprint
        .as_deref()
        .map(|fp| split_name(fp).1.to_owned())
        .unwrap_or_default();
    Entry {
        kind: Kind::Symbol,
        source,
        library: library.to_owned(),
        name: name.to_owned(),
        description: def.description.clone(),
        keywords: def.keywords.clone(),
        value: def.value.clone(),
        package,
        mpn,
        manufacturer,
        datasheet: if def.datasheet == "~" {
            String::new()
        } else {
            def.datasheet.clone()
        },
        params: params.join("\n"),
        pins: u32::try_from(numbers.len()).unwrap_or(u32::MAX),
        units: u32::try_from(def.units.len()).unwrap_or(u32::MAX),
        mounting: None,
        size: None,
        model: None,
        file: None,
        mesh: None,
        data: compact(def),
        print: Print::Symbol(fingerprint::symbol(def)),
    }
}

fn mm(nm: i64) -> f64 {
    (nm as f64 / 1e6 * 1000.0).round() / 1000.0
}

/// Width and depth of a footprint in mm: the courtyard, or the pads if it
/// has no courtyard.
fn footprint_size(def: &FootprintDef) -> Option<[f64; 3]> {
    let mut points: Vec<(i64, i64)> = Vec::new();
    let mut add = |x: i64, y: i64| points.push((x, y));
    for graphic in &def.graphics {
        if !matches!(graphic.layer, Layer::Courtyard(_)) {
            continue;
        }
        match &graphic.geometry {
            Geometry::Line { start, end }
            | Geometry::Rect { start, end }
            | Geometry::Bezier { start, end, .. } => {
                add(start.x, start.y);
                add(end.x, end.y);
            }
            Geometry::Circle { center, radius } => {
                add(center.x - radius, center.y - radius);
                add(center.x + radius, center.y + radius);
            }
            Geometry::Arc { start, mid, end } => {
                for p in [start, mid, end] {
                    add(p.x, p.y);
                }
            }
            Geometry::Polyline { points } | Geometry::Polygon { points } => {
                for p in points {
                    add(p.x, p.y);
                }
            }
        }
    }
    if points.is_empty() {
        for pad in &def.pads {
            let r = pad.size.w.max(pad.size.h) / 2;
            points.push((pad.at.x - r, pad.at.y - r));
            points.push((pad.at.x + r, pad.at.y + r));
        }
    }
    let (x0, x1) = (
        points.iter().map(|p| p.0).min()?,
        points.iter().map(|p| p.0).max()?,
    );
    let (y0, y1) = (
        points.iter().map(|p| p.1).min()?,
        points.iter().map(|p| p.1).max()?,
    );
    Some([mm(x1 - x0), mm(y1 - y0), 0.0])
}

/// The entry of a footprint definition (`library:name`).
#[must_use]
pub fn footprint_entry(def: &FootprintDef, source: usize) -> Entry {
    let (library, name) = split_name(&def.name);
    let mut numbers: Vec<&str> = def
        .pads
        .iter()
        .map(|pad| pad.number.as_str())
        .filter(|number| !number.is_empty())
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    let mounting = match def.attributes.mounting {
        Mounting::Smd => "smd",
        Mounting::ThroughHole => "tht",
        Mounting::Other => "other",
    };
    Entry {
        kind: Kind::Footprint,
        source,
        library: library.to_owned(),
        name: name.to_owned(),
        description: def.description.clone(),
        keywords: def.keywords.clone(),
        value: String::new(),
        package: name.to_owned(),
        mpn: String::new(),
        manufacturer: String::new(),
        datasheet: String::new(),
        params: String::new(),
        pins: u32::try_from(numbers.len()).unwrap_or(u32::MAX),
        units: 0,
        mounting: Some(mounting.to_owned()),
        size: footprint_size(def),
        model: def
            .models
            .iter()
            .find(|model| !model.hidden)
            .and_then(|model| model_key(&model.path)),
        file: None,
        mesh: None,
        data: compact(def),
        print: Print::Footprint(fingerprint::footprint(def)),
    }
}

/// The entry of a 3D model file; `mesh` is its GLB content hash and
/// fingerprint, if it was tessellated.
#[must_use]
pub fn model_entry(
    library: &str,
    name: &str,
    file: &str,
    mesh: Option<(String, MeshPrint)>,
    source: usize,
) -> Entry {
    let (hash, print, size) = match mesh {
        Some((hash, print)) => {
            let [x0, y0, z0, x1, y1, z1] = print.bbox;
            let size = [x1 - x0, y1 - y0, z1 - z0].map(|um| (um as f64).round() / 1000.0);
            (Some(hash), Print::Model(print), Some(size))
        }
        None => (None, Print::None, None),
    };
    Entry {
        kind: Kind::Model,
        source,
        library: library.to_owned(),
        name: name.to_owned(),
        description: String::new(),
        keywords: String::new(),
        value: String::new(),
        package: name.to_owned(),
        mpn: String::new(),
        manufacturer: String::new(),
        datasheet: String::new(),
        params: String::new(),
        pins: 0,
        units: 0,
        mounting: None,
        size,
        model: None,
        file: Some(file.to_owned()),
        mesh: hash,
        data: None,
        print,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_keys_ignore_variables_and_extensions() {
        assert_eq!(
            model_key("${KICAD10_3DMODEL_DIR}/Resistor_SMD.3dshapes/R_0805_2012Metric.wrl")
                .as_deref(),
            Some("Resistor_SMD/R_0805_2012Metric")
        );
        assert_eq!(
            model_key(r"C:\libs\My.3dshapes\Part.step").as_deref(),
            Some("My/Part")
        );
        assert_eq!(model_key("model.step"), None);
    }
}
