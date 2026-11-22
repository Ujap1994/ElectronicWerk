# Systems of several boards

Many devices are more than one board: a **shield** stacked on a carrier
board, a main board with a **front panel board**, boards joined by
**cables**. A mistake between the boards — two signals swapped on a
header, a supply on a signal pin, a pin that is connected on one board but
open on the other — shows up only when the boards are built and plugged
together. A **system** in the project brings the boards together and
checks every connection between them before anything is made:

- the **boards** of the system — boards of this project, boards of other
  ElectronicWerk projects, or reference designs imported from their open
  design files — each at its place in 3D,
- the **connections**: connectors that mate directly (stacked boards,
  board-to-board connectors) and **cables** with their wires,
- the **connection check** across the boards, with renaming a signal on
  all boards at once,
- the **system in 3D**, checked for collisions between the boards and with
  their enclosures, exported as one STEP file,
- the **outputs**: a bill of materials of the whole system, a connection
  report, wiring lists and harness drawings.

![The system editor with the boards of the example system](/screenshots/system-boards.png)

The pictures on this page show the example system _Station_: a carrier
board, a shield stacked on it through a 2 × 8 header, and a front panel
board joined by a four-wire cable. It contains three mistakes on purpose.
Make it yourself with `ewerk synth system Station` and open it.

## Create a system

In the project tree, open the menu of the **Systems** group and choose
**Add system**. The system is a document of the project (a `.ewsys` file
in `systems/`) like a board or an output job: it is saved with the
project, every change can be undone, and it is part of archives and Git.

Selecting the system in the tree shows its boards and cables in the
details; a double-click opens the **system editor** in a tab.

![The details of a system](/screenshots/system-details.png)

## Add the boards

**Add board** in the editor's toolbar offers:

- **Boards of this project** — every board document of the project.
- **Board of another project…** — choose the other project's `.ewproj`
  file. The system keeps its path relative to this project (for example
  `../Shield/Shield.ewproj`), so both projects can move together. The other
  project is read from its files whenever it changes; it does not need to
  be open.
- **Reference design from KiCad…** or **from Eagle…** — the open design
  files of a board you build on, for example the carrier board of a
  development kit. The design is imported as its own ElectronicWerk project in
  `systems/references/<name>/` (with its import report) and added to the
  system.

![Adding a board](/screenshots/system-add-board.png)

The **Boards** tab lists the boards with:

| Column                    | What it does                                                                                                                                   |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| **Name**                  | The board's name in the system (in findings, reports and the 3D view).                                                                         |
| **Board**                 | Where it comes from: _This project_ or the other project's path, with the board's name. A board that cannot be read is marked with the reason. |
| **Quantity**              | How many of it the system has — for the bill of materials.                                                                                     |
| **Position x, y, z (mm)** | Where the board's origin goes in the system. **z is the height of the board's bottom face.**                                                   |
| **Turned (°)**            | Rotation about the vertical axis.                                                                                                              |
| **Upside down**           | The board turned over (a mezzanine board facing down).                                                                                         |
| **Shown**                 | Shown in the 3D view and checked for collisions.                                                                                               |

## Connections: connectors that mate

On the **Connections** tab, **Add connection** joins two connectors that
plug into each other: the socket on the carrier and the header on the
shield, or the two halves of a board-to-board connector. Choose the board
and the connector on each side (the lists show the connectors of each
board — footprints with designators like J, P, X or CN, or with
connector, header or socket in their name).

![A connection with its pin pairs](/screenshots/system-connections.png)

**Pins pair** says which pin meets which:

| Pairing               | When                                                                                                                                                                                                                           |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **by number**         | Pin 1 meets pin 1, pin 2 meets pin 2 … — most headers and sockets, many board-to-board pairs.                                                                                                                                  |
| **mirrored rows**     | Pin 1 meets pin 2, pin 3 meets pin 4 … — a two-row connector turned over.                                                                                                                                                      |
| **by position in 3D** | The pins meet where they lie on each other in the 3D assembly. Use this for stacked boards: a header on the bottom side of a shield is mirrored when seen from above, and the position tells the truth whatever the numbering. |

The table below lists every pin pair with the signal on each side and its
status: **OK**, **Different signals**, **Swapped**, **Supply** (a supply
meets a signal or another supply), **One side open**, **No partner** or
**Not used** (open on both sides).

### Stack a board

**Stack** places the second board of the connection onto the first one's
connector:

- turned about the vertical axis so that the pins lie on each other (with
  pairing by position, the numbering by number and with mirrored rows are
  both tried and the better fit is taken),
- at the height of the first connector: the top of its 3D model above the
  board (8.5 mm, a 2.54 mm socket strip, when it has no model), measured
  between the copper surfaces of the two boards.

The command line can give another distance (`ewerk system stack … --gap`)
or turn the board over.

## Cables

The **Cables** tab holds the cables and harnesses between the boards.
**Add cable** makes a cable between the first two boards; then set:

- **Name**, **cable type** (a part number or a description such as
  _Ribbon 10-way_) and **length**,
- the **ends**: each with a label (X1, X2 …), the connector it plugs into
  (or **Free end** for a switch, a sensor or a loose end) and the
  **housing** — the cable's own connector, which goes into the bill of
  materials,
- the **wires**: from an end and pin to an end and pin, with colour
  (names such as _red_, IEC codes such as _RD_, or `#rrggbb`), gauge
  (_AWG 24_, _0.25 mm²_) and the signal (taken from the boards when left
  empty). **Wire one to one** makes one wire per pin of the first end's
  connector to the same pin of the second end.

![A cable with its ends and wires](/screenshots/system-cables.png)

Crossed wires — a UART's TX to RX — are simply wires from pin 3 to pin 4.
Their signals have different names on purpose; accept the finding, or name
the signals the same on both boards.

## The connection check

The check runs after every change — on the system and on any of its boards
— and its result is in the editor's toolbar (_2 errors · 1 warnings_). The
**Check** tab lists the findings:

| Finding            | Severity | Meaning                                                                                                                                     |
| ------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `system-mismatch`  | Error    | Two mating pins (or the two ends of a wire) carry different signals. Two pins whose signals are exchanged are reported once as **swapped**. |
| `system-power`     | Error    | A supply meets a signal, or two different supplies meet (+5V on 3V3).                                                                       |
| `system-missing`   | Warning  | A signal meets a pin that is not connected on the other board.                                                                              |
| `system-pins`      | Warning  | The connectors have different pin counts, or a pin with a signal has no partner.                                                            |
| `system-connector` | Error    | A board cannot be read, or a connection or cable names a connector the board does not have.                                                 |
| `system-cable`     | Warning  | A wire ends on a pin the connector does not have, a wire joins two unconnected pins, or a pin with a signal has no wire.                    |
| `system-collision` | Error    | Two boards (their bodies, parts or enclosures) collide in 3D.                                                                               |
| `system-clearance` | Warning  | They come closer than the minimum clearance to the enclosure of the [design rules](./design-rules#board-minimums).                          |

![The findings of the example system](/screenshots/system-check.png)

Signals are compared by name **without the sheet path** (`/Sensors/SDA` is
`SDA`) and without regard to case. Supplies are recognised by their names
(GND, VCC, VDD, VSS, VIN, +5V, 3V3 …) or by a net class with _power_ in its
name. Nets the tools name by themselves (`Net-(R1-Pad2)`) are not compared.

### Rename a signal on the other boards

For two different signals the finding offers **Use SDA on Shield** and the
other way round. ElectronicWerk renames the signal on that board **and on
every other board of this project it reaches** through connections and
cables — the board's net and the labels in the schematics — as one step
you can undo. Boards of other projects are named in the message, so you
rename the signal there too.

### Signals with different names

Some signals are meant to be the same under different names: `BUTTON` on
one board, `BTN` on the other. Enter them under **Signals with different
names**, one group per line (`BUTTON = BTN`); the check then treats them as
one signal.

### Accept a finding

**Accept** keeps a finding that is right as it is (a crossed UART, a pin
left open on purpose); it no longer counts, and **Show accepted findings**
brings it back.

## The system in 3D

The **3D** tab shows all shown boards at their places with their parts and
their enclosures, in the [3D view](./view-3d) with all its tools — views,
section, measuring, screenshots. **Export** writes the whole system as one
STEP file: every board's assembly with its enclosure at its place, ready
for mechanical CAD.

![The example system in 3D: the shield stacked on the carrier](/screenshots/system-3d.png)

The check also looks at the boards in 3D: their bodies, parts and
enclosures must not collide, and they must keep the minimum clearance.
Connectors that mate may touch each other and the other board.

![A collision: the shield placed too low hits the carrier's controller](/screenshots/system-collision.png)

## Outputs

**Outputs** in the toolbar writes into `outputs/<system>/` of the project:

| File                                   | Content                                                                                                                                                                                                                                   |
| -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `<system>-bom.csv`                     | The bill of materials of the whole system: every board's parts times the board's quantity (with _Board: designator_), the boards, every cable, the housings of the cable ends, and the wires by gauge and colour with their total length. |
| `<system>-connections.csv`, `.html`    | The connection report: every pin pair of every connection and wire with both signals and the status, and the findings.                                                                                                                    |
| `<system>-<cable>-wiring.csv`          | The wiring list of each cable.                                                                                                                                                                                                            |
| `<system>-<cable>-harness.pdf`, `.svg` | The harness drawing of each cable: both ends as pin tables, the wires between them in their colours with signal and gauge, the wiring list and a title block (A4 landscape).                                                              |
| `<system>.step`                        | The 3D assembly.                                                                                                                                                                                                                          |

![The harness drawing of the example cable](/screenshots/system-harness.png)

## Command line and AI agents

```sh
ewerk system new MyProject "Station"
ewerk system add-board MyProject --board Carrier
ewerk system add-board MyProject --other ../Shield --name Shield
ewerk system link MyProject Carrier:J1 Shield:P1 --pairing position
ewerk system stack MyProject Shield
ewerk system check MyProject            # exit code 1 on errors
ewerk system outputs MyProject
ewerk system export-3d MyProject -o station.step
```

See [`ewerk system`](../reference/cli#ewerk-system), the
[engine API](../reference/engine-api#systems) (`system.*`) and
[AI agents](./automation#systems-of-boards) (`system_info`,
`system_check`).

## How it works

- A system is one document with its boards, connections, cables, aliases
  and accepted findings; see the [file format](../reference/file-format#systems-ewsys).
- Boards of other projects are read from their files when they change.
  Their models are found from their own project folder.
- Connector pins are the pads of the footprint with their numbers; the
  signal of a pin is the name of its pad's net.
- The decisions behind the design are in
  [ADR-0019](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0019-systems.md).
