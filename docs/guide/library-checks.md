# Library checks

The library editor checks every symbol, footprint and component while you
work, in the spirit of KiCad's library conventions (KLC). The findings help
you catch the mistakes that otherwise show up much later: a pin that wires
cannot reach, a pad without paste, a component whose pins have no pads.

## Where the findings show up

- **In the item list**: a red number counts the errors of an item, a yellow
  one its warnings. Notes do not show up there.
- **In the Checks tab** below the drawing: every finding of the shown item,
  with its severity, its text and its code. **Show** selects the pin or pad
  concerned (and switches to its unit or body style first).
- **In the footprint wizard**: the findings of the footprint about to be
  created.
- **In the component editor**: the findings of the component, next to the
  pin and pad table.

**Check the whole library** in the Checks tab lists every item with findings.
Expand an item to see them; click its name to open it.

![The checks of the whole library: the starter capacitor uses KiCad's 2.794 mm pins](/screenshots/libedit-checks.png)

The checks run again after every change, so a finding disappears as soon as
it is fixed.

## Severities

| Severity    | Meaning                                                                                  |
| ----------- | ---------------------------------------------------------------------------------------- |
| **Error**   | The item does not work as intended: a pin without number, an SMD pad with a drill.       |
| **Warning** | Probably a mistake: a pin off the grid, silkscreen on a pad.                             |
| **Note**    | A good habit is missing: a description, a 3D model. Ignore notes when they do not apply. |

## Symbol checks

| Code                    | Severity | What it means and how to fix it                                                                                                                       |
| ----------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `no-prefix`             | Error    | The symbol has no designator prefix. Set one (`R`, `U`) in the properties.                                                                            |
| `no-description`        | Note     | The symbol has no description, so it is hard to find in the symbol chooser.                                                                           |
| `power-symbol-pins`     | Error    | A power symbol must have exactly one pin.                                                                                                             |
| `unit-without-pins`     | Warning  | A unit has no pins. Add pins or remove the unit.                                                                                                      |
| `pin-without-number`    | Error    | A pin has no number, so it cannot connect to a pad. Number it, or use **Renumber** in the pin table.                                                  |
| `duplicate-pin-number`  | Error    | Two pins share a number but differ in position or name (in one body), or in name or type (across units). Stacked pins with the same name are allowed. |
| `pin-off-grid`          | Warning  | The pin's connection point is not on the 1.27 mm grid, so wires on the schematic grid cannot reach it.                                                |
| `pin-length`            | Note     | The pin length is not a multiple of 1.27 mm. KiCad's own symbols use such lengths (2.794 mm for `C`), so this is only a note.                         |
| `overlapping-pins`      | Warning  | Two visible pins with different names share a spot; they would be connected by accident.                                                              |
| `hidden-power-pin`      | Warning  | A power input pin is hidden. Hidden power pins connect to the net of their name without a wire, which is easy to overlook.                            |
| `alternate-pins-differ` | Error    | The De Morgan body of a unit has other pin numbers than its normal body. Both bodies must have the same pins.                                         |

## Footprint checks

| Code                    | Severity | What it means and how to fix it                                                                                     |
| ----------------------- | -------- | ------------------------------------------------------------------------------------------------------------------- |
| `no-courtyard`          | Error    | The footprint has no courtyard. Use **Draw a courtyard around pads and body** in the side panel.                    |
| `pad-outside-courtyard` | Error    | A pad reaches outside the courtyard. Draw the courtyard again after moving pads.                                    |
| `no-fab-outline`        | Warning  | There is no outline on the fabrication layer, so assembly drawings show no body.                                    |
| `no-designator`         | Error    | The footprint has no designator text.                                                                               |
| `no-value`              | Warning  | The footprint has no value text.                                                                                    |
| `no-3d-model`           | Note     | The footprint has no 3D model.                                                                                      |
| `pad-without-number`    | Error    | A copper pad has no number, so no pin connects to it. Non-plated holes need no number.                              |
| `smd-pad-with-drill`    | Error    | An SMD pad has a drill hole. Change the kind to through-hole, or remove the drill by setting the kind to SMD again. |
| `smd-pad-without-paste` | Warning  | An SMD pad has no paste opening. Intended for pads you solder by hand; otherwise add the paste layer.               |
| `hole-without-drill`    | Error    | A through-hole pad has no drill.                                                                                    |
| `drill-too-large`       | Error    | The drill is as large as the pad, so no copper ring is left.                                                        |
| `no-pad-1`              | Warning  | No pad is numbered `1` (or `A1`), so the orientation of the part is unclear.                                        |
| `silk-over-pad`         | Warning  | Silkscreen lies on a pad or closer than 0.1 mm to it; it would be printed onto solder. Move or shorten the line.    |

## Component checks

| Code                | Severity | What it means and how to fix it                                                                                       |
| ------------------- | -------- | --------------------------------------------------------------------------------------------------------------------- |
| `missing-symbol`    | Error    | The component refers to a symbol that does not exist (anywhere in the project's libraries).                           |
| `no-mpn`            | Note     | The component has no manufacturer part number.                                                                        |
| `no-footprint`      | Warning  | The component has no footprint.                                                                                       |
| `missing-footprint` | Error    | The component refers to a footprint that does not exist.                                                              |
| `pin-without-pad`   | Error    | A pin of the symbol has no pad with the same number, so it stays unconnected on the board.                            |
| `pad-without-pin`   | Warning  | A pad has no pin with the same number. Fine for exposed pads and mounting tabs that stay unconnected: then ignore it. |

The checks are also available to scripts as `libedit.check` (see the
[Engine API](../reference/engine-api#library-editor)).
