//! The `pcb.*` methods of the layout editor: design rules check, zone
//! filling, ratsnest and the design rules. The logic lives in `ew-pcb`;
//! this module finds the documents and turns results into change requests.

use std::collections::{BTreeMap, HashMap};

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::pcb::Pcb;
use ew_model::project::Project;
use ew_model::rules::{DRC_CHECKS, DrcSettings, ErcSeverity, Rules, RulesProps};
use ew_pcb::drc::{self, Report};
use ew_pcb::geom::{BBox, V};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::change::Request;
use crate::error::{EngineError, ErrorCode};

/// History labels of the board methods.
pub mod labels {
    /// `pcb.fillZones`.
    pub const FILL: &str = "fill-zones";
    /// `pcb.unfillZones`.
    pub const UNFILL: &str = "unfill-zones";
    /// `pcb.setRules`.
    pub const RULES: &str = "design-rules";
    /// `pcb.exclude`.
    pub const EXCLUDE: &str = "drc-exclusions";
}

/// What a method does.
pub enum Outcome {
    /// A reply without changes.
    Read(Value),
    /// Requests to commit as one undo step, and the reply to add to the
    /// commit result.
    Commit {
        /// History label.
        label: &'static str,
        /// Requests.
        requests: Vec<Request>,
        /// Extra reply fields.
        reply: Value,
    },
}

/// The last design rules check of every board, for online checks of a
/// region.
#[derive(Debug, Default)]
pub struct DrcCache {
    reports: HashMap<Id, Report>,
}

impl DrcCache {
    /// Forgets every report (a project was opened or closed).
    pub fn clear(&mut self) {
        self.reports.clear();
    }
}

fn parse<T: DeserializeOwned>(params: &Value) -> Result<T, EngineError> {
    let value = if params.is_null() {
        Value::Object(Map::new())
    } else {
        params.clone()
    };
    serde_json::from_value(value).map_err(|error| EngineError::invalid_params(error.to_string()))
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(EngineError::internal)
}

/// The board with this id.
fn board(project: &Project, doc: Id) -> Result<&Pcb, EngineError> {
    match project.document(doc) {
        Some(Document::Pcb(pcb)) => Ok(pcb),
        Some(_) => Err(EngineError::invalid_params(format!("{doc} is not a board"))),
        None => Err(EngineError::not_found(format!("no document {doc}"))),
    }
}

/// The first rules document of the project.
fn rules(project: &Project) -> Option<(Id, &Rules)> {
    let entry = project.documents_of(DocumentKind::Rules).next()?;
    match project.document(entry.id) {
        Some(Document::Rules(rules)) => Some((entry.id, rules)),
        _ => None,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DrcParams {
    doc: Id,
    /// Only check this area (`[x0, y0, x1, y1]` in nm), merged with the
    /// previous report of the board.
    #[serde(default)]
    region: Option<[i64; 4]>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ZonesParams {
    doc: Id,
    #[serde(default)]
    zones: Option<Vec<Id>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocParams {
    #[serde(default)]
    doc: Option<Id>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetRulesParams {
    props: RulesProps,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValidateParams {
    condition: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExcludeParams {
    doc: Id,
    keys: Vec<String>,
    #[serde(default = "yes")]
    excluded: bool,
}

const fn yes() -> bool {
    true
}

/// Runs a `pcb.*` method.
///
/// # Errors
///
/// Unknown documents, invalid parameters, or an unknown method.
#[allow(clippy::too_many_lines, reason = "one match over the methods")]
pub fn call(
    project: &Project,
    method: &str,
    params: &Value,
    cache: &mut DrcCache,
) -> Result<Outcome, EngineError> {
    match method {
        "pcb.drc" => {
            let params: DrcParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let rules = rules(project).map(|(_, r)| r);
            #[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
            let region = params.region.map(|[x0, y0, x1, y1]| {
                BBox::of(V::new(x0 as f64, y0 as f64)).with(V::new(x1 as f64, y1 as f64))
            });
            let report = match (region, cache.reports.get(&params.doc)) {
                (Some(region), Some(previous)) => {
                    let fresh = drc::check(
                        pcb,
                        rules,
                        &drc::Options {
                            region: Some(region),
                            ..drc::Options::default()
                        },
                    );
                    let reach = ew_pcb::rules::RuleSet::new(rules).max_clearance();
                    drc::merge(previous, fresh, region, reach)
                }
                _ => drc::check(pcb, rules, &drc::Options::default()),
            };
            let value = to_value(&report)?;
            cache.reports.insert(params.doc, report);
            Ok(Outcome::Read(value))
        }
        "pcb.ratsnest" => {
            let params: DocParams = parse(params)?;
            let doc = params
                .doc
                .ok_or_else(|| EngineError::invalid_params("missing doc"))?;
            let pcb = board(project, doc)?;
            let board = ew_pcb::items::Board::new(pcb);
            let connectivity = ew_pcb::connectivity::analyze(&board);
            Ok(Outcome::Read(json!({
                "airwires": connectivity.airwires,
                "unrouted": connectivity.airwires.len(),
            })))
        }
        "pcb.fillZones" | "pcb.unfillZones" => {
            let params: ZonesParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let fill = method == "pcb.fillZones";
            let fills: Vec<(Id, Vec<ew_model::pcb::FilledArea>)> = if fill {
                ew_pcb::fill::fill(pcb, rules(project).map(|(_, r)| r), params.zones.as_deref())
                    .into_iter()
                    .map(|filled| (filled.zone, filled.areas))
                    .collect()
            } else {
                pcb.zones
                    .iter()
                    .filter(|z| params.zones.as_ref().is_none_or(|ids| ids.contains(&z.id)))
                    .map(|z| (z.id, Vec::new()))
                    .collect()
            };
            let mut requests = Vec::new();
            let mut areas = 0;
            for (id, filled) in fills {
                let Some(zone) = pcb.zones.get(id) else {
                    continue;
                };
                if zone.filled == filled {
                    continue;
                }
                areas += filled.len();
                let mut zone = zone.clone();
                zone.filled = filled;
                requests.push(Request::Update {
                    doc: params.doc,
                    object: Object::Zone(zone),
                });
            }
            let zones = requests.len();
            if requests.is_empty() {
                return Ok(Outcome::Read(json!({"seq": null, "zones": 0, "areas": 0})));
            }
            Ok(Outcome::Commit {
                label: if fill { labels::FILL } else { labels::UNFILL },
                requests,
                reply: json!({"zones": zones, "areas": areas}),
            })
        }
        "pcb.rules" => {
            let params: DocParams = parse(params)?;
            Ok(Outcome::Read(rules_info(project, params.doc)?))
        }
        "pcb.setRules" => {
            let params: SetRulesParams = parse(params)?;
            let Value::Object(props) = to_value(&params.props)? else {
                return Err(EngineError::internal("rules are not an object"));
            };
            let requests = if let Some((doc, _)) = rules(project) {
                vec![Request::SetProps { doc, props }]
            } else {
                let doc = Id::random();
                vec![
                    Request::AddDocument {
                        kind: DocumentKind::Rules,
                        name: params.props.name.clone(),
                        path: None,
                        id: Some(doc),
                    },
                    Request::SetProps { doc, props },
                ]
            };
            Ok(Outcome::Commit {
                label: labels::RULES,
                requests,
                reply: json!({}),
            })
        }
        "pcb.validateRule" => {
            let params: ValidateParams = parse(params)?;
            Ok(Outcome::Read(
                match ew_pcb::rules::validate(&params.condition) {
                    None => json!({"ok": true}),
                    Some((message, position)) => {
                        json!({"ok": false, "message": message, "position": position})
                    }
                },
            ))
        }
        "pcb.exclude" => {
            let params: ExcludeParams = parse(params)?;
            let pcb = board(project, params.doc)?;
            let mut list = pcb.drc_exclusions.clone();
            if params.excluded {
                for key in params.keys {
                    if !list.contains(&key) {
                        list.push(key);
                    }
                }
            } else {
                list.retain(|key| !params.keys.contains(key));
            }
            list.sort();
            if let Some(report) = cache.reports.get_mut(&params.doc) {
                for finding in &mut report.findings {
                    finding.excluded = list.contains(&finding.key);
                }
            }
            Ok(Outcome::Commit {
                label: labels::EXCLUDE,
                requests: vec![Request::SetProps {
                    doc: params.doc,
                    props: Map::from_iter([("drcExclusions".to_owned(), json!(list))]),
                }],
                reply: json!({}),
            })
        }
        other => Err(EngineError::new(
            ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

/// The design rules with what the editors need around them: the checks
/// with their severities and, for a board, which nets fall into which
/// class.
fn rules_info(project: &Project, doc: Option<Id>) -> Result<Value, EngineError> {
    let found = rules(project);
    let defaults = Rules::new(Id::from_u128(0), "Design rules");
    let (rules_doc, rules) = found.map_or((None, &defaults), |(id, r)| (Some(id), r));
    let settings: &DrcSettings = &rules.drc;
    let checks: Vec<Value> = DRC_CHECKS
        .iter()
        .map(|(code, default)| {
            json!({
                "code": code,
                "default": default,
                "severity": settings.severity(code),
            })
        })
        .collect();
    let mut classes: BTreeMap<String, Vec<String>> = rules
        .net_classes
        .iter()
        .map(|class| (class.name.clone(), Vec::new()))
        .collect();
    if let Some(doc) = doc {
        let pcb = board(project, doc)?;
        let set = ew_pcb::rules::RuleSet::new(Some(rules));
        for net in &pcb.nets {
            let class = set.class_name(&net.name, &net.class);
            classes.entry(class).or_default().push(net.name.clone());
        }
        for nets in classes.values_mut() {
            nets.sort_by(|a, b| ew_schematic::connectivity::natural_cmp(a, b));
        }
    }
    let errors = ew_pcb::rules::RuleSet::new(Some(rules)).errors;
    Ok(json!({
        "doc": rules_doc,
        "rules": rules.props(),
        "checks": checks,
        "nets": classes,
        "errors": errors,
        "severities": [ErcSeverity::Error, ErcSeverity::Warning, ErcSeverity::Ignore],
    }))
}
