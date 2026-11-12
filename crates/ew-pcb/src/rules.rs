//! Design rules: net classes, board constraints and custom rules written in
//! a small query language.
//!
//! A condition compares properties of the item `A` (and, for rules between
//! two items, `B`) with values:
//!
//! ```text
//! A.netclass == 'HV' && B.netclass != 'HV'
//! A.type == 'via' && A.layer == 'cu.in*'
//! A.net == 'USB_D?' || A.insideZone('RF')
//! A.width < 0.2mm
//! ```
//!
//! Properties: `type` (`pad`, `track`, `via`, `zone`), `net`, `netclass`,
//! `layer`, `designator`, `footprint`, `pad` (pad number), `side`, `width`,
//! `diameter`, `drill` (numbers in mm). Strings compare with `*` and `?`
//! wildcards; numbers take the units `mm`, `mil`, `um` and `in`. The
//! function `insideZone('name')` is true for items inside a named zone.

use std::collections::HashMap;
use std::fmt;

use ew_model::id::Id;
use ew_model::layer::CopperLayer;
use ew_model::rules::{Constraints, CustomRule, NetClass, Rules, matches_pattern};
use ew_model::units::Nm;
use serde::Serialize;

use crate::geom::V;

/// A syntax error in a rule condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleError {
    /// The rule.
    pub rule: String,
    /// What is wrong.
    pub message: String,
    /// Character position in the condition.
    pub position: usize,
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at {}): {}", self.rule, self.position, self.message)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Str(String),
    Num(f64),
    Op(&'static str),
    Dot,
    Comma,
    Open,
    Close,
}

fn tokenize(text: &str) -> Result<Vec<(Token, usize)>, (String, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
        let op = ["==", "!=", "<=", ">=", "&&", "||"]
            .into_iter()
            .find(|op| *op == two);
        if let Some(op) = op {
            out.push((Token::Op(op), start));
            i += 2;
            continue;
        }
        match c {
            '<' => out.push((Token::Op("<"), start)),
            '>' => out.push((Token::Op(">"), start)),
            '!' => out.push((Token::Op("!"), start)),
            '.' if !chars.get(i + 1).is_some_and(char::is_ascii_digit) => {
                out.push((Token::Dot, start));
            }
            ',' => out.push((Token::Comma, start)),
            '(' => out.push((Token::Open, start)),
            ')' => out.push((Token::Close, start)),
            '\'' | '"' => {
                let quote = c;
                i += 1;
                let mut value = String::new();
                while i < chars.len() && chars[i] != quote {
                    value.push(chars[i]);
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(("unclosed string".into(), start));
                }
                out.push((Token::Str(value), start));
            }
            _ if c.is_ascii_digit() || c == '.' => {
                let mut number = String::new();
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    number.push(chars[i]);
                    i += 1;
                }
                let mut unit = String::new();
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    unit.push(chars[i]);
                    i += 1;
                }
                let value: f64 = number
                    .parse()
                    .map_err(|_| (format!("bad number `{number}`"), start))?;
                let factor = match unit.as_str() {
                    "" | "mm" => 1.0,
                    "mil" | "mils" => 0.0254,
                    "um" => 0.001,
                    "in" => 25.4,
                    _ => return Err((format!("unknown unit `{unit}`"), start)),
                };
                out.push((Token::Num(value * factor), start));
                continue;
            }
            _ if c.is_alphabetic() || c == '_' => {
                let mut name = String::new();
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    name.push(chars[i]);
                    i += 1;
                }
                out.push((Token::Ident(name), start));
                continue;
            }
            _ => return Err((format!("unexpected `{c}`"), start)),
        }
        i += 1;
    }
    Ok(out)
}

/// Which item a property belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    A,
    B,
}

/// A property of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prop {
    Type,
    Net,
    NetClass,
    Layer,
    Designator,
    Footprint,
    Pad,
    Side,
    Width,
    Diameter,
    Drill,
}

impl Prop {
    fn parse(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "type" => Self::Type,
            "net" => Self::Net,
            "netclass" => Self::NetClass,
            "layer" => Self::Layer,
            "designator" | "reference" => Self::Designator,
            "footprint" => Self::Footprint,
            "pad" => Self::Pad,
            "side" => Self::Side,
            "width" => Self::Width,
            "diameter" => Self::Diameter,
            "drill" => Self::Drill,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Bool(bool),
    Str(String),
    Num(f64),
    Prop(Who, Prop),
    InsideZone(Who, String),
    Not(Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Cmp(&'static str, Box<Self>, Box<Self>),
}

struct Parser {
    tokens: Vec<(Token, usize)>,
    pos: usize,
    end: usize,
}

type Parsed = Result<Expr, (String, usize)>;

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn at(&self) -> usize {
        self.tokens.get(self.pos).map_or(self.end, |(_, p)| *p)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).map(|(t, _)| t.clone());
        self.pos += 1;
        token
    }

    fn expect(&mut self, token: &Token, what: &str) -> Result<(), (String, usize)> {
        let at = self.at();
        if self.next().as_ref() == Some(token) {
            Ok(())
        } else {
            Err((format!("expected {what}"), at))
        }
    }

    fn or(&mut self) -> Parsed {
        let mut left = self.and()?;
        while self.peek() == Some(&Token::Op("||")) {
            self.pos += 1;
            left = Expr::Or(Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Parsed {
        let mut left = self.not()?;
        while self.peek() == Some(&Token::Op("&&")) {
            self.pos += 1;
            left = Expr::And(Box::new(left), Box::new(self.not()?));
        }
        Ok(left)
    }

    fn not(&mut self) -> Parsed {
        if self.peek() == Some(&Token::Op("!")) {
            self.pos += 1;
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        self.compare()
    }

    fn compare(&mut self) -> Parsed {
        let left = self.primary()?;
        if let Some(Token::Op(op)) = self.peek() {
            let op = *op;
            if ["==", "!=", "<", "<=", ">", ">="].contains(&op) {
                self.pos += 1;
                let right = self.primary()?;
                return Ok(Expr::Cmp(op, Box::new(left), Box::new(right)));
            }
        }
        Ok(left)
    }

    fn primary(&mut self) -> Parsed {
        let at = self.at();
        match self.next() {
            Some(Token::Open) => {
                let inner = self.or()?;
                self.expect(&Token::Close, "`)`")?;
                Ok(inner)
            }
            Some(Token::Str(s)) => Ok(Expr::Str(s)),
            Some(Token::Num(n)) => Ok(Expr::Num(n)),
            Some(Token::Ident(name)) => {
                match name.as_str() {
                    "true" => return Ok(Expr::Bool(true)),
                    "false" => return Ok(Expr::Bool(false)),
                    _ => {}
                }
                let who = match name.as_str() {
                    "A" | "a" => Who::A,
                    "B" | "b" => Who::B,
                    _ => return Err((format!("unknown name `{name}` (use A. or B.)"), at)),
                };
                self.expect(&Token::Dot, "`.` after the item")?;
                let at = self.at();
                let Some(Token::Ident(prop)) = self.next() else {
                    return Err(("expected a property".into(), at));
                };
                if self.peek() == Some(&Token::Open) {
                    self.pos += 1;
                    if !prop.eq_ignore_ascii_case("insideZone") {
                        return Err((format!("unknown function `{prop}`"), at));
                    }
                    let arg_at = self.at();
                    let Some(Token::Str(zone)) = self.next() else {
                        return Err(("expected a zone name in quotes".into(), arg_at));
                    };
                    self.expect(&Token::Close, "`)`")?;
                    return Ok(Expr::InsideZone(who, zone));
                }
                let property =
                    Prop::parse(&prop).ok_or_else(|| (format!("unknown property `{prop}`"), at))?;
                Ok(Expr::Prop(who, property))
            }
            Some(token) => Err((format!("unexpected {token:?}"), at)),
            None => Err(("unexpected end of the condition".into(), at)),
        }
    }
}

/// A parsed condition.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    expr: Expr,
    mentions_b: bool,
    mentions_layer: bool,
}

impl Condition {
    /// Parses a condition (empty: always true).
    ///
    /// # Errors
    ///
    /// A message and the character position of the syntax error.
    pub fn parse(text: &str) -> Result<Self, (String, usize)> {
        if text.trim().is_empty() {
            return Ok(Self {
                expr: Expr::Bool(true),
                mentions_b: false,
                mentions_layer: false,
            });
        }
        let tokens = tokenize(text)?;
        let mut parser = Parser {
            tokens,
            pos: 0,
            end: text.chars().count(),
        };
        let expr = parser.or()?;
        if parser.pos < parser.tokens.len() {
            return Err(("unexpected text after the condition".into(), parser.at()));
        }
        let mut mentions_b = false;
        let mut mentions_layer = false;
        visit(&expr, &mut |e| match e {
            Expr::Prop(who, prop) => {
                mentions_b |= *who == Who::B;
                mentions_layer |= *prop == Prop::Layer;
            }
            Expr::InsideZone(who, _) => mentions_b |= *who == Who::B,
            _ => {}
        });
        Ok(Self {
            expr,
            mentions_b,
            mentions_layer,
        })
    }

    /// `true` if the condition talks about a second item.
    #[must_use]
    pub const fn is_pairwise(&self) -> bool {
        self.mentions_b
    }

    /// Evaluates the condition.
    #[must_use]
    pub fn matches(&self, a: &Props<'_>, b: Option<&Props<'_>>, zones: &dyn ZoneTest) -> bool {
        truthy(&eval(&self.expr, a, b, zones))
    }
}

fn visit(expr: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(expr);
    match expr {
        Expr::Not(inner) => visit(inner, f),
        Expr::And(l, r) | Expr::Or(l, r) | Expr::Cmp(_, l, r) => {
            visit(l, f);
            visit(r, f);
        }
        _ => {}
    }
}

/// The properties of one item, as rules see them.
#[derive(Debug, Clone, Default)]
pub struct Props<'a> {
    /// `pad`, `track`, `via` or `zone`.
    pub kind: &'a str,
    /// Net name (empty without a net).
    pub net: &'a str,
    /// Net class.
    pub netclass: &'a str,
    /// The copper layer being checked.
    pub layer: Option<CopperLayer>,
    /// Designator of the footprint (pads).
    pub designator: &'a str,
    /// Footprint name (pads).
    pub footprint: &'a str,
    /// Pad number.
    pub pad: &'a str,
    /// `top` or `bottom` (pads).
    pub side: &'a str,
    /// Track width in nm.
    pub width: Option<f64>,
    /// Via or pad diameter in nm.
    pub diameter: Option<f64>,
    /// Drill in nm.
    pub drill: Option<f64>,
    /// A point of the item (for `insideZone`).
    pub at: V,
}

/// Answers `insideZone` for the rules.
pub trait ZoneTest {
    /// `true` if `p` lies inside the zone named `name`.
    fn inside(&self, name: &str, p: V) -> bool;
}

/// No zones.
pub struct NoZones;

impl ZoneTest for NoZones {
    fn inside(&self, _name: &str, _p: V) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Bool(bool),
    Str(String),
    Num(f64),
    Missing,
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::Str(s) => !s.is_empty(),
        Value::Num(n) => *n != 0.0,
        Value::Missing => false,
    }
}

fn prop(props: &Props<'_>, prop: Prop) -> Value {
    let text = |s: &str| Value::Str(s.to_owned());
    let mm = |n: Option<f64>| n.map_or(Value::Missing, |n| Value::Num(n / 1e6));
    match prop {
        Prop::Type => text(props.kind),
        Prop::Net => text(props.net),
        Prop::NetClass => text(props.netclass),
        Prop::Layer => props
            .layer
            .map_or(Value::Missing, |layer| Value::Str(layer.to_string())),
        Prop::Designator => text(props.designator),
        Prop::Footprint => text(props.footprint),
        Prop::Pad => text(props.pad),
        Prop::Side => text(props.side),
        Prop::Width => mm(props.width),
        Prop::Diameter => mm(props.diameter),
        Prop::Drill => mm(props.drill),
    }
}

fn eval(expr: &Expr, a: &Props<'_>, b: Option<&Props<'_>>, zones: &dyn ZoneTest) -> Value {
    let item = |who: Who| match who {
        Who::A => Some(a),
        Who::B => b,
    };
    match expr {
        Expr::Bool(v) => Value::Bool(*v),
        Expr::Str(s) => Value::Str(s.clone()),
        Expr::Num(n) => Value::Num(*n),
        Expr::Prop(who, p) => item(*who).map_or(Value::Missing, |props| prop(props, *p)),
        Expr::InsideZone(who, name) => {
            Value::Bool(item(*who).is_some_and(|props| zones.inside(name, props.at)))
        }
        Expr::Not(inner) => Value::Bool(!truthy(&eval(inner, a, b, zones))),
        Expr::And(l, r) => {
            Value::Bool(truthy(&eval(l, a, b, zones)) && truthy(&eval(r, a, b, zones)))
        }
        Expr::Or(l, r) => {
            Value::Bool(truthy(&eval(l, a, b, zones)) || truthy(&eval(r, a, b, zones)))
        }
        Expr::Cmp(op, l, r) => {
            let (left, right) = (eval(l, a, b, zones), eval(r, a, b, zones));
            Value::Bool(compare(op, &left, &right))
        }
    }
}

fn compare(op: &str, left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Num(l), Value::Num(r)) => match op {
            "==" => (l - r).abs() < 1e-9,
            "!=" => (l - r).abs() >= 1e-9,
            "<" => l < r,
            "<=" => l <= r,
            ">" => l > r,
            ">=" => l >= r,
            _ => false,
        },
        (Value::Str(l), Value::Str(r)) => {
            // A literal on either side may hold wildcards.
            let equal = matches_pattern(r, l) || matches_pattern(l, r);
            match op {
                "==" => equal,
                "!=" => !equal,
                "<" => l < r,
                "<=" => l <= r,
                ">" => l > r,
                ">=" => l >= r,
                _ => false,
            }
        }
        (Value::Bool(l), Value::Bool(r)) => match op {
            "==" => l == r,
            "!=" => l != r,
            _ => false,
        },
        _ => op == "!=",
    }
}

/// Where a constraint value comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "name", rename_all = "camelCase")]
pub enum Source {
    /// The board-wide minimum.
    Board,
    /// A net class.
    NetClass(String),
    /// A custom rule.
    Rule(String),
    /// A pad's own clearance.
    Pad,
    /// A zone's own clearance.
    Zone,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Board => f.write_str("board constraints"),
            Self::NetClass(name) => write!(f, "net class {name}"),
            Self::Rule(name) => write!(f, "rule {name}"),
            Self::Pad => f.write_str("pad clearance"),
            Self::Zone => f.write_str("zone clearance"),
        }
    }
}

/// A resolved limit.
#[derive(Debug, Clone, PartialEq)]
pub struct Limit {
    /// The value in nm.
    pub value: f64,
    /// Where it comes from.
    pub source: Source,
}

/// The limits for a track.
#[derive(Debug, Clone, PartialEq)]
pub struct WidthLimits {
    /// Minimum width.
    pub min: Limit,
    /// Maximum width, if a rule sets one.
    pub max: Option<Limit>,
    /// Width for new tracks.
    pub preferred: f64,
}

/// The limits for a via or a hole.
#[derive(Debug, Clone, PartialEq)]
pub struct ViaLimits {
    /// Minimum pad diameter.
    pub diameter: Limit,
    /// Minimum drill.
    pub drill: Limit,
    /// Minimum annular ring.
    pub annular: Limit,
    /// Minimum hole.
    pub hole: Limit,
    /// Diameter for new vias.
    pub preferred_diameter: f64,
    /// Drill for new vias.
    pub preferred_drill: f64,
}

#[allow(clippy::cast_precision_loss, reason = "nm values far below 2^52")]
const fn f(value: Nm) -> f64 {
    value as f64
}

struct Compiled {
    rule: CustomRule,
    condition: Condition,
}

/// The design rules of a project, ready for checks.
pub struct RuleSet {
    /// Board-wide constraints.
    pub constraints: Constraints,
    classes: Vec<NetClass>,
    assignments: Vec<ew_model::rules::NetClassAssignment>,
    custom: Vec<Compiled>,
    /// Syntax errors in custom rules (those rules are skipped).
    pub errors: Vec<RuleError>,
    layer_rules: bool,
    class_cache: HashMap<String, String>,
    fallback: NetClass,
}

impl RuleSet {
    /// The rules of a project (defaults without a rules document).
    #[must_use]
    pub fn new(rules: Option<&Rules>) -> Self {
        let defaults = Rules::new(Id::from_u128(0), "Design rules");
        let rules = rules.unwrap_or(&defaults);
        let mut custom = Vec::new();
        let mut errors = Vec::new();
        for rule in rules.custom.iter().filter(|rule| rule.enabled) {
            match Condition::parse(&rule.condition) {
                Ok(condition) => custom.push(Compiled {
                    rule: rule.clone(),
                    condition,
                }),
                Err((message, position)) => errors.push(RuleError {
                    rule: rule.name.clone(),
                    message,
                    position,
                }),
            }
        }
        let layer_rules = custom.iter().any(|c| c.condition.mentions_layer);
        Self {
            constraints: rules.constraints,
            classes: rules.net_classes.clone(),
            assignments: rules.assignments.clone(),
            custom,
            errors,
            layer_rules,
            class_cache: HashMap::new(),
            fallback: rules
                .net_classes
                .first()
                .cloned()
                .unwrap_or_else(NetClass::default_class),
        }
    }

    /// `true` if some custom rule tests the layer (checks then run per
    /// layer).
    #[must_use]
    pub const fn has_layer_rules(&self) -> bool {
        self.layer_rules
    }

    /// The net class of a net (the board net's own class wins over the
    /// pattern assignments).
    #[must_use]
    pub fn class_name(&self, net: &str, board_class: &str) -> String {
        if !board_class.is_empty() {
            return board_class.to_owned();
        }
        if let Some(name) = self.class_cache.get(net) {
            return name.clone();
        }
        self.assignments
            .iter()
            .find(|assignment| matches_pattern(&assignment.pattern, net))
            .map_or_else(
                || ew_model::rules::DEFAULT_NET_CLASS.to_owned(),
                |assignment| assignment.class.clone(),
            )
    }

    /// Fills the cache of net classes for the nets of a board.
    pub fn cache_classes(&mut self, nets: impl IntoIterator<Item = (String, String)>) {
        let mut cache = HashMap::new();
        for (net, class) in nets {
            let name = self.class_name(&net, &class);
            cache.insert(net, name);
        }
        self.class_cache = cache;
    }

    /// The net class with this name (the first class if unknown).
    #[must_use]
    pub fn class(&self, name: &str) -> &NetClass {
        self.classes
            .iter()
            .find(|class| class.name == name)
            .unwrap_or(&self.fallback)
    }

    /// The largest clearance any rule can ask for (for spatial queries).
    #[must_use]
    pub fn max_clearance(&self) -> f64 {
        let classes = self.classes.iter().map(|c| c.clearance).max().unwrap_or(0);
        let custom = self
            .custom
            .iter()
            .filter_map(|c| c.rule.constraints.clearance)
            .max()
            .unwrap_or(0);
        f(classes.max(custom).max(self.constraints.min_clearance))
    }

    /// The last matching custom rule that sets a constraint, tried as (a, b)
    /// and (b, a).
    fn rule_for<T>(
        &self,
        a: &Props<'_>,
        b: Option<&Props<'_>>,
        zones: &dyn ZoneTest,
        pick: impl Fn(&CustomRule) -> Option<T>,
    ) -> Option<(T, &str)> {
        self.custom.iter().rev().find_map(|compiled| {
            let value = pick(&compiled.rule)?;
            let hit = compiled.condition.matches(a, b, zones)
                || b.is_some_and(|b| compiled.condition.matches(b, Some(a), zones));
            hit.then_some((value, compiled.rule.name.as_str()))
        })
    }

    /// The clearance between two copper items.
    #[must_use]
    pub fn clearance(&self, a: &Props<'_>, b: &Props<'_>, zones: &dyn ZoneTest) -> Limit {
        let floor = f(self.constraints.min_clearance);
        if let Some((value, name)) = self.rule_for(a, Some(b), zones, |r| r.constraints.clearance) {
            return Limit {
                value: f(value).max(floor),
                source: Source::Rule(name.to_owned()),
            };
        }
        let (ca, cb) = (self.class(a.netclass), self.class(b.netclass));
        let (class, value) = if ca.clearance >= cb.clearance {
            (&ca.name, ca.clearance)
        } else {
            (&cb.name, cb.clearance)
        };
        if f(value) >= floor {
            Limit {
                value: f(value),
                source: Source::NetClass(class.clone()),
            }
        } else {
            Limit {
                value: floor,
                source: Source::Board,
            }
        }
    }

    /// The track width limits of an item.
    #[must_use]
    pub fn track_width(&self, a: &Props<'_>, zones: &dyn ZoneTest) -> WidthLimits {
        let class = self.class(a.netclass);
        let board = f(self.constraints.min_track_width);
        let min = self
            .rule_for(a, None, zones, |r| r.constraints.track_width_min)
            .map_or(
                Limit {
                    value: board,
                    source: Source::Board,
                },
                |(value, name)| Limit {
                    value: f(value).max(board),
                    source: Source::Rule(name.to_owned()),
                },
            );
        let max = self
            .rule_for(a, None, zones, |r| r.constraints.track_width_max)
            .map(|(value, name)| Limit {
                value: f(value),
                source: Source::Rule(name.to_owned()),
            });
        let preferred = self
            .rule_for(a, None, zones, |r| r.constraints.track_width)
            .map_or(f(class.track_width), |(value, _)| f(value))
            .max(min.value);
        WidthLimits {
            min,
            max,
            preferred,
        }
    }

    /// The via and hole limits of an item.
    #[must_use]
    pub fn via(&self, a: &Props<'_>, zones: &dyn ZoneTest) -> ViaLimits {
        let class = self.class(a.netclass);
        let c = &self.constraints;
        let limit = |board: Nm, pick: &dyn Fn(&CustomRule) -> Option<Nm>| {
            self.rule_for(a, None, zones, pick).map_or(
                Limit {
                    value: f(board),
                    source: Source::Board,
                },
                |(value, name)| Limit {
                    value: f(value).max(f(board)),
                    source: Source::Rule(name.to_owned()),
                },
            )
        };
        let diameter = limit(c.min_via_diameter, &|r| r.constraints.via_diameter_min);
        let drill = limit(c.min_via_drill, &|r| r.constraints.via_drill_min);
        ViaLimits {
            preferred_diameter: f(class.via_diameter).max(diameter.value),
            preferred_drill: f(class.via_drill).max(drill.value),
            diameter,
            drill,
            annular: limit(c.min_annular_ring, &|r| r.constraints.annular_ring_min),
            hole: limit(c.min_hole, &|r| r.constraints.hole_min),
        }
    }

    /// The minimum distance between two holes.
    #[must_use]
    pub fn hole_to_hole(&self, a: &Props<'_>, b: &Props<'_>, zones: &dyn ZoneTest) -> Limit {
        let board = f(self.constraints.min_hole_to_hole);
        self.rule_for(a, Some(b), zones, |r| r.constraints.hole_to_hole_min)
            .map_or(
                Limit {
                    value: board,
                    source: Source::Board,
                },
                |(value, name)| Limit {
                    value: f(value).max(board),
                    source: Source::Rule(name.to_owned()),
                },
            )
    }

    /// The minimum copper distance to the board edge.
    #[must_use]
    pub fn edge_clearance(&self, a: &Props<'_>, zones: &dyn ZoneTest) -> Limit {
        let board = f(self.constraints.min_copper_to_edge);
        self.rule_for(a, None, zones, |r| r.constraints.edge_clearance)
            .map_or(
                Limit {
                    value: board,
                    source: Source::Board,
                },
                |(value, name)| Limit {
                    value: f(value),
                    source: Source::Rule(name.to_owned()),
                },
            )
    }
}

/// Checks the syntax of a condition: `None` if it is fine.
#[must_use]
pub fn validate(condition: &str) -> Option<(String, usize)> {
    Condition::parse(condition).err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::rules::RuleConstraints;
    use ew_model::units::um;

    fn props<'a>(kind: &'a str, net: &'a str, class: &'a str) -> Props<'a> {
        Props {
            kind,
            net,
            netclass: class,
            ..Props::default()
        }
    }

    #[test]
    fn conditions_parse_and_match() {
        let hv = props("track", "HV_IN", "HV");
        let low = props("pad", "GND", "Default");
        let c = Condition::parse("A.netclass == 'HV' && B.netclass != 'HV'").unwrap();
        assert!(c.is_pairwise());
        assert!(c.matches(&hv, Some(&low), &NoZones));
        assert!(!c.matches(&low, Some(&hv), &NoZones));
        let c = Condition::parse("A.net == 'HV*' || (A.type == \"via\")").unwrap();
        assert!(c.matches(&hv, None, &NoZones));
        assert!(!c.matches(&low, None, &NoZones));
        let mut wide = props("track", "X", "Default");
        wide.width = Some(300_000.0);
        assert!(
            Condition::parse("A.width > 0.25mm")
                .unwrap()
                .matches(&wide, None, &NoZones)
        );
        assert!(
            Condition::parse("A.width < 12mil")
                .unwrap()
                .matches(&wide, None, &NoZones)
        );
        assert!(
            Condition::parse("!(A.width >= 300um)")
                .unwrap()
                .matches(&low, None, &NoZones)
        );
        assert!(Condition::parse("").unwrap().matches(&low, None, &NoZones));
    }

    #[test]
    fn syntax_errors_name_the_position() {
        assert_eq!(
            validate("A.net == 'GND"),
            Some(("unclosed string".into(), 9))
        );
        assert_eq!(validate("A.colour == 'red'").unwrap().1, 2);
        assert_eq!(validate("C.net == 'x'").unwrap().1, 0);
        assert!(validate("A.net ==").is_some());
        assert!(validate("A.net == 'a' 'b'").is_some());
        assert!(validate("A.insideZone('RF') && B.layer == 'cu.top'").is_none());
    }

    #[test]
    fn clearance_comes_from_classes_rules_and_the_board() {
        let mut rules = Rules::new(Id::random(), "Rules");
        rules.net_classes.push(NetClass {
            name: "HV".into(),
            clearance: um(800),
            ..NetClass::default_class()
        });
        let set = RuleSet::new(Some(&rules));
        let (hv, low) = (props("track", "HV", "HV"), props("pad", "GND", "Default"));
        let limit = set.clearance(&hv, &low, &NoZones);
        assert_eq!(limit.value, 800_000.0);
        assert_eq!(limit.source, Source::NetClass("HV".into()));
        rules.custom.push(CustomRule {
            name: "hv-to-gnd".into(),
            enabled: true,
            condition: "A.netclass == 'HV' && B.net == 'GND'".into(),
            constraints: RuleConstraints {
                clearance: Some(um(1_500)),
                ..RuleConstraints::default()
            },
        });
        rules.custom.push(CustomRule {
            name: "broken".into(),
            enabled: true,
            condition: "A.net ==".into(),
            constraints: RuleConstraints::default(),
        });
        let set = RuleSet::new(Some(&rules));
        assert_eq!(set.errors.len(), 1);
        // The rule matches in either order.
        let limit = set.clearance(&low, &hv, &NoZones);
        assert_eq!(limit.value, 1_500_000.0);
        assert_eq!(limit.source, Source::Rule("hv-to-gnd".into()));
        assert_eq!(set.max_clearance(), 1_500_000.0);
        // Rules never go below the board minimum.
        rules.custom[0].constraints.clearance = Some(um(10));
        let set = RuleSet::new(Some(&rules));
        assert_eq!(set.clearance(&low, &hv, &NoZones).value, 200_000.0);
    }

    #[test]
    fn widths_and_vias() {
        let mut rules = Rules::new(Id::random(), "Rules");
        rules.custom.push(CustomRule {
            name: "power".into(),
            enabled: true,
            condition: "A.netclass == 'Power'".into(),
            constraints: RuleConstraints {
                track_width_min: Some(um(400)),
                via_drill_min: Some(um(400)),
                ..RuleConstraints::default()
            },
        });
        let set = RuleSet::new(Some(&rules));
        let power = props("track", "+5V", "Power");
        let w = set.track_width(&power, &NoZones);
        assert_eq!(w.min.value, 400_000.0);
        assert_eq!(w.preferred, 500_000.0, "the Power class width");
        let signal = props("track", "SDA", "Default");
        assert_eq!(set.track_width(&signal, &NoZones).min.value, 200_000.0);
        let via = set.via(&power, &NoZones);
        assert_eq!(via.drill.value, 400_000.0);
        assert_eq!(via.preferred_drill, 400_000.0);
        assert_eq!(via.annular.value, 130_000.0);
    }
}
