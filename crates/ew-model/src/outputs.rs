//! Output jobs: which manufacturing files to generate.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::default_true;
use crate::id::Id;

/// Kind of generated output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputKind {
    /// Gerber X2 layer files.
    Gerber,
    /// Excellon drill files.
    Drill,
    /// Drill map drawing.
    DrillMap,
    /// Bill of materials.
    Bom,
    /// Pick-and-place (component placement) file.
    PickAndPlace,
    /// PDF print.
    Pdf,
    /// SVG print.
    Svg,
    /// STEP 3D model.
    Step,
    /// IPC-D-356 netlist.
    Ipc356,
    /// IPC-2581 package.
    Ipc2581,
    /// Netlist.
    Netlist,
}

/// One output in a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputSpec {
    /// Output id.
    pub id: Id,
    /// Display name.
    pub name: String,
    /// Kind.
    pub kind: OutputKind,
    /// Generated when the job runs.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Source document (schematic or board); `None` picks the obvious one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Id>,
    /// Kind-specific options.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, serde_json::Value>,
}

/// An output job document (`.ewout`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputJob {
    /// Document id.
    pub id: Id,
    /// Name.
    pub name: String,
    /// Output folder, relative to the project.
    pub directory: String,
    /// Outputs in run order.
    #[serde(default)]
    pub outputs: Vec<OutputSpec>,
}

/// Editable output job properties (the whole document except its id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputJobProps {
    /// Name.
    pub name: String,
    /// Output folder.
    pub directory: String,
    /// Outputs.
    pub outputs: Vec<OutputSpec>,
}

/// Default output folder.
pub const DEFAULT_OUTPUT_DIRECTORY: &str = "outputs/generated";

impl OutputJob {
    /// An empty job.
    #[must_use]
    pub fn new(id: Id, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            directory: DEFAULT_OUTPUT_DIRECTORY.to_owned(),
            outputs: Vec::new(),
        }
    }

    /// A job with the usual fabrication and assembly outputs. `make_id`
    /// supplies the output ids.
    #[must_use]
    pub fn fabrication(id: Id, name: impl Into<String>, mut make_id: impl FnMut() -> Id) -> Self {
        let outputs = [
            ("Gerber", OutputKind::Gerber),
            ("Drill", OutputKind::Drill),
            ("Bill of materials", OutputKind::Bom),
            ("Pick and place", OutputKind::PickAndPlace),
            ("Schematic PDF", OutputKind::Pdf),
            ("3D model", OutputKind::Step),
        ]
        .into_iter()
        .map(|(name, kind)| OutputSpec {
            id: make_id(),
            name: name.to_owned(),
            kind,
            enabled: true,
            source: None,
            options: BTreeMap::new(),
        })
        .collect();
        Self {
            outputs,
            ..Self::new(id, name)
        }
    }

    /// The editable properties.
    #[must_use]
    pub fn props(&self) -> OutputJobProps {
        OutputJobProps {
            name: self.name.clone(),
            directory: self.directory.clone(),
            outputs: self.outputs.clone(),
        }
    }

    /// Replaces the editable properties.
    pub fn set_props(&mut self, props: OutputJobProps) {
        self.name = props.name;
        self.directory = props.directory;
        self.outputs = props.outputs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabrication_job() {
        let job = OutputJob::fabrication(Id::random(), "Fabrication", Id::random);
        assert_eq!(job.outputs.len(), 6);
        assert_eq!(job.directory, DEFAULT_OUTPUT_DIRECTORY);
        let json = serde_json::to_string(&job).unwrap();
        assert_eq!(serde_json::from_str::<OutputJob>(&json).unwrap(), job);
    }
}
