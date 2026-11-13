//! Reads Gerber (RS-274X and X2) and Excellon drill files into images: a
//! list of dark and clear shapes in nanometres. The Gerber viewer draws
//! them, and the tests rasterize them to compare our files with reference
//! files.
//!
//! The reader covers what board tools write today: standard and macro
//! apertures (with variables and expressions), linear and circular
//! interpolation (multi- and single-quadrant), regions with several
//! contours, dark and clear polarity, step and repeat, and the X2 file
//! function. Deprecated features (moiré, aperture holes, mirroring and
//! scaling of later objects) are ignored.

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use ew_pcb::geom::{Shape, V};

/// A shape of an image.
#[derive(Debug, Clone)]
pub struct Object {
    /// Dark (adds) or clear (removes).
    pub dark: bool,
    /// The shape.
    pub shape: Shape,
}

/// A parsed file.
#[derive(Debug, Clone, Default)]
pub struct Image {
    /// The X2 `.FileFunction` (Gerber) or `Plated…` function (drill).
    pub function: Option<String>,
    /// Shapes in drawing order.
    pub objects: Vec<Object>,
    /// Problems that were skipped.
    pub warnings: Vec<String>,
}

impl Image {
    /// Bounding box `[min x, min y, max x, max y]` of the dark shapes.
    #[must_use]
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut out: Option<[f64; 4]> = None;
        for object in self.objects.iter().filter(|o| o.dark) {
            let b = object.shape.bbox();
            out = Some(out.map_or([b.min.x, b.min.y, b.max.x, b.max.y], |o| {
                [
                    o[0].min(b.min.x),
                    o[1].min(b.min.y),
                    o[2].max(b.max.x),
                    o[3].max(b.max.y),
                ]
            }));
        }
        out
    }

    /// `true` if the file uses clear polarity anywhere.
    #[must_use]
    pub fn has_clear(&self) -> bool {
        self.objects.iter().any(|o| !o.dark)
    }
}

/// Chord tolerance for arcs that become line segments (nm).
const ARC_TOLERANCE: f64 = 1_000.0;

fn arc_steps(radius: f64, sweep: f64) -> usize {
    ew_pcb::geom::arc_segments(radius, sweep.abs(), ARC_TOLERANCE)
}

/// Kind of file, guessed from the content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// Gerber.
    Gerber,
    /// Excellon drill.
    Excellon,
}

/// Guesses the kind of a file from its first bytes.
#[must_use]
pub fn detect(text: &str) -> Option<FileKind> {
    let head: String = text.chars().take(4096).collect();
    if head.contains("%FS") || head.contains("%MO") || head.contains("G04") || head.contains("%TF")
    {
        Some(FileKind::Gerber)
    } else if head.starts_with("M48")
        || head.contains("\nM48")
        || head.contains("METRIC")
        || head.contains("INCH")
    {
        Some(FileKind::Excellon)
    } else {
        None
    }
}

/// Reads a Gerber or Excellon file.
#[must_use]
pub fn read(text: &str) -> Option<Image> {
    match detect(text)? {
        FileKind::Gerber => Some(gerber(text)),
        FileKind::Excellon => Some(excellon(text)),
    }
}

// ---------------------------------------------------------------- macros

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Var(usize),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
}

impl Expr {
    fn eval(&self, vars: &HashMap<usize, f64>) -> f64 {
        match self {
            Self::Num(v) => *v,
            Self::Var(n) => vars.get(n).copied().unwrap_or(0.0),
            Self::Neg(e) => -e.eval(vars),
            Self::Bin(op, a, b) => {
                let (a, b) = (a.eval(vars), b.eval(vars));
                match op {
                    '+' => a + b,
                    '-' => a - b,
                    'x' | 'X' => a * b,
                    '/' => {
                        if b == 0.0 {
                            0.0
                        } else {
                            a / b
                        }
                    }
                    _ => 0.0,
                }
            }
        }
    }
}

struct ExprParser<'a> {
    chars: Vec<char>,
    pos: usize,
    _text: &'a str,
}

impl<'a> ExprParser<'a> {
    fn parse(text: &'a str) -> Expr {
        let mut parser = Self {
            chars: text.chars().filter(|c| !c.is_whitespace()).collect(),
            pos: 0,
            _text: text,
        };
        parser.sum()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn sum(&mut self) -> Expr {
        let mut left = self.product();
        while let Some(op @ ('+' | '-')) = self.peek() {
            self.pos += 1;
            let right = self.product();
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        left
    }

    fn product(&mut self) -> Expr {
        let mut left = self.unary();
        while let Some(op @ ('x' | 'X' | '/')) = self.peek() {
            self.pos += 1;
            let right = self.unary();
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        left
    }

    fn unary(&mut self) -> Expr {
        match self.peek() {
            Some('-') => {
                self.pos += 1;
                Expr::Neg(Box::new(self.unary()))
            }
            Some('+') => {
                self.pos += 1;
                self.unary()
            }
            _ => self.atom(),
        }
    }

    fn atom(&mut self) -> Expr {
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                let e = self.sum();
                if self.peek() == Some(')') {
                    self.pos += 1;
                }
                e
            }
            Some('$') => {
                self.pos += 1;
                let start = self.pos;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.pos += 1;
                }
                let n: String = self.chars[start..self.pos].iter().collect();
                Expr::Var(n.parse().unwrap_or(0))
            }
            _ => {
                let start = self.pos;
                while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                    self.pos += 1;
                }
                let n: String = self.chars[start..self.pos].iter().collect();
                if n.is_empty() {
                    // Skip an unexpected character.
                    self.pos += 1;
                }
                Expr::Num(n.parse().unwrap_or(0.0))
            }
        }
    }
}

#[derive(Debug, Clone)]
enum MacroStatement {
    Assign(usize, Expr),
    Primitive(u32, Vec<Expr>),
}

fn parse_macro(body: &[String]) -> Vec<MacroStatement> {
    let mut out = Vec::new();
    for statement in body {
        let statement = statement.trim();
        if statement.is_empty() {
            continue;
        }
        if let Some(rest) = statement.strip_prefix('$')
            && let Some((n, e)) = rest.split_once('=')
        {
            out.push(MacroStatement::Assign(
                n.trim().parse().unwrap_or(0),
                ExprParser::parse(e),
            ));
            continue;
        }
        let mut parts = statement.split(',');
        let Some(code) = parts.next().and_then(|c| c.trim().parse::<u32>().ok()) else {
            continue;
        };
        if code == 0 {
            continue;
        }
        out.push(MacroStatement::Primitive(
            code,
            parts.map(ExprParser::parse).collect(),
        ));
    }
    out
}

fn rotate(p: V, degrees: f64) -> V {
    if degrees == 0.0 {
        p
    } else {
        p.rotate(degrees.to_radians())
    }
}

/// The shapes of a macro aperture for the given parameters (in file units),
/// relative to the flash point. Exposure-off primitives are returned as
/// clear shapes.
#[allow(clippy::too_many_lines, reason = "one arm per primitive")]
fn macro_shapes(statements: &[MacroStatement], params: &[f64], scale: f64) -> Vec<(bool, Shape)> {
    let mut vars: HashMap<usize, f64> = params
        .iter()
        .enumerate()
        .map(|(i, v)| (i + 1, *v))
        .collect();
    let mut out = Vec::new();
    for statement in statements {
        match statement {
            MacroStatement::Assign(n, e) => {
                let v = e.eval(&vars);
                vars.insert(*n, v);
            }
            MacroStatement::Primitive(code, args) => {
                let a: Vec<f64> = args.iter().map(|e| e.eval(&vars)).collect();
                let get = |i: usize| a.get(i).copied().unwrap_or(0.0);
                let on = get(0) != 0.0;
                let pt = |x: f64, y: f64, rot: f64| rotate(V::new(x * scale, y * scale), rot);
                match code {
                    1 => {
                        let c = pt(get(2), get(3), get(4));
                        out.push((on, Shape::disc(c, get(1) * scale / 2.0)));
                    }
                    20 | 2 => {
                        let rot = get(6);
                        let (s, e) = (pt(get(2), get(3), rot), pt(get(4), get(5), rot));
                        let w = get(1) * scale / 2.0;
                        let d = e.sub(s);
                        let len = d.len();
                        if len > 0.0 {
                            let n = V::new(-d.y / len * w, d.x / len * w);
                            out.push((
                                on,
                                Shape::polygon(
                                    vec![s.add(n), e.add(n), e.sub(n), s.sub(n)],
                                    Vec::new(),
                                    0.0,
                                ),
                            ));
                        }
                    }
                    21 | 22 => {
                        let (w, h) = (get(1), get(2));
                        let (cx, cy) = if *code == 21 {
                            (get(3), get(4))
                        } else {
                            (get(3) + w / 2.0, get(4) + h / 2.0)
                        };
                        let rot = get(5);
                        let ring = [
                            (cx - w / 2.0, cy - h / 2.0),
                            (cx + w / 2.0, cy - h / 2.0),
                            (cx + w / 2.0, cy + h / 2.0),
                            (cx - w / 2.0, cy + h / 2.0),
                        ]
                        .iter()
                        .map(|(x, y)| pt(*x, *y, rot))
                        .collect();
                        out.push((on, Shape::polygon(ring, Vec::new(), 0.0)));
                    }
                    4 => {
                        let n = get(1).max(0.0) as usize;
                        let rot = get(2 + 2 * (n + 1));
                        let ring: Vec<V> = (0..n)
                            .map(|i| pt(get(2 + 2 * i), get(3 + 2 * i), rot))
                            .collect();
                        if ring.len() >= 3 {
                            out.push((on, Shape::polygon(ring, Vec::new(), 0.0)));
                        }
                    }
                    5 => {
                        let n = get(1).max(3.0) as usize;
                        let (cx, cy, d, rot) = (get(2), get(3), get(4), get(5));
                        let ring = (0..n)
                            .map(|i| {
                                let a = TAU * i as f64 / n as f64;
                                pt(cx + d / 2.0 * a.cos(), cy + d / 2.0 * a.sin(), rot)
                            })
                            .collect();
                        out.push((on, Shape::polygon(ring, Vec::new(), 0.0)));
                    }
                    7 => {
                        // Thermal: a ring with four gaps (exposure is always on).
                        let (cx, cy, outer, inner, gap, rot) =
                            (get(0), get(1), get(2), get(3), get(4), get(5));
                        let (ro, ri) = (outer / 2.0, inner / 2.0);
                        if ro > ri && ro > gap / 2.0 {
                            for quadrant in 0..4 {
                                let base = f64::from(quadrant) * PI / 2.0;
                                let go = (gap / 2.0 / ro).clamp(-1.0, 1.0).asin();
                                let gi = if ri > gap / 2.0 {
                                    (gap / 2.0 / ri).clamp(-1.0, 1.0).asin()
                                } else {
                                    PI / 4.0
                                };
                                let mut ring = Vec::new();
                                let steps = 16;
                                for i in 0..=steps {
                                    let a = base
                                        + go
                                        + (PI / 2.0 - 2.0 * go) * f64::from(i) / f64::from(steps);
                                    ring.push(pt(cx + ro * a.cos(), cy + ro * a.sin(), rot));
                                }
                                if ri > gap / 2.0 {
                                    for i in (0..=steps).rev() {
                                        let a = base
                                            + gi
                                            + (PI / 2.0 - 2.0 * gi) * f64::from(i)
                                                / f64::from(steps);
                                        ring.push(pt(cx + ri * a.cos(), cy + ri * a.sin(), rot));
                                    }
                                } else {
                                    ring.push(pt(
                                        cx + gap / 2.0 * (base + PI / 4.0).cos() * 1.414,
                                        cy + gap / 2.0 * (base + PI / 4.0).sin() * 1.414,
                                        rot,
                                    ));
                                }
                                out.push((true, Shape::polygon(ring, Vec::new(), 0.0)));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

// --------------------------------------------------------------- Gerber

#[derive(Debug, Clone)]
enum Aperture {
    Circle(f64),
    Rect(f64, f64),
    Obround(f64, f64),
    Polygon(f64, usize, f64),
    Macro(Vec<(bool, Shape)>),
}

impl Aperture {
    /// The shapes of a flash at `at`.
    fn flash(&self, at: V) -> Vec<(bool, Shape)> {
        match self {
            Self::Circle(d) => vec![(true, Shape::disc(at, d / 2.0))],
            Self::Rect(w, h) => vec![(
                true,
                Shape::polygon(
                    vec![
                        at.add(V::new(-w / 2.0, -h / 2.0)),
                        at.add(V::new(w / 2.0, -h / 2.0)),
                        at.add(V::new(w / 2.0, h / 2.0)),
                        at.add(V::new(-w / 2.0, h / 2.0)),
                    ],
                    Vec::new(),
                    0.0,
                ),
            )],
            Self::Obround(w, h) => {
                let r = w.min(*h) / 2.0;
                let (dx, dy) = ((w / 2.0 - r).max(0.0), (h / 2.0 - r).max(0.0));
                vec![(
                    true,
                    Shape::stadium(at.add(V::new(-dx, -dy)), at.add(V::new(dx, dy)), r),
                )]
            }
            Self::Polygon(d, n, rot) => {
                let ring = (0..*n)
                    .map(|i| {
                        let a = TAU * i as f64 / *n as f64 + rot.to_radians();
                        at.add(V::new(d / 2.0 * a.cos(), d / 2.0 * a.sin()))
                    })
                    .collect();
                vec![(true, Shape::polygon(ring, Vec::new(), 0.0))]
            }
            Self::Macro(shapes) => shapes
                .iter()
                .map(|(on, shape)| (*on, translate(shape, at)))
                .collect(),
        }
    }
}

fn translate(shape: &Shape, by: V) -> Shape {
    use ew_pcb::geom::Core;
    match &shape.core {
        Core::Point(p) => Shape::disc(p.add(by), shape.radius),
        Core::Segment(a, b) => Shape::stadium(a.add(by), b.add(by), shape.radius),
        Core::Poly(poly) => Shape::polygon(
            poly.outline().iter().map(|p| p.add(by)).collect(),
            poly.holes()
                .iter()
                .map(|h| h.iter().map(|p| p.add(by)).collect())
                .collect(),
            shape.radius,
        ),
    }
}

#[derive(Debug, Clone, Copy)]
struct Format {
    integer: u32,
    decimal: u32,
    trailing: bool,
}

impl Default for Format {
    fn default() -> Self {
        Self {
            integer: 4,
            decimal: 6,
            trailing: false,
        }
    }
}

impl Format {
    fn value(self, text: &str, unit: f64) -> f64 {
        if text.contains('.') {
            return text.parse::<f64>().unwrap_or(0.0) * unit;
        }
        let (negative, digits) = match text.as_bytes().first() {
            Some(b'-') => (true, &text[1..]),
            Some(b'+') => (false, &text[1..]),
            _ => (false, text),
        };
        let value = if self.trailing {
            // Trailing zeros omitted: digits are left-aligned.
            let total = (self.integer + self.decimal) as usize;
            let padded = format!("{digits:0<total$}");
            padded.parse::<f64>().unwrap_or(0.0)
                / 10f64.powi(i32::try_from(self.decimal).unwrap_or(6))
        } else {
            digits.parse::<f64>().unwrap_or(0.0)
                / 10f64.powi(i32::try_from(self.decimal).unwrap_or(6))
        };
        if negative {
            -value * unit
        } else {
            value * unit
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Interpolation {
    Linear,
    Clockwise,
    CounterClockwise,
}

struct GerberState {
    format: Format,
    unit: f64,
    apertures: HashMap<u32, Aperture>,
    macros: HashMap<String, Vec<MacroStatement>>,
    current: Option<u32>,
    pos: V,
    interpolation: Interpolation,
    multi_quadrant: bool,
    dark: bool,
    region: Option<Vec<Vec<V>>>,
    image: Image,
    step_repeat: Option<(u32, u32, f64, f64, usize)>,
}

impl GerberState {
    fn push(&mut self, dark: bool, shape: Shape) {
        self.image.objects.push(Object { dark, shape });
    }

    fn aperture(&self) -> Option<&Aperture> {
        self.current.and_then(|c| self.apertures.get(&c))
    }

    /// Points of an arc from `from` to `to` (excluding `from`).
    fn arc_points(&self, from: V, to: V, i: f64, j: f64) -> Vec<V> {
        let ccw = self.interpolation == Interpolation::CounterClockwise;
        let center = if self.multi_quadrant {
            from.add(V::new(i, j))
        } else {
            // Single quadrant: pick the signs that make a valid arc ≤ 90°.
            let mut best: Option<(f64, V)> = None;
            for (si, sj) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                let c = from.add(V::new(si * i.abs(), sj * j.abs()));
                let error = (c.dist(from) - c.dist(to)).abs();
                let sweep = sweep_of(c, from, to, ccw, false);
                if sweep <= PI / 2.0 + 1e-3 && best.is_none_or(|(e, _)| error < e) {
                    best = Some((error, c));
                }
            }
            best.map_or_else(|| from.add(V::new(i, j)), |(_, c)| c)
        };
        let full = self.multi_quadrant && from.dist(to) < 1.0;
        let sweep = sweep_of(center, from, to, ccw, full);
        let radius = center.dist(from);
        let start = (from.y - center.y).atan2(from.x - center.x);
        let end_radius = center.dist(to);
        let steps = arc_steps(radius, sweep);
        let signed = if ccw { sweep } else { -sweep };
        (1..=steps)
            .map(|k| {
                let t = k as f64 / steps as f64;
                if k == steps {
                    return to;
                }
                let a = start + signed * t;
                let r = radius + (end_radius - radius) * t;
                center.add(V::new(r * a.cos(), r * a.sin()))
            })
            .collect()
    }

    fn draw(&mut self, from: V, to: V, i: f64, j: f64) {
        let points = if self.interpolation == Interpolation::Linear {
            vec![to]
        } else {
            self.arc_points(from, to, i, j)
        };
        if let Some(region) = &mut self.region {
            if region.is_empty() {
                region.push(vec![from]);
            }
            if let Some(contour) = region.last_mut() {
                if contour.is_empty() {
                    contour.push(from);
                }
                contour.extend(points);
            }
            return;
        }
        let Some(aperture) = self.aperture().cloned() else {
            self.image
                .warnings
                .push("draw without an aperture".to_owned());
            return;
        };
        let dark = self.dark;
        let mut last = from;
        for p in points {
            match &aperture {
                Aperture::Circle(d) => self.push(dark, Shape::stadium(last, p, d / 2.0)),
                other => {
                    // Draws with other apertures: the hull of the start and
                    // end flashes.
                    let mut hull: Vec<V> = Vec::new();
                    for (_, s) in other.flash(last).into_iter().chain(other.flash(p)) {
                        let b = s.bbox();
                        hull.extend([
                            V::new(b.min.x, b.min.y),
                            V::new(b.max.x, b.min.y),
                            V::new(b.max.x, b.max.y),
                            V::new(b.min.x, b.max.y),
                        ]);
                    }
                    let hull = convex_hull(hull);
                    if hull.len() >= 3 {
                        self.push(dark, Shape::polygon(hull, Vec::new(), 0.0));
                    }
                }
            }
            last = p;
        }
    }

    fn flash(&mut self, at: V) {
        let Some(aperture) = self.aperture().cloned() else {
            self.image
                .warnings
                .push("flash without an aperture".to_owned());
            return;
        };
        let dark = self.dark;
        for (on, shape) in aperture.flash(at) {
            self.push(if on { dark } else { !dark }, shape);
        }
    }

    fn close_region(&mut self) {
        if let Some(region) = self.region.take() {
            let dark = self.dark;
            for contour in region {
                if contour.len() >= 3 {
                    self.push(dark, Shape::polygon(contour, Vec::new(), 0.0));
                }
            }
        }
    }
}

/// The sweep (radians, ≥ 0) of an arc around `c` from `a` to `b`.
fn sweep_of(c: V, a: V, b: V, ccw: bool, full: bool) -> f64 {
    if full {
        return TAU;
    }
    let sa = (a.y - c.y).atan2(a.x - c.x);
    let sb = (b.y - c.y).atan2(b.x - c.x);
    let mut sweep = if ccw { sb - sa } else { sa - sb };
    while sweep < 0.0 {
        sweep += TAU;
    }
    while sweep > TAU {
        sweep -= TAU;
    }
    sweep
}

fn convex_hull(mut points: Vec<V>) -> Vec<V> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| a.dist(*b) < 1e-9);
    if points.len() < 3 {
        return points;
    }
    let cross = |o: V, a: V, b: V| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut lower: Vec<V> = Vec::new();
    for p in &points {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0 {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper: Vec<V> = Vec::new();
    for p in points.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0 {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Splits Gerber text into extended (`%…%`) and word (`…*`) commands.
fn commands(text: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    let mut word = String::new();
    while let Some(c) = chars.next() {
        match c {
            '%' => {
                let mut block = String::new();
                for d in chars.by_ref() {
                    if d == '%' {
                        break;
                    }
                    if d != '\n' && d != '\r' {
                        block.push(d);
                    }
                }
                out.push((true, block));
            }
            '*' => {
                let w = word.trim().to_owned();
                if !w.is_empty() {
                    out.push((false, w));
                }
                word.clear();
            }
            '\n' | '\r' => {}
            c => word.push(c),
        }
    }
    out
}

/// Reads the number after a letter in a word (`X…`, `Y…`, `I…`, `J…`).
fn field(word: &str, letter: char) -> Option<&str> {
    let start = word.find(letter)? + 1;
    let rest = &word[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '+' || c == '.'))
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

#[allow(clippy::too_many_lines, reason = "one arm per command")]
fn extended(state: &mut GerberState, block: &str) {
    // A block may hold several commands separated by `*`.
    let statements: Vec<String> = block
        .split('*')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    let Some(first) = statements.first() else {
        return;
    };
    if let Some(name) = first.strip_prefix("AM") {
        let body = parse_macro(&statements[1..]);
        state.macros.insert(name.to_owned(), body);
        return;
    }
    for statement in &statements {
        let s = statement.as_str();
        if let Some(rest) = s.strip_prefix("FS") {
            let trailing = rest.starts_with('T');
            if let (Some(x), true) = (rest.find('X'), rest.len() > 2) {
                let digits: Vec<u32> = rest[x + 1..]
                    .chars()
                    .take(2)
                    .filter_map(|c| c.to_digit(10))
                    .collect();
                if let [integer, decimal] = digits[..] {
                    state.format = Format {
                        integer,
                        decimal,
                        trailing,
                    };
                }
            }
        } else if let Some(rest) = s.strip_prefix("MO") {
            state.unit = if rest.starts_with("IN") {
                25_400_000.0
            } else {
                1_000_000.0
            };
        } else if let Some(rest) = s.strip_prefix("AD") {
            let rest = rest.trim_start_matches('D');
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let Ok(code) = digits.parse::<u32>() else {
                continue;
            };
            let def = &rest[digits.len()..];
            let (name, params) = def.split_once(',').unwrap_or((def, ""));
            let values: Vec<f64> = params
                .split('X')
                .filter(|v| !v.is_empty())
                .map(|v| v.trim().parse::<f64>().unwrap_or(0.0))
                .collect();
            let u = state.unit;
            let get = |i: usize| values.get(i).copied().unwrap_or(0.0);
            let aperture = match name {
                "C" => Aperture::Circle(get(0) * u),
                "R" => Aperture::Rect(get(0) * u, get(1) * u),
                "O" => Aperture::Obround(get(0) * u, get(1) * u),
                "P" => Aperture::Polygon(get(0) * u, get(1).max(3.0) as usize, get(2)),
                other => {
                    let Some(body) = state.macros.get(other) else {
                        state
                            .image
                            .warnings
                            .push(format!("unknown aperture template {other}"));
                        continue;
                    };
                    Aperture::Macro(macro_shapes(body, &values, u))
                }
            };
            state.apertures.insert(code, aperture);
        } else if let Some(rest) = s.strip_prefix("LP") {
            state.dark = !rest.starts_with('C');
        } else if let Some(rest) = s.strip_prefix("TF.FileFunction,") {
            state.image.function = Some(rest.to_owned());
        } else if let Some(rest) = s.strip_prefix("SR") {
            finish_step_repeat(state);
            let number = |letter: char| field(rest, letter).unwrap_or("");
            let x: u32 = number('X').parse().unwrap_or(1);
            let y: u32 = number('Y').parse().unwrap_or(1);
            let i: f64 = number('I').parse().unwrap_or(0.0);
            let j: f64 = number('J').parse().unwrap_or(0.0);
            if x > 1 || y > 1 {
                state.step_repeat = Some((
                    x,
                    y,
                    i * state.unit,
                    j * state.unit,
                    state.image.objects.len(),
                ));
            }
        }
    }
}

fn finish_step_repeat(state: &mut GerberState) {
    if let Some((nx, ny, dx, dy, start)) = state.step_repeat.take() {
        let block: Vec<Object> = state.image.objects[start..].to_vec();
        for ix in 0..nx {
            for iy in 0..ny {
                if ix == 0 && iy == 0 {
                    continue;
                }
                let by = V::new(f64::from(ix) * dx, f64::from(iy) * dy);
                for object in &block {
                    state.image.objects.push(Object {
                        dark: object.dark,
                        shape: translate(&object.shape, by),
                    });
                }
            }
        }
    }
}

/// Reads a Gerber file.
#[must_use]
pub fn gerber(text: &str) -> Image {
    let mut state = GerberState {
        format: Format::default(),
        unit: 1_000_000.0,
        apertures: HashMap::new(),
        macros: HashMap::new(),
        current: None,
        pos: V::new(0.0, 0.0),
        interpolation: Interpolation::Linear,
        multi_quadrant: true,
        dark: true,
        region: None,
        image: Image::default(),
        step_repeat: None,
    };
    for (is_extended, command) in commands(text) {
        if is_extended {
            extended(&mut state, &command);
            continue;
        }
        let mut word = command.as_str();
        if word.starts_with("G04") || word.starts_with("G4 ") {
            continue;
        }
        if word.starts_with("M02") || word.starts_with("M00") || word.starts_with("M2") {
            break;
        }
        // Mode codes at the start of the word.
        while let Some(rest) = word.strip_prefix('G') {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let code: u32 = digits.parse().unwrap_or(0);
            match code {
                1 => state.interpolation = Interpolation::Linear,
                2 => state.interpolation = Interpolation::Clockwise,
                3 => state.interpolation = Interpolation::CounterClockwise,
                36 => state.region = Some(Vec::new()),
                37 => state.close_region(),
                74 => state.multi_quadrant = false,
                75 => state.multi_quadrant = true,
                _ => {}
            }
            word = &rest[digits.len()..];
        }
        if word.is_empty() {
            continue;
        }
        // D codes.
        let d = word
            .rfind('D')
            .map(|i| &word[i + 1..])
            .and_then(|s| s.parse::<u32>().ok());
        let coords = d.map_or(word, |_| &word[..word.rfind('D').unwrap_or(word.len())]);
        let unit = state.unit;
        let format = state.format;
        let x = field(coords, 'X').map(|v| format.value(v, unit));
        let y = field(coords, 'Y').map(|v| format.value(v, unit));
        let i = field(coords, 'I').map_or(0.0, |v| format.value(v, unit));
        let j = field(coords, 'J').map_or(0.0, |v| format.value(v, unit));
        let target = V::new(x.unwrap_or(state.pos.x), y.unwrap_or(state.pos.y));
        match d {
            Some(1) => {
                let from = state.pos;
                state.draw(from, target, i, j);
                state.pos = target;
            }
            Some(2) => {
                if let Some(region) = &mut state.region {
                    region.push(vec![target]);
                }
                state.pos = target;
            }
            Some(3) => {
                state.pos = target;
                state.flash(target);
            }
            Some(code) if code >= 10 => state.current = Some(code),
            Some(_) => {}
            None => {
                // A coordinate without a D code repeats the last operation
                // in old files; treat it as a draw.
                if x.is_some() || y.is_some() {
                    let from = state.pos;
                    state.draw(from, target, i, j);
                    state.pos = target;
                }
            }
        }
    }
    state.close_region();
    finish_step_repeat(&mut state);
    state.image
}

// ------------------------------------------------------------- Excellon

/// Reads an Excellon drill file.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one branch per command")]
pub fn excellon(text: &str) -> Image {
    let mut image = Image::default();
    let mut unit = 1_000_000.0_f64;
    let mut tools: HashMap<u32, f64> = HashMap::new();
    let mut tool: Option<u32> = None;
    let mut pos = V::new(0.0, 0.0);
    let mut header = true;
    // Implicit decimals: metric 3.3, inch 2.4; leading zeros kept (LZ)
    // means trailing zeros are suppressed.
    let mut decimals = 3_i32;
    let mut leading_kept = false;
    let mut route_down: Option<V> = None;
    let mut plated: Option<bool> = None;
    let value = |text: &str, unit: f64, decimals: i32, leading_kept: bool| -> f64 {
        if text.contains('.') {
            return text.parse::<f64>().unwrap_or(0.0) * unit;
        }
        let (negative, digits) = match text.as_bytes().first() {
            Some(b'-') => (true, &text[1..]),
            Some(b'+') => (false, &text[1..]),
            _ => (false, text),
        };
        let v = if leading_kept {
            let integer = if decimals == 4 { 2 } else { 3 };
            let total = (integer + decimals) as usize;
            format!("{digits:0<total$}").parse::<f64>().unwrap_or(0.0) / 10f64.powi(decimals)
        } else {
            digits.parse::<f64>().unwrap_or(0.0) / 10f64.powi(decimals)
        };
        if negative { -v * unit } else { v * unit }
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(comment) = line.strip_prefix(';') {
            if let Some(f) = comment.trim().strip_prefix("#@! TF.FileFunction,") {
                image.function = Some(f.to_owned());
                plated = Some(!f.starts_with("NonPlated"));
            }
            continue;
        }
        if line.starts_with("METRIC") {
            unit = 1_000_000.0;
            decimals = 3;
            leading_kept = line.contains("LZ");
            continue;
        }
        if line.starts_with("INCH") {
            unit = 25_400_000.0;
            decimals = 4;
            leading_kept = line.contains("LZ");
            continue;
        }
        if line == "%" || line.starts_with("M95") {
            header = false;
            continue;
        }
        if line.starts_with("M30") || line.starts_with("M00") {
            break;
        }
        if line.starts_with("M71") {
            unit = 1_000_000.0;
            continue;
        }
        if line.starts_with("M72") {
            unit = 25_400_000.0;
            continue;
        }
        if let Some(rest) = line.strip_prefix('T') {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let Ok(number) = digits.parse::<u32>() else {
                continue;
            };
            if let Some(c) = rest.find('C') {
                let d: String = rest[c + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                tools.insert(number, d.parse::<f64>().unwrap_or(0.0) * unit);
            }
            if !header || !rest.contains('C') {
                tool = Some(number).filter(|n| *n != 0);
            }
            continue;
        }
        let diameter = tool.and_then(|t| tools.get(&t)).copied().unwrap_or(0.0);
        let coordinates = line.starts_with('X')
            || line.starts_with('Y')
            || line.starts_with("G00")
            || line.starts_with("G01");
        if coordinates && (line.contains('X') || line.contains('Y')) {
            let (head, slot_end) = match line.split_once("G85") {
                Some((a, b)) => (a, Some(b)),
                None => (line, None),
            };
            let read_point = |text: &str, base: V| {
                V::new(
                    field(text, 'X').map_or(base.x, |v| value(v, unit, decimals, leading_kept)),
                    field(text, 'Y').map_or(base.y, |v| value(v, unit, decimals, leading_kept)),
                )
            };
            let p = read_point(head, pos);
            if line.starts_with("G00") {
                pos = p;
                route_down = None;
                continue;
            }
            if line.starts_with("G01") {
                if let Some(start) = route_down {
                    image.objects.push(Object {
                        dark: true,
                        shape: Shape::stadium(start, p, diameter / 2.0),
                    });
                    route_down = Some(p);
                }
                pos = p;
                continue;
            }
            if let Some(end) = slot_end {
                let q = read_point(end, p);
                image.objects.push(Object {
                    dark: true,
                    shape: Shape::stadium(p, q, diameter / 2.0),
                });
                pos = q;
            } else {
                image.objects.push(Object {
                    dark: true,
                    shape: Shape::disc(p, diameter / 2.0),
                });
                pos = p;
            }
            continue;
        }
        if line.starts_with("M15") {
            route_down = Some(pos);
        } else if line.starts_with("M16") || line.starts_with("M17") {
            route_down = None;
        }
    }
    if image.function.is_none() {
        image.function = plated.map(|p| if p { "Plated" } else { "NonPlated" }.to_owned());
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions() {
        let vars: HashMap<usize, f64> = [(1, 2.0), (2, 3.0)].into_iter().collect();
        assert!((ExprParser::parse("$1+$1").eval(&vars) - 4.0).abs() < 1e-12);
        assert!((ExprParser::parse("$1x$2-1").eval(&vars) - 5.0).abs() < 1e-12);
        assert!((ExprParser::parse("-($1+$2)/2").eval(&vars) + 2.5).abs() < 1e-12);
    }

    #[test]
    fn coordinate_formats() {
        let f = Format::default();
        assert!((f.value("-110095000", 1e6) + 110_095_000.0).abs() < 1e-6);
        let t = Format {
            integer: 2,
            decimal: 4,
            trailing: true,
        };
        assert!((t.value("15", 25.4e6) - 15.0 * 25.4e6).abs() < 1e-3);
    }

    #[test]
    fn reads_flashes_draws_and_regions() {
        let text = "%FSLAX46Y46*%\n%MOMM*%\n%TF.FileFunction,Copper,L1,Top*%\n\
            %AMRR*\n1,1,$1+$1,$2,$3*\n%\n%ADD10C,0.5*%\n%ADD11R,2X1*%\n%ADD12RR,0.25X1X0*%\n\
            D10*\nX0Y0D02*\nX10000000Y0D01*\nD11*\nX5000000Y5000000D03*\nD12*X0Y0D03*\n\
            G36*\nX0Y0D02*\nX1000000Y0D01*\nX1000000Y1000000D01*\nX0Y0D01*\nG37*\n\
            %LPC*%\nD10*\nX0Y0D03*\nM02*\n";
        let image = gerber(text);
        assert_eq!(image.function.as_deref(), Some("Copper,L1,Top"));
        assert_eq!(image.objects.len(), 5, "{:?}", image.warnings);
        assert!(image.has_clear());
        let b = image.bounds().unwrap();
        assert!((b[2] - 10_250_000.0).abs() < 1.0, "{b:?}");
        // The macro circle has a diameter of 2 × 0.25 mm at (1, 0).
        assert!(image.objects[2].shape.contains(V::new(1_200_000.0, 0.0)));
    }

    #[test]
    fn reads_arcs() {
        let text = "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.1*%\nG75*\nD10*\n\
            X1000000Y0D02*\nG03*\nX-1000000Y0I-1000000J0D01*\nM02*\n";
        let image = gerber(text);
        assert!(image.objects.len() > 8);
        let b = image.bounds().unwrap();
        assert!((b[3] - 1_050_000.0).abs() < 2_000.0, "{b:?}");
        assert!(b[1] > -60_000.0);
    }

    #[test]
    fn reads_excellon() {
        let text = "M48\n; #@! TF.FileFunction,Plated,1,2,PTH\nFMAT,2\nMETRIC\nT1C0.400\nT2C1.000\n%\nG90\nG05\n\
            T1\nX125.0Y-115.0\nX137.0Y-108.0\nT2\nX1.0Y1.0G85X3.0Y1.0\nM30\n";
        let image = excellon(text);
        assert_eq!(image.objects.len(), 3);
        assert_eq!(image.function.as_deref(), Some("Plated,1,2,PTH"));
        assert!((image.objects[0].shape.radius - 200_000.0).abs() < 1e-6);
        assert!(
            image.objects[2]
                .shape
                .contains(V::new(2_000_000.0, 1_000_000.0))
        );
        assert_eq!(detect(text), Some(FileKind::Excellon));
    }
}
