//! Categories of parts: a tree (passive parts, discrete semiconductors, ICs
//! by function, …) derived from the names of the symbol libraries and, for
//! KiCad's `Device` library, the symbol names. Nothing is stored: the KiCad
//! libraries are named systematically (`MCU_ST_STM32F4`, `Sensor_Humidity`,
//! `Regulator_Linear`), so the category follows from the name.

/// A category: its id (a path like `ic/mcu`) and its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Category {
    /// Path id, `/`-separated (`passive/resistor`).
    pub id: &'static str,
}

/// The fixed tree, parents before their children, in display order.
pub const TREE: &[&str] = &[
    "passive",
    "passive/resistor",
    "passive/capacitor",
    "passive/inductor",
    "passive/crystal",
    "passive/protection",
    "passive/filter",
    "passive/other",
    "discrete",
    "discrete/diode",
    "discrete/led",
    "discrete/transistor",
    "discrete/thyristor",
    "ic",
    "ic/mcu",
    "ic/processor",
    "ic/memory",
    "ic/power",
    "ic/analog",
    "ic/converter",
    "ic/interface",
    "ic/driver",
    "ic/logic",
    "ic/timing",
    "ic/sensor",
    "ic/audio-video",
    "ic/other",
    "rf",
    "connector",
    "connector/header",
    "connector/other",
    "electromechanical",
    "electromechanical/switch",
    "electromechanical/relay",
    "electromechanical/motor",
    "electromechanical/display",
    "electromechanical/audio",
    "electromechanical/battery",
    "module",
    "mechanical",
    "other",
];

/// Libraries that hold no parts to place (power symbols, simulation
/// sources, graphics).
const NOT_PARTS: &[&str] = &["power", "Simulation_SPICE", "Graphic"];

/// Library name prefixes and their category (first match wins).
const LIBRARIES: &[(&str, &str)] = &[
    ("MCU_Module", "module"),
    ("MCU_", "ic/mcu"),
    ("CPU", "ic/processor"),
    ("DSP_", "ic/processor"),
    ("GPU", "ic/processor"),
    ("FPGA_", "ic/processor"),
    ("CPLD_", "ic/processor"),
    ("Memory_", "ic/memory"),
    ("Regulator_", "ic/power"),
    ("Converter_", "ic/power"),
    ("Power_", "ic/power"),
    ("Battery_Management", "ic/power"),
    ("Reference_", "ic/power"),
    ("Amplifier_Audio", "ic/audio-video"),
    ("Amplifier_", "ic/analog"),
    ("Comparator", "ic/analog"),
    ("Analog_ADC", "ic/converter"),
    ("Analog_DAC", "ic/converter"),
    ("Analog", "ic/analog"),
    ("Potentiometer_Digital", "ic/analog"),
    ("Interface", "ic/interface"),
    ("Isolator", "ic/interface"),
    ("Logic_LevelTranslator", "ic/interface"),
    ("Driver_", "ic/driver"),
    ("74x", "ic/logic"),
    ("4xxx", "ic/logic"),
    ("Logic_", "ic/logic"),
    ("Buffer", "ic/logic"),
    ("Timer", "ic/timing"),
    ("Oscillator", "ic/timing"),
    ("Sensor", "ic/sensor"),
    ("Audio", "ic/audio-video"),
    ("Video", "ic/audio-video"),
    ("Security", "ic/other"),
    ("Fiber_Optic", "ic/other"),
    ("RF", "rf"),
    ("Connector_Generic", "connector/header"),
    ("Connector", "connector/other"),
    ("Switch", "electromechanical/switch"),
    ("Relay", "electromechanical/relay"),
    ("Motor", "electromechanical/motor"),
    ("Display_", "electromechanical/display"),
    ("Diode", "discrete/diode"),
    ("LED", "discrete/led"),
    ("Transistor_", "discrete/transistor"),
    ("Triac_Thyristor", "discrete/thyristor"),
    ("Transformer", "passive/inductor"),
    ("Filter", "passive/filter"),
    ("Jumper", "passive/other"),
    ("Mechanical", "mechanical"),
];

/// Symbol name prefixes of KiCad's `Device` library.
const DEVICE: &[(&str, &str)] = &[
    ("R_Pot", "passive/resistor"),
    ("Thermistor", "passive/resistor"),
    ("Varistor", "passive/protection"),
    ("LED", "discrete/led"),
    ("Q_", "discrete/transistor"),
    ("DIAC", "discrete/thyristor"),
    ("Crystal", "passive/crystal"),
    ("Resonator", "passive/crystal"),
    ("Fuse", "passive/protection"),
    ("Polyfuse", "passive/protection"),
    ("CircuitBreaker", "passive/protection"),
    ("GDT", "passive/protection"),
    ("FerriteBead", "passive/inductor"),
    ("Filter_EMI", "passive/filter"),
    ("Transformer", "passive/inductor"),
    ("Battery", "electromechanical/battery"),
    ("Buzzer", "electromechanical/audio"),
    ("Speaker", "electromechanical/audio"),
    ("Earphone", "electromechanical/audio"),
    ("Microphone", "electromechanical/audio"),
    ("Antenna", "rf"),
    ("Motor", "electromechanical/motor"),
    ("Heater", "passive/other"),
    ("R", "passive/resistor"),
    ("C", "passive/capacitor"),
    ("L", "passive/inductor"),
    ("D", "discrete/diode"),
];

/// The category of a symbol, or `None` for symbols that are not parts.
#[must_use]
pub fn of_symbol(library: &str, name: &str) -> Option<&'static str> {
    if NOT_PARTS.contains(&library) {
        return None;
    }
    if library == "Device" {
        // `R`, `R_Small`, `C_Polarized` … but not `Crystal` as `C`.
        let fits = |prefix: &str| {
            name == prefix
                || name
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('_') || prefix.len() > 1)
        };
        return Some(
            DEVICE
                .iter()
                .find(|(prefix, _)| fits(prefix))
                .map_or("passive/other", |(_, category)| category),
        );
    }
    Some(
        LIBRARIES
            .iter()
            .find(|(prefix, _)| library.starts_with(prefix))
            .map_or("other", |(_, category)| category),
    )
}

/// The vendor of a microcontroller library (`MCU_ST_STM32F4` → `ST`), the
/// third level below `ic/mcu`.
#[must_use]
pub fn mcu_vendor(library: &str) -> Option<&str> {
    let rest = library.strip_prefix("MCU_")?;
    let vendor = rest.split('_').next()?;
    (!vendor.is_empty() && vendor != "Module").then_some(vendor)
}

/// The full category path of a symbol, with the vendor of a
/// microcontroller (`ic/mcu/ST`).
#[must_use]
pub fn path_of_symbol(library: &str, name: &str) -> Option<String> {
    let category = of_symbol(library, name)?;
    Some(match (category, mcu_vendor(library)) {
        ("ic/mcu", Some(vendor)) => format!("ic/mcu/{vendor}"),
        _ => category.to_owned(),
    })
}

/// Whether a category path lies in `filter` (the same or below it).
#[must_use]
pub fn within(path: &str, filter: &str) -> bool {
    path == filter
        || path
            .strip_prefix(filter)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_get_their_category() {
        assert_eq!(of_symbol("Device", "R"), Some("passive/resistor"));
        assert_eq!(of_symbol("Device", "R_Small"), Some("passive/resistor"));
        assert_eq!(
            of_symbol("Device", "C_Polarized"),
            Some("passive/capacitor")
        );
        assert_eq!(of_symbol("Device", "Crystal"), Some("passive/crystal"));
        assert_eq!(of_symbol("Device", "LED_RGB"), Some("discrete/led"));
        assert_eq!(of_symbol("Device", "D_Schottky"), Some("discrete/diode"));
        assert_eq!(
            of_symbol("Device", "Q_NPN_BCE"),
            Some("discrete/transistor")
        );
        assert_eq!(
            of_symbol("Device", "FerriteBead_Small"),
            Some("passive/inductor")
        );
        assert_eq!(
            of_symbol("Regulator_Linear", "AMS1117-3.3"),
            Some("ic/power")
        );
        assert_eq!(
            of_symbol("Amplifier_Audio", "LM386"),
            Some("ic/audio-video")
        );
        assert_eq!(
            of_symbol("Connector_Generic", "Conn_01x04"),
            Some("connector/header")
        );
        assert_eq!(
            of_symbol("Connector", "USB_C_Receptacle"),
            Some("connector/other")
        );
        assert_eq!(of_symbol("power", "GND"), None);
        assert_eq!(of_symbol("Simulation_SPICE", "VDC"), None);
        assert_eq!(
            path_of_symbol("MCU_ST_STM32F4", "STM32F401CCUx").as_deref(),
            Some("ic/mcu/ST")
        );
        assert_eq!(
            path_of_symbol("MCU_Module", "Arduino_Nano_v3.x").as_deref(),
            Some("module")
        );
        assert!(within("ic/mcu/ST", "ic"));
        assert!(within("ic/mcu/ST", "ic/mcu"));
        assert!(!within("ic/mcus", "ic/mcu"));
    }

    #[test]
    fn every_category_has_its_parent_before_it() {
        for (index, id) in TREE.iter().enumerate() {
            if let Some((parent, _)) = id.rsplit_once('/') {
                let at = TREE.iter().position(|p| *p == parent);
                assert!(at.is_some_and(|at| at < index), "{id}");
            }
        }
        for (_, category) in LIBRARIES.iter().chain(DEVICE) {
            assert!(TREE.contains(category), "{category}");
        }
    }
}
