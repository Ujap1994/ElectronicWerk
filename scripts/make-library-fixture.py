#!/usr/bin/env python3
"""Generates the synthetic KiCad library `crates/ew-library/tests/fixtures/kicad`.

A small library with known duplicates of every class, for the tests of the
deduplication engine, the library manager and the documentation:

- symbols: `Demo:R` and `Demo_Copy:R` (exact duplicate), `Demo:LM_A` and
  `Demo:LM_B` (aliases: same drawing), `Demo:LM_A` and `Demo_Copy:LM_A`
  (near duplicate: same name and pins, other body)
- footprints: `Demo:R_0805` and `Demo_Copy:R_0805` (exact), `Demo:C_0805`
  (same pads, other graphics), `Demo:R_0805_Wide` (pads 0.03 mm apart)
- 3D models (STEP boxes made here): `R_0805` and `R_0805_Copy` (exact
  after tessellation), `R_0805_Red` (same mesh, other colour)

Everything is made up for tests; nothing is taken from the KiCad libraries.
Run from the repository root: `python3 scripts/make-library-fixture.py`.
"""

from pathlib import Path

OUT = Path("crates/ew-library/tests/fixtures/kicad")
FONT = "(effects (font (size 1.27 1.27)))"
HIDDEN = "(effects (font (size 1.27 1.27)) (hide yes))"


def prop(key, value, x=0, y=0, hide=False):
    return f'(property "{key}" "{value}" (at {x} {y} 0) {HIDDEN if hide else FONT})'


def pin(kind, num, name, x, y, angle, length=2.54):
    return (f'(pin {kind} line (at {x} {y} {angle}) (length {length}) '
            f'(name "{name}" {FONT}) (number "{num}" {FONT}))')


def symbol(name, ref, value, body, pins, props=()):
    extra = "\n    ".join(prop(k, v, hide=True) for k, v in props)
    return f'''  (symbol "{name}" (pin_names (offset 0.254)) (exclude_from_sim no) (in_bom yes) (on_board yes)
    {prop("Reference", ref, 2.54, 0)}
    {prop("Value", value, 2.54, -2.54)}
    {prop("Footprint", "", 0, 0, True)}
    {prop("Datasheet", "~", 0, 0, True)}
    {extra}
    (symbol "{name}_0_1" {body})
    (symbol "{name}_1_1" {" ".join(pins)}))'''


def library(symbols):
    return ('(kicad_symbol_lib (version 20251024) (generator "kicad_symbol_editor") (generator_version "10.0")\n'
            + "\n".join(symbols) + "\n)\n")


RECT_R = '(rectangle (start -1.016 -2.54) (end 1.016 2.54) (stroke (width 0.254) (type default)) (fill (type none)))'
R_PINS = [pin("passive", "1", "~", 0, 3.81, 270, 1.27), pin("passive", "2", "~", 0, -3.81, 90, 1.27)]
TRIANGLE = ('(polyline (pts (xy -5.08 5.08) (xy 5.08 0) (xy -5.08 -5.08) (xy -5.08 5.08)) '
            '(stroke (width 0.254) (type default)) (fill (type background)))')
BOX = '(rectangle (start -5.08 5.08) (end 5.08 -5.08) (stroke (width 0.254) (type default)) (fill (type background)))'
LM_PINS = [pin("input", "3", "+", -7.62, 2.54, 0), pin("input", "2", "-", -7.62, -2.54, 0),
           pin("output", "1", "~", 7.62, 0, 180), pin("power_in", "8", "V+", -2.54, 7.62, 270, 3.81),
           pin("power_in", "4", "V-", -2.54, -7.62, 90, 3.81)]
R_PROPS = [("Description", "Resistor"), ("ki_keywords", "R res resistor"), ("ki_fp_filters", "R_*")]

DEMO = library([
    symbol("R", "R", "R", RECT_R, R_PINS, R_PROPS),
    symbol("C", "C", "C",
           '(polyline (pts (xy -2.032 -0.762) (xy 2.032 -0.762)) (stroke (width 0.508) (type default)) (fill (type none))) '
           '(polyline (pts (xy -2.032 0.762) (xy 2.032 0.762)) (stroke (width 0.508) (type default)) (fill (type none)))',
           [pin("passive", "1", "~", 0, 3.81, 270, 2.794), pin("passive", "2", "~", 0, -3.81, 90, 2.794)],
           [("Description", "Unpolarized capacitor"), ("ki_keywords", "cap capacitor")]),
    symbol("LM_A", "U", "LM_A", TRIANGLE, LM_PINS,
           [("Description", "Demo operational amplifier A"), ("MPN", "LMA-123"), ("Manufacturer", "Demo Semi")]),
    symbol("LM_B", "U", "LM_B", TRIANGLE, LM_PINS,
           [("Description", "Demo operational amplifier B"), ("MPN", "LMB-456"), ("Manufacturer", "Demo Semi")]),
])
DEMO_COPY = library([
    symbol("R", "R", "R", RECT_R, R_PINS, R_PROPS),
    symbol("LM_A", "U", "LM_A", BOX, LM_PINS, [("Description", "Demo operational amplifier A (box style)")]),
])


def pad(num, x, y, w, h):
    return (f'(pad "{num}" smd roundrect (at {x} {y}) (size {w} {h}) (layers "F.Cu" "F.Mask" "F.Paste") '
            f'(roundrect_rratio 0.25))')


def footprint(name, description, tags, pads, graphics, model=None, attr="smd"):
    model_text = (f'\n  (model "{model}" (offset (xyz 0 0 0)) (scale (xyz 1 1 1)) (rotate (xyz 0 0 0)))'
                  if model else "")
    return f'''(footprint "{name}" (version 20241229) (generator "pcbnew") (generator_version "10.0")
  (layer "F.Cu")
  (descr "{description}")
  (tags "{tags}")
  (property "Reference" "REF**" (at 0 -1.65 0) (layer "F.SilkS") (effects (font (size 1 1) (thickness 0.15))))
  (property "Value" "{name}" (at 0 1.65 0) (layer "F.Fab") (effects (font (size 1 1) (thickness 0.15))))
  (attr {attr})
  {" ".join(graphics)}
  {" ".join(pads)}{model_text}
)
'''


def line(x1, y1, x2, y2, layer, width):
    return (f'(fp_line (start {x1} {y1}) (end {x2} {y2}) (stroke (width {width}) (type solid)) '
            f'(layer "{layer}"))')


def rect(x1, y1, x2, y2, layer, width):
    return (f'(fp_rect (start {x1} {y1}) (end {x2} {y2}) (stroke (width {width}) (type solid)) (fill no) '
            f'(layer "{layer}"))')


R_PADS = [pad("1", -0.9125, 0, 1.025, 1.4), pad("2", 0.9125, 0, 1.025, 1.4)]
R_GRAPHICS = [line(-0.227064, -0.735, 0.227064, -0.735, "F.SilkS", 0.12),
              line(-0.227064, 0.735, 0.227064, 0.735, "F.SilkS", 0.12),
              rect(-1.68, -0.95, 1.68, 0.95, "F.CrtYd", 0.05),
              rect(-1, -0.625, 1, 0.625, "F.Fab", 0.1)]
C_GRAPHICS = [line(-0.261252, -0.735, 0.261252, -0.735, "F.SilkS", 0.12),
              line(-0.261252, 0.735, 0.261252, 0.735, "F.SilkS", 0.12),
              rect(-1.7, -0.98, 1.7, 0.98, "F.CrtYd", 0.05),
              rect(-1, -0.625, 1, 0.625, "F.Fab", 0.1)]
MODEL = "${KICAD10_3DMODEL_DIR}/Demo.3dshapes/%s.wrl"
FOOTPRINTS = {
    "Demo.pretty/R_0805.kicad_mod": footprint(
        "R_0805", "Resistor SMD 0805 (demo)", "resistor", R_PADS, R_GRAPHICS, MODEL % "R_0805"),
    "Demo.pretty/C_0805.kicad_mod": footprint(
        "C_0805", "Capacitor SMD 0805 (demo)", "capacitor", R_PADS, C_GRAPHICS, MODEL % "R_0805_Red"),
    "Demo.pretty/R_0805_Wide.kicad_mod": footprint(
        "R_0805_Wide", "Resistor SMD 0805, wider pad spacing (demo)", "resistor",
        [pad("1", -0.94, 0, 1.025, 1.4), pad("2", 0.94, 0, 1.025, 1.4)], R_GRAPHICS),
    "Demo.pretty/SOIC-8.kicad_mod": footprint(
        "SOIC-8", "SOIC, 8 pins, 3.9 x 4.9 mm body, 1.27 mm pitch (demo)", "SOIC SO",
        [pad(str(i + 1), -2.475 if i < 4 else 2.475, -1.905 + 1.27 * (i if i < 4 else 7 - i), 1.95, 0.6)
         for i in range(8)],
        [rect(-3.7, -2.7, 3.7, 2.7, "F.CrtYd", 0.05), rect(-1.95, -2.45, 1.95, 2.45, "F.Fab", 0.1),
         line(-1.95, -2.56, 1.95, -2.56, "F.SilkS", 0.12), line(-1.95, 2.56, 1.95, 2.56, "F.SilkS", 0.12)],
        MODEL % "SOIC-8"),
    "Demo.pretty/MountingHole_3.2mm.kicad_mod": footprint(
        "MountingHole_3.2mm", "Mounting hole, 3.2 mm (demo)", "mounting hole",
        ['(pad "" np_thru_hole circle (at 0 0) (size 3.2 3.2) (drill 3.2) (layers "*.Cu" "*.Mask"))'],
        ['(fp_circle (center 0 0) (end 3.45 0) (stroke (width 0.05) (type solid)) (fill no) (layer "F.CrtYd"))'],
        attr="exclude_from_pos_files exclude_from_bom"),
    "Demo_Copy.pretty/R_0805.kicad_mod": footprint(
        "R_0805", "Resistor SMD 0805 (demo)", "resistor", R_PADS, R_GRAPHICS, MODEL % "R_0805"),
}


# --- STEP boxes ----------------------------------------------------------------

def step_box(name, sx, sy, sz, color):
    """A box (mm, z up, bottom at z = 0) as an AP214 STEP file with a colour."""
    entities = []

    def add(text):
        entities.append(text)
        return f"#{len(entities)}"

    ctx_app = add("APPLICATION_CONTEXT('core data for automotive mechanical design processes')")
    add(f"APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,{ctx_app})")
    ctx_product = add(f"PRODUCT_CONTEXT('',{ctx_app},'mechanical')")
    product = add(f"PRODUCT('{name}','{name}','',({ctx_product}))")
    formation = add(f"PRODUCT_DEFINITION_FORMATION('','',{product})")
    ctx_def = add(f"PRODUCT_DEFINITION_CONTEXT('part definition',{ctx_app},'design')")
    definition = add(f"PRODUCT_DEFINITION('design','',{formation},{ctx_def})")
    shape = add(f"PRODUCT_DEFINITION_SHAPE('','',{definition})")
    mm = add("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.))")
    rad = add("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))")
    sr = add("(NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT())")
    uncertainty = add(f"UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-07),{mm},'distance_accuracy_value','confusion accuracy')")
    context = add(f"(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT(({uncertainty})) "
                  f"GLOBAL_UNIT_ASSIGNED_CONTEXT(({mm},{rad},{sr})) REPRESENTATION_CONTEXT('Context3D','3D'))")

    def point(p):
        return add(f"CARTESIAN_POINT('',({p[0]:.4f},{p[1]:.4f},{p[2]:.4f}))")

    def direction(d):
        return add(f"DIRECTION('',({d[0]:.6f},{d[1]:.6f},{d[2]:.6f}))")

    x, y = sx / 2, sy / 2
    corners = [(-x, -y, 0), (x, -y, 0), (x, y, 0), (-x, y, 0),
               (-x, -y, sz), (x, -y, sz), (x, y, sz), (-x, y, sz)]
    vertices = [add(f"VERTEX_POINT('',{point(c)})") for c in corners]
    # Faces as corner indices, counterclockwise seen from outside.
    faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    edges = {}
    for face in faces:
        for i in range(4):
            a, b = face[i], face[(i + 1) % 4]
            key = (min(a, b), max(a, b))
            if key not in edges:
                pa, pb = corners[key[0]], corners[key[1]]
                d = [pb[k] - pa[k] for k in range(3)]
                length = sum(v * v for v in d) ** 0.5
                vector = add(f"VECTOR('',{direction([v / length for v in d])},1.)")
                curve = add(f"LINE('',{point(pa)},{vector})")
                edges[key] = add(f"EDGE_CURVE('',{vertices[key[0]]},{vertices[key[1]]},{curve},.T.)")
    face_ids = []
    for face in faces:
        oriented = []
        for i in range(4):
            a, b = face[i], face[(i + 1) % 4]
            key = (min(a, b), max(a, b))
            sense = ".T." if a == key[0] else ".F."
            oriented.append(add(f"ORIENTED_EDGE('',*,*,{edges[key]},{sense})"))
        loop = add(f"EDGE_LOOP('',({','.join(oriented)}))")
        bound = add(f"FACE_OUTER_BOUND('',{loop},.T.)")
        p0, p1, p3 = (corners[face[0]], corners[face[1]], corners[face[3]])
        u = [p1[k] - p0[k] for k in range(3)]
        v = [p3[k] - p0[k] for k in range(3)]
        n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
        nl = sum(c * c for c in n) ** 0.5
        ul = sum(c * c for c in u) ** 0.5
        axis = add(f"AXIS2_PLACEMENT_3D('',{point(p0)},{direction([c / nl for c in n])},"
                   f"{direction([c / ul for c in u])})")
        plane = add(f"PLANE('',{axis})")
        face_ids.append(add(f"ADVANCED_FACE('',({bound}),{plane},.T.)"))
    shell = add(f"CLOSED_SHELL('',({','.join(face_ids)}))")
    solid = add(f"MANIFOLD_SOLID_BREP('{name}',{shell})")
    origin = add(f"AXIS2_PLACEMENT_3D('',{point((0, 0, 0))},{direction((0, 0, 1))},{direction((1, 0, 0))})")
    representation = add(f"ADVANCED_BREP_SHAPE_REPRESENTATION('',({solid},{origin}),{context})")
    add(f"SHAPE_DEFINITION_REPRESENTATION({shape},{representation})")
    colour = add(f"COLOUR_RGB('',{color[0]},{color[1]},{color[2]})")
    fill_colour = add(f"FILL_AREA_STYLE_COLOUR('',{colour})")
    fill = add(f"FILL_AREA_STYLE('',({fill_colour}))")
    area = add(f"SURFACE_STYLE_FILL_AREA({fill})")
    side = add(f"SURFACE_SIDE_STYLE('',({area}))")
    usage = add(f"SURFACE_STYLE_USAGE(.BOTH.,{side})")
    assignment = add(f"PRESENTATION_STYLE_ASSIGNMENT(({usage}))")
    styled = add(f"STYLED_ITEM('color',({assignment}),{solid})")
    add(f"MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION('',({styled}),{context})")
    data = "\n".join(f"#{i + 1}={text};" for i, text in enumerate(entities))
    return f"""ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('{name}: synthetic test model'),'2;1');
FILE_NAME('{name}.step','2026-11-08T00:00:00',('ElectronicWerk'),(''),'ElectronicWerk','ElectronicWerk','');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN {{ 1 0 10303 214 1 1 1 1 }}'));
ENDSEC;
DATA;
{data}
ENDSEC;
END-ISO-10303-21;
"""


DARK = (0.15, 0.15, 0.15)
MODELS = {
    "R_0805": step_box("R_0805", 2.0, 1.25, 0.45, DARK),
    "R_0805_Copy": step_box("R_0805_Copy", 2.0, 1.25, 0.45, DARK),
    "R_0805_Red": step_box("R_0805_Red", 2.0, 1.25, 0.45, (0.8, 0.1, 0.1)),
    "SOIC-8": step_box("SOIC-8", 4.9, 3.9, 1.5, DARK),
}

(OUT / "symbols").mkdir(parents=True, exist_ok=True)
(OUT / "symbols" / "Demo.kicad_sym").write_text(DEMO)
(OUT / "symbols" / "Demo_Copy.kicad_sym").write_text(DEMO_COPY)
for path, text in FOOTPRINTS.items():
    target = OUT / "footprints" / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text)
for name, text in MODELS.items():
    target = OUT / "3dmodels" / "Demo.3dshapes" / f"{name}.step"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text)
print(f"wrote {OUT}")
