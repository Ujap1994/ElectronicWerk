#!/usr/bin/env python3
"""Records where KiCad 10 places the 3D models of the blinky test board.

Exports `crates/ew-kicad/tests/fixtures/blinky/blinky.kicad_pcb` with
`kicad-cli pcb export glb --no-extra-pad-thickness` (parts stand on the
copper, as in ElectronicWerk) and writes the placement of every part,
converted to the board frame (millimetres, z up), to
`crates/ew-board3d/tests/fixtures/blinky-kicad-3d.json`. The Rust test
`kicad_reference` compares ElectronicWerk's placements with it.

Needs KiCad 10 (set KICAD_CLI if `kicad-cli` is not on PATH). Run from the
repository root: `python3 scripts/make-kicad-3d-reference.py`.
"""

import json
import os
import shutil
import struct
import subprocess
import tempfile
from pathlib import Path

BOARD = Path("crates/ew-kicad/tests/fixtures/blinky/blinky.kicad_pcb")
OUT = Path("crates/ew-board3d/tests/fixtures/blinky-kicad-3d.json")
MAC_CLI = "/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli"


def cli() -> str:
    found = os.environ.get("KICAD_CLI") or shutil.which("kicad-cli")
    if found:
        return found
    if Path(MAC_CLI).exists():
        return MAC_CLI
    raise SystemExit("kicad-cli not found; set KICAD_CLI")


def quat_matrix(q):
    x, y, z, w = q
    return [
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ]


# glTF (x, y up, z towards the viewer) → board (x, y, z up): (x, -z, y).
C = [[1, 0, 0], [0, 0, -1], [0, 1, 0]]
C_T = [[1, 0, 0], [0, 0, 1], [0, -1, 0]]


def mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def clean(v: float) -> float:
    v = round(v, 6)
    return 0.0 if v == 0 else v


def main() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        board = Path(tmp) / "blinky.kicad_pcb"
        shutil.copy(BOARD, board)
        glb = Path(tmp) / "blinky.glb"
        subprocess.run(
            [cli(), "pcb", "export", "glb", "--force", "--no-extra-pad-thickness",
             "-o", str(glb), str(board)],
            check=True, capture_output=True,
        )
        data = glb.read_bytes()
    length = struct.unpack("<I", data[12:16])[0]
    gltf = json.loads(data[20:20 + length])
    # The board body: the bounds of every primitive of its mesh.
    lo, hi = [1e9] * 3, [-1e9] * 3
    for mesh in gltf["meshes"]:
        if not mesh.get("name", "").endswith("_PCB"):
            continue
        for primitive in mesh["primitives"]:
            accessor = gltf["accessors"][primitive["attributes"]["POSITION"]]
            for axis in range(3):
                lo[axis] = min(lo[axis], accessor["min"][axis])
                hi[axis] = max(hi[axis], accessor["max"][axis])
    # glTF → board: x, -z, y.
    board = {
        "min": [clean(lo[0] * 1000), clean(-hi[2] * 1000), clean(lo[1] * 1000)],
        "max": [clean(hi[0] * 1000), clean(-lo[2] * 1000), clean(hi[1] * 1000)],
    }
    parts = {}
    for node in gltf["nodes"]:
        name = node.get("name", "")
        if "mesh" in node or name.startswith("=>") or not name:
            continue
        t = node.get("translation", [0, 0, 0])
        r = quat_matrix(node.get("rotation", [0, 0, 0, 1]))
        board_r = mul(mul(C, r), C_T)
        parts[name] = {
            "at": [clean(t[0] * 1000), clean(-t[2] * 1000), clean(t[1] * 1000)],
            "rotation": [[clean(v) for v in row] for row in board_r],
        }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps({"source": "kicad-cli 10 pcb export glb --no-extra-pad-thickness",
                               "board": board, "parts": parts}, indent=2) + "\n")
    print(f"wrote {OUT} ({len(parts)} parts)")


if __name__ == "__main__":
    main()
