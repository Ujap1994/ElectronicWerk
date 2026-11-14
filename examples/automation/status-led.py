"""A status LED through the engine API (JSON-RPC 2.0 over `ewerk rpc`).

Usage: python3 status-led.py [folder]   (EWERK may name the ewerk binary)
"""

import json
import os
import subprocess
import sys


class Engine:
    """One `ewerk rpc` session: one open project, undo, everything in memory."""

    def __init__(self):
        self.process = subprocess.Popen(
            [os.environ.get("EWERK", "ewerk"), "rpc", "--no-journal"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )
        self.next_id = 0

    def call(self, method, params=None):
        self.next_id += 1
        request = {"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        reply = json.loads(self.process.stdout.readline())
        if "error" in reply:
            raise RuntimeError(f"{method}: {reply['error']['message']}")
        return reply["result"]

    def close(self):
        self.process.stdin.close()
        self.process.wait()


folder = sys.argv[1] if len(sys.argv) > 1 else "."
engine = Engine()
engine.call(
    "project.create",
    {
        "path": os.path.join(folder, "Status LED"),
        "name": "Status LED",
        "template": "board-2l",
        # Engine lengths are nanometres; design.* methods take millimetres.
        "board": {"width": 30_000_000, "height": 20_000_000, "layers": 2, "thickness": 1_600_000},
    },
)
for symbol, value, footprint in [("R", "330", "0805"), ("LED", "green", "0805"), ("C", "100n", "0603")]:
    part = engine.call("design.place", {"symbol": symbol, "value": value, "footprint": footprint})
    print("placed", part["designator"], part["value"])
engine.call("design.connect", {"net": "VCC", "pins": ["R1.1", "C1.1"], "style": "power"})
engine.call("design.connect", {"net": "LED_A", "pins": ["R1.2", "D1.A"]})
engine.call("design.connect", {"net": "GND", "pins": ["D1.K", "C1.2"], "style": "power"})

erc = engine.call("schematic.erc")
print("ERC:", erc["errors"], "errors,", erc["warnings"], "warnings")
engine.call("design.updateBoard")
engine.call("design.arrange")
routed = engine.call("design.route")
print("routed", routed["routed"], "connections,", routed["unrouted"], "left")
engine.call("project.save")
engine.close()
sys.exit(1 if erc["errors"] or routed["unrouted"] else 0)
