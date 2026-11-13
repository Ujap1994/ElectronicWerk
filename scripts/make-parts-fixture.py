#!/usr/bin/env python3
"""Generates the synthetic part downloads `crates/ew-core/tests/fixtures/parts`.

Two ZIP files laid out like the KiCad downloads of the online part libraries
(the layout only; every symbol, footprint and model is made up here):

- `NE555-DEMO.zip` like a SnapEDA download: a symbol library, a footprint
  whose pads equal `Demo:SOIC-8` of the library fixture (so the duplicate
  check finds it), a STEP model and an instructions page, all in the root.
- `ul_OPAMP-DEMO.zip` like an Ultra Librarian download: a folder per CAD
  format, KiCad 6+ files in `KiCADv6` (a symbol library and a `.pretty`
  footprint library with a new footprint that references its model by file
  name), a legacy KiCad 5 library in `KiCAD`, the STEP model in `STEP`.

Run from the repository root: `python3 scripts/make-parts-fixture.py`.
"""

import zipfile
from pathlib import Path

OUT = Path("crates/ew-core/tests/fixtures/parts")
LIBRARY = Path("crates/ew-library/tests/fixtures/kicad")
FONT = "(effects (font (size 1.27 1.27)))"
HIDDEN = "(effects (font (size 1.27 1.27)) (hide yes))"
# Fixed timestamps: the files are byte-identical on every run.
DATE = (2026, 11, 13, 12, 0, 0)


def prop(key, value, x=0.0, y=0.0, hide=False):
    return f'(property "{key}" "{value}" (at {x} {y} 0) {HIDDEN if hide else FONT})'


def pin(kind, num, name, x, y, angle, length=2.54):
    return (f'(pin {kind} line (at {x} {y} {angle}) (length {length}) '
            f'(name "{name}" {FONT}) (number "{num}" {FONT}))')


def symbol_lib(name, ref, footprint, description, body, pins, extra=()):
    props = "\n    ".join(prop(k, v, hide=True) for k, v in extra)
    return f'''(kicad_symbol_lib (version 20241209) (generator "demo_export")
  (symbol "{name}" (pin_names (offset 0.254)) (exclude_from_sim no) (in_bom yes) (on_board yes)
    {prop("Reference", ref, 0, 7.62)}
    {prop("Value", name, 0, -7.62)}
    {prop("Footprint", footprint, hide=True)}
    {prop("Datasheet", "~", hide=True)}
    {prop("Description", description, hide=True)}
    {props}
    (symbol "{name}_0_1" {body})
    (symbol "{name}_1_1" {" ".join(pins)}))
)
'''


def rect_body(w, h):
    return (f'(rectangle (start {-w} {h}) (end {w} {-h}) '
            f'(stroke (width 0.254) (type default)) (fill (type background)))')


TIMER_PINS = [
    pin("power_in", 1, "GND", -10.16, -5.08, 0),
    pin("input", 2, "TR", -10.16, -2.54, 0),
    pin("output", 3, "Q", 10.16, 0, 180),
    pin("input", 4, "R", -10.16, 5.08, 0),
    pin("input", 5, "CV", -10.16, 2.54, 0),
    pin("input", 6, "THR", 10.16, -2.54, 180),
    pin("open_collector", 7, "DIS", 10.16, -5.08, 180),
    pin("power_in", 8, "VCC", 10.16, 5.08, 180),
]

OPAMP_PINS = [
    pin("output", 1, "OUT", 10.16, 0, 180),
    pin("power_in", 2, "V-", 0, -7.62, 90),
    pin("input", 3, "+IN", -10.16, 2.54, 0),
    pin("input", 4, "-IN", -10.16, -2.54, 0),
    pin("power_in", 5, "V+", 0, 7.62, 270),
]


def sot23_5(name, model):
    pads = []
    for number, (x, y) in enumerate([(-1.1375, -0.95), (-1.1375, 0), (-1.1375, 0.95),
                                     (1.1375, 0.95), (1.1375, -0.95)], start=1):
        pads.append(f'(pad "{number}" smd rect (at {x} {y}) (size 1.075 0.6) '
                    f'(layers "F.Cu" "F.Mask" "F.Paste"))')
    return f'''(footprint "{name}" (version 20241229) (generator "demo_export")
  (layer "F.Cu")
  (descr "SOT-23-5 (demo download)")
  (property "Reference" "REF**" (at 0 -2.4 0) (layer "F.SilkS") (effects (font (size 1 1) (thickness 0.15))))
  (property "Value" "{name}" (at 0 2.4 0) (layer "F.Fab") (effects (font (size 1 1) (thickness 0.15))))
  (attr smd)
  (fp_rect (start -2 -1.75) (end 2 1.75) (stroke (width 0.05) (type solid)) (fill no) (layer "F.CrtYd"))
  (fp_rect (start -0.8 -1.45) (end 0.8 1.45) (stroke (width 0.1) (type solid)) (fill no) (layer "F.Fab"))
  {" ".join(pads)}
  (model "{model}" (offset (xyz 0 0 0)) (scale (xyz 1 1 1)) (rotate (xyz 0 0 0)))
)
'''


def write_zip(path, files):
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in files:
            info = zipfile.ZipInfo(name, DATE)
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, data)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    soic = (LIBRARY / "footprints/Demo.pretty/SOIC-8.kicad_mod").read_text()
    # Same pads and graphics as Demo:SOIC-8, another name, no model.
    snap_footprint = (soic.replace('(footprint "SOIC-8"', '(footprint "SOIC127P600X175-8N"')
                      .replace('"Value" "SOIC-8"', '"Value" "SOIC127P600X175-8N"'))
    snap_footprint = "\n".join(line for line in snap_footprint.splitlines()
                               if not line.strip().startswith("(model"))
    step = (LIBRARY / "3dmodels/Demo.3dshapes/SOIC-8.step").read_text()
    write_zip(OUT / "NE555-DEMO.zip", [
        ("NE555-DEMO.kicad_sym", symbol_lib(
            "NE555-DEMO", "U", "SOIC127P600X175-8N", "Demo timer (synthetic download)",
            rect_body(7.62, 7.62), TIMER_PINS,
            [("MPN", "NE555-DEMO"), ("Manufacturer", "Demo Semi")])),
        ("SOIC127P600X175-8N.kicad_mod", snap_footprint + "\n"),
        ("NE555-DEMO.step", step),
        ("how-to-import.htm", "<html><body><p>Demo download for tests.</p></body></html>\n"),
    ])
    opamp_step = step.replace("SOIC-8", "DBV0005A")
    write_zip(OUT / "ul_OPAMP-DEMO.zip", [
        ("ul_OPAMP-DEMO/KiCADv6/2026-11-13_OPAMP-DEMO.kicad_sym", symbol_lib(
            "OPAMP-DEMO", "U", "DBV0005A", "Demo amplifier (synthetic download)",
            "(polyline (pts (xy -7.62 7.62) (xy 7.62 0) (xy -7.62 -7.62) (xy -7.62 7.62)) "
            "(stroke (width 0.254) (type default)) (fill (type background)))",
            OPAMP_PINS, [("MPN", "OPAMP-DEMO")])),
        ("ul_OPAMP-DEMO/KiCADv6/footprints.pretty/DBV0005A.kicad_mod",
         sot23_5("DBV0005A", "DBV0005A.step")),
        ("ul_OPAMP-DEMO/KiCAD/OPAMP-DEMO.lib", "EESchema-LIBRARY Version 2.4\n#End Library\n"),
        ("ul_OPAMP-DEMO/STEP/DBV0005A.step", opamp_step),
        ("ul_OPAMP-DEMO/README.txt", "Demo download for tests.\n"),
    ])
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
