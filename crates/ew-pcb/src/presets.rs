//! Board presets: a stack-up and manufacturing constraints that fit a
//! class of board, from home etching to fine-pitch multilayer boards.

use ew_model::pcb::Stackup;
use ew_model::rules::{Constraints, NetClass, Rules};
use ew_model::units::{Nm, um};
use serde::Serialize;

/// A board preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    /// Stable id.
    pub id: &'static str,
    /// English name.
    pub name: &'static str,
    /// English description.
    pub description: &'static str,
    /// Copper layers.
    pub layers: u8,
    /// Board thickness.
    pub thickness: Nm,
    /// Surface finish.
    pub finish: &'static str,
    /// Manufacturing constraints.
    pub constraints: Constraints,
    /// Track width of the `Default` net class.
    pub track_width: Nm,
    /// Clearance of the `Default` net class.
    pub clearance: Nm,
    /// Via diameter of the `Default` net class.
    pub via_diameter: Nm,
    /// Via drill of the `Default` net class.
    pub via_drill: Nm,
}

const fn constraints(
    clearance: Nm,
    track: Nm,
    via_diameter: Nm,
    via_drill: Nm,
    annular: Nm,
    hole: Nm,
) -> Constraints {
    Constraints {
        min_clearance: clearance,
        min_track_width: track,
        min_via_diameter: via_diameter,
        min_via_drill: via_drill,
        min_annular_ring: annular,
        min_hole: hole,
        min_hole_to_hole: um(250),
        min_copper_to_edge: um(300),
        min_silk_width: um(150),
    }
}

/// Every preset.
#[must_use]
pub fn presets() -> Vec<Preset> {
    vec![
        Preset {
            id: "home-1l",
            name: "Home etching, 1 layer",
            description: "Wide tracks and clearances that survive toner transfer and etching at home",
            layers: 1,
            thickness: um(1_600),
            finish: "None",
            constraints: constraints(um(400), um(500), um(1_600), um(800), um(400), um(800)),
            track_width: um(600),
            clearance: um(400),
            via_diameter: um(1_800),
            via_drill: um(800),
        },
        Preset {
            id: "standard-2l",
            name: "Standard, 2 layers",
            description: "The usual low-cost prototype board: 0.2 mm tracks and clearances, 0.3 mm vias",
            layers: 2,
            thickness: um(1_600),
            finish: "HASL",
            constraints: Constraints::default(),
            track_width: um(250),
            clearance: um(200),
            via_diameter: um(600),
            via_drill: um(300),
        },
        Preset {
            id: "standard-4l",
            name: "Standard, 4 layers",
            description: "Four layers with 0.127 mm (5 mil) tracks and clearances, 0.2 mm vias",
            layers: 4,
            thickness: um(1_600),
            finish: "ENIG",
            constraints: constraints(um(127), um(127), um(450), um(200), um(125), um(200)),
            track_width: um(200),
            clearance: um(150),
            via_diameter: um(450),
            via_drill: um(200),
        },
        Preset {
            id: "fine-6l",
            name: "Fine pitch, 6 layers",
            description: "Six layers for fine-pitch parts: 0.1 mm tracks and clearances, 0.15 mm vias",
            layers: 6,
            thickness: um(1_600),
            finish: "ENIG",
            constraints: constraints(um(100), um(100), um(350), um(150), um(100), um(150)),
            track_width: um(127),
            clearance: um(127),
            via_diameter: um(400),
            via_drill: um(200),
        },
        Preset {
            id: "thin-2l",
            name: "Thin, 2 layers (0.8 mm)",
            description: "A thin board for wearables and plug-in modules",
            layers: 2,
            thickness: um(800),
            finish: "ENIG",
            constraints: Constraints::default(),
            track_width: um(200),
            clearance: um(200),
            via_diameter: um(550),
            via_drill: um(250),
        },
    ]
}

/// The preset with this id.
#[must_use]
pub fn preset(id: &str) -> Option<Preset> {
    presets().into_iter().find(|p| p.id == id)
}

impl Preset {
    /// The stack-up of the preset.
    #[must_use]
    pub fn stackup(&self) -> Stackup {
        let mut stackup = Stackup::standard(self.layers, self.thickness);
        self.finish.clone_into(&mut stackup.finish);
        stackup
    }

    /// The rules with the preset's constraints and `Default` class.
    #[must_use]
    pub fn apply_to(&self, rules: &Rules) -> Rules {
        let mut rules = rules.clone();
        rules.constraints = self.constraints;
        let default = NetClass {
            track_width: self.track_width,
            clearance: self.clearance,
            via_diameter: self.via_diameter,
            via_drill: self.via_drill,
            ..NetClass::default_class()
        };
        match rules
            .net_classes
            .iter_mut()
            .find(|c| c.name == ew_model::rules::DEFAULT_NET_CLASS)
        {
            Some(class) => {
                class.track_width = default.track_width;
                class.clearance = default.clearance;
                class.via_diameter = default.via_diameter;
                class.via_drill = default.via_drill;
            }
            None => rules.net_classes.insert(0, default),
        }
        // Other classes never go below the new minimums.
        for class in &mut rules.net_classes {
            class.clearance = class.clearance.max(self.constraints.min_clearance);
            class.track_width = class.track_width.max(self.constraints.min_track_width);
            class.via_drill = class.via_drill.max(self.constraints.min_via_drill);
            class.via_diameter = class
                .via_diameter
                .max(self.constraints.min_via_diameter)
                .max(class.via_drill + 2 * self.constraints.min_annular_ring);
        }
        rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::id::Id;

    #[test]
    fn presets_are_consistent() {
        for preset in presets() {
            let stackup = preset.stackup();
            assert_eq!(stackup.copper_count(), preset.layers, "{}", preset.id);
            assert_eq!(stackup.thickness(), preset.thickness, "{}", preset.id);
            let c = preset.constraints;
            assert!(
                preset.via_diameter >= preset.via_drill + 2 * c.min_annular_ring,
                "{}",
                preset.id
            );
            assert!(preset.track_width >= c.min_track_width && preset.clearance >= c.min_clearance);
            let rules = preset.apply_to(&Rules::new(Id::random(), "Rules"));
            for class in &rules.net_classes {
                assert!(class.clearance >= c.min_clearance, "{}", preset.id);
                assert!(class.via_diameter >= class.via_drill + 2 * c.min_annular_ring);
            }
        }
        assert!(preset("standard-4l").is_some());
    }
}
