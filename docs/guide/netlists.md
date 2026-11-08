# Netlists

A netlist lists which pins are connected to each other. ElectronicWerk reads
netlists from KiCad and other tools, compares them with a board, and can
apply them to the board. Use this to:

- check that an [imported KiCad project](./import-kicad) is electrically
  complete,
- bring a board up to date after the schematic was changed in another tool,
- check a board against a netlist that someone else sent you.

![A netlist that matches the board](/screenshots/netlist-match.png)

## Supported formats

ElectronicWerk detects the format from the content of the file, not from
its name.

| Format            | How to get it                                                                                                     |
| ----------------- | ----------------------------------------------------------------------------------------------------------------- |
| **KiCad netlist** | In the KiCad schematic editor: **File → Export → Netlist…**, format **KiCad** (`.net`).                           |
| **KiCad XML**     | In the KiCad schematic editor: the **KiCad XML** export (`.xml`), or the intermediate file that BOM scripts read. |
| **CSV**           | Any table with one row per pin: from a spreadsheet, a script or another ECAD tool.                                |

### CSV netlists

A CSV netlist has a header row and one row per connected pin:

```csv
Net;Designator;Pin
/CV;C2;1
/CV;U1;5
GND;C1;2
GND;U1;1
```

- **Separator:** semicolon, comma or tab. ElectronicWerk picks the one that
  occurs most often in the header row.
- **Columns:** the order does not matter, and other columns are ignored.
  Upper and lower case do not matter, and neither do spaces in the names.

| Column     | Accepted names                                                  |
| ---------- | --------------------------------------------------------------- |
| Net        | `Net`, `Net name`, `Signal`                                     |
| Designator | `Designator`, `Ref`, `Reference`, `RefDes`, `Component`, `Part` |
| Pin        | `Pin`, `Pad`, `Pin number`, `Terminal`, `Node`                  |

- **Quotes:** values may be quoted (`"GND"`); the quotes are removed.
- **Empty lines** are ignored.

## Comparing a netlist with a board

1. In the project tree, right-click a board and choose **Import netlist…**.
   You can also run **Import netlist** in the
   [command palette](./command-palette); it uses the first board of the
   project.
2. Choose the netlist file.

![The board's context menu](/screenshots/netlist-menu.png)

The **Netlist** dialog shows the format, the file, and the number of nets
and components in the netlist. Below that it says how the board compares:

- **The board is connected exactly like the netlist.** Every net of the
  netlist connects the same pins on the board. Net names may differ: the
  comparison looks at which pins are connected, not at the names. The count
  of nets with the same name tells you how many names match too.
- **n of m connected nets match the board.** Some pins are connected
  differently. The **Differences** table lists them.

![Differences between a netlist and the board](/screenshots/netlist-differences.png)

| Column      | Meaning                                                                                                                   |
| ----------- | ------------------------------------------------------------------------------------------------------------------------- |
| **Pin**     | Designator and pin number, for example `D1-2`.                                                                            |
| **Netlist** | The net of the pin in the netlist. _Not connected_ means that the netlist leaves the pin open, but the board connects it. |
| **Board**   | The net of the pad on the board, or **—** if the pad has no net or does not exist.                                        |

The table shows the first 30 differences and says how many more there are.

### How nets are compared

- Two nets match if they connect exactly the same pins.
- A net with a single pin means that the pin is **not connected**. Tools give
  such nets different names (KiCad, for example, uses
  `unconnected-(U1-NC-Pad8)`), so their names are not compared. The pin is
  only a difference if the board connects it to other pins.
- Pads without a net on the board count as not connected.
- Components in the netlist without a footprint on the board are listed as
  **missing footprints**. Footprints on the board that the netlist does not
  mention (mounting holes, logos, test points) are listed separately and are
  left alone.

## Applying a netlist

If the board differs from the netlist, the dialog says how many pads would
change their net and how many nets would be created. Click **Apply to the
board** to change the board:

- Every pad of every footprint that the netlist mentions gets the net of its
  pin in the netlist. Pads that the netlist does not list lose their net.
- Nets that the board does not have yet are created.
- Footprints that the netlist does not mention keep their nets.
- Tracks, vias and zones are not changed. Check them afterwards: a track
  can now connect two different nets.

Applying a netlist is one step in the history: **Undo** takes it back
completely. The board is saved with the next **Save**, as with any other
change.

Missing footprints are not placed on the board yet; placing footprints
arrives with the layout editor.

## Command line

```sh
# Compare (exit code 0: the board matches, 1: it differs)
ewerk import netlist blinky.net --project Blinky

# Apply the netlist and save the project
ewerk import netlist changed.csv --project Blinky --apply
```

Example output for a netlist with one changed pin:

```text
csv netlist: 8 nets, 8 components. 6 of 6 connected nets match the board (6 with the same name).
  D1-2: netlist /LED_B (not connected) / board /LED_A
  R3-2: netlist /LED_A (not connected) / board /LED_A
Run with --apply to change 1 pads.
```

See [`ewerk import netlist`](../reference/cli#ewerk-import-netlist) for all
options, including JSON output for scripts.
