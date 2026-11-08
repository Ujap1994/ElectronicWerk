//! The deduplication engine (ADR-0005): finds exact duplicates, near
//! duplicates and aliases among library entries from any source.
//!
//! - **Exact duplicates** are the same object under different names or in
//!   different libraries: footprints with the same pads and graphics,
//!   symbols with the same name and drawing, models with the same mesh and
//!   colours. They are merged by default.
//! - **Near duplicates** match within a tolerance or differ only in details:
//!   footprints with the same pads but other graphics, or pads within
//!   0.05 mm; symbols with the same name and pin numbers but another
//!   drawing; models with the same mesh in other colours, or the same size
//!   and triangle count. They wait for a review.
//! - **Aliases** are different parts that share one drawing (KiCad's derived
//!   symbols, for example `LM358` and `TL072`). They stay separate.
//!
//! The result only depends on the entries, never on the order in which the
//! files were read or on threads, so the report is reproducible.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::entry::{Entry, Kind, Print};
use crate::error::LibraryError;
use crate::fingerprint::{self, pads_close};

/// The class of a duplicate group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Class {
    /// The same object twice.
    Exact,
    /// Equal within a tolerance, or different only in details.
    Near,
    /// Different parts with the same drawing.
    Alias,
}

impl Class {
    /// `exact`, `near` or `alias`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Near => "near",
            Self::Alias => "alias",
        }
    }

    /// Parses [`Class::as_str`].
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "exact" => Some(Self::Exact),
            "near" => Some(Self::Near),
            "alias" => Some(Self::Alias),
            _ => None,
        }
    }

    /// The decision a group gets until someone reviews it.
    #[must_use]
    pub const fn default_decision(self) -> Decision {
        match self {
            Self::Exact => Decision::Merge,
            Self::Near => Decision::Pending,
            Self::Alias => Decision::Separate,
        }
    }
}

/// What happens with a duplicate group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    /// One entry stays visible; the others become its aliases.
    Merge,
    /// All entries stay.
    Separate,
    /// Not reviewed yet (all entries stay).
    Pending,
}

impl Decision {
    /// `merge`, `separate` or `pending`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Separate => "separate",
            Self::Pending => "pending",
        }
    }

    /// Parses [`Decision::as_str`].
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "merge" => Some(Self::Merge),
            "separate" => Some(Self::Separate),
            "pending" => Some(Self::Pending),
            _ => None,
        }
    }
}

/// A group of duplicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// Kind of the members.
    pub kind: Kind,
    /// Class.
    pub class: Class,
    /// Members (entry indices), best first: source order, then name.
    pub members: Vec<usize>,
    /// Why the members belong together.
    pub reason: &'static str,
    /// Stable key: a hash of the kind, class and member names.
    pub key: String,
}

/// A union-find over entry indices.
struct Sets {
    parent: Vec<usize>,
}

impl Sets {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, mut i: usize) -> usize {
        while self.parent[i] != i {
            self.parent[i] = self.parent[self.parent[i]];
            i = self.parent[i];
        }
        i
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.parent[a.max(b)] = a.min(b);
        }
    }
}

/// Sort key of an entry: source order, then library and name.
fn rank(entries: &[Entry], i: usize) -> (usize, &str, &str) {
    let e = &entries[i];
    (e.source, e.library.as_str(), e.name.as_str())
}

fn group(
    entries: &[Entry],
    kind: Kind,
    class: Class,
    reason: &'static str,
    mut members: Vec<usize>,
) -> Group {
    members.sort_by(|&a, &b| rank(entries, a).cmp(&rank(entries, b)));
    members.dedup();
    let names: BTreeSet<String> = members.iter().map(|&i| entries[i].full_name()).collect();
    let key = fingerprint::hash(&format!(
        "{}|{}|{}",
        kind.as_str(),
        class.as_str(),
        names.into_iter().collect::<Vec<_>>().join("\n")
    ));
    Group {
        kind,
        class,
        members,
        reason,
        key,
    }
}

/// Groups of entries with the same key (two or more members).
fn buckets<K: Ord>(items: impl Iterator<Item = (K, usize)>) -> Vec<Vec<usize>> {
    let mut map: BTreeMap<K, Vec<usize>> = BTreeMap::new();
    for (key, index) in items {
        map.entry(key).or_default().push(index);
    }
    map.into_values().filter(|list| list.len() > 1).collect()
}

/// Connected components of `edges` (two or more members), each with the
/// reason of its first edge.
fn components(
    entries: &[Entry],
    edges: &[(usize, usize, &'static str)],
) -> Vec<(Vec<usize>, &'static str)> {
    let mut sets = Sets::new(entries.len());
    for &(a, b, _) in edges {
        sets.join(a, b);
    }
    let mut members: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut reasons: BTreeMap<usize, &'static str> = BTreeMap::new();
    for &(a, b, reason) in edges {
        let root = sets.find(a);
        members.entry(root).or_default().extend([a, b]);
        reasons.entry(root).or_insert(reason);
    }
    members
        .into_iter()
        .map(|(root, set)| (set.into_iter().collect(), reasons[&root]))
        .collect()
}

/// Finds the duplicate groups of all entries. `tolerance` is in µm.
#[must_use]
pub fn find(entries: &[Entry], tolerance: i64) -> Vec<Group> {
    let mut groups = Vec::new();
    footprint_groups(entries, tolerance, &mut groups);
    symbol_groups(entries, &mut groups);
    model_groups(entries, tolerance, &mut groups);
    groups.sort_by(|a, b| {
        (a.kind, a.class, rank(entries, a.members[0]))
            .cmp(&(b.kind, b.class, rank(entries, b.members[0])))
            .then_with(|| a.key.cmp(&b.key))
    });
    groups
}

fn footprint_print(entries: &[Entry], i: usize) -> Option<&fingerprint::FootprintPrint> {
    match &entries[i].print {
        Print::Footprint(print) => Some(print),
        _ => None,
    }
}

fn footprint_groups(entries: &[Entry], tolerance: i64, out: &mut Vec<Group>) {
    let prints: Vec<(usize, &fingerprint::FootprintPrint)> = (0..entries.len())
        .filter_map(|i| footprint_print(entries, i).map(|p| (i, p)))
        .collect();
    for members in buckets(prints.iter().map(|(i, p)| (p.exact.as_str(), *i))) {
        out.push(group(
            entries,
            Kind::Footprint,
            Class::Exact,
            "identical pads and graphics",
            members,
        ));
    }
    // Footprints without pads (logos, scales) only match exactly.
    let prints: Vec<(usize, &fingerprint::FootprintPrint)> = prints
        .into_iter()
        .filter(|(_, p)| !p.canon.is_empty())
        .collect();
    let mut edges = Vec::new();
    for members in buckets(prints.iter().map(|(i, p)| (p.pads.as_str(), *i))) {
        for &other in &members[1..] {
            edges.push((members[0], other, "identical pads, different graphics"));
        }
    }
    // Pads within the tolerance (only footprints with the same pad numbers
    // and kinds can match).
    for bucket in buckets(prints.iter().map(|(i, p)| (p.coarse.as_str(), *i))) {
        let list: Vec<(usize, &fingerprint::FootprintPrint, (i64, i64))> = bucket
            .iter()
            .filter_map(|&i| footprint_print(entries, i).map(|p| (i, p, extent(&p.canon))))
            .collect();
        for (n, (a, pa, ea)) in list.iter().enumerate() {
            for (b, pb, eb) in &list[n + 1..] {
                if pa.pads == pb.pads
                    || (ea.0 - eb.0).abs() > 2 * tolerance
                    || (ea.1 - eb.1).abs() > 2 * tolerance
                {
                    continue;
                }
                if pads_close(&pa.canon, &pb.canon, tolerance) {
                    edges.push((*a, *b, "pads within the tolerance"));
                }
            }
        }
    }
    for (members, reason) in components(entries, &edges) {
        // A component whose members are all identical is no near duplicate.
        let distinct: BTreeSet<&str> = members
            .iter()
            .filter_map(|&i| footprint_print(entries, i).map(|p| p.exact.as_str()))
            .collect();
        if distinct.len() > 1 {
            out.push(group(
                entries,
                Kind::Footprint,
                Class::Near,
                reason,
                members,
            ));
        }
    }
}

/// Width and height of the pad centres, the smaller first (independent of
/// a quarter turn).
fn extent(pads: &[fingerprint::CanonPad]) -> (i64, i64) {
    let (mut x, mut y) = (0, 0);
    for pad in pads {
        x = x.max(pad.x.abs());
        y = y.max(pad.y.abs());
    }
    (x.min(y), x.max(y))
}

fn symbol_groups(entries: &[Entry], out: &mut Vec<Group>) {
    let prints: Vec<(usize, &fingerprint::SymbolPrint, String)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match &e.print {
            Print::Symbol(print) => Some((i, print, e.name.to_lowercase())),
            _ => None,
        })
        .collect();
    for members in buckets(
        prints
            .iter()
            .map(|(i, p, name)| ((p.drawing.as_str(), name.as_str()), *i)),
    ) {
        out.push(group(
            entries,
            Kind::Symbol,
            Class::Exact,
            "same name and drawing",
            members,
        ));
    }
    for members in buckets(prints.iter().map(|(i, p, _)| (p.drawing.as_str(), *i))) {
        let names: BTreeSet<String> = members
            .iter()
            .map(|&i| entries[i].name.to_lowercase())
            .collect();
        if names.len() > 1 {
            out.push(group(
                entries,
                Kind::Symbol,
                Class::Alias,
                "same drawing, different names",
                members,
            ));
        }
    }
    // Different bodies with the same pins are usually different part types
    // (diode, Zener, Schottky), so only symbols with the same name count as
    // near duplicates.
    let mut edges = Vec::new();
    for members in buckets(
        prints
            .iter()
            .map(|(i, p, name)| ((name.as_str(), p.numbers.as_str()), *i)),
    ) {
        for &other in &members[1..] {
            edges.push((
                members[0],
                other,
                "same name and pin numbers, different drawing",
            ));
        }
    }
    for (members, reason) in components(entries, &edges) {
        let drawings: BTreeSet<&str> = members
            .iter()
            .filter_map(|&i| match &entries[i].print {
                Print::Symbol(p) => Some(p.drawing.as_str()),
                _ => None,
            })
            .collect();
        if drawings.len() > 1 {
            out.push(group(entries, Kind::Symbol, Class::Near, reason, members));
        }
    }
}

fn model_groups(entries: &[Entry], tolerance: i64, out: &mut Vec<Group>) {
    let prints: Vec<(usize, &crate::mesh::MeshPrint)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match &e.print {
            Print::Model(print) => Some((i, print)),
            _ => None,
        })
        .collect();
    for members in buckets(prints.iter().map(|(i, p)| (p.colored.as_str(), *i))) {
        out.push(group(
            entries,
            Kind::Model,
            Class::Exact,
            "identical mesh and colours",
            members,
        ));
    }
    let mut edges = Vec::new();
    for members in buckets(prints.iter().map(|(i, p)| (p.geometry.as_str(), *i))) {
        for &other in &members[1..] {
            edges.push((members[0], other, "identical mesh, different colours"));
        }
    }
    for bucket in buckets(prints.iter().map(|(i, p)| (p.triangles, *i))) {
        let list: Vec<(usize, &crate::mesh::MeshPrint)> = bucket
            .iter()
            .filter_map(|&i| match &entries[i].print {
                Print::Model(print) => Some((i, print)),
                _ => None,
            })
            .collect();
        for (n, (a, pa)) in list.iter().enumerate() {
            for (b, pb) in &list[n + 1..] {
                if pa.geometry != pb.geometry
                    && pa
                        .bbox
                        .iter()
                        .zip(&pb.bbox)
                        .all(|(u, v)| (u - v).abs() <= tolerance)
                {
                    edges.push((*a, *b, "same size and triangle count"));
                }
            }
        }
    }
    for (members, reason) in components(entries, &edges) {
        let distinct: BTreeSet<&str> = members
            .iter()
            .filter_map(|&i| match &entries[i].print {
                Print::Model(p) => Some(p.colored.as_str()),
                _ => None,
            })
            .collect();
        if distinct.len() > 1 {
            out.push(group(entries, Kind::Model, Class::Near, reason, members));
        }
    }
}

/// The decisions taken in reviews, by group key. They are kept in
/// `decisions.json` next to the index and survive library updates.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decisions {
    /// File format version.
    pub version: u32,
    /// Decisions by group key.
    pub groups: BTreeMap<String, Choice>,
}

/// One decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    /// The decision.
    pub decision: Decision,
    /// The entry that stays visible when merged (`library:name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical: Option<String>,
}

impl Decisions {
    /// File name next to the index.
    pub const FILE: &'static str = "decisions.json";

    /// Reads the decisions (an empty set if the file does not exist).
    ///
    /// # Errors
    ///
    /// Fails if the file exists but cannot be read or parsed.
    pub fn load(dir: &Path) -> Result<Self, LibraryError> {
        let path = dir.join(Self::FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|error| LibraryError::Format(format!("{}: {error}", path.display()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                version: 1,
                ..Self::default()
            }),
            Err(source) => Err(LibraryError::Io { path, source }),
        }
    }

    /// Writes the decisions atomically.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be written.
    pub fn save(&self, dir: &Path) -> Result<(), LibraryError> {
        let path = dir.join(Self::FILE);
        let temp = dir.join(format!("{}.tmp", Self::FILE));
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        std::fs::write(&temp, text)
            .and_then(|()| std::fs::rename(&temp, &path))
            .map_err(|source| LibraryError::Io { path, source })
    }
}

/// A group in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupReport {
    /// Stable key.
    pub key: String,
    /// Kind.
    pub kind: Kind,
    /// Class.
    pub class: Class,
    /// Why the members belong together.
    pub reason: String,
    /// Members (`library:name`), best first.
    pub members: Vec<String>,
    /// The entry that stays visible when merged.
    pub canonical: String,
    /// The decision.
    pub decision: Decision,
}

/// Numbers per kind in the report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindSummary {
    /// Kind.
    pub kind: Option<Kind>,
    /// Entries.
    pub entries: usize,
    /// Groups of exact duplicates.
    pub exact_groups: usize,
    /// Groups of near duplicates.
    pub near_groups: usize,
    /// Groups of aliases.
    pub alias_groups: usize,
    /// Entries merged into another one.
    pub merged: usize,
    /// Near-duplicate groups that wait for a review.
    pub pending: usize,
}

/// The deduplication report: reproducible, so two builds from the same
/// libraries give the same file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Tolerance for near duplicates in mm.
    pub tolerance: String,
    /// Numbers per kind.
    pub kinds: Vec<KindSummary>,
    /// Every group.
    pub groups: Vec<GroupReport>,
}

impl Report {
    /// The report as pretty JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    /// A Markdown summary with every group that is not an alias.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut out = String::from("# Library deduplication report\n\n");
        let _ = writeln!(
            out,
            "Tolerance for near duplicates: {} mm\n",
            self.tolerance
        );
        out.push_str("| Kind | Entries | Exact groups | Near groups | Alias groups | Merged | Pending review |\n");
        out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: |\n");
        for kind in &self.kinds {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} |",
                kind.kind.map_or("model", Kind::as_str),
                kind.entries,
                kind.exact_groups,
                kind.near_groups,
                kind.alias_groups,
                kind.merged,
                kind.pending
            );
        }
        for class in [Class::Exact, Class::Near] {
            let title = match class {
                Class::Exact => "Exact duplicates",
                _ => "Near duplicates",
            };
            let _ = writeln!(out, "\n## {title}\n");
            for group in self.groups.iter().filter(|g| g.class == class) {
                let _ = writeln!(
                    out,
                    "- **{}** ({}, {}): {}",
                    group.canonical,
                    group.kind.as_str(),
                    group.decision.as_str(),
                    group.reason
                );
                for member in group.members.iter().filter(|m| **m != group.canonical) {
                    let _ = writeln!(out, "  - {member}");
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::model_entry;
    use crate::fingerprint::TOLERANCE_UM;
    use crate::mesh::MeshPrint;

    fn model(name: &str, geometry: &str, colored: &str, bbox: [i64; 6], triangles: u64) -> Entry {
        model_entry(
            "Lib",
            name,
            &format!("Lib.3dshapes/{name}.step"),
            Some((
                format!("mesh-{name}"),
                MeshPrint {
                    geometry: geometry.to_owned(),
                    colored: colored.to_owned(),
                    bbox,
                    triangles,
                },
            )),
            0,
        )
    }

    #[test]
    fn models_are_grouped_by_class_and_reproducibly() {
        let box_a = [0, 0, 0, 2000, 1250, 500];
        let entries = vec![
            model("A", "g1", "c1", box_a, 12),
            model("B", "g1", "c1", box_a, 12),
            model("C", "g1", "c2", box_a, 12),
            model("D", "g2", "c3", [0, 0, 0, 2010, 1250, 500], 12),
            model("E", "g3", "c4", [0, 0, 0, 5000, 5000, 500], 12),
        ];
        let groups = find(&entries, TOLERANCE_UM);
        let names = |g: &Group| {
            g.members
                .iter()
                .map(|&i| entries[i].name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].class, Class::Exact);
        assert_eq!(names(&groups[0]), ["A", "B"]);
        assert_eq!(groups[1].class, Class::Near);
        assert_eq!(names(&groups[1]), ["A", "B", "C", "D"]);
        // The same entries in another order give the same groups.
        let mut reversed = entries.clone();
        reversed.reverse();
        let again = find(&reversed, TOLERANCE_UM);
        let keys = |list: &[Group]| list.iter().map(|g| g.key.clone()).collect::<Vec<_>>();
        assert_eq!(keys(&groups), keys(&again));
    }

    #[test]
    fn decisions_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut decisions = Decisions::load(dir.path()).unwrap();
        assert!(decisions.groups.is_empty());
        decisions.groups.insert(
            "k".to_owned(),
            Choice {
                decision: Decision::Merge,
                canonical: Some("Lib:A".to_owned()),
            },
        );
        decisions.save(dir.path()).unwrap();
        assert_eq!(Decisions::load(dir.path()).unwrap(), decisions);
    }
}
