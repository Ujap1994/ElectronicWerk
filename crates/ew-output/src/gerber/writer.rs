//! Writes Gerber X2 (RS-274X with attributes) files.
//!
//! Coordinates use the format 4.6 in millimetres, so one unit of the file is
//! one nanometre of the model and nothing is rounded. Pads and vias are
//! flashed with standard apertures where they fit (circles, rectangles,
//! obrounds) and with aperture macros otherwise: the shapes of the board are
//! cores (a point, a segment or a polygon) grown by a radius, and a macro
//! builds them exactly from an outline primitive, vector lines and circles.

use std::collections::HashMap;
use std::fmt::Write as _;

use ew_pcb::geom::{Core, Shape, V};

use crate::art::{Attributes, Feature, Function};
use crate::fracture::fracture;

/// File attributes of a Gerber file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Application version for `.GenerationSoftware`.
    pub version: String,
    /// ISO 8601 creation time for `.CreationDate`; empty leaves it out.
    pub created: String,
    /// Project name, GUID and revision for `.ProjectId`.
    pub project: Option<(String, String, String)>,
    /// `.FileFunction`, for example `Copper,L1,Top`.
    pub function: String,
    /// `.FilePolarity` (`Positive` or `Negative`), for layers that have one.
    pub polarity: Option<&'static str>,
    /// Write X2 attributes (file, aperture and object attributes).
    pub attributes: bool,
}

/// Escapes a Gerber attribute value (`,`, `*`, `%` and `\` are reserved).
#[must_use]
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            ',' | '*' | '%' | '\\' => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c) & 0xFFFF);
            }
            c => out.push(c),
        }
    }
    out
}

/// A length in millimetres with six decimals (aperture sizes).
fn mm(value: f64) -> String {
    let value = if value.abs() < 0.5 { 0.0 } else { value };
    format!("{:.6}", value / 1e6)
}

/// A coordinate in file units (nanometres).
fn unit(value: f64) -> i64 {
    value.round() as i64
}

#[derive(Debug, Default)]
struct Writer {
    attributes: bool,
    macros: String,
    macro_ids: HashMap<String, usize>,
    apertures: String,
    aperture_ids: HashMap<(String, Option<&'static str>), u32>,
    body: String,
    current: Option<u32>,
    object: Option<Attributes>,
    at: Option<(i64, i64)>,
    arc_mode: bool,
}

impl Writer {
    fn new(attributes: bool) -> Self {
        Self {
            attributes,
            ..Self::default()
        }
    }

    /// The D code of an aperture template, defining it on first use.
    fn aperture(&mut self, template: String, function: Function) -> u32 {
        let function = if self.attributes {
            function.gerber()
        } else {
            None
        };
        let key = (template, function);
        if let Some(code) = self.aperture_ids.get(&key) {
            return *code;
        }
        let code = 10 + u32::try_from(self.aperture_ids.len()).unwrap_or(0);
        if let Some(function) = function {
            let _ = writeln!(self.apertures, "%TA.AperFunction,{function}*%");
        }
        let _ = writeln!(self.apertures, "%ADD{code}{}*%", key.0);
        if function.is_some() {
            self.apertures.push_str("%TD*%\n");
        }
        self.aperture_ids.insert(key, code);
        code
    }

    /// The name of a fixed aperture macro, defining it on first use.
    fn macro_name(&mut self, primitives: String) -> String {
        if let Some(id) = self.macro_ids.get(&primitives) {
            return format!("M{id}");
        }
        let id = self.macro_ids.len() + 1;
        let _ = writeln!(self.macros, "%AMM{id}*\n{primitives}%");
        self.macro_ids.insert(primitives, id);
        format!("M{id}")
    }

    fn select(&mut self, code: u32) {
        if self.current != Some(code) {
            let _ = writeln!(self.body, "D{code}*");
            self.current = Some(code);
        }
    }

    fn set_object(&mut self, attributes: &Attributes) {
        if !self.attributes || self.object.as_ref() == Some(attributes) {
            return;
        }
        let had = self
            .object
            .as_ref()
            .is_some_and(|a| *a != Attributes::default());
        if had {
            self.body.push_str("%TD*%\n");
        }
        if let Some((designator, pin)) = &attributes.pin {
            let _ = writeln!(self.body, "%TO.P,{},{}*%", escape(designator), escape(pin));
        }
        if let Some(net) = &attributes.net {
            let _ = writeln!(self.body, "%TO.N,{}*%", escape(net));
        }
        if let Some(component) = &attributes.component {
            let _ = writeln!(self.body, "%TO.C,{}*%", escape(component));
        }
        self.object = Some(attributes.clone());
    }

    fn linear(&mut self) {
        if self.arc_mode {
            self.body.push_str("G01*\n");
            self.arc_mode = false;
        }
    }

    fn move_to(&mut self, p: V) {
        let (x, y) = (unit(p.x), unit(p.y));
        if self.at != Some((x, y)) {
            let _ = writeln!(self.body, "X{x}Y{y}D02*");
            self.at = Some((x, y));
        }
    }

    fn draw_to(&mut self, p: V) {
        let (x, y) = (unit(p.x), unit(p.y));
        let _ = writeln!(self.body, "X{x}Y{y}D01*");
        self.at = Some((x, y));
    }

    fn flash(&mut self, at: V, shape: &Shape, function: Function) {
        let (template, at) = flash_template(self, at, shape);
        let code = self.aperture(template, function);
        self.select(code);
        let (x, y) = (unit(at.x), unit(at.y));
        let _ = writeln!(self.body, "X{x}Y{y}D03*");
        self.at = Some((x, y));
    }

    fn feature(&mut self, feature: &Feature) {
        self.set_object(feature.attributes());
        match feature {
            Feature::Flash {
                at,
                shape,
                function,
                ..
            } => self.flash(*at, shape, *function),
            Feature::Line {
                a,
                b,
                width,
                function,
                ..
            } => {
                if *width <= 0.0 {
                    return;
                }
                let code = self.aperture(format!("C,{}", mm(*width)), *function);
                self.select(code);
                self.linear();
                self.move_to(*a);
                self.draw_to(*b);
            }
            Feature::Arc {
                start,
                end,
                center,
                ccw,
                width,
                function,
                ..
            } => {
                if *width <= 0.0 {
                    return;
                }
                let code = self.aperture(format!("C,{}", mm(*width)), *function);
                self.select(code);
                self.move_to(*start);
                let _ = writeln!(self.body, "{}*", if *ccw { "G03" } else { "G02" });
                self.arc_mode = true;
                let (x, y) = (unit(end.x), unit(end.y));
                let (sx, sy) = (unit(start.x), unit(start.y));
                let (i, j) = (unit(center.x) - sx, unit(center.y) - sy);
                let _ = writeln!(self.body, "X{x}Y{y}I{i}J{j}D01*");
                self.at = Some((x, y));
            }
            Feature::Region {
                outline,
                holes,
                function,
                ..
            } => {
                let contour = fracture(outline, holes);
                if contour.len() < 3 {
                    return;
                }
                let tagged = self.attributes && function.gerber().is_some();
                if let Some(value) = function.gerber().filter(|_| tagged) {
                    let _ = writeln!(self.body, "%TA.AperFunction,{value}*%");
                }
                self.linear();
                self.body.push_str("G36*\n");
                self.at = None;
                self.move_to(contour[0]);
                for p in &contour[1..] {
                    self.draw_to(*p);
                }
                self.draw_to(contour[0]);
                self.body.push_str("G37*\n");
                if tagged {
                    self.body.push_str("%TD.AperFunction*%\n");
                }
            }
        }
    }
}

/// `true` if a ring is an axis-aligned rectangle.
fn axis_rect(ring: &[V]) -> Option<(V, f64, f64)> {
    if ring.len() != 4 {
        return None;
    }
    for i in 0..4 {
        let (p, q) = (ring[i], ring[(i + 1) % 4]);
        if (p.x - q.x).abs() > 0.5 && (p.y - q.y).abs() > 0.5 {
            return None;
        }
    }
    let (mut lo, mut hi) = (ring[0], ring[0]);
    for p in ring {
        lo = V::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = V::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let (w, h) = (hi.x - lo.x, hi.y - lo.y);
    (w > 0.5 && h > 0.5).then(|| {
        (
            V::new(f64::midpoint(lo.x, hi.x), f64::midpoint(lo.y, hi.y)),
            w,
            h,
        )
    })
}

/// The aperture template of a flashed shape and the point to flash it at.
fn flash_template(writer: &mut Writer, at: V, shape: &Shape) -> (String, V) {
    let r = shape.radius;
    match &shape.core {
        Core::Point(p) => (format!("C,{}", mm(2.0 * r)), *p),
        Core::Segment(a, b) => {
            let mid = V::new(f64::midpoint(a.x, b.x), f64::midpoint(a.y, b.y));
            let len = a.dist(*b);
            if (a.y - b.y).abs() < 0.5 {
                (format!("O,{}X{}", mm(len + 2.0 * r), mm(2.0 * r)), mid)
            } else if (a.x - b.x).abs() < 0.5 {
                (format!("O,{}X{}", mm(2.0 * r), mm(len + 2.0 * r)), mid)
            } else {
                let (ra, rb) = (a.sub(mid), b.sub(mid));
                let d = mm(2.0 * r);
                let primitives = format!(
                    "1,1,{d},{},{}*\n1,1,{d},{},{}*\n20,1,{d},{},{},{},{},0*\n",
                    mm(ra.x),
                    mm(ra.y),
                    mm(rb.x),
                    mm(rb.y),
                    mm(ra.x),
                    mm(ra.y),
                    mm(rb.x),
                    mm(rb.y)
                );
                (writer.macro_name(primitives), mid)
            }
        }
        Core::Poly(poly) => {
            let ring = poly.outline();
            if r < 0.5
                && let Some((center, w, h)) = axis_rect(ring)
            {
                return (format!("R,{}X{}", mm(w), mm(h)), center);
            }
            let mut primitives = String::new();
            let local: Vec<V> = ring.iter().map(|p| p.sub(at)).collect();
            let _ = write!(primitives, "4,1,{}", local.len());
            for p in local.iter().chain(local.first()) {
                let _ = write!(primitives, ",{},{}", mm(p.x), mm(p.y));
            }
            primitives.push_str(",0*\n");
            if r >= 0.5 {
                let d = mm(2.0 * r);
                for i in 0..local.len() {
                    let (p, q) = (local[i], local[(i + 1) % local.len()]);
                    let _ = writeln!(
                        primitives,
                        "20,1,{d},{},{},{},{},0*",
                        mm(p.x),
                        mm(p.y),
                        mm(q.x),
                        mm(q.y)
                    );
                }
                for p in &local {
                    let _ = writeln!(primitives, "1,1,{d},{},{}*", mm(p.x), mm(p.y));
                }
            }
            (writer.macro_name(primitives), at)
        }
    }
}

/// Writes one Gerber file with the given features (in drawing order).
#[must_use]
pub fn write(header: &Header, features: &[Feature]) -> String {
    let mut writer = Writer::new(header.attributes);
    for feature in features {
        writer.feature(feature);
    }
    let mut out = String::new();
    if header.attributes {
        let _ = writeln!(
            out,
            "%TF.GenerationSoftware,ElectronicWerk,ElectronicWerk,{}*%",
            escape(&header.version)
        );
        if !header.created.is_empty() {
            let _ = writeln!(out, "%TF.CreationDate,{}*%", escape(&header.created));
        }
        if let Some((name, guid, revision)) = &header.project {
            let _ = writeln!(
                out,
                "%TF.ProjectId,{},{},{}*%",
                escape(name),
                escape(guid),
                escape(revision)
            );
        }
        out.push_str("%TF.SameCoordinates,Original*%\n");
        let _ = writeln!(out, "%TF.FileFunction,{}*%", header.function);
        if let Some(polarity) = header.polarity {
            let _ = writeln!(out, "%TF.FilePolarity,{polarity}*%");
        }
    } else {
        let _ = writeln!(out, "G04 File function: {}*", header.function);
    }
    out.push_str("%FSLAX46Y46*%\n");
    let _ = writeln!(
        out,
        "G04 Gerber Fmt 4.6, Leading zero omitted, Abs format (unit mm)*\n\
         G04 Created by ElectronicWerk {}*",
        header.version.replace('*', "")
    );
    out.push_str("%MOMM*%\n%LPD*%\nG75*\nG01*\n");
    out.push_str("G04 Aperture macros*\n");
    out.push_str(&writer.macros);
    out.push_str("G04 Apertures*\n");
    out.push_str(&writer.apertures);
    out.push_str("G04 Image*\n");
    out.push_str(&writer.body);
    if writer
        .object
        .as_ref()
        .is_some_and(|a| *a != Attributes::default())
    {
        out.push_str("%TD*%\n");
    }
    out.push_str("M02*\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Header {
        Header {
            version: "1.0".into(),
            created: String::new(),
            project: None,
            function: "Copper,L1,Top".into(),
            polarity: Some("Positive"),
            attributes: true,
        }
    }

    #[test]
    fn escapes_reserved_characters() {
        assert_eq!(escape("A,B*C%D\\"), "A\\u002CB\\u002AC\\u0025D\\u005C");
        assert_eq!(escape("/LED_A"), "/LED_A");
    }

    #[test]
    fn lines_share_apertures_and_attributes() {
        let net = Attributes {
            net: Some("GND".into()),
            ..Attributes::default()
        };
        let line = |a: V, b: V| Feature::Line {
            a,
            b,
            width: 250_000.0,
            function: Function::Conductor,
            attributes: net.clone(),
        };
        let out = write(
            &header(),
            &[
                line(V::new(0.0, 0.0), V::new(1e6, 0.0)),
                line(V::new(1e6, 0.0), V::new(1e6, 1e6)),
            ],
        );
        assert_eq!(out.matches("%ADD10C,0.250000*%").count(), 1, "{out}");
        assert!(out.contains("%TA.AperFunction,Conductor*%"));
        assert_eq!(out.matches("%TO.N,GND*%").count(), 1);
        assert!(out.contains("X0Y0D02*\nX1000000Y0D01*\nX1000000Y1000000D01*"));
        assert!(out.ends_with("%TD*%\nM02*\n"));
    }

    #[test]
    fn pads_use_standard_apertures_or_macros() {
        let flash = |shape: Shape, at: V| Feature::Flash {
            at,
            shape,
            function: Function::SmdPad,
            attributes: Attributes::default(),
        };
        let rect = Shape::polygon(
            vec![
                V::new(-1e6, -5e5),
                V::new(1e6, -5e5),
                V::new(1e6, 5e5),
                V::new(-1e6, 5e5),
            ],
            Vec::new(),
            0.0,
        );
        let rounded = Shape::polygon(
            vec![
                V::new(-1e6, -5e5),
                V::new(1e6, -5e5),
                V::new(1e6, 5e5),
                V::new(-1e6, 5e5),
            ],
            Vec::new(),
            1e5,
        );
        let out = write(
            &header(),
            &[
                flash(Shape::disc(V::new(0.0, 0.0), 4e5), V::new(0.0, 0.0)),
                flash(rect, V::new(0.0, 0.0)),
                flash(
                    Shape::stadium(V::new(-5e5, 0.0), V::new(5e5, 0.0), 3e5),
                    V::new(0.0, 0.0),
                ),
                flash(
                    Shape::stadium(V::new(-5e5, -5e5), V::new(5e5, 5e5), 3e5),
                    V::new(0.0, 0.0),
                ),
                flash(rounded, V::new(0.0, 0.0)),
            ],
        );
        assert!(out.contains("ADD10C,0.800000*%"), "{out}");
        assert!(out.contains("ADD11R,2.000000X1.000000*%"));
        assert!(out.contains("ADD12O,1.600000X0.600000*%"));
        assert!(out.contains("%AMM1*\n1,1,0.600000,-0.500000,-0.500000*"));
        assert!(out.contains("%AMM2*\n4,1,4,-1.000000,-0.500000,"));
        assert!(out.contains("%TA.AperFunction,SMDPad,CuDef*%"));
    }

    #[test]
    fn regions_and_arcs() {
        let region = Feature::Region {
            outline: vec![V::new(0.0, 0.0), V::new(1e6, 0.0), V::new(1e6, 1e6)],
            holes: Vec::new(),
            function: Function::Conductor,
            attributes: Attributes::default(),
        };
        let arc = Feature::Arc {
            start: V::new(1e6, 0.0),
            end: V::new(-1e6, 0.0),
            center: V::new(0.0, 0.0),
            ccw: true,
            width: 1e5,
            function: Function::Other,
            attributes: Attributes::default(),
        };
        let out = write(&header(), &[region, arc]);
        assert!(out.contains("G36*\nX0Y0D02*\nX1000000Y0D01*"), "{out}");
        assert!(out.contains("G37*\n%TD.AperFunction*%"));
        assert!(out.contains("G03*\nX-1000000Y0I-1000000J0D01*"));
    }

    #[test]
    fn x1_files_have_no_attributes() {
        let out = write(
            &Header {
                attributes: false,
                ..header()
            },
            &[Feature::Line {
                a: V::new(0.0, 0.0),
                b: V::new(1.0, 0.0),
                width: 1e5,
                function: Function::Conductor,
                attributes: Attributes {
                    net: Some("A".into()),
                    ..Attributes::default()
                },
            }],
        );
        assert!(!out.contains("%T"), "{out}");
    }
}
