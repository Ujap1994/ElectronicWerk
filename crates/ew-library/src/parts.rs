//! The part catalog on top of the index: the category of every symbol and
//! the packages a part can have.
//!
//! A symbol that names its footprint (79 % of the KiCad symbols) is a
//! complete part with that one package. A basic part without a footprint
//! (resistor, capacitor, LED, pin header …) offers the footprints its KiCad
//! footprint filters allow with the same number of pads, the common
//! packages first.

use std::sync::atomic::Ordering;

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::LibraryError;
use crate::category;
use crate::index::{Library, sql_error};

/// A category with its number of parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCount {
    /// Path (`passive/resistor`, `ic/mcu/ST`).
    pub path: String,
    /// Parts in it and below it.
    pub count: u64,
}

/// A package (footprint) a part can have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    /// Index id of the footprint (0 if the library does not have it).
    pub id: i64,
    /// Footprint library.
    pub library: String,
    /// Footprint name.
    pub name: String,
    /// Number of pads.
    pub pads: u32,
    /// `smd`, `tht` or `other`.
    pub mounting: Option<String>,
    /// It has a 3D model.
    pub model: bool,
    /// One of the common packages of this kind of part.
    pub recommended: bool,
}

/// The packages of a part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Packages {
    /// The symbol names its footprint: the part has just that package.
    pub fixed: bool,
    /// The packages, the default first.
    pub packages: Vec<Package>,
}

/// At most this many packages are offered.
const MAX_PACKAGES: usize = 60;

impl Library {
    /// Computes the category of every symbol once per open index (a
    /// temporary table; the index files stay as they are).
    fn ensure_categories(&self) -> Result<(), LibraryError> {
        if self.categories_ready.load(Ordering::Acquire) {
            return Ok(());
        }
        let conn = &self.conn;
        conn.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS part_categories (
                 item INTEGER PRIMARY KEY, path TEXT NOT NULL);
             DELETE FROM part_categories;",
        )
        .map_err(sql_error)?;
        let rows: Vec<(i64, String, String)> = conn
            .prepare("SELECT id, library, name FROM items WHERE kind = 0")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                    .collect()
            })
            .map_err(sql_error)?;
        let tx = conn.unchecked_transaction().map_err(sql_error)?;
        {
            let mut insert = tx
                .prepare("INSERT INTO part_categories (item, path) VALUES (?1, ?2)")
                .map_err(sql_error)?;
            for (id, library, name) in rows {
                if let Some(path) = category::path_of_symbol(&library, &name) {
                    insert
                        .execute(rusqlite::params![id, path])
                        .map_err(sql_error)?;
                }
            }
        }
        tx.commit().map_err(sql_error)?;
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS temp.part_categories_path ON part_categories(path)",
        )
        .map_err(sql_error)?;
        self.categories_ready.store(true, Ordering::Release);
        Ok(())
    }

    /// The SQL condition that keeps the symbols of a category (and below
    /// it; `*` for every part), with the category as parameter `?{slot}`.
    pub(crate) fn category_condition(&self, slot: usize) -> Result<String, LibraryError> {
        self.ensure_categories()?;
        Ok(format!(
            "i.id IN (SELECT item FROM part_categories
                      WHERE ?{slot} = '*' OR path = ?{slot} OR path LIKE ?{slot} || '/%')"
        ))
    }

    /// Every category with its number of parts, in the order of the tree
    /// (microcontroller vendors sorted by name below `ic/mcu`); empty
    /// categories are left out.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn categories(&self) -> Result<Vec<CategoryCount>, LibraryError> {
        self.ensure_categories()?;
        let counts: Vec<(String, u64)> = self
            .conn
            .prepare(
                "SELECT c.path, count(*) FROM part_categories c JOIN items i ON i.id = c.item
                 WHERE i.canonical IS NULL GROUP BY c.path",
            )
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                    ))
                })?
                .collect()
            })
            .map_err(sql_error)?;
        let total = |path: &str| -> u64 {
            counts
                .iter()
                .filter(|(p, _)| category::within(p, path))
                .map(|(_, n)| n)
                .sum()
        };
        let mut out = Vec::new();
        for path in category::TREE {
            let count = total(path);
            if count == 0 {
                continue;
            }
            out.push(CategoryCount {
                path: (*path).to_owned(),
                count,
            });
            if *path == "ic/mcu" {
                let mut vendors: Vec<&(String, u64)> = counts
                    .iter()
                    .filter(|(p, _)| p.starts_with("ic/mcu/"))
                    .collect();
                vendors.sort_by_key(|(p, _)| p.to_lowercase());
                out.extend(vendors.into_iter().map(|(path, count)| CategoryCount {
                    path: path.clone(),
                    count: *count,
                }));
            }
        }
        Ok(out)
    }

    /// The packages of a symbol.
    ///
    /// # Errors
    ///
    /// Fails if there is no such symbol.
    pub fn packages(&self, symbol: i64) -> Result<Packages, LibraryError> {
        let (library, name, pins, package, params): (String, String, u32, String, String) = self
            .conn
            .query_row(
                "SELECT library, name, pins, package, params FROM items WHERE id = ?1 AND kind = 0",
                [symbol],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(sql_error)?
            .ok_or_else(|| LibraryError::NotFound(format!("library symbol {symbol}")))?;
        if !package.is_empty() {
            let (lib, fp) = package.split_once(':').unwrap_or(("", package.as_str()));
            let found = self.footprint(lib, fp)?;
            return Ok(Packages {
                fixed: true,
                packages: vec![found.unwrap_or(Package {
                    id: 0,
                    library: lib.to_owned(),
                    name: fp.to_owned(),
                    pads: pins,
                    mounting: None,
                    model: false,
                    recommended: true,
                })],
            });
        }
        let filters = footprint_filters(&params);
        if filters.is_empty() {
            return Ok(Packages {
                fixed: false,
                packages: Vec::new(),
            });
        }
        // Merged duplicates count only as common packages (whose names
        // people know), otherwise their canonical entry stands for them.
        let candidates: Vec<(Package, bool)> = self
            .conn
            .prepare_cached(
                "SELECT id, library, name, pins, mounting, model IS NOT NULL, canonical IS NOT NULL
                 FROM items WHERE kind = 1 AND pins = ?1",
            )
            .and_then(|mut stmt| {
                stmt.query_map([pins], |row| {
                    Ok((
                        Package {
                            id: row.get(0)?,
                            library: row.get(1)?,
                            name: row.get(2)?,
                            pads: row.get(3)?,
                            mounting: row.get(4)?,
                            model: row.get(5)?,
                            recommended: false,
                        },
                        row.get(6)?,
                    ))
                })?
                .collect()
            })
            .map_err(sql_error)?;
        let preferred = preferred(&library, &name, pins);
        let rank = |p: &Package| {
            let full = format!("{}:{}", p.library, p.name);
            preferred.iter().position(|q| *q == full)
        };
        let mut fitting: Vec<Package> = candidates
            .into_iter()
            .filter(|(p, merged)| {
                (!merged || rank(p).is_some())
                    && filters.iter().any(|f| f.matches(&p.library, &p.name))
            })
            .map(|(p, _)| p)
            .collect();
        for p in &mut fitting {
            p.recommended = rank(p).is_some();
        }
        fitting.sort_by(|a, b| {
            rank(a)
                .unwrap_or(usize::MAX)
                .cmp(&rank(b).unwrap_or(usize::MAX))
                .then_with(|| a.library.cmp(&b.library))
                .then_with(|| a.name.cmp(&b.name))
        });
        fitting.truncate(MAX_PACKAGES);
        Ok(Packages {
            fixed: false,
            packages: fitting,
        })
    }

    fn footprint(&self, library: &str, name: &str) -> Result<Option<Package>, LibraryError> {
        self.conn
            .query_row(
                "SELECT id, library, name, pins, mounting, model IS NOT NULL FROM items
                 WHERE kind = 1 AND name = ?2 AND (library = ?1 OR ?1 = '')
                 ORDER BY canonical IS NOT NULL LIMIT 1",
                [library, name],
                |row| {
                    Ok(Package {
                        id: row.get(0)?,
                        library: row.get(1)?,
                        name: row.get(2)?,
                        pads: row.get(3)?,
                        mounting: row.get(4)?,
                        model: row.get(5)?,
                        recommended: true,
                    })
                },
            )
            .optional()
            .map_err(sql_error)
    }

    /// Whether the categories were computed (tests).
    #[must_use]
    pub fn categories_computed(&self) -> bool {
        self.categories_ready.load(Ordering::Acquire)
    }
}

/// One footprint filter of a KiCad symbol (`R_*`, `Connector*:*_1x??_*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    library: Option<String>,
    name: String,
}

impl Filter {
    /// Whether a footprint passes the filter (case does not matter).
    #[must_use]
    pub fn matches(&self, library: &str, name: &str) -> bool {
        self.library.as_deref().is_none_or(|l| glob(l, library)) && glob(&self.name, name)
    }
}

/// The footprint filters in the parameters of a symbol.
#[must_use]
pub fn footprint_filters(params: &str) -> Vec<Filter> {
    params
        .lines()
        .filter_map(|line| line.strip_prefix("Footprint filters:"))
        .flat_map(str::split_whitespace)
        .map(|pattern| match pattern.split_once(':') {
            Some((library, name)) => Filter {
                library: Some(library.to_owned()),
                name: name.to_owned(),
            },
            None => Filter {
                library: None,
                name: pattern.to_owned(),
            },
        })
        .collect()
}

/// `*` and `?` wildcards, ignoring case.
#[must_use]
pub fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

/// The common packages of basic parts, in the order they are offered.
#[allow(clippy::too_many_lines, reason = "one list per kind of part")]
fn preferred(library: &str, name: &str, pins: u32) -> Vec<String> {
    let list = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let starts = |prefix: &str| name == prefix || name.starts_with(&format!("{prefix}_"));
    if library == "Connector_Generic" {
        let rows = |n: u32| format!("{n:02}");
        if name.starts_with("Conn_01x") {
            let n = pins;
            return vec![
                format!(
                    "Connector_PinHeader_2.54mm:PinHeader_1x{}_P2.54mm_Vertical",
                    rows(n)
                ),
                format!(
                    "Connector_PinSocket_2.54mm:PinSocket_1x{}_P2.54mm_Vertical",
                    rows(n)
                ),
                format!(
                    "Connector_JST:JST_XH_B{n}B-XH-A_1x{}_P2.50mm_Vertical",
                    rows(n)
                ),
                format!(
                    "Connector_JST:JST_PH_B{n}B-PH-K_1x{}_P2.00mm_Vertical",
                    rows(n)
                ),
                format!(
                    "Connector_PinHeader_1.27mm:PinHeader_1x{}_P1.27mm_Vertical",
                    rows(n)
                ),
            ];
        }
        if name.starts_with("Conn_02x") {
            let n = pins / 2;
            return vec![
                format!(
                    "Connector_PinHeader_2.54mm:PinHeader_2x{}_P2.54mm_Vertical",
                    rows(n)
                ),
                format!(
                    "Connector_PinSocket_2.54mm:PinSocket_2x{}_P2.54mm_Vertical",
                    rows(n)
                ),
                format!("Connector_IDC:IDC-Header_2x{}_P2.54mm_Vertical", rows(n)),
                format!(
                    "Connector_PinHeader_1.27mm:PinHeader_2x{}_P1.27mm_Vertical",
                    rows(n)
                ),
            ];
        }
        return Vec::new();
    }
    if library != "Device" {
        return Vec::new();
    }
    if starts("C_Polarized") || starts("CP") {
        return list(&[
            "Capacitor_SMD:CP_Elec_5x5.4",
            "Capacitor_SMD:CP_Elec_6.3x5.4",
            "Capacitor_SMD:CP_Elec_8x10",
            "Capacitor_Tantalum_SMD:CP_EIA-3216-18_Kemet-A",
            "Capacitor_Tantalum_SMD:CP_EIA-3528-21_Kemet-B",
            "Capacitor_THT:CP_Radial_D5.0mm_P2.00mm",
            "Capacitor_THT:CP_Radial_D6.3mm_P2.50mm",
            "Capacitor_THT:CP_Radial_D8.0mm_P3.50mm",
        ]);
    }
    if starts("R") || starts("R_US") {
        return list(&[
            "Resistor_SMD:R_0603_1608Metric",
            "Resistor_SMD:R_0805_2012Metric",
            "Resistor_SMD:R_0402_1005Metric",
            "Resistor_SMD:R_1206_3216Metric",
            "Resistor_SMD:R_2512_6332Metric",
            "Resistor_THT:R_Axial_DIN0207_L6.3mm_D2.5mm_P10.16mm_Horizontal",
            "Resistor_THT:R_Axial_DIN0207_L6.3mm_D2.5mm_P7.62mm_Horizontal",
        ]);
    }
    if starts("C") {
        return list(&[
            "Capacitor_SMD:C_0603_1608Metric",
            "Capacitor_SMD:C_0805_2012Metric",
            "Capacitor_SMD:C_0402_1005Metric",
            "Capacitor_SMD:C_1206_3216Metric",
            "Capacitor_SMD:C_1210_3225Metric",
            "Capacitor_THT:C_Disc_D5.0mm_W2.5mm_P5.00mm",
            "Capacitor_THT:C_Rect_L7.2mm_W2.5mm_P5.00mm_FKS2_FKP2_MKS2_MKP2",
        ]);
    }
    if starts("L") || starts("FerriteBead") {
        return list(&[
            "Inductor_SMD:L_0603_1608Metric",
            "Inductor_SMD:L_0805_2012Metric",
            "Inductor_SMD:L_1206_3216Metric",
            "Inductor_SMD:L_1210_3225Metric",
        ]);
    }
    if starts("LED") {
        return list(&[
            "LED_SMD:LED_0603_1608Metric",
            "LED_SMD:LED_0805_2012Metric",
            "LED_SMD:LED_1206_3216Metric",
            "LED_THT:LED_D3.0mm",
            "LED_THT:LED_D5.0mm",
        ]);
    }
    if starts("D") || name.starts_with("D_Schottky") || name.starts_with("D_Zener") {
        return list(&[
            "Diode_SMD:D_SOD-123",
            "Diode_SMD:D_SOD-323",
            "Diode_SMD:D_MiniMELF",
            "Diode_SMD:D_SMA",
            "Diode_SMD:D_SMB",
            "Diode_THT:D_DO-41_SOD81_P10.16mm_Horizontal",
            "Diode_THT:D_DO-35_SOD27_P7.62mm_Horizontal",
        ]);
    }
    if name.starts_with("Crystal") {
        return list(&[
            "Crystal:Crystal_SMD_3215-2Pin_3.2x1.5mm",
            "Crystal:Crystal_SMD_5032-2Pin_5.0x3.2mm",
            "Crystal:Crystal_SMD_3225-4Pin_3.2x2.5mm",
            "Crystal:Crystal_HC49-U_Vertical",
            "Crystal:Crystal_HC49-4H_Vertical",
        ]);
    }
    if name.starts_with("Fuse") {
        return list(&[
            "Fuse:Fuse_0603_1608Metric",
            "Fuse:Fuse_1206_3216Metric",
            "Fuse:Fuse_1812_4532Metric",
        ]);
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footprint_filters_match_like_kicad() {
        let filters =
            footprint_filters("Sim.Device: D\nFootprint filters: R_* Connector*:*_1x??_*");
        assert_eq!(filters.len(), 2);
        assert!(filters[0].matches("Resistor_SMD", "R_0603_1608Metric"));
        assert!(!filters[0].matches("Resistor_SMD", "RV_0603"));
        assert!(filters[1].matches(
            "Connector_PinHeader_2.54mm",
            "PinHeader_1x04_P2.54mm_Vertical"
        ));
        assert!(!filters[1].matches(
            "Connector_PinHeader_2.54mm",
            "PinHeader_2x04_P2.54mm_Vertical"
        ));
        assert!(!filters[1].matches("Resistor_SMD", "R_1x04_x"));
        assert!(glob("*Diode*", "SOD-123_diode_small"));
        assert!(glob("TO-???*", "TO-220-3_Vertical"));
        assert!(!glob("TO-???*", "TO-92"));
    }

    #[test]
    fn basic_parts_prefer_common_packages() {
        assert_eq!(
            preferred("Device", "R", 2)[0],
            "Resistor_SMD:R_0603_1608Metric"
        );
        assert_eq!(
            preferred("Device", "R_Small", 2)[0],
            "Resistor_SMD:R_0603_1608Metric"
        );
        assert_eq!(
            preferred("Device", "C_Polarized", 2)[0],
            "Capacitor_SMD:CP_Elec_5x5.4"
        );
        assert_eq!(
            preferred("Device", "Crystal", 2)[1],
            "Crystal:Crystal_SMD_5032-2Pin_5.0x3.2mm"
        );
        assert_eq!(
            preferred("Connector_Generic", "Conn_01x04", 4)[0],
            "Connector_PinHeader_2.54mm:PinHeader_1x04_P2.54mm_Vertical"
        );
        assert_eq!(
            preferred("Connector_Generic", "Conn_02x05_Odd_Even", 10)[0],
            "Connector_PinHeader_2.54mm:PinHeader_2x05_P2.54mm_Vertical"
        );
        assert_eq!(
            preferred("MCU_ST_STM32F4", "STM32F401", 48),
            Vec::<String>::new()
        );
    }
}
