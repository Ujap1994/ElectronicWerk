//! STEP files of simple solids: planar polygons extruded along a vector
//! (prisms), each written as an exact B-rep with planar faces. The part
//! generator builds package models from them.

use super::{Writer, ascii, common, direction, point, product, real, styled, text};

/// A prism: a planar polygon and the vector it is extruded along, with a
/// colour.
#[derive(Debug, Clone, PartialEq)]
pub struct Prism {
    /// Corners of the polygon (millimetres), in order, at least three.
    pub outline: Vec<[f64; 3]>,
    /// Extrusion.
    pub extrude: [f64; 3],
    /// Colour (linear RGB 0…1).
    pub color: [f32; 3],
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = length(a).max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn neg(a: [f64; 3]) -> [f64; 3] {
    [-a[0], -a[1], -a[2]]
}

/// Normal of a polygon (Newell's method, not normalised).
fn normal(points: &[[f64; 3]]) -> [f64; 3] {
    let mut n = [0.0; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

fn refs(ids: &[u64]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn line(w: &mut Writer, from: [f64; 3], to: [f64; 3]) -> u64 {
    let d = sub(to, from);
    let p = point(w, from);
    let dir = direction(w, unit(d));
    let vector = w.add(format!("VECTOR('',#{dir},{})", real(length(d))));
    w.add(format!("LINE('',#{p},#{vector})"))
}

fn plane(w: &mut Writer, origin: [f64; 3], normal: [f64; 3], x_axis: [f64; 3]) -> u64 {
    let o = point(w, origin);
    let n = direction(w, unit(normal));
    let x = direction(w, unit(x_axis));
    let placement = w.add(format!("AXIS2_PLACEMENT_3D('',#{o},#{n},#{x})"));
    w.add(format!("PLANE('',#{placement})"))
}

/// Writes one prism; returns its `MANIFOLD_SOLID_BREP`.
pub fn write_prism(w: &mut Writer, prism: &Prism) -> u64 {
    let v = prism.extrude;
    let mut bottom = prism.outline.clone();
    // Counter-clockwise around the extrusion.
    if dot(normal(&bottom), v) < 0.0 {
        bottom.reverse();
    }
    let n = bottom.len();
    let top: Vec<[f64; 3]> = bottom.iter().map(|p| add(*p, v)).collect();
    let vertex = |w: &mut Writer, p: [f64; 3]| {
        let id = point(w, p);
        w.add(format!("VERTEX_POINT('',#{id})"))
    };
    let bottom_v: Vec<u64> = bottom.iter().map(|p| vertex(w, *p)).collect();
    let top_v: Vec<u64> = top.iter().map(|p| vertex(w, *p)).collect();
    let mut bottom_e = Vec::with_capacity(n);
    let mut top_e = Vec::with_capacity(n);
    let mut vertical = Vec::with_capacity(n);
    for i in 0..n {
        let j = (i + 1) % n;
        let curve = line(w, bottom[i], bottom[j]);
        bottom_e.push(w.add(format!(
            "EDGE_CURVE('',#{},#{},#{curve},.T.)",
            bottom_v[i], bottom_v[j]
        )));
        let curve = line(w, top[i], top[j]);
        top_e.push(w.add(format!(
            "EDGE_CURVE('',#{},#{},#{curve},.T.)",
            top_v[i], top_v[j]
        )));
        let curve = line(w, bottom[i], top[i]);
        vertical.push(w.add(format!(
            "EDGE_CURVE('',#{},#{},#{curve},.T.)",
            bottom_v[i], top_v[i]
        )));
    }
    let mut faces = Vec::new();
    let x_axis = sub(bottom[1 % n], bottom[0]);
    // Top: counter-clockwise seen from outside.
    let oriented: Vec<u64> = top_e
        .iter()
        .map(|e| w.add(format!("ORIENTED_EDGE('',*,*,#{e},.T.)")))
        .collect();
    let edge_loop = w.add(format!("EDGE_LOOP('',({}))", refs(&oriented)));
    let bound = w.add(format!("FACE_OUTER_BOUND('',#{edge_loop},.T.)"));
    let surface = plane(w, top[0], v, x_axis);
    faces.push(w.add(format!("ADVANCED_FACE('',(#{bound}),#{surface},.T.)")));
    // Bottom: the loop runs the other way.
    let oriented: Vec<u64> = bottom_e
        .iter()
        .rev()
        .map(|e| w.add(format!("ORIENTED_EDGE('',*,*,#{e},.F.)")))
        .collect();
    let edge_loop = w.add(format!("EDGE_LOOP('',({}))", refs(&oriented)));
    let bound = w.add(format!("FACE_OUTER_BOUND('',#{edge_loop},.T.)"));
    let surface = plane(w, bottom[0], neg(v), x_axis);
    faces.push(w.add(format!("ADVANCED_FACE('',(#{bound}),#{surface},.T.)")));
    // Sides: outward is right of the walking direction.
    for i in 0..n {
        let j = (i + 1) % n;
        let edges = [
            w.add(format!("ORIENTED_EDGE('',*,*,#{},.T.)", bottom_e[i])),
            w.add(format!("ORIENTED_EDGE('',*,*,#{},.T.)", vertical[j])),
            w.add(format!("ORIENTED_EDGE('',*,*,#{},.F.)", top_e[i])),
            w.add(format!("ORIENTED_EDGE('',*,*,#{},.F.)", vertical[i])),
        ];
        let edge_loop = w.add(format!("EDGE_LOOP('',({}))", refs(&edges)));
        let bound = w.add(format!("FACE_OUTER_BOUND('',#{edge_loop},.T.)"));
        let d = sub(bottom[j], bottom[i]);
        let outward = cross(d, v);
        let surface = plane(w, bottom[i], outward, d);
        faces.push(w.add(format!("ADVANCED_FACE('',(#{bound}),#{surface},.T.)")));
    }
    let shell = w.add(format!("CLOSED_SHELL('',({}))", refs(&faces)));
    w.add(format!("MANIFOLD_SOLID_BREP('',#{shell})"))
}

/// A STEP file of one part made of prisms.
#[must_use]
pub fn file(name: &str, prisms: &[Prism], version: &str) -> String {
    let mut w = Writer::new();
    let c = common(&mut w);
    let solids: Vec<u64> = prisms.iter().map(|p| write_prism(&mut w, p)).collect();
    let mut items = vec![c.origin];
    items.extend(&solids);
    let representation = w.add(format!(
        "ADVANCED_BREP_SHAPE_REPRESENTATION('{}',({}),#{})",
        text(&ascii(name)),
        refs(&items),
        c.context
    ));
    let styles: Vec<u64> = solids
        .iter()
        .zip(prisms)
        .map(|(solid, prism)| styled(&mut w, *solid, prism.color))
        .collect();
    w.add(format!(
        "MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION('',({}),#{})",
        refs(&styles),
        c.context
    ));
    product(&mut w, &c, name, representation);
    let header = format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ElectronicWerk package model'),'2;1');\nFILE_NAME('{}.step','',('ElectronicWerk'),('ElectronicWerk'),'ElectronicWerk {}','ElectronicWerk','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN {{ 1 0 10303 214 1 1 1 1 }}'));\nENDSEC;\n",
        text(&ascii(name)),
        text(version)
    );
    w.finish(&header)
}
