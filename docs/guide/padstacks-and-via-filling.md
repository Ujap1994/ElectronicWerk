# Padstacks and via filling

## Padstacks

A through-hole pad is copper on every layer it passes. Normally it has the
same shape on all of them. A **padstack** gives it another shape or size on
the **inner layers** and on the **back**:

- smaller rings on the inner layers, so more tracks fit between the pins
  of a connector on a dense multilayer board,
- a round pad on the back of a square pin-1 pad,
- a larger pad on the soldering side of a wave-soldered part.

![A DIP-8 footprint in the footprint editor, pin 1 selected](/screenshots/padstack-editor.png)

Select a through-hole pad in the [footprint editor](./footprint-editor). The
properties have a **Padstack** section:

![Own shapes on the inner layers and on the back](/screenshots/padstack.png)

- **Own shape on the inner layers** — a shape and size for every inner layer.
- **Own shape on the back** — a shape and size for the back of the
  footprint (the bottom for a part on the top; it moves with the part when
  the part is flipped).

Where no own shape is set, the pad's shape is used. The shapes offered are
circle, rectangle, oval and rounded rectangle. A pad that becomes an SMD pad
loses its padstack.

The design rules check, zone filling, the router, the screen and every
output use the copper of each layer, so clearances on an inner
layer are measured from the smaller ring there. The solder mask and paste
openings follow the copper of their side.

## Via filling and covering (IPC-4761)

A via can stay open, be covered by solder mask, be plugged or be filled. The
standard IPC-4761 names seven types:

| Type | In ElectronicWerk   | What the board maker does                                                   | Solder mask opening |
| ---- | ------------------- | --------------------------------------------------------------------------- | ------------------- |
| I    | tented              | A dry film mask covers the via.                                             | none                |
| II   | tented and covered  | Tented, then covered with liquid mask.                                      | none                |
| III  | plugged             | The hole is plugged with a non-conductive material from one side.           | open                |
| IV   | plugged and covered | Plugged, then covered with mask.                                            | none                |
| V    | filled              | The hole is filled completely (non-conductive or conductive paste).         | open                |
| VI   | filled and covered  | Filled, then covered with mask.                                             | none                |
| VII  | filled and capped   | Filled, planarized and plated over with copper: a flat pad. **Via in pad.** | open                |

![The filling of a via in its properties](/screenshots/via-fill.png)

Choose it per via in the properties under **Filling and covering
(IPC-4761)**. **As in the output settings** follows the Gerber option
[**Cover vias with solder mask (tented)**](./gerber-drill), as before.

The fill shows up in the outputs:

- **Solder mask** — covered vias get no opening; open, plugged and filled
  ones do.
- **Drill table** — the [drill map](./gerber-drill) and the
  [fabrication drawing](./fabrication-drawing) count every fill on a line of
  its own, for example _0.3 mm, via, filled and capped (IPC-4761 VII)_.
- **Notes** — the fabrication drawing adds a note per fill:
  _Vias 0.3 mm as marked in the drill table: filled and capped (IPC-4761
  VII)._ For type VII also: _Filled and capped vias lie in pads (via in
  pad): planarize and plate over them._

Filling costs extra at most board makers; ask before ordering.

### Vias in pads

A via inside an SMD pad saves room under fine-pitch parts, but an open via
draws the solder into the hole while the part is soldered. The design rules
check therefore warns with `via-in-pad` about every via whose centre lies in
an SMD pad and which is not **filled and capped (VII)**. Set the via to type
VII — or move it out of the pad.

## KiCad

KiCad 9 and newer have both: padstacks (mode _front / inner / back_) and the
via protection features (tenting, covering, plugging, filling, capping).
ElectronicWerk [imports](./import-kicad) and [exports](./export-kicad) them;
KiCad 10 reads exported boards with the same shapes and fills. A KiCad
padstack with a different shape on every inner layer is imported with the
shape of its first inner layer, and a custom shape on a single layer becomes
a rectangle (the import report says so).
