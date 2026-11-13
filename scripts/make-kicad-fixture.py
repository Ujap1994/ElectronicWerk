#!/usr/bin/env python3
"""Generates the synthetic KiCad 10 test project `crates/ew-kicad/tests/fixtures/blinky`.

A 555 LED blinker with a power sub-sheet and a 4-layer board. Everything is
made up for tests and documentation; it is not derived from any real
design. Run from the repository root: `python3 scripts/make-kicad-fixture.py`.
"""

import json
import math
import uuid
from pathlib import Path

OUT = Path("crates/ew-kicad/tests/fixtures/blinky")
NS = uuid.UUID("6f1c2b7e-1d1a-4c55-9a77-2f0d8c3e5b10")


def uid(*parts: str) -> str:
    return str(uuid.uuid5(NS, "/".join(parts)))


def fmt(v: float) -> str:
    text = f"{v:.4f}".rstrip("0").rstrip(".")
    return "0" if text in ("-0", "") else text


ROOT_UUID = uid("sheet", "root")
POWER_SHEET_UUID = uid("sheet-symbol", "power")
POWER_FILE_UUID = uid("sheet", "power")

# --- Library symbols (library coordinates, y up) -----------------------------

FONT = "(effects (font (size 1.27 1.27)))"
HIDDEN = "(effects (font (size 1.27 1.27)) (hide yes))"


def lib_pin(kind, num, name, x, y, angle, length, hide=False):
    hidden = " (hide yes)" if hide else ""
    return (f'(pin {kind} line (at {fmt(x)} {fmt(y)} {angle}) (length {fmt(length)}){hidden} '
            f'(name "{name}" {FONT}) (number "{num}" {FONT}))')


def prop(key, value, x, y, angle=0, hide=False):
    return f'(property "{key}" "{value}" (at {fmt(x)} {fmt(y)} {angle}) {HIDDEN if hide else FONT})'


SYMBOLS = {}
PINS = {}


def define(lib_id, ref, value, body, pins, footprint="", power=False, extra="",
           pin_names="(pin_names (offset 0.254))"):
    name = lib_id.split(":")[1]
    pin_text = "\n".join(lib_pin(*p) for p in pins)
    flags = "(power) " if power else ""
    SYMBOLS[lib_id] = f'''(symbol "{lib_id}" {flags}{pin_names} (exclude_from_sim no) (in_bom yes) (on_board yes)
      {prop("Reference", ref, 2.54, 0, 0, hide=power)}
      {prop("Value", value, 2.54, -2.54)}
      {prop("Footprint", footprint, 0, 0, hide=True)}
      {prop("Datasheet", "~", 0, 0, hide=True)}
      {prop("Description", value + " (demo symbol)", 0, 0, hide=True)}
      {extra}
      (symbol "{name}_0_1" {body})
      (symbol "{name}_1_1" {pin_text}))'''
    PINS[lib_id] = {p[1]: (p[3], p[4], p[5]) for p in pins}


RECT = '(rectangle (start {}) (end {}) (stroke (width 0.254) (type default)) (fill (type background)))'
define("Device:R", "R", "R", RECT.format("-1.016 -2.54", "1.016 2.54"),
       [("passive", "1", "~", 0, 3.81, 270, 1.27), ("passive", "2", "~", 0, -3.81, 90, 1.27)],
       pin_names="(pin_numbers (hide yes)) (pin_names (offset 0))")
define("Device:C", "C", "C",
       '(polyline (pts (xy -2.032 -0.762) (xy 2.032 -0.762)) (stroke (width 0.508) (type default)) (fill (type none))) '
       '(polyline (pts (xy -2.032 0.762) (xy 2.032 0.762)) (stroke (width 0.508) (type default)) (fill (type none)))',
       [("passive", "1", "~", 0, 3.81, 270, 3.048), ("passive", "2", "~", 0, -3.81, 90, 3.048)],
       pin_names="(pin_numbers (hide yes)) (pin_names (offset 0.254))")
define("Device:LED", "D", "LED",
       '(polyline (pts (xy -1.27 -1.27) (xy -1.27 1.27)) (stroke (width 0.254) (type default)) (fill (type none))) '
       '(polyline (pts (xy 1.27 -1.27) (xy 1.27 1.27) (xy -1.27 0) (xy 1.27 -1.27)) (stroke (width 0.254) (type default)) (fill (type none)))',
       [("passive", "1", "K", -3.81, 0, 0, 2.54), ("passive", "2", "A", 3.81, 0, 180, 2.54)],
       pin_names="(pin_numbers (hide yes)) (pin_names (offset 1.016) (hide yes))")
define("Timer:NE555P", "U", "NE555", RECT.format("-7.62 -10.16", "7.62 10.16"),
       [("power_in", "8", "VCC", 0, 15.24, 270, 5.08), ("power_in", "1", "GND", 0, -15.24, 90, 5.08),
        ("input", "4", "R", -12.7, 7.62, 0, 5.08), ("input", "2", "TR", -12.7, 2.54, 0, 5.08),
        ("input", "6", "THR", -12.7, -2.54, 0, 5.08), ("input", "7", "DIS", -12.7, -5.08, 0, 5.08),
        ("output", "3", "Q", 12.7, 2.54, 180, 5.08), ("input", "5", "CV", 12.7, -5.08, 180, 5.08)])
define("Connector:Conn_01x02", "J", "Conn_01x02", RECT.format("-1.27 -3.81", "1.27 1.27"),
       [("passive", "1", "Pin_1", -5.08, 0, 0, 3.81), ("passive", "2", "Pin_2", -5.08, -2.54, 0, 3.81)])
define("power:VCC", "#PWR", "VCC",
       '(polyline (pts (xy 0 0) (xy 0 1.27)) (stroke (width 0) (type default)) (fill (type none))) '
       '(polyline (pts (xy -0.762 1.27) (xy 0.762 1.27)) (stroke (width 0) (type default)) (fill (type none)))',
       [("power_in", "1", "VCC", 0, 0, 90, 0, True)], power=True)
define("power:GND", "#PWR", "GND",
       '(polyline (pts (xy 0 0) (xy 0 -1.27) (xy 1.27 -1.27) (xy 0 -2.54) (xy -1.27 -1.27) (xy 0 -1.27)) '
       '(stroke (width 0) (type default)) (fill (type none)))',
       [("power_in", "1", "GND", 0, 0, 270, 0, True)], power=True)

# KiCad's symbol transform for a rotation (library y up → sheet y down).
# A file angle of 90 is one counterclockwise turn: library "up" ends up on the left.
TRANSFORMS = {0: (1, 0, 0, -1), 90: (0, -1, -1, 0), 180: (-1, 0, 0, 1), 270: (0, 1, 1, 0)}


def pin_at(at, rotation, lib_xy):
    x1, y1, x2, y2 = TRANSFORMS[rotation]
    x, y = lib_xy
    return (at[0] + x1 * x + y1 * y, at[1] + x2 * x + y2 * y)


def outward(rotation, pin_angle, distance):
    """Direction from a pin's end away from the body, in sheet coordinates."""
    a = math.radians(pin_angle + 180)
    dx, dy = math.cos(a) * distance, math.sin(a) * distance
    x1, y1, x2, y2 = TRANSFORMS[rotation]
    return (x1 * dx + y1 * dy, x2 * dx + y2 * dy)


# --- Sheets ------------------------------------------------------------------

class Sheet:
    def __init__(self, name, file_uuid, instance_path):
        self.name = name
        self.file_uuid = file_uuid
        self.path = instance_path
        self.items = []
        self.libs = set()
        self.pwr = 0

    def symbol(self, lib_id, ref, value, at, rotation=0, footprint="", fields=None):
        self.libs.add(lib_id)
        s_uuid = uid("symbol", self.name, ref)
        hide_ref = ref.startswith("#")
        fx, fy = fields or (at[0] + 3.81, at[1] - 1.27)
        # KiCad stores field angles relative to the symbol: on a symbol turned by
        # 90° or 270°, a field with angle 90 reads horizontally.
        turned = rotation in (90, 270)
        field_angle = 90 if turned else 0
        justify = "" if turned or lib_id.startswith("power:") else " (justify left)"
        self.items.append(f'''(symbol (lib_id "{lib_id}") (at {fmt(at[0])} {fmt(at[1])} {rotation}) (unit 1)
      (exclude_from_sim no) (in_bom {"no" if hide_ref else "yes"}) (on_board {"no" if hide_ref else "yes"}) (dnp no)
      (uuid "{s_uuid}")
      (property "Reference" "{ref}" (at {fmt(fx)} {fmt(fy)} {field_angle}) (effects (font (size 1.27 1.27)){justify}{" (hide yes)" if hide_ref else ""}))
      (property "Value" "{value}" (at {fmt(fx)} {fmt(fy + 2.54)} {field_angle}) (effects (font (size 1.27 1.27)){justify}))
      (property "Footprint" "{footprint}" (at {fmt(at[0])} {fmt(at[1])} 0) {HIDDEN})
      (property "Datasheet" "~" (at {fmt(at[0])} {fmt(at[1])} 0) {HIDDEN})
      (instances (project "blinky" (path "{self.path}" (reference "{ref}") (unit 1)))))''')
        return s_uuid

    def connect(self, lib_id, at, rotation, pin, net):
        """A short wire from a pin, ended by a label or a power symbol."""
        lx, ly, angle = PINS[lib_id][pin]
        start = pin_at(at, rotation, (lx, ly))
        dx, dy = outward(rotation, angle, 2.54)
        end = (start[0] + dx, start[1] + dy)
        self.wire(start, end)
        if net in ("VCC", "GND"):
            self.pwr += 1
            power_rotation = 0
            self.symbol(f"power:{net}", f"#PWR0{self.pwr}{self.name[0]}", net, end, power_rotation,
                        fields=(end[0], end[1] + (1.27 if net == "GND" else -6.35)))
        else:
            label_angle = {(1, 0): 0, (-1, 0): 180, (0, -1): 90, (0, 1): 270}[(round(dx / 2.54), round(dy / 2.54))]
            self.label(net, end, label_angle)

    def label(self, net, at, angle):
        justify = "left" if angle in (0, 90) else "right"
        self.items.append(f'''(label "{net}" (at {fmt(at[0])} {fmt(at[1])} {angle}) (fields_autoplaced yes)
      (effects (font (size 1.27 1.27)) (justify {justify} bottom)) (uuid "{uid('label', self.name, net, fmt(at[0]), fmt(at[1]))}"))''')

    def wire(self, a, b):
        self.items.append(f'(wire (pts (xy {fmt(a[0])} {fmt(a[1])}) (xy {fmt(b[0])} {fmt(b[1])})) '
                          f'(stroke (width 0) (type default)) (uuid "{uid("wire", self.name, str(a), str(b))}"))')

    def text(self, value, at, size=1.27, bold=False):
        b = " (bold yes)" if bold else ""
        self.items.append(f'''(text "{value}" (exclude_from_sim no) (at {fmt(at[0])} {fmt(at[1])} 0)
      (effects (font (size {size} {size}){b}) (justify left bottom)) (uuid "{uid('text', self.name, value)}"))''')

    def render(self, title, sheets=""):
        libs = "\n    ".join(SYMBOLS[name] for name in sorted(self.libs))
        body = "\n  ".join(self.items)
        return f'''(kicad_sch
  (version 20260306)
  (generator "eeschema")
  (generator_version "10.0")
  (uuid "{self.file_uuid}")
  (paper "A4")
  (title_block (title "{title}") (date "2026-11-08") (rev "A") (company "Demo Lab") (comment 1 "Synthetic test project"))
  (lib_symbols
    {libs})
  {body}
  {sheets}
  (sheet_instances (path "/" (page "1")))
  (embedded_fonts no)
)
'''


root = Sheet("root", ROOT_UUID, f"/{ROOT_UUID}")
U1, R1, R2, C1, C2, R3, D1 = (127, 88.9), (96.52, 76.2), (96.52, 91.44), (96.52, 106.68), (154.94, 104.14), (165.1, 86.36), (185.42, 86.36)
root.symbol("Timer:NE555P", "U1", "NE555", U1, footprint="Package_SO:SOIC-8_3.9x4.9mm_P1.27mm", fields=(129.54, 74.93))
for pin, net in [("8", "VCC"), ("1", "GND"), ("4", "VCC"), ("2", "TRIG"), ("6", "TRIG"), ("7", "DIS"), ("3", "OUT"), ("5", "CV")]:
    root.connect("Timer:NE555P", U1, 0, pin, net)
R_FP, C_FP = "Resistor_SMD:R_0805_2012Metric", "Capacitor_SMD:C_0805_2012Metric"
root.symbol("Device:R", "R1", "1k", R1, footprint=R_FP)
root.symbol("Device:R", "R2", "10k", R2, footprint=R_FP)
root.symbol("Device:C", "C1", "10u", C1, footprint=C_FP)
root.connect("Device:R", R1, 0, "1", "VCC")
# The timing chain R1 – R2 – C1: one wire and one label between neighbours.
for (upper_lib, upper), (lower_lib, lower), net in [(("Device:R", R1), ("Device:R", R2), "DIS"),
                                                     (("Device:R", R2), ("Device:C", C1), "TRIG")]:
    a = pin_at(upper, 0, PINS[upper_lib]["2"][:2])
    b = pin_at(lower, 0, PINS[lower_lib]["1"][:2])
    root.wire(a, b)
    root.label(net, (a[0], (a[1] + b[1]) / 2), 180)
root.connect("Device:C", C1, 0, "2", "GND")
root.symbol("Device:C", "C2", "10n", C2, footprint=C_FP)
root.connect("Device:C", C2, 0, "1", "CV")
root.connect("Device:C", C2, 0, "2", "GND")
root.symbol("Device:R", "R3", "330", R3, 90, footprint="Resistor_SMD:R_0805_2012Metric", fields=(165.1, 81.28))
root.connect("Device:R", R3, 90, "1", "OUT")
root.symbol("Device:LED", "D1", "red", D1, 180, footprint="LED_SMD:LED_0805_2012Metric", fields=(182.88, 81.28))
r3_out = pin_at(R3, 90, PINS["Device:R"]["2"][:2])
d1_anode = pin_at(D1, 180, PINS["Device:LED"]["2"][:2])
root.wire(r3_out, d1_anode)
root.label("LED_A", (r3_out[0] + 2.54, r3_out[1]), 0)
root.connect("Device:LED", D1, 180, "1", "GND")
root.text("555 astable LED blinker, about 1 Hz", (76.2, 50.8), 2.0, True)
root.text("f = 1.44 / ((R1 + 2·R2) · C1)", (76.2, 58.42))
SHEET = f'''(sheet (at 172.72 99.06) (size 25.4 12.7) (exclude_from_sim no) (in_bom yes) (on_board yes) (dnp no)
    (stroke (width 0.15) (type solid)) (fill (color 0 0 0 0)) (uuid "{POWER_SHEET_UUID}")
    (property "Sheetname" "Power" (at 172.72 98.36 0) (effects (font (size 1.27 1.27)) (justify left bottom)))
    (property "Sheetfile" "power.kicad_sch" (at 172.72 112.35 0) (effects (font (size 1.27 1.27)) (justify left top)))
    (instances (project "blinky" (path "/{ROOT_UUID}" (page "2")))))'''

power = Sheet("power", POWER_FILE_UUID, f"/{ROOT_UUID}/{POWER_SHEET_UUID}")
J1 = (139.7, 101.6)
power.symbol("Connector:Conn_01x02", "J1", "Supply 5 V", J1, footprint="Connector_PinHeader_2.54mm:PinHeader_1x02_P2.54mm_Vertical")
power.connect("Connector:Conn_01x02", J1, 0, "1", "VCC")
power.connect("Connector:Conn_01x02", J1, 0, "2", "GND")
power.text("Supply: 5 V from the header", (114.3, 88.9), 1.8, True)

OUT.mkdir(parents=True, exist_ok=True)
(OUT / "blinky.kicad_sch").write_text(root.render("Blinky", SHEET))
(OUT / "power.kicad_sch").write_text(power.render("Power"))

# --- Board -------------------------------------------------------------------

NETS = {"VCC", "GND", "/DIS", "/TRIG", "/CV", "/OUT", "/LED_A"}
CONNECTIONS = {
    "U1": {"1": "GND", "2": "/TRIG", "3": "/OUT", "4": "VCC", "5": "/CV", "6": "/TRIG", "7": "/DIS", "8": "VCC"},
    "R1": {"1": "VCC", "2": "/DIS"},
    "R2": {"1": "/DIS", "2": "/TRIG"},
    "C1": {"1": "/TRIG", "2": "GND"},
    "C2": {"1": "/CV", "2": "GND"},
    "R3": {"1": "/OUT", "2": "/LED_A"},
    "D1": {"1": "GND", "2": "/LED_A"},
    "J1": {"1": "VCC", "2": "GND"},
}
SHEET_PATHS = {ref: f"/{uid('symbol', 'root', ref)}" for ref in ["U1", "R1", "R2", "C1", "C2", "R3", "D1"]}
SHEET_PATHS["J1"] = f"/{POWER_SHEET_UUID}/{uid('symbol', 'power', 'J1')}"


def fp_line(a, b, layer, width):
    return (f'(fp_line (start {fmt(a[0])} {fmt(a[1])}) (end {fmt(b[0])} {fmt(b[1])}) '
            f'(stroke (width {width}) (type solid)) (layer "{layer}") (uuid "{uid("fpline", layer, str(a), str(b))}"))')


def fp_rect(a, b, layer, width, salt):
    return (f'(fp_rect (start {fmt(a[0])} {fmt(a[1])}) (end {fmt(b[0])} {fmt(b[1])}) '
            f'(stroke (width {width}) (type solid)) (fill no) (layer "{layer}") (uuid "{uid("fprect", layer, salt)}"))')


def footprint(lib, ref, value, at, rotation, pads, body, bottom=False, smd=True):
    side = "B" if bottom else "F"
    flip = (lambda y: -y) if bottom else (lambda y: y)
    hw, hh = body
    pad_text = []
    for number, kind, shape, (px, py), (sx, sy), extra in pads:
        layers = f'"{side}.Cu" "{side}.Paste" "{side}.Mask"' if kind == "smd" else '"*.Cu" "*.Mask"'
        net = CONNECTIONS[ref].get(number)
        net_text = f' (net "{net}")' if net else ""
        pad_text.append(
            f'(pad "{number}" {kind} {shape} (at {fmt(px)} {fmt(flip(py))} {(rotation) % 360}) (size {fmt(sx)} {fmt(sy)}) '
            f'{extra}(layers {layers}){net_text} (uuid "{uid("pad", ref, number)}"))')
    silk = [fp_line((-hw, flip(-hh - 0.2)), (hw, flip(-hh - 0.2)), f"{side}.SilkS", 0.12),
            fp_line((-hw, flip(hh + 0.2)), (hw, flip(hh + 0.2)), f"{side}.SilkS", 0.12)]
    graphics = silk + [fp_rect((-hw - 0.5, -hh - 0.5), (hw + 0.5, hh + 0.5), f"{side}.CrtYd", 0.05, ref),
                       fp_rect((-hw, -hh), (hw, hh), f"{side}.Fab", 0.1, ref + "fab")]
    justify = " (justify mirror)" if bottom else ""
    return f'''(footprint "{lib}" (layer "{side}.Cu") (uuid "{uid("fp", ref)}") (at {fmt(at[0])} {fmt(at[1])} {rotation})
    (descr "{lib.split(':')[1]} (demo footprint)") (tags "demo")
    (property "Reference" "{ref}" (at 0 {fmt(flip(-hh - 1.4))} {rotation}) (layer "{side}.SilkS") (uuid "{uid("ref", ref)}")
      (effects (font (size 1 1) (thickness 0.15)){justify}))
    (property "Value" "{value}" (at 0 {fmt(flip(hh + 1.4))} {rotation}) (layer "{side}.Fab") (uuid "{uid("val", ref)}")
      (effects (font (size 1 1) (thickness 0.15)){justify}))
    (property "Datasheet" "" (at 0 0 {rotation}) (layer "{side}.Fab") (hide yes) (uuid "{uid("ds", ref)}") (effects (font (size 1.27 1.27))))
    (path "{SHEET_PATHS[ref]}")
    (attr {"smd" if smd else "through_hole"})
    {" ".join(graphics)}
    {" ".join(pad_text)}
    (model "${{KICAD10_3DMODEL_DIR}}/{lib.split(':')[0]}.3dshapes/{lib.split(':')[1]}.step" (offset (xyz 0 0 0)) (scale (xyz 1 1 1)) (rotate (xyz 0 0 0))))'''


RR = "(roundrect_rratio 0.25) "
TWO = lambda w, h, x: [("1", "smd", "roundrect", (-x, 0), (w, h), RR), ("2", "smd", "roundrect", (x, 0), (w, h), RR)]
SOIC = [(str(i + 1), "smd", "roundrect", (-2.475, -1.905 + i * 1.27), (1.95, 0.6), RR) for i in range(4)] + \
       [(str(8 - i), "smd", "roundrect", (2.475, -1.905 + i * 1.27), (1.95, 0.6), RR) for i in range(4)]
HEADER = [("1", "thru_hole", "rect", (0, 0), (1.7, 1.7), "(drill 1) "), ("2", "thru_hole", "oval", (0, 2.54), (1.7, 1.7), "(drill 1) ")]

FOOTPRINTS = [
    footprint("Package_SO:SOIC-8_3.9x4.9mm_P1.27mm", "U1", "NE555", (120, 112), 0, SOIC, (1.95, 2.45)),
    footprint("Resistor_SMD:R_0805_2012Metric", "R1", "1k", (108, 104), 90, TWO(1.025, 1.4, 0.9125), (1, 0.625)),
    footprint("Resistor_SMD:R_0805_2012Metric", "R2", "10k", (108, 111), 90, TWO(1.025, 1.4, 0.9125), (1, 0.625)),
    footprint("Capacitor_SMD:C_0805_2012Metric", "C1", "10u", (108, 118), 90, TWO(1.0, 1.45, 0.95), (1, 0.625)),
    footprint("Capacitor_SMD:C_0805_2012Metric", "C2", "10n", (128, 119), 0, TWO(1.0, 1.45, 0.95), (1, 0.625), bottom=True),
    footprint("Resistor_SMD:R_0805_2012Metric", "R3", "330", (130, 108), 0, TWO(1.025, 1.4, 0.9125), (1, 0.625)),
    footprint("LED_SMD:LED_0805_2012Metric", "D1", "red", (135.5, 108), 0, TWO(1.05, 1.2, 1.05), (1, 0.625)),
    footprint("Connector_PinHeader_2.54mm:PinHeader_1x02_P2.54mm_Vertical", "J1", "Supply 5 V", (104, 126), 270,
              HEADER, (1.27, 2.54), smd=False),
]


def seg(a, b, layer, net, width=0.25):
    return (f'(segment (start {fmt(a[0])} {fmt(a[1])}) (end {fmt(b[0])} {fmt(b[1])}) (width {width}) '
            f'(layer "{layer}") (net "{net}") (uuid "{uid("seg", str(a), str(b), layer)}"))')


TRACKS = [
    # Corners are chamfered at 45° (no right angles, as ElectronicWerk routes).
    seg((108, 103.0875), (108, 102), "F.Cu", "VCC", 0.5),
    seg((108, 102), (109, 101), "F.Cu", "VCC", 0.5),
    seg((109, 101), (121.475, 101), "F.Cu", "VCC", 0.5),
    seg((121.475, 101), (122.475, 102), "F.Cu", "VCC", 0.5),
    seg((122.475, 102), (122.475, 110.095), "F.Cu", "VCC", 0.5),
    seg((108, 104.9125), (108, 110.0875), "F.Cu", "/DIS"),
    seg((108, 111.9125), (108, 117.05), "F.Cu", "/TRIG"),
    seg((122.475, 113.905), (126, 113.905), "F.Cu", "/OUT"),
    seg((126, 113.905), (129.0875, 110.8175), "F.Cu", "/OUT"),
    seg((129.0875, 110.8175), (129.0875, 108), "F.Cu", "/OUT"),
    seg((130.9125, 108), (134.45, 108), "F.Cu", "/LED_A"),
    seg((122.475, 112.635), (125, 115), "In2.Cu", "/CV"),
    seg((125, 115), (127.05, 119), "B.Cu", "/CV"),
    '(arc (start 104 126) (mid 105.5 121.5) (end 108 118.95) (width 0.5) (layer "B.Cu") (net "GND") '
    f'(uuid "{uid("arc", "gnd")}"))',
]
VIAS = [
    f'(via (at 125 115) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (net "/CV") (uuid "{uid("via", "cv")}"))',
    f'(via (at 137 108) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (net "GND") (uuid "{uid("via", "gnd")}"))',
]
ZONE = f'''(zone (net "GND") (layer "In1.Cu") (uuid "{uid("zone", "gnd")}") (name "GND") (hatch edge 0.5) (priority 0)
    (connect_pads (clearance 0.3)) (min_thickness 0.25) (filled_areas_thickness no)
    (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5) (island_removal_mode 0))
    (polygon (pts (xy 100.5 100.5) (xy 139.5 100.5) (xy 139.5 129.5) (xy 100.5 129.5)))
    (filled_polygon (layer "In1.Cu") (pts (xy 100.625 100.625) (xy 139.375 100.625) (xy 139.375 129.375) (xy 100.625 129.375))))'''

stack = '''(stackup
      (layer "F.SilkS" (type "Top Silk Screen") (color "White"))
      (layer "F.Paste" (type "Top Solder Paste"))
      (layer "F.Mask" (type "Top Solder Mask") (color "Green") (thickness 0.01))
      (layer "F.Cu" (type "copper") (thickness 0.035))
      (layer "dielectric 1" (type "prepreg") (thickness 0.2104) (material "FR4") (epsilon_r 4.4) (loss_tangent 0.02))
      (layer "In1.Cu" (type "copper") (thickness 0.035))
      (layer "dielectric 2" (type "core") (thickness 1.065) (material "FR4") (epsilon_r 4.5) (loss_tangent 0.02))
      (layer "In2.Cu" (type "copper") (thickness 0.035))
      (layer "dielectric 3" (type "prepreg") (thickness 0.2104) (material "FR4") (epsilon_r 4.4) (loss_tangent 0.02))
      (layer "B.Cu" (type "copper") (thickness 0.035))
      (layer "B.Mask" (type "Bottom Solder Mask") (color "Green") (thickness 0.01))
      (layer "B.Paste" (type "Bottom Solder Paste"))
      (layer "B.SilkS" (type "Bottom Silk Screen") (color "White"))
      (copper_finish "ENIG") (dielectric_constraints no))'''

BOARD = f'''(kicad_pcb
  (version 20260206)
  (generator "pcbnew")
  (generator_version "10.0")
  (general (thickness 1.6) (legacy_teardrops no))
  (paper "A4")
  (title_block (title "Blinky") (rev "A"))
  (layers
    (0 "F.Cu" signal) (4 "In1.Cu" power "GND plane") (6 "In2.Cu" signal) (2 "B.Cu" signal)
    (9 "F.Adhes" user "F.Adhesive") (11 "B.Adhes" user "B.Adhesive") (13 "F.Paste" user) (15 "B.Paste" user)
    (5 "F.SilkS" user "F.Silkscreen") (7 "B.SilkS" user "B.Silkscreen") (1 "F.Mask" user) (3 "B.Mask" user)
    (17 "Dwgs.User" user "User.Drawings") (19 "Cmts.User" user "User.Comments") (25 "Edge.Cuts" user)
    (27 "Margin" user) (31 "F.CrtYd" user "F.Courtyard") (29 "B.CrtYd" user "B.Courtyard")
    (35 "F.Fab" user) (33 "B.Fab" user))
  (setup
    {stack}
    (pad_to_mask_clearance 0)
    (aux_axis_origin 100 130))
  {chr(10).join("  " + f for f in FOOTPRINTS)}
  {chr(10).join("  " + t for t in TRACKS)}
  {chr(10).join("  " + v for v in VIAS)}
  {ZONE}
  (gr_rect (start 100 100) (end 140 130) (stroke (width 0.1) (type solid)) (fill no) (layer "Edge.Cuts") (uuid "{uid("outline")}"))
  (gr_text "BLINKY rev A" (at 121 128 0) (layer "F.SilkS") (uuid "{uid("silk-text")}")
    (effects (font (size 1.5 1.5) (thickness 0.25) bold) (justify left bottom)))
  (gr_circle (center 137 103) (end 138.2 103) (stroke (width 0.1) (type solid)) (fill no) (layer "Dwgs.User") (uuid "{uid("mark")}"))
  (dimension (type aligned) (layer "Dwgs.User") (uuid "{uid("dim")}") (pts (xy 100 132) (xy 140 132)) (height 2))
  (embedded_fonts no)
)
'''
(OUT / "blinky.kicad_pcb").write_text(BOARD)

# --- Project files -------------------------------------------------------------

pro = {
    "board": {"design_settings": {"rules": {
        "min_clearance": 0.2, "min_track_width": 0.2, "min_via_diameter": 0.6, "min_through_hole_diameter": 0.3,
        "min_via_annular_width": 0.15, "min_hole_to_hole": 0.25, "min_copper_edge_clearance": 0.5,
        "min_text_thickness": 0.12}}},
    "meta": {"filename": "blinky.kicad_pro", "version": 3},
    "net_settings": {
        "classes": [
            {"name": "Default", "clearance": 0.2, "track_width": 0.25, "via_diameter": 0.8, "via_drill": 0.4,
             "diff_pair_width": 0.2, "diff_pair_gap": 0.25},
            {"name": "Power", "clearance": 0.25, "track_width": 0.5, "via_diameter": 1.0, "via_drill": 0.5,
             "diff_pair_width": 0.2, "diff_pair_gap": 0.25}],
        "meta": {"version": 5},
        "netclass_patterns": [{"netclass": "Power", "pattern": "VCC"}, {"netclass": "Power", "pattern": "GND"}]},
    "sheets": [[ROOT_UUID, "Root"], [POWER_SHEET_UUID, "Power"]],
    "text_variables": {"AUTHOR": "Demo Lab"},
}
(OUT / "blinky.kicad_pro").write_text(json.dumps(pro, indent=2) + "\n")
(OUT / "blinky.kicad_dru").write_text(
    '(version 1)\n(rule "LED clearance"\n  (condition "A.NetName == \'/LED_A\'")\n  (constraint clearance (min 0.3mm)))\n')
(OUT / "sym-lib-table").write_text(
    '(sym_lib_table\n  (version 7)\n  (lib (name "Blinky")(type "KiCad")(uri "${KIPRJMOD}/Blinky.kicad_sym")(options "")(descr ""))\n'
    '  (lib (name "Device")(type "KiCad")(uri "${KICAD10_SYMBOL_DIR}/Device.kicad_sym")(options "")(descr ""))\n)\n')
(OUT / "fp-lib-table").write_text(
    '(fp_lib_table\n  (version 7)\n  (lib (name "Blinky")(type "KiCad")(uri "${KIPRJMOD}/Blinky.pretty")(options "")(descr ""))\n)\n')
(OUT / "Blinky.kicad_sym").write_text(f'''(kicad_symbol_lib (version 20251024) (generator "kicad_symbol_editor") (generator_version "10.0")
  (symbol "TestPoint" (pin_numbers (hide yes)) (pin_names (offset 0.762) (hide yes)) (exclude_from_sim no) (in_bom yes) (on_board yes)
    {prop("Reference", "TP", 0, 3.048)}
    {prop("Value", "TestPoint", 0, 2.032)}
    {prop("Description", "Test point (demo symbol)", 0, 0, hide=True)}
    (symbol "TestPoint_0_1" (circle (center 0 1.524) (radius 0.762) (stroke (width 0) (type default)) (fill (type none))))
    (symbol "TestPoint_1_1" {lib_pin("passive", "1", "1", 0, 0, 90, 0.762)}))
  (symbol "TestPoint_Small" (extends "TestPoint") {prop("Value", "TestPoint_Small", 0, 2.032)}))
''')
pretty = OUT / "Blinky.pretty"
pretty.mkdir(exist_ok=True)
(pretty / "TestPoint_Pad_1.0mm.kicad_mod").write_text('''(footprint "TestPoint_Pad_1.0mm" (version 20241229) (generator "pcbnew") (layer "F.Cu")
  (descr "SMD round test point (demo footprint)") (tags "test point")
  (property "Reference" "REF**" (at 0 -1.5 0) (layer "F.SilkS") (effects (font (size 1 1) (thickness 0.15))))
  (property "Value" "TestPoint_Pad_1.0mm" (at 0 1.5 0) (layer "F.Fab") (effects (font (size 1 1) (thickness 0.15))))
  (attr smd)
  (fp_circle (center 0 0) (end 0.8 0) (stroke (width 0.12) (type solid)) (fill no) (layer "F.SilkS"))
  (pad "1" smd circle (at 0 0) (size 1 1) (layers "F.Cu" "F.Mask")))
''')

# --- Netlists -------------------------------------------------------------------

by_net = {}
for ref, pins in CONNECTIONS.items():
    for pin, net in pins.items():
        by_net.setdefault(net, []).append((ref, pin))
values = {"U1": "NE555", "R1": "1k", "R2": "10k", "C1": "10u", "C2": "10n", "R3": "330", "D1": "red", "J1": "Supply 5 V"}
comps = "\n".join(f'    (comp (ref "{r}") (value "{v}"))' for r, v in values.items())
nets = "\n".join(
    f'    (net (code "{i + 1}") (name "{name}") (class "Default")\n' +
    "\n".join(f'      (node (ref "{r}") (pin "{p}"))' for r, p in sorted(nodes)) + ")"
    for i, (name, nodes) in enumerate(sorted(by_net.items())))
(OUT / "blinky.net").write_text(f'(export (version "E")\n  (design (source "blinky.kicad_sch") (tool "Eeschema 10.0"))\n  (components\n{comps})\n  (nets\n{nets}))\n')
xml_nets = "\n".join(
    f'    <net code="{i + 1}" name="{name}">' + "".join(f'<node ref="{r}" pin="{p}"/>' for r, p in sorted(nodes)) + "</net>"
    for i, (name, nodes) in enumerate(sorted(by_net.items())))
(OUT / "blinky-netlist.xml").write_text(f'<?xml version="1.0" encoding="UTF-8"?>\n<export version="E">\n  <nets>\n{xml_nets}\n  </nets>\n</export>\n')
rows = "\n".join(f"{name};{r};{p}" for name, nodes in sorted(by_net.items()) for r, p in sorted(nodes))
(OUT / "blinky-netlist.csv").write_text(f"Net;Designator;Pin\n{rows}\n")
print(f"wrote {OUT}")
