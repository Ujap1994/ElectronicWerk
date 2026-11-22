//! Engineering calculators (`calc.*`): E series, voltage dividers and
//! regulator feedback resistors, current capacity of tracks and vias
//! (IPC-2152, IPC-2221) and the fusing current of copper (Onderdonk).
//!
//! They need no project. Lengths are nanometres like everywhere in the
//! engine; currents amperes, voltages volts, resistances ohms, temperatures
//! degrees Celsius, times seconds, areas square millimetres.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::{EngineError, ErrorCode};

/// The E24 values; E12, E6 and E3 are every second, fourth and eighth one.
const E24: [f64; 24] = [
    1.0, 1.1, 1.2, 1.3, 1.5, 1.6, 1.8, 2.0, 2.2, 2.4, 2.7, 3.0, 3.3, 3.6, 3.9, 4.3, 4.7, 5.1, 5.6,
    6.2, 6.8, 7.5, 8.2, 9.1,
];

/// A series of preferred numbers (IEC 60063).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Series {
    /// 3 values per decade (> 20 %).
    E3,
    /// 6 values (20 %).
    E6,
    /// 12 values (10 %).
    E12,
    /// 24 values (5 %).
    E24,
    /// 48 values (2 %).
    E48,
    /// 96 values (1 %).
    E96,
    /// 192 values (0.5 %).
    E192,
}

impl Series {
    /// The values of one decade, from 1.0 to below 10.
    #[must_use]
    pub fn values(self) -> Vec<f64> {
        let every = |step: usize| E24.iter().copied().step_by(step).collect();
        let computed = |n: i32| {
            (0..n)
                .map(|i| {
                    // E192 has one value off the formula: 9.20 instead of 9.19.
                    if n == 192 && i == 185 {
                        return 9.2;
                    }
                    (10f64.powf(f64::from(i) / f64::from(n)) * 100.0).round() / 100.0
                })
                .collect()
        };
        match self {
            Self::E3 => every(8),
            Self::E6 => every(4),
            Self::E12 => every(2),
            Self::E24 => E24.to_vec(),
            Self::E48 => computed(48),
            Self::E96 => computed(96),
            Self::E192 => computed(192),
        }
    }

    /// The tolerance the series is made for (percent).
    #[must_use]
    pub const fn tolerance(self) -> f64 {
        match self {
            Self::E3 => 40.0,
            Self::E6 => 20.0,
            Self::E12 => 10.0,
            Self::E24 => 5.0,
            Self::E48 => 2.0,
            Self::E96 => 1.0,
            Self::E192 => 0.5,
        }
    }
}

/// The decade exponent of a positive, finite number (−308 … 308).
#[allow(
    clippy::cast_possible_truncation,
    reason = "log10 of a finite f64 lies within ±400"
)]
fn exponent(value: f64, round_up: bool) -> i32 {
    let log = value.abs().log10();
    (if round_up { log.ceil() } else { log.floor() }).clamp(-400.0, 400.0) as i32
}

/// Rounds to `digits` significant digits (keeps `1.2e3` from printing as
/// `1200.0000000000002`).
fn significant(value: f64, digits: i32) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let scale = 10f64.powi(digits - 1 - exponent(value, false));
    (value * scale).round() / scale
}

/// The series values next to `value`: the largest not above and the
/// smallest not below it.
#[must_use]
pub fn neighbours(value: f64, series: Series) -> (f64, f64) {
    let decade = value.log10().floor();
    let base = 10f64.powf(decade);
    let mut values: Vec<f64> = series.values().iter().map(|v| v * base).collect();
    values.push(10.0 * base);
    values.insert(
        0,
        series.values().last().copied().unwrap_or(1.0) * base / 10.0,
    );
    let below = values
        .iter()
        .copied()
        .filter(|v| *v <= value * (1.0 + 1e-9))
        .fold(f64::NAN, f64::max);
    let above = values
        .iter()
        .copied()
        .filter(|v| *v >= value * (1.0 - 1e-9))
        .fold(f64::NAN, f64::min);
    (significant(below, 4), significant(above, 4))
}

/// The series value nearest to `value` (by ratio).
#[must_use]
pub fn nearest(value: f64, series: Series) -> f64 {
    let (below, above) = neighbours(value, series);
    if value / below <= above / value {
        below
    } else {
        above
    }
}

/// Series values from `min` to `max`.
fn values_between(series: Series, min: f64, max: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let first = exponent(min, false);
    let last = exponent(max, true);
    for decade in first..=last {
        for v in series.values() {
            let value = significant(v * 10f64.powi(decade), 4);
            if value >= min * (1.0 - 1e-9) && value <= max * (1.0 + 1e-9) {
                out.push(value);
            }
        }
    }
    out
}

fn invalid(message: &str) -> EngineError {
    EngineError::invalid_params(message.to_owned())
}

fn positive(value: f64, name: &str) -> Result<f64, EngineError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(invalid(&format!("`{name}` must be a positive number")))
    }
}

// --- E series -------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeriesParams {
    value: f64,
    #[serde(default = "e24")]
    series: Series,
}

const fn e24() -> Series {
    Series::E24
}

fn e_series(params: &SeriesParams) -> Result<Value, EngineError> {
    let value = positive(params.value, "value")?;
    let (below, above) = neighbours(value, params.series);
    let best = nearest(value, params.series);
    Ok(json!({
        "nearest": best,
        "below": below,
        "above": above,
        "error": (best - value) / value * 100.0,
        "tolerance": params.series.tolerance(),
    }))
}

// --- Dividers and regulators ------------------------------------------------

/// One pair of resistors.
struct Pair {
    upper: f64,
    lower: f64,
    out: f64,
    error: f64,
}

/// What a search for resistor pairs looks for.
struct Search {
    series: Series,
    /// The wanted output.
    wanted: f64,
    /// Among equal ratios, the total resistance nearest to this wins.
    target_total: f64,
    /// Allowed total resistance.
    total_range: (f64, f64),
    count: usize,
}

/// Pairs of series values for an output, best first: `out(upper, lower)`
/// gives the output of a pair, `ideal(lower)` the exact upper resistor.
fn pairs(
    search: &Search,
    lowers: &[f64],
    ideal: impl Fn(f64) -> f64,
    out: impl Fn(f64, f64) -> f64,
) -> Vec<Pair> {
    let Search {
        series,
        wanted,
        target_total,
        total_range,
        count,
    } = *search;
    let mut found: Vec<Pair> = Vec::new();
    for &lower in lowers {
        let exact = ideal(lower);
        if !exact.is_finite() || exact <= 0.0 {
            continue;
        }
        let (a, b) = neighbours(exact, series);
        for upper in [a, b] {
            let total = upper + lower;
            if total < total_range.0 || total > total_range.1 {
                continue;
            }
            let value = out(upper, lower);
            found.push(Pair {
                upper,
                lower,
                out: value,
                error: (value - wanted) / wanted * 100.0,
            });
        }
    }
    // Smallest error first; equal ratios in other decades: the total
    // nearest to the target.
    found.sort_by(|x, y| {
        let ex = (x.error.abs() * 1e6).round();
        let ey = (y.error.abs() * 1e6).round();
        ex.total_cmp(&ey).then_with(|| {
            let dx = ((x.upper + x.lower) / target_total).ln().abs();
            let dy = ((y.upper + y.lower) / target_total).ln().abs();
            dx.total_cmp(&dy)
        })
    });
    let mut kept: Vec<Pair> = Vec::new();
    for pair in found {
        let ratio = pair.upper / pair.lower;
        if kept
            .iter()
            .all(|k| ((k.upper / k.lower) / ratio - 1.0).abs() > 1e-9)
        {
            kept.push(pair);
        }
        if kept.len() == count {
            break;
        }
    }
    kept
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DividerParams {
    /// Input voltage.
    vin: f64,
    /// Wanted output voltage.
    vout: f64,
    #[serde(default = "e24")]
    series: Series,
    /// Range of R1 + R2 (ohms).
    #[serde(default = "default_min_total")]
    min_total: f64,
    #[serde(default = "default_max_total")]
    max_total: f64,
    #[serde(default = "default_count")]
    count: usize,
}

const fn default_min_total() -> f64 {
    1e3
}
const fn default_max_total() -> f64 {
    1e6
}
const fn default_count() -> usize {
    8
}

fn divider(params: &DividerParams) -> Result<Value, EngineError> {
    let vin = positive(params.vin, "vin")?;
    let vout = positive(params.vout, "vout")?;
    if vout >= vin {
        return Err(invalid("`vout` must be below `vin`"));
    }
    let (min, max) = (
        positive(params.min_total, "minTotal")?,
        positive(params.max_total, "maxTotal")?,
    );
    if min >= max {
        return Err(invalid("`minTotal` must be below `maxTotal`"));
    }
    let ratio = vin / vout - 1.0;
    let lowers = values_between(params.series, min / (1.0 + ratio) / 1.5, max);
    let search = Search {
        series: params.series,
        wanted: vout,
        target_total: (min * max).sqrt(),
        total_range: (min, max),
        count: params.count.clamp(1, 50),
    };
    let found = pairs(
        &search,
        &lowers,
        |r2| r2 * ratio,
        |r1, r2| vin * r2 / (r1 + r2),
    );
    let list: Vec<Value> = found
        .iter()
        .map(|p| {
            let total = p.upper + p.lower;
            json!({
                "r1": p.upper,
                "r2": p.lower,
                "vout": p.out,
                "error": p.error,
                "current": vin / total,
                "power": vin * vin / total,
            })
        })
        .collect();
    Ok(json!({"ratio": ratio, "pairs": list}))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RegulatorParams {
    /// Reference voltage of the regulator (1.25 V for an LM317).
    vref: f64,
    /// Wanted output voltage.
    vout: f64,
    /// Current out of the adjust pin (50 µA for an LM317; 0 for most).
    #[serde(default)]
    iadj: f64,
    #[serde(default = "e24")]
    series: Series,
    /// Range of the lower resistor (ohms).
    #[serde(default = "default_min_lower")]
    min_lower: f64,
    #[serde(default = "default_max_lower")]
    max_lower: f64,
    #[serde(default = "default_count")]
    count: usize,
}

const fn default_min_lower() -> f64 {
    100.0
}
const fn default_max_lower() -> f64 {
    1e6
}

fn regulator(params: &RegulatorParams) -> Result<Value, EngineError> {
    let vref = positive(params.vref, "vref")?;
    let vout = positive(params.vout, "vout")?;
    if vout <= vref {
        return Err(invalid("`vout` must be above `vref`"));
    }
    if !params.iadj.is_finite() || params.iadj < 0.0 {
        return Err(invalid("`iadj` must not be negative"));
    }
    let (min, max) = (
        positive(params.min_lower, "minLower")?,
        positive(params.max_lower, "maxLower")?,
    );
    if min >= max {
        return Err(invalid("`minLower` must be below `maxLower`"));
    }
    let iadj = params.iadj;
    let lowers = values_between(params.series, min, max);
    let search = Search {
        series: params.series,
        wanted: vout,
        target_total: (min * max).sqrt() * 2.0,
        total_range: (0.0, f64::INFINITY),
        count: params.count.clamp(1, 50),
    };
    let found = pairs(
        &search,
        &lowers,
        |lower| (vout - vref) / (vref / lower + iadj),
        |upper, lower| vref * (1.0 + upper / lower) + iadj * upper,
    );
    let list: Vec<Value> = found
        .iter()
        .map(|p| {
            json!({
                "upper": p.upper,
                "lower": p.lower,
                "vout": p.out,
                "error": p.error,
                "current": vref / p.lower,
            })
        })
        .collect();
    Ok(json!({"pairs": list}))
}

// --- Current capacity ---------------------------------------------------------

/// Resistivity of copper at 20 °C (Ω·m).
const RHO_COPPER: f64 = 1.724e-8;
/// Temperature coefficient of copper (1/K).
const ALPHA_COPPER: f64 = 0.003_93;
/// Melting point of copper (°C).
const COPPER_MELTS: f64 = 1083.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Standard {
    /// The fit of IPC-2152 figure 5-2 (no modifiers: conservative).
    Ipc2152,
    /// The older IPC-2221 formula, outer or inner layer.
    Ipc2221,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Placement {
    External,
    Internal,
}

/// Cross-section (mil²) that carries `current` at a temperature rise.
fn area_for(standard: Standard, placement: Placement, current: f64, rise: f64) -> f64 {
    match standard {
        Standard::Ipc2152 => {
            (117.555 * rise.powf(-0.913) + 1.15) * current.powf(0.84 * rise.powf(-0.108) + 1.159)
        }
        Standard::Ipc2221 => {
            let k = if placement == Placement::External {
                0.048
            } else {
                0.024
            };
            (current / (k * rise.powf(0.44))).powf(1.0 / 0.725)
        }
    }
}

/// Current a cross-section (mil²) carries at a temperature rise.
fn current_for(standard: Standard, placement: Placement, area: f64, rise: f64) -> f64 {
    match standard {
        Standard::Ipc2152 => (area / (117.555 * rise.powf(-0.913) + 1.15))
            .powf(1.0 / (0.84 * rise.powf(-0.108) + 1.159)),
        Standard::Ipc2221 => {
            let k = if placement == Placement::External {
                0.048
            } else {
                0.024
            };
            k * rise.powf(0.44) * area.powf(0.725)
        }
    }
}

/// Resistance (Ω) of copper with a cross-section (mm²) and length (m) at
/// a temperature.
fn resistance(area_mm2: f64, length_m: f64, celsius: f64) -> f64 {
    RHO_COPPER * length_m / (area_mm2 * 1e-6) * (1.0 + ALPHA_COPPER * (celsius - 20.0))
}

/// Notes on the limits of a result.
fn notes(standard: Standard, current: f64, rise: f64) -> Vec<&'static str> {
    let mut out = Vec::new();
    if standard == Standard::Ipc2152 && !(2.0..=100.0).contains(&rise) {
        out.push("riseOutsideFit");
    }
    if standard == Standard::Ipc2152 && current > 30.0 {
        out.push("currentOutsideFit");
    }
    if standard == Standard::Ipc2221 && current > 35.0 {
        out.push("currentOutsideFit");
    }
    out
}

fn nm_to_mil(nm: f64) -> f64 {
    nm / 25_400.0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TrackParams {
    /// Current to carry: gives the width. Or `width`: gives the current.
    #[serde(default)]
    current: Option<f64>,
    #[serde(default)]
    width: Option<f64>,
    /// Copper thickness (default 35 µm, 1 oz).
    #[serde(default = "default_thickness")]
    thickness: f64,
    /// Allowed temperature rise (default 10 °C).
    #[serde(default = "default_rise")]
    rise: f64,
    #[serde(default = "default_ambient")]
    ambient: f64,
    /// Track length for resistance and voltage drop (default 10 mm).
    #[serde(default = "default_length")]
    length: f64,
    #[serde(default = "ipc2152")]
    standard: Standard,
    #[serde(default = "external")]
    placement: Placement,
}

const fn default_thickness() -> f64 {
    35_000.0
}
const fn default_rise() -> f64 {
    10.0
}
const fn default_ambient() -> f64 {
    25.0
}
const fn default_length() -> f64 {
    10_000_000.0
}
const fn ipc2152() -> Standard {
    Standard::Ipc2152
}
const fn external() -> Placement {
    Placement::External
}

fn track(params: &TrackParams) -> Result<Value, EngineError> {
    let thickness = positive(params.thickness, "thickness")?;
    let rise = positive(params.rise, "rise")?;
    let length = positive(params.length, "length")?;
    let (current, width) = match (params.current, params.width) {
        (Some(current), None) => {
            let current = positive(current, "current")?;
            let area = area_for(params.standard, params.placement, current, rise);
            (current, area / nm_to_mil(thickness) * 25_400.0)
        }
        (None, Some(width)) => {
            let width = positive(width, "width")?;
            let area = nm_to_mil(width) * nm_to_mil(thickness);
            (
                current_for(params.standard, params.placement, area, rise),
                width,
            )
        }
        _ => return Err(invalid("give either `current` or `width`")),
    };
    let area_mm2 = width * 1e-6 * thickness * 1e-6;
    let ohms = resistance(area_mm2, length * 1e-9, params.ambient + rise);
    Ok(json!({
        "current": current,
        "width": width,
        "area": area_mm2,
        "resistance": ohms,
        "voltageDrop": current * ohms,
        "power": current * current * ohms,
        "temperature": params.ambient + rise,
        "notes": notes(params.standard, current, rise),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ViaParams {
    /// Finished hole diameter.
    drill: f64,
    /// Plating thickness of the barrel (default 25 µm).
    #[serde(default = "default_plating")]
    plating: f64,
    /// Length of the barrel: the board thickness (default 1.6 mm).
    #[serde(default = "default_board")]
    length: f64,
    #[serde(default = "default_rise")]
    rise: f64,
    #[serde(default = "default_ambient")]
    ambient: f64,
    #[serde(default = "ipc2152")]
    standard: Standard,
}

const fn default_plating() -> f64 {
    25_000.0
}
const fn default_board() -> f64 {
    1_600_000.0
}

fn via(params: &ViaParams) -> Result<Value, EngineError> {
    let drill = positive(params.drill, "drill")?;
    let plating = positive(params.plating, "plating")?;
    let length = positive(params.length, "length")?;
    let rise = positive(params.rise, "rise")?;
    // The plated ring around the finished hole.
    let ring = std::f64::consts::PI * (drill + plating) * plating;
    let ring_in_mils = ring / (25_400.0 * 25_400.0);
    let current = current_for(params.standard, Placement::External, ring_in_mils, rise);
    let ring_in_mm = ring * 1e-12;
    let ohms = resistance(ring_in_mm, length * 1e-9, params.ambient + rise);
    Ok(json!({
        "current": current,
        "area": ring_in_mm,
        "resistance": ohms,
        "voltageDrop": current * ohms,
        "power": current * current * ohms,
        "notes": notes(params.standard, current, rise),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FusingParams {
    /// Track width, or `drill` (with `plating`) for a via.
    #[serde(default)]
    width: Option<f64>,
    #[serde(default = "default_thickness")]
    thickness: f64,
    #[serde(default)]
    drill: Option<f64>,
    #[serde(default = "default_plating")]
    plating: f64,
    /// How long the current flows (default 1 s).
    #[serde(default = "default_time")]
    time: f64,
    #[serde(default = "default_ambient")]
    ambient: f64,
}

const fn default_time() -> f64 {
    1.0
}

/// Onderdonk's equation: the current that melts copper of a cross-section
/// in a time, with no heat leaving (short pulses).
fn fusing(params: &FusingParams) -> Result<Value, EngineError> {
    let time = positive(params.time, "time")?;
    let area_nm2 = match (params.width, params.drill) {
        (Some(width), None) => positive(width, "width")? * positive(params.thickness, "thickness")?,
        (None, Some(drill)) => {
            let plating = positive(params.plating, "plating")?;
            std::f64::consts::PI * (positive(drill, "drill")? + plating) * plating
        }
        _ => return Err(invalid("give either `width` or `drill`")),
    };
    if params.ambient >= COPPER_MELTS {
        return Err(invalid(
            "`ambient` must be below the melting point of copper",
        ));
    }
    let circular_mils = area_nm2 / (25_400.0 * 25_400.0) * 4.0 / std::f64::consts::PI;
    let ambient = params.ambient;
    let current = circular_mils
        * (((COPPER_MELTS - ambient) / (234.0 + ambient) + 1.0).log10() / (33.0 * time)).sqrt();
    let mut notes = Vec::new();
    if time > 10.0 {
        notes.push("longPulse");
    }
    Ok(json!({
        "current": current,
        "area": area_nm2 * 1e-12,
        "notes": notes,
    }))
}

/// Runs a `calc.*` method.
pub(crate) fn call(method: &str, params: &Value) -> Result<Value, EngineError> {
    fn parse<T: for<'de> Deserialize<'de>>(params: &Value) -> Result<T, EngineError> {
        serde_json::from_value(params.clone())
            .map_err(|e| EngineError::invalid_params(e.to_string()))
    }
    match method {
        "calc.eSeries" => e_series(&parse(params)?),
        "calc.divider" => divider(&parse(params)?),
        "calc.regulator" => regulator(&parse(params)?),
        "calc.track" => track(&parse(params)?),
        "calc.via" => via(&parse(params)?),
        "calc.fusing" => fusing(&parse(params)?),
        other => Err(EngineError::new(
            ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}
