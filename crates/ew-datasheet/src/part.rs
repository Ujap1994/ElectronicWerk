//! From a draft to a part: the footprint wizard of the package with the
//! dimensions of the datasheet (IPC-7351 lands), the symbol from the pins
//! of the chosen package, and the component that joins them with the
//! manufacturer data.

use std::collections::{BTreeMap, BTreeSet};

use ew_libedit::generate::{PinSpec, SymbolLayout, symbol_units};
use ew_libedit::wizard::{self, Density, Wizard};
use ew_model::id::Id;
use ew_model::library::{ComponentDef, FootprintDef, PinKind, PinText, SymbolDef};
use ew_model::units::{Nm, Size};
use serde::{Deserialize, Serialize};

use crate::draft::{Dimension, Draft, Family, PackageDraft, PinType, pad_order};

/// Choices of the user for the generated part.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartOptions {
    /// IPC-7351 density level of the lands.
    pub density: Density,
    /// Name of the symbol and component (default: part and package).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Name of the footprint (default: the wizard's name).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footprint: Option<String>,
    /// Split the symbol into units above this many pins (0: never;
    /// default 64).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_above: Option<usize>,
    /// Largest unit after splitting (default 40).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_size: Option<usize>,
}

/// A generated part, not yet stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Generated {
    /// The symbol.
    pub symbol: SymbolDef,
    /// The footprint.
    pub footprint: FootprintDef,
    /// The component joining them.
    pub component: ComponentDef,
    /// The wizard parameters of the footprint.
    pub wizard: Wizard,
    /// The package with its dimensions (for the 3D model).
    pub package: PackageDraft,
    /// What the user should know (missing pins, pads without pins …).
    pub warnings: Vec<String>,
}

/// Why a part cannot be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct PartError(pub String);

fn error(message: impl Into<String>) -> PartError {
    PartError(message.into())
}

fn nm(mm: f64) -> Nm {
    ((mm * 1_000.0).round() * 1_000.0) as Nm
}

fn value(p: &PackageDraft, key: &str) -> Option<f64> {
    p.dimensions.get(key).and_then(Dimension::value)
}

fn need(p: &PackageDraft, key: &str) -> Result<f64, PartError> {
    value(p, key).ok_or_else(|| error(format!("the dimension {key} of {} is missing", p.name)))
}

fn largest(p: &PackageDraft, key: &str) -> Option<f64> {
    p.dimensions.get(key).and_then(Dimension::most)
}

/// Rows and columns of a ball grid from its ball names (`A1` … `H8`).
fn grid(numbers: &[String]) -> (u32, u32) {
    let mut rows = BTreeSet::new();
    let mut columns = 0u32;
    for n in numbers {
        let letters: String = n.chars().take_while(char::is_ascii_alphabetic).collect();
        let digits: String = n.chars().skip_while(char::is_ascii_alphabetic).collect();
        if !letters.is_empty() {
            rows.insert(letters);
        }
        columns = columns.max(digits.parse().unwrap_or(0));
    }
    (u32::try_from(rows.len()).unwrap_or(0), columns)
}

/// The footprint wizard of a package.
///
/// # Errors
///
/// A family without a wizard, or missing dimensions.
#[allow(clippy::too_many_lines, reason = "one mapping per package family")]
pub fn wizard_for(
    p: &PackageDraft,
    numbers: &[String],
    density: Density,
) -> Result<Wizard, PartError> {
    let pins = if p.pins > 0 {
        p.pins
    } else {
        u32::try_from(numbers.len()).unwrap_or(0)
    };
    let exposed = |p: &PackageDraft| match (value(p, "E2"), value(p, "D2")) {
        (Some(w), Some(h)) if p.exposed_pad => Some(Size::new(nm(w), nm(h))),
        _ => None,
    };
    Ok(match p.family {
        Family::Sop | Family::Qfp => {
            let quad = p.family == Family::Qfp;
            let (body_x, body_y) = if quad {
                (
                    value(p, "E1")
                        .or_else(|| value(p, "D1"))
                        .ok_or_else(|| error("the body size E1 is missing"))?,
                    value(p, "D1")
                        .or_else(|| value(p, "E1"))
                        .ok_or_else(|| error("the body size D1 is missing"))?,
                )
            } else {
                (need(p, "E1")?, need(p, "D")?)
            };
            Wizard::GullWing {
                pins,
                quad,
                pitch: nm(need(p, "e")?),
                lead_span: nm(need(p, "E")?),
                lead_length: nm(need(p, "L")?),
                lead_width: nm(need(p, "b")?),
                body_width: nm(body_x),
                body_length: nm(body_y),
                exposed_pad: exposed(p),
                density,
            }
        }
        Family::Qfn | Family::Dfn => Wizard::NoLead {
            pins,
            quad: p.family == Family::Qfn,
            pitch: nm(need(p, "e")?),
            body_width: nm(need(p, "E")?),
            body_length: nm(need(p, "D")?),
            terminal_length: nm(need(p, "L")?),
            terminal_width: nm(need(p, "b")?),
            exposed_pad: exposed(p),
            density,
        },
        Family::Bga => {
            let (rows, columns) = grid(numbers);
            if rows == 0 || columns == 0 {
                return Err(error("the ball names (A1 …) are needed for a BGA"));
            }
            Wizard::Bga {
                rows,
                columns,
                pitch: nm(need(p, "e")?),
                ball: nm(need(p, "b")?),
                body_width: nm(need(p, "E")?),
                body_length: nm(need(p, "D")?),
                density,
            }
        }
        Family::Sot23 => Wizard::Sot {
            pins,
            pitch: nm(need(p, "e")?),
            lead_span: nm(need(p, "E")?),
            lead_length: nm(need(p, "L")?),
            lead_width: nm(need(p, "b")?),
            body_width: nm(need(p, "E1")?),
            body_length: nm(need(p, "D")?),
            density,
        },
        Family::Sot223 | Family::Sot89 | Family::Dpak => {
            let tab_length = match p.family {
                Family::Sot223 => 1.5,
                Family::Sot89 => 2.1,
                _ => value(p, "E").unwrap_or(10.0) * 0.55,
            };
            let leads = if p.family == Family::Dpak {
                3
            } else {
                pins.clamp(1, 5)
            };
            Wizard::Tab {
                leads,
                omit_middle: p.family == Family::Dpak,
                pitch: nm(need(p, "e")?),
                lead_span: nm(need(p, "E")?),
                lead_length: nm(need(p, "L")?),
                lead_width: nm(need(p, "b")?),
                tab_width: nm(value(p, "b2").unwrap_or(3.0)),
                tab_length: nm(tab_length),
                body_width: nm(need(p, "E1")?),
                body_length: nm(need(p, "D")?),
                density,
            }
        }
        Family::Dip => {
            let lead = largest(p, "b").unwrap_or(0.56);
            let drill = ((lead + 0.25) * 10.0).ceil() / 10.0;
            // Row spacing: the standard one nearest to E (some datasheets
            // give the lead spread there).
            let e = need(p, "E")?;
            let row = [7.62, 10.16, 15.24, 22.86]
                .into_iter()
                .min_by(|a, b| (a - e).abs().total_cmp(&(b - e).abs()))
                .filter(|r| (r - e).abs() < 0.8)
                .unwrap_or(e);
            Wizard::Dip {
                pins,
                pitch: nm(need(p, "e")?),
                row_spacing: nm(row),
                drill: nm(drill),
                pad: nm((drill + 0.8).min(need(p, "e")? - 0.4)),
                body_width: nm(need(p, "E1")?),
                body_length: nm(need(p, "D")?),
            }
        }
        Family::To220 | Family::To92 => {
            let pitch = need(p, "e")?;
            let lead = largest(p, "b").unwrap_or(0.9);
            let drill = ((lead + 0.3) * 10.0).ceil() / 10.0;
            let (width, depth) = if p.family == Family::To220 {
                (need(p, "E")?, value(p, "A").unwrap_or(4.5))
            } else {
                (need(p, "D")?, need(p, "E")?)
            };
            Wizard::Inline {
                pins,
                pitch: nm(pitch),
                drill: nm(drill),
                pad: nm((drill + 0.8).min(pitch * 2.0)),
                body_width: nm(width),
                body_depth: nm(depth),
            }
        }
        Family::Chip => Wizard::Chip {
            body_length: nm(need(p, "D")?),
            body_width: nm(need(p, "E")?),
            terminal: nm(value(p, "L").or_else(|| value(p, "b")).unwrap_or(0.3)),
            density,
        },
        Family::Header | Family::Other => {
            return Err(error(format!(
                "no footprint wizard for {}: choose the package family",
                p.name
            )));
        }
    })
}

fn short(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A footprint name in the style of the KiCad libraries, with the
/// package name of the datasheet (`TSSOP-8_4.4x3mm_P0.65mm`).
fn footprint_name(p: &PackageDraft, w: &Wizard) -> String {
    let base = crate::package::recognise(&p.name)
        .map(|r| r.canonical.split(' ').next().unwrap_or("").to_owned())
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| p.name.clone());
    let mm = |v: Nm| v as f64 / 1_000_000.0;
    let ep = |s: Option<Size>| {
        s.map_or_else(String::new, |s| {
            format!("_EP{}x{}mm", short(mm(s.w)), short(mm(s.h)))
        })
    };
    let name = match *w {
        Wizard::GullWing {
            pitch,
            body_width,
            body_length,
            exposed_pad,
            ..
        }
        | Wizard::NoLead {
            pitch,
            body_width,
            body_length,
            exposed_pad,
            ..
        } => format!(
            "{base}{}_{}x{}mm_P{}mm{}",
            if exposed_pad.is_some() { "-1EP" } else { "" },
            short(mm(body_width)),
            short(mm(body_length)),
            short(mm(pitch)),
            ep(exposed_pad)
        ),
        Wizard::Sot {
            pitch,
            body_width,
            body_length,
            ..
        }
        | Wizard::Tab {
            pitch,
            body_width,
            body_length,
            ..
        }
        | Wizard::Bga {
            pitch,
            body_width,
            body_length,
            ..
        } => format!(
            "{base}_{}x{}mm_P{}mm",
            short(mm(body_width)),
            short(mm(body_length)),
            short(mm(pitch))
        ),
        Wizard::Dip { row_spacing, .. } => format!("{base}_W{}mm", short(mm(row_spacing))),
        Wizard::Inline { .. } => format!("{base}_Vertical"),
        Wizard::Chip { .. } | Wizard::Header { .. } => wizard::suggest_name(w),
    };
    clean_name(&name)
}

/// The pin type of the library model.
#[must_use]
pub const fn pin_kind(kind: PinType) -> PinKind {
    match kind {
        PinType::Input => PinKind::Input,
        PinType::Output => PinKind::Output,
        PinType::Bidirectional => PinKind::Bidirectional,
        PinType::TriState => PinKind::TriState,
        PinType::Passive => PinKind::Passive,
        PinType::PowerIn => PinKind::PowerIn,
        PinType::PowerOut => PinKind::PowerOut,
        PinType::OpenCollector => PinKind::OpenCollector,
        PinType::NotConnected => PinKind::NoConnect,
        PinType::Unspecified => PinKind::Unspecified,
    }
}

/// A library name from free text: letters, digits, `-`, `_`, `.`.
#[must_use]
pub fn clean_name(text: &str) -> String {
    let out: String = text
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut out = out.trim_matches('_').to_owned();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out
}

/// Generates the part for one package of the draft.
///
/// # Errors
///
/// An unknown package, a package without pins or a footprint the wizard
/// cannot make.
pub fn generate(
    draft: &Draft,
    package: &str,
    options: &PartOptions,
) -> Result<Generated, PartError> {
    let p = draft
        .packages
        .iter()
        .find(|p| p.name == package)
        .ok_or_else(|| error(format!("the draft has no package {package}")))?;
    let pins = draft.pins_of(package);
    if pins.is_empty() {
        return Err(error(format!("no pin has a pad in {package}")));
    }
    let numbers: Vec<String> = pins.iter().flat_map(|(_, n)| n.iter().cloned()).collect();
    let wizard = wizard_for(p, &numbers, options.density)?;
    let footprint_name = options
        .footprint
        .clone()
        .map(|n| clean_name(&n))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| footprint_name(p, &wizard));
    let mut footprint = wizard::generate(&wizard, &footprint_name).map_err(|e| error(e.0))?;
    let mut warnings = Vec::new();
    // Pins against pads: pins without a pad are left out, pads without a
    // pin get one (the exposed pad or tab as `EP`, others as `NC`).
    let pads: BTreeSet<String> = footprint
        .pads
        .iter()
        .map(|pad| pad.number.clone())
        .collect();
    let used: BTreeSet<String> = numbers.iter().cloned().collect();
    for n in used.difference(&pads) {
        warnings.push(format!(
            "Pin {n} has no pad in the {package} footprint; it is left out."
        ));
    }
    let mut extra: Vec<(String, String, PinKind)> = Vec::new();
    for n in pads.difference(&used) {
        if n.parse::<u32>().ok() == Some(p.pins + 1) {
            warnings.push(format!(
                "Pad {n} (exposed pad or tab) has no pin in the datasheet tables; it was added as pin EP. Connect it as the datasheet says, often to GND."
            ));
            extra.push((n.clone(), "EP".to_owned(), PinKind::Passive));
        } else {
            warnings.push(format!(
                "Pad {n} has no pin in the datasheet tables; it was added as a not connected pin (NC)."
            ));
            extra.push((n.clone(), "NC".to_owned(), PinKind::NoConnect));
        }
    }
    let name = options
        .name
        .clone()
        .map(|n| clean_name(&n))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            if draft.packages.len() > 1 {
                clean_name(&format!("{}_{}", draft.name, package))
            } else {
                clean_name(&draft.name)
            }
        });
    if name.is_empty() {
        return Err(error("the part needs a name"));
    }
    let mut specs: Vec<PinSpec> = Vec::new();
    for (pin, list) in &pins {
        for n in list.iter().filter(|n| pads.contains(*n)) {
            specs.push(PinSpec {
                number: n.clone(),
                name: pin.name.clone(),
                kind: pin_kind(pin.kind),
            });
        }
    }
    for (number, name, kind) in extra {
        specs.push(PinSpec { number, name, kind });
    }
    specs.sort_by(|a, b| pad_order(&a.number, &b.number));
    let layout = SymbolLayout {
        split_above: options.split_above.unwrap_or(64),
        unit_size: options.unit_size.unwrap_or(40),
        ..SymbolLayout::default()
    };
    let description = if draft.description.is_empty() {
        format!("{} in {package}", draft.name)
    } else {
        format!("{}, {package}", draft.description)
    };
    let keywords = [draft.name.as_str(), draft.manufacturer.as_str(), package]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    footprint.description = format!("{package}: {}", footprint.description);
    let symbol = SymbolDef {
        id: Id::random(),
        name: name.clone(),
        description: description.clone(),
        keywords: keywords.clone(),
        prefix: "U".into(),
        value: draft.name.clone(),
        footprint: Some(footprint.name.clone()),
        datasheet: draft.datasheet.clone(),
        power: false,
        units: symbol_units(&specs, &layout),
        properties: BTreeMap::new(),
        pin_text: PinText::default(),
    };
    let mut properties = BTreeMap::new();
    properties.insert("Package".to_owned(), package.to_owned());
    let component = ComponentDef {
        id: Id::random(),
        name,
        description,
        keywords,
        symbol: symbol.name.clone(),
        footprint: Some(footprint.name.clone()),
        value: draft.name.clone(),
        manufacturer: draft.manufacturer.clone(),
        mpn: if draft.mpn.is_empty() {
            draft.name.clone()
        } else {
            draft.mpn.clone()
        },
        datasheet: draft.datasheet.clone(),
        properties,
        suppliers: Vec::new(),
    };
    Ok(Generated {
        symbol,
        footprint,
        component,
        wizard,
        package: p.clone(),
        warnings,
    })
}
