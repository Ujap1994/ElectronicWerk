#!/usr/bin/env python3
"""Generates the synthetic Eagle test project `crates/ew-eagle/tests/fixtures/blinky`.

A 555 LED blinker in Eagle 9 XML: a schematic with two sheets (`blinky.sch`),
the board (`blinky.brd`) and the library (`blinky.lbr`). Everything is made
up for tests and documentation; it is not derived from any real design.
Run from the repository root: `python3 scripts/make-eagle-fixture.py`.

Orientations follow Eagle (and KiCad's reading of Eagle files): `R90`
turns counter-clockwise, `MR90` turns first and then mirrors x; mirrored
parts on a board sit on the bottom side.
"""

import math
from pathlib import Path
from xml.sax.saxutils import escape, quoteattr

OUT = Path("crates/ew-eagle/tests/fixtures/blinky")


def fmt(v: float) -> str:
    text = f"{v:.4f}".rstrip("0").rstrip(".")
    return "0" if text in ("-0", "") else text


def place(x: float, y: float, rot: str) -> tuple:
    """A local point placed with an Eagle orientation like `MR90`."""
    mirror = rot.startswith("M")
    angle = float(rot.lstrip("MSR") or 0)
    a = math.radians(angle)
    rx = x * math.cos(a) - y * math.sin(a)
    ry = x * math.sin(a) + y * math.cos(a)
    if mirror:
        rx = -rx
    return (round(rx, 4), round(ry, 4))


def attrs(**values) -> str:
    return " ".join(f"{k.rstrip('_')}={quoteattr(str(v))}" for k, v in values.items())


# --- Layers ------------------------------------------------------------------

LAYERS = [
    (1, "Top"), (2, "Route2"), (15, "Route15"), (16, "Bottom"), (17, "Pads"), (18, "Vias"),
    (19, "Unrouted"), (20, "Dimension"), (21, "tPlace"), (22, "bPlace"), (25, "tNames"),
    (26, "bNames"), (27, "tValues"), (28, "bValues"), (29, "tStop"), (30, "bStop"),
    (31, "tCream"), (32, "bCream"), (35, "tGlue"), (36, "bGlue"), (39, "tKeepout"),
    (40, "bKeepout"), (41, "tRestrict"), (42, "bRestrict"), (43, "vRestrict"), (44, "Drills"),
    (45, "Holes"), (46, "Milling"), (47, "Measures"), (48, "Document"), (49, "Reference"),
    (51, "tDocu"), (52, "bDocu"), (91, "Nets"), (92, "Busses"), (93, "Pins"), (94, "Symbols"),
    (95, "Names"), (96, "Values"), (97, "Info"), (98, "Guide"),
]


def layers_xml() -> str:
    rows = [
        f'<layer number="{n}" name="{name}" color="7" fill="1" visible="yes" active="yes"/>'
        for n, name in LAYERS
    ]
    return "<layers>\n" + "\n".join(rows) + "\n</layers>"


# --- Library -----------------------------------------------------------------

# Symbol pins: name, x, y, rot, direction, length.
SYMBOLS = {
    "R": {
        "graphics": [
            '<wire x1="-2.54" y1="-0.889" x2="2.54" y2="-0.889" width="0.254" layer="94"/>',
            '<wire x1="2.54" y1="0.889" x2="-2.54" y2="0.889" width="0.254" layer="94"/>',
            '<wire x1="2.54" y1="-0.889" x2="2.54" y2="0.889" width="0.254" layer="94"/>',
            '<wire x1="-2.54" y1="-0.889" x2="-2.54" y2="0.889" width="0.254" layer="94"/>',
        ],
        "name": (-3.81, 1.4986),
        "value": (-3.81, -3.302),
        "pins": [("1", -5.08, 0, "R0", "pas", "short"), ("2", 5.08, 0, "R180", "pas", "short")],
        "visible": "off",
    },
    "C": {
        "graphics": [
            '<rectangle x1="-2.032" y1="-2.032" x2="2.032" y2="-1.524" layer="94"/>',
            '<rectangle x1="-2.032" y1="-1.016" x2="2.032" y2="-0.508" layer="94"/>',
            '<wire x1="0" y1="0" x2="0" y2="-0.508" width="0.1524" layer="94"/>',
            '<wire x1="0" y1="-2.54" x2="0" y2="-2.032" width="0.1524" layer="94"/>',
        ],
        "name": (1.016, 0.4826),
        "value": (1.016, -4.191),
        "pins": [("1", 0, 2.54, "R270", "pas", "short"), ("2", 0, -5.08, "R90", "pas", "short")],
        "visible": "off",
    },
    "LED": {
        "graphics": [
            '<wire x1="1.27" y1="0" x2="0" y2="-2.54" width="0.254" layer="94"/>',
            '<wire x1="0" y1="-2.54" x2="-1.27" y2="0" width="0.254" layer="94"/>',
            '<wire x1="1.27" y1="-2.54" x2="-1.27" y2="-2.54" width="0.254" layer="94"/>',
            '<wire x1="-1.27" y1="0" x2="1.27" y2="0" width="0.254" layer="94"/>',
            '<wire x1="2" y1="-1" x2="3" y2="0" width="0.1524" layer="94" curve="-90"/>',
            '<circle x="0" y="-1.27" radius="2.2" width="0.1524" layer="94"/>',
        ],
        "name": (3.556, -4.572),
        "value": (5.715, -4.572),
        "pins": [("A", 0, 2.54, "R270", "pas", "short"), ("C", 0, -5.08, "R90", "pas", "short")],
        "visible": "off",
    },
    "NE555": {
        "graphics": [
            '<wire x1="-7.62" y1="-10.16" x2="7.62" y2="-10.16" width="0.4064" layer="94"/>',
            '<wire x1="7.62" y1="-10.16" x2="7.62" y2="10.16" width="0.4064" layer="94"/>',
            '<wire x1="7.62" y1="10.16" x2="-7.62" y2="10.16" width="0.4064" layer="94"/>',
            '<wire x1="-7.62" y1="10.16" x2="-7.62" y2="-10.16" width="0.4064" layer="94"/>',
            '<text x="-5" y="0" size="1.27" layer="94">555</text>',
        ],
        "name": (-7.62, 10.795),
        "value": (-7.62, -12.7),
        "pins": [
            ("GND", -12.7, -7.62, "R0", "pwr", "middle"),
            ("TRIG", -12.7, -2.54, "R0", "in", "middle"),
            ("OUT", 12.7, 0, "R180", "out", "middle"),
            ("RESET", -12.7, 7.62, "R0", "in", "middle"),
            ("CV", -12.7, 2.54, "R0", "in", "middle"),
            ("THR", 12.7, -5.08, "R180", "in", "middle"),
            ("DIS", 12.7, 5.08, "R180", "oc", "middle"),
            ("VCC", 12.7, -10.16 + 17.78, "R180", "pwr", "middle"),
        ],
        "visible": "both",
    },
    "CONN2": {
        "graphics": [
            '<wire x1="-1.27" y1="-2.54" x2="3.81" y2="-2.54" width="0.4064" layer="94"/>',
            '<wire x1="3.81" y1="-2.54" x2="3.81" y2="5.08" width="0.4064" layer="94"/>',
            '<wire x1="3.81" y1="5.08" x2="-1.27" y2="5.08" width="0.4064" layer="94"/>',
            '<wire x1="-1.27" y1="5.08" x2="-1.27" y2="-2.54" width="0.4064" layer="94"/>',
        ],
        "name": (-1.27, 5.842),
        "value": (-1.27, -5.08),
        "pins": [("1", -5.08, 0, "R0", "pas", "middle"), ("2", -5.08, 2.54, "R0", "pas", "middle")],
        "visible": "pad",
    },
    "GND": {
        "graphics": [
            '<wire x1="-1.905" y1="0" x2="1.905" y2="0" width="0.254" layer="94"/>',
        ],
        "name": None,
        "value": (-2.54, -2.54),
        "pins": [("GND", 0, 2.54, "R270", "sup", "short")],
        "visible": "off",
    },
    "VCC": {
        "graphics": [
            '<wire x1="1.27" y1="-1.905" x2="0" y2="0" width="0.254" layer="94"/>',
            '<wire x1="0" y1="0" x2="-1.27" y2="-1.905" width="0.254" layer="94"/>',
        ],
        "name": None,
        "value": (-2.54, -2.54),
        "pins": [("VCC", 0, -2.54, "R90", "sup", "short")],
        "visible": "off",
    },
}


def symbol_xml(name: str, sym: dict) -> str:
    rows = list(sym["graphics"])
    if sym["name"] is not None:
        x, y = sym["name"]
        rows.append(f'<text x="{fmt(x)}" y="{fmt(y)}" size="1.778" layer="95">&gt;NAME</text>')
    x, y = sym["value"]
    rows.append(f'<text x="{fmt(x)}" y="{fmt(y)}" size="1.778" layer="96">&gt;VALUE</text>')
    for pin, px, py, rot, direction, length in sym["pins"]:
        rot_attr = f' rot="{rot}"' if rot != "R0" else ""
        rows.append(
            f'<pin name="{escape(pin)}" x="{fmt(px)}" y="{fmt(py)}" visible="{sym["visible"]}" '
            f'length="{length}" direction="{direction}"{rot_attr}/>'
        )
    return f'<symbol name="{name}">\n' + "\n".join(rows) + "\n</symbol>"


def smd(name, x, y, dx, dy, rot="R0", roundness=0, layer=1):
    extra = f' rot="{rot}"' if rot != "R0" else ""
    if roundness:
        extra += f' roundness="{roundness}"'
    return {"kind": "smd", "name": name, "x": x, "y": y, "dx": dx, "dy": dy, "rot": rot,
            "xml": f'<smd name="{name}" x="{fmt(x)}" y="{fmt(y)}" dx="{fmt(dx)}" dy="{fmt(dy)}" layer="{layer}"{extra}/>'}


def pad(name, x, y, drill, diameter, shape="round", rot="R0"):
    extra = f' shape="{shape}"' if shape != "round" else ""
    if rot != "R0":
        extra += f' rot="{rot}"'
    return {"kind": "pad", "name": name, "x": x, "y": y, "rot": rot,
            "xml": f'<pad name="{name}" x="{fmt(x)}" y="{fmt(y)}" drill="{fmt(drill)}" diameter="{fmt(diameter)}"{extra}/>'}


PACKAGES = {
    "R0805": {
        "pads": [smd("1", -0.95, 0, 1.3, 1.5), smd("2", 0.95, 0, 1.3, 1.5)],
        "graphics": [
            '<wire x1="-0.41" y1="0.635" x2="0.41" y2="0.635" width="0.1524" layer="21"/>',
            '<wire x1="-0.41" y1="-0.635" x2="0.41" y2="-0.635" width="0.1524" layer="21"/>',
            '<rectangle x1="-1" y1="-0.65" x2="1" y2="0.65" layer="51"/>',
        ],
    },
    "C0603": {
        "pads": [smd("1", -0.85, 0, 1.1, 1.0, roundness=25), smd("2", 0.85, 0, 1.1, 1.0, roundness=25)],
        "graphics": ['<wire x1="-0.356" y1="0.432" x2="0.356" y2="0.432" width="0.1016" layer="51"/>'],
    },
    "SO08": {
        "pads": [smd(str(n), -1.905 + 1.27 * ((n - 1) % 4) if n <= 4 else 1.905 - 1.27 * ((n - 5) % 4),
                     -2.6 if n <= 4 else 2.6, 0.6, 2.2) for n in range(1, 9)],
        "graphics": [
            '<wire x1="2.4" y1="1.9" x2="2.4" y2="-1.9" width="0.2032" layer="21"/>',
            '<wire x1="-2.4" y1="-1.9" x2="-2.4" y2="1.9" width="0.2032" layer="21"/>',
            '<circle x="-1.9" y="-1.3" radius="0.3" width="0" layer="21"/>',
        ],
    },
    "LED3MM": {
        "pads": [pad("A", -1.27, 0, 0.8, 1.6, "square"), pad("C", 1.27, 0, 0.8, 1.6)],
        "graphics": ['<circle x="0" y="0" radius="1.5" width="0.2032" layer="21"/>'],
    },
    "HDR2": {
        "pads": [pad("1", 0, -1.27, 1.0, 1.7, "long", "R90"), pad("2", 0, 1.27, 1.0, 1.7, "octagon")],
        "graphics": ['<wire x1="-1.27" y1="-2.54" x2="1.27" y2="-2.54" width="0.1524" layer="21"/>'],
    },
}


def package_xml(name: str, pkg: dict) -> str:
    rows = [p["xml"] for p in pkg["pads"]] + list(pkg["graphics"])
    rows.append('<text x="-1.27" y="1.27" size="0.8" layer="25">&gt;NAME</text>')
    rows.append('<text x="-1.27" y="-2" size="0.8" layer="27">&gt;VALUE</text>')
    return f'<package name="{name}">\n<description>Synthetic {name}</description>\n' + "\n".join(rows) + "\n</package>"


# Device sets: name, prefix, gates [(gate, symbol, x, y)], devices {device: (package, {pin: pad})}.
DEVICESETS = {
    "R": ("R", [("G$1", "R")], {"0805": ("R0805", {"1": "1", "2": "2"})}, "Resistor"),
    "C": ("C", [("G$1", "C")], {"0603": ("C0603", {"1": "1", "2": "2"})}, "Capacitor"),
    "LED": ("D", [("G$1", "LED")], {"": ("LED3MM", {"A": "A", "C": "C"})}, "Red LED, 3 mm"),
    "NE555": ("U", [("A", "NE555")], {"D": ("SO08", {
        "GND": "1", "TRIG": "2", "OUT": "3", "RESET": "4", "CV": "5", "THR": "6", "DIS": "7", "VCC": "8"})},
        "Timer"),
    "CONN2": ("J", [("G$1", "CONN2")], {"": ("HDR2", {"1": "1", "2": "2"})}, "Two pins"),
    "GND": ("", [("1", "GND")], {"": (None, {})}, "Ground"),
    "VCC": ("", [("1", "VCC")], {"": (None, {})}, "Supply"),
}


def deviceset_xml(name: str) -> str:
    prefix, gates, devices, description = DEVICESETS[name]
    gate_rows = "\n".join(f'<gate name="{g}" symbol="{s}" x="0" y="0"/>' for g, s in gates)
    device_rows = []
    for device, (package, pins) in devices.items():
        package_attr = f' package="{package}"' if package else ""
        connects = "".join(
            f'<connect gate="{gates[0][0]}" pin="{escape(pin)}" pad="{pad_name}"/>'
            for pin, pad_name in pins.items()
        )
        device_rows.append(
            f'<device name="{device}"{package_attr}>\n<connects>{connects}</connects>\n'
            '<technologies><technology name=""/></technologies>\n</device>'
        )
    prefix_attr = f' prefix="{prefix}"' if prefix else ""
    return (f'<deviceset name="{name}"{prefix_attr} uservalue="yes">\n<description>{escape(description)}</description>\n'
            f'<gates>\n{gate_rows}\n</gates>\n<devices>\n' + "\n".join(device_rows) + "\n</devices>\n</deviceset>")


def library_xml() -> str:
    return ('<library name="blinky">\n<description>Synthetic test library</description>\n<packages>\n'
            + "\n".join(package_xml(n, p) for n, p in PACKAGES.items())
            + "\n</packages>\n<symbols>\n"
            + "\n".join(symbol_xml(n, s) for n, s in SYMBOLS.items())
            + "\n</symbols>\n<devicesets>\n"
            + "\n".join(deviceset_xml(n) for n in DEVICESETS)
            + "\n</devicesets>\n</library>")


# --- Parts and nets ----------------------------------------------------------

# part: (deviceset, device, value, sheet, x, y, rot)
PARTS = {
    "U1": ("NE555", "D", "NE555", 1, 101.6, 101.6, "R0"),
    "R1": ("R", "0805", "10k", 1, 60.96, 121.92, "R90"),
    "R2": ("R", "0805", "47k", 1, 60.96, 86.36, "R270"),
    "C1": ("C", "0603", "10u", 1, 40.64, 71.12, "R0"),
    "C2": ("C", "0603", "10n", 1, 152.4, 71.12, "MR0"),
    "R3": ("R", "0805", "330", 1, 152.4, 111.76, "MR90"),
    "D1": ("LED", "", "red", 1, 172.72, 91.44, "R180"),
    "J1": ("CONN2", "", "POWER", 2, 50.8, 101.6, "R0"),
    "GND1": ("GND", "", "GND", 1, 101.6, 50.8, "R0"),
    "GND2": ("GND", "", "GND", 2, 76.2, 76.2, "R0"),
    "VCC1": ("VCC", "", "VCC", 1, 127.0, 139.7, "R0"),
    "VCC2": ("VCC", "", "VCC", 2, 76.2, 127.0, "R0"),
}

# net name -> [(part, pin)]
NETS = {
    "VCC": [("U1", "VCC"), ("U1", "RESET"), ("R1", "2"), ("VCC1", "VCC"), ("J1", "2"), ("VCC2", "VCC")],
    "GND": [("U1", "GND"), ("C1", "2"), ("C2", "2"), ("D1", "C"), ("GND1", "GND"), ("J1", "1"), ("GND2", "GND")],
    "N$1": [("R1", "1"), ("R2", "2"), ("U1", "DIS")],
    "TIMING": [("R2", "1"), ("C1", "1"), ("U1", "TRIG"), ("U1", "THR")],
    "N$2": [("U1", "CV"), ("C2", "1")],
    "N$3": [("U1", "OUT"), ("R3", "1")],
    "LED": [("R3", "2"), ("D1", "A")],
}


def pin_point(part: str, pin: str) -> tuple:
    deviceset, _, _, _, x, y, rot = PARTS[part]
    symbol = SYMBOLS[DEVICESETS[deviceset][1][0][1]]
    for name, px, py, *_ in symbol["pins"]:
        if name == pin:
            dx, dy = place(px, py, rot)
            return (round(x + dx, 4), round(y + dy, 4))
    raise KeyError(f"{part}.{pin}")


def gate_of(part: str) -> str:
    return DEVICESETS[PARTS[part][0]][1][0][0]


def schematic_nets(sheet: int) -> str:
    """Each net on a sheet: every pin goes a short way out from its part to
    a column of its own (between the grid lines, so it meets no pin), then
    down to the net's row below the parts. A label names the net on each
    segment (Eagle connects segments of the same name)."""
    rows = []
    used = set()
    for index, (net, pins) in enumerate(NETS.items()):
        here = [(p, n) for p, n in pins if PARTS[p][3] == sheet]
        if not here:
            continue
        row_y = 25.4 - 2.54 * index
        wires = []
        xs = []
        for part, pin in here:
            px, py = pin_point(part, pin)
            side = -1 if px < PARTS[part][4] else 1
            step = 1
            cx = round(px + side * 1.27, 4)
            while cx in used:
                step += 1
                cx = round(px + side * (1.27 + 2.54 * (step - 1)), 4)
            used.add(cx)
            wires.append((px, py, cx, py))
            wires.append((cx, py, cx, row_y))
            xs.append(cx)
        if len(xs) > 1:
            wires.append((min(xs), row_y, max(xs), row_y))
        segment = [f'<pinref part="{p}" gate="{gate_of(p)}" pin="{escape(n)}"/>' for p, n in here]
        segment += [
            f'<wire x1="{fmt(a)}" y1="{fmt(b)}" x2="{fmt(c)}" y2="{fmt(d)}" width="0.1524" layer="91"/>'
            for a, b, c, d in wires
        ]
        for cx in sorted(xs)[1:-1]:
            segment.append(f'<junction x="{fmt(cx)}" y="{fmt(row_y)}"/>')
        segment.append(f'<label x="{fmt(min(xs) + 1.27)}" y="{fmt(row_y + 0.5)}" size="1.778" layer="95"/>')
        rows.append(f'<net name="{escape(net)}" class="{1 if net in ("VCC", "GND") else 0}">\n<segment>\n'
                    + "\n".join(segment) + "\n</segment>\n</net>")
    return "<nets>\n" + "\n".join(rows) + "\n</nets>"


def sheet_xml(sheet: int) -> str:
    instances = [
        f'<instance part="{p}" gate="{gate_of(p)}" x="{fmt(v[4])}" y="{fmt(v[5])}"'
        + (f' rot="{v[6]}"' if v[6] != "R0" else "") + "/>"
        for p, v in PARTS.items() if v[3] == sheet
    ]
    title = "Timer" if sheet == 1 else "Power input"
    return (f'<sheet>\n<description>{title}</description>\n<plain>\n'
            f'<text x="20.32" y="180.34" size="2.54" layer="97">{title} (synthetic)</text>\n'
            '<wire x1="15.24" y1="175.26" x2="80" y2="175.26" width="0.1524" layer="97" style="shortdash"/>\n'
            '</plain>\n<instances>\n' + "\n".join(instances) + "\n</instances>\n<busses/>\n"
            + schematic_nets(sheet) + "\n</sheet>")


def schematic_file() -> str:
    parts = [
        f'<part {attrs(name=p, library="blinky", deviceset=v[0], device=v[1], value=v[2])}>'
        + ('<attribute name="MPN" value="NE555DR"/>' if p == "U1" else "") + "</part>"
        for p, v in PARTS.items()
    ]
    return ('<?xml version="1.0" encoding="utf-8"?>\n<!DOCTYPE eagle SYSTEM "eagle.dtd">\n<eagle version="9.6.2">\n<drawing>\n'
            '<settings>\n<setting alwaysvectorfont="no"/>\n<setting verticaltext="up"/>\n</settings>\n'
            '<grid distance="0.1" unitdist="inch" unit="inch" style="lines" multiple="1" display="no"/>\n'
            + layers_xml() + '\n<schematic xreflabel="%F%N/%S.%C%R" xrefpart="/%S.%C%R">\n'
            '<description>Synthetic 555 blinker (test data)</description>\n<libraries>\n' + library_xml()
            + '\n</libraries>\n<attributes/>\n<variantdefs/>\n<classes>\n'
            '<class number="0" name="default" width="0.254" drill="0.3" clearance="0.2"/>\n'
            '<class number="1" name="power" width="0.5" drill="0.4" clearance="0.3"/>\n</classes>\n<parts>\n'
            + "\n".join(parts) + "\n</parts>\n<sheets>\n" + sheet_xml(1) + "\n" + sheet_xml(2)
            + "\n</sheets>\n</schematic>\n</drawing>\n</eagle>\n")


# --- Board -------------------------------------------------------------------

# element: (package, x, y, rot) — mirrored ones are on the bottom.
ELEMENTS = {
    "U1": ("SO08", 30, 20, "R0"),
    "R1": ("R0805", 15, 30, "R90"),
    "R2": ("R0805", 15, 10, "R270"),
    "C1": ("C0603", 8, 20, "R0"),
    "C2": ("C0603", 45, 30, "MR0"),
    "R3": ("R0805", 45, 15, "MR90"),
    "D1": ("LED3MM", 55, 22, "R180"),
    "J1": ("HDR2", 5, 5, "R0"),
}


def pad_point(element: str, pad_name: str) -> tuple:
    package, x, y, rot = ELEMENTS[element]
    for p in PACKAGES[package]["pads"]:
        if p["name"] == pad_name:
            dx, dy = place(p["x"], p["y"], rot)
            return (round(x + dx, 4), round(y + dy, 4))
    raise KeyError(f"{element}.{pad_name}")


def pad_of(part: str, pin: str) -> str:
    deviceset, device = PARTS[part][0], PARTS[part][1]
    return DEVICESETS[deviceset][2][device][1][pin]


def board_file() -> str:
    outline = [(0, 0), (65, 0), (65, 40), (0, 40)]
    plain = [
        f'<wire x1="{a[0]}" y1="{a[1]}" x2="{b[0]}" y2="{b[1]}" width="0" layer="20"/>'
        for a, b in zip(outline, outline[1:] + outline[:1])
    ]
    plain.append('<text x="35" y="35" size="1.5" layer="21">BLINKY</text>')
    plain.append('<hole x="60" y="35" drill="3.2"/>')
    elements = []
    for name, (package, x, y, rot) in ELEMENTS.items():
        deviceset, _, value, *_ = PARTS[name]
        rot_attr = f' rot="{rot}"' if rot != "R0" else ""
        elements.append(f'<element name="{name}" library="blinky" package="{package}" value="{escape(value)}" '
                        f'x="{fmt(x)}" y="{fmt(y)}"{rot_attr}/>')
    signals = []
    for net, pins in NETS.items():
        refs = [(p, pad_of(p, n)) for p, n in pins if p in ELEMENTS]
        rows = [f'<contactref element="{e}" pad="{pad_name}"/>' for e, pad_name in refs]
        if net == "LED":
            # Top layer from R3 (bottom, so a via first) to the LED.
            a = pad_point("R3", "2")
            b = pad_point("D1", "A")
            via = (a[0] + 2, a[1])
            rows.append(f'<wire x1="{fmt(a[0])}" y1="{fmt(a[1])}" x2="{fmt(via[0])}" y2="{fmt(via[1])}" width="0.3" layer="16"/>')
            rows.append(f'<via x="{fmt(via[0])}" y="{fmt(via[1])}" extent="1-16" drill="0.4" diameter="0.8"/>')
            rows.append(f'<wire x1="{fmt(via[0])}" y1="{fmt(via[1])}" x2="{fmt(b[0])}" y2="{fmt(b[1])}" width="0.3" layer="1"/>')
        if net == "N$2":
            # C2 is on the bottom: a via next to U1 pin 5.
            a = pad_point("U1", "5")
            b = pad_point("C2", "1")
            via = (a[0], a[1] + 2)
            rows.append(f'<wire x1="{fmt(a[0])}" y1="{fmt(a[1])}" x2="{fmt(via[0])}" y2="{fmt(via[1])}" width="0.254" layer="1"/>')
            rows.append(f'<via x="{fmt(via[0])}" y="{fmt(via[1])}" extent="1-16" drill="0.4" diameter="0.8"/>')
            rows.append(f'<wire x1="{fmt(via[0])}" y1="{fmt(via[1])}" x2="{fmt(b[0])}" y2="{fmt(b[1])}" width="0.254" layer="16"/>')
        if net == "GND":
            rows.append('<polygon width="0.254" layer="16" isolate="0.3" rank="1">'
                        '<vertex x="1" y="1"/><vertex x="64" y="1"/><vertex x="64" y="39"/><vertex x="1" y="39"/></polygon>')
        signals.append(f'<signal name="{escape(net)}" class="{1 if net in ("VCC", "GND") else 0}">\n'
                       + "\n".join(rows) + "\n</signal>")
    return ('<?xml version="1.0" encoding="utf-8"?>\n<!DOCTYPE eagle SYSTEM "eagle.dtd">\n<eagle version="9.6.2">\n<drawing>\n'
            '<settings>\n<setting alwaysvectorfont="no"/>\n</settings>\n'
            '<grid distance="0.05" unitdist="mm" unit="mm" style="lines" multiple="1" display="no"/>\n'
            + layers_xml() + '\n<board>\n<plain>\n' + "\n".join(plain) + '\n</plain>\n<libraries>\n'
            + library_xml() + '\n</libraries>\n<attributes/>\n<variantdefs/>\n<classes>\n'
            '<class number="0" name="default" width="0.254" drill="0.3" clearance="0.2"/>\n'
            '<class number="1" name="power" width="0.5" drill="0.4" clearance="0.3"/>\n</classes>\n'
            '<designrules name="default">\n<param name="layerSetup" value="(1*16)"/>\n'
            '<param name="mdWireWire" value="8mil"/>\n<param name="msWidth" value="6mil"/>\n'
            '<param name="msDrill" value="0.3mm"/>\n</designrules>\n<autorouter/>\n<elements>\n'
            + "\n".join(elements) + '\n</elements>\n<signals>\n' + "\n".join(signals)
            + '\n</signals>\n</board>\n</drawing>\n</eagle>\n')


def library_file() -> str:
    return ('<?xml version="1.0" encoding="utf-8"?>\n<!DOCTYPE eagle SYSTEM "eagle.dtd">\n<eagle version="9.6.2">\n<drawing>\n'
            '<settings>\n<setting alwaysvectorfont="no"/>\n</settings>\n'
            '<grid distance="0.1" unitdist="inch" unit="inch" style="lines" multiple="1" display="no"/>\n'
            + layers_xml() + '\n' + library_xml() + '\n</drawing>\n</eagle>\n')


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "blinky.sch").write_text(schematic_file(), encoding="utf-8")
    (OUT / "blinky.brd").write_text(board_file(), encoding="utf-8")
    (OUT / "blinky.lbr").write_text(library_file(), encoding="utf-8")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
