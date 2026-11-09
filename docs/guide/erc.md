# Electrical rules check

The electrical rules check (ERC) looks at the connections of all sheets and
finds mistakes a drawing does not show: pins that are not connected, two
outputs driving the same net, an integrated circuit without supply, labels
that connect to nothing, and more. Run it whenever you finish a part of the
circuit, and always before you start the layout.

![The results of the check below the drawing](/screenshots/schematic-erc.png)

## Running the check

Click the check button (✓ in a circle) in the view toolbar of a sheet, or run
**Schematic: run the electrical rules check** from the
[command palette](./command-palette). The results open below the drawing:

![The results panel](/screenshots/schematic-erc-panel.png)

- The header counts errors and warnings. The two checkboxes show or hide each
  kind.
- Each line names the problem, with the pins, nets and labels involved, and the
  sheet path on the right (`/` is the top sheet, `/Power/` the sheet `Power`
  below it).
- **Click a line** to see the problem: the view opens the sheet, zooms to the
  spot and selects the objects concerned.
- While the panel is open, the check runs again after every change, so a line
  disappears as soon as you fix its problem.
- When some symbols are not annotated, **Annotate now** opens the
  [annotation dialog](./schematic-editor#annotation); when wire ends are off
  the grid, **Snap to grid** fixes them on the current sheet.

The check takes a few milliseconds even for large projects: a real 15-sheet
design with 480 nets and 2,700 pins is checked in about 4 ms.

## What is checked

| Check                                   | Default | Reported when                                                                                                              |
| --------------------------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------- |
| Pin type conflicts                      | matrix  | Two pins on one net must not be connected according to the [pin type matrix](#pin-type-matrix), for example two outputs.   |
| Power inputs without a source           | Error   | A power input pin (the supply pin of an IC) is on a net without a power output (a regulator, a battery) or a `PWR_FLAG`.   |
| Inputs without a driver                 | Error   | A net connects pins, at least one is an input, and no pin can drive it (no output, bidirectional, passive or similar pin). |
| Unconnected pins                        | Error   | A pin is connected to nothing and has no no-connect flag.                                                                  |
| No-connect flags on connected pins      | Warning | A no-connect flag sits on a net that connects pins.                                                                        |
| No-connect flags not on a pin           | Warning | A no-connect flag is not on a pin end.                                                                                     |
| Loose wire ends                         | Warning | A wire end touches nothing: no pin, wire, label or junction.                                                               |
| Unconnected labels                      | Error   | A label is connected to no wire and no pin.                                                                                |
| Hierarchical labels without a sheet pin | Error   | A hierarchical label has no pin of the same name on the sheet symbol above.                                                |
| Sheet pins without a hierarchical label | Error   | A sheet pin has no hierarchical label of the same name inside its sheet.                                                   |
| Global labels used once                 | Ignore  | A global label name appears only once in the project (often a typing mistake).                                             |
| Nets with several names                 | Warning | Labels with different names are on one net. The net gets the first name; the others are lost.                              |
| Duplicate designators                   | Error   | Two parts share a designator. Units of one multi-unit part may share it.                                                   |
| Unannotated symbols                     | Error   | A designator ends in `?`.                                                                                                  |
| Missing symbols                         | Error   | A placed symbol refers to a symbol that is in no project library.                                                          |
| Missing sheets                          | Error   | A sheet symbol refers to a schematic that does not exist.                                                                  |
| Sheets that include themselves          | Error   | A sheet places itself, directly or through other sheets.                                                                   |
| Wire ends off the grid                  | Warning | A wire end is not on the [grid](./schematic-editor#grid-and-snapping) (it may miss a pin by a fraction of a millimetre).   |

The defaults follow KiCad's, so an imported KiCad project gives the same
results as in KiCad. Hidden power pins of ICs and the pins of power symbols
are never reported as unconnected.

### Why "power input has no source"?

A supply net such as `VCC` usually gets its voltage from outside the
schematic: a connector, a lab supply, a battery. The check cannot know that.
Place a `PWR_FLAG` (one of the starter symbols) on the net once, for example
next to the power connector, to tell the check that the net is powered.

## Settings

Click the settings button in the results panel (the list icon). The settings
are part of the project's rules document, so everybody who opens the project
checks with the same rules. Changing them is one undo step.

![Check settings: severities](/screenshots/schematic-erc-settings.png)

### Severities

Each check can report **errors**, **warnings**, or be **ignored**. Errors are
mistakes that make the board wrong; warnings are things to look at.

### Pin type matrix

Every pin has an electrical type. The matrix says what it means to connect
two pin types on one net: **·** is fine, **W** is a warning, **E** an error.
Click a cell to switch between the three. The matrix is symmetric: a cell
below the diagonal also sets its mirror image.

![The pin type matrix](/screenshots/schematic-erc-matrix.png)

The default matrix (KiCad's):

| With →             | In  | Out | Bi  | 3S  | Pas | NIC | Uns | PwI | PwO | OC  | OE  | NC  |
| ------------------ | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Input              | ·   | ·   | ·   | ·   | ·   | ·   | W   | ·   | ·   | ·   | ·   | E   |
| Output             | ·   | E   | ·   | W   | ·   | ·   | W   | ·   | E   | E   | E   | E   |
| Bidirectional      | ·   | ·   | ·   | ·   | ·   | ·   | W   | ·   | W   | ·   | W   | E   |
| Tri-state          | ·   | W   | ·   | ·   | ·   | ·   | W   | W   | E   | W   | W   | E   |
| Passive            | ·   | ·   | ·   | ·   | ·   | ·   | W   | ·   | ·   | ·   | ·   | E   |
| Not int. connected | ·   | ·   | ·   | ·   | ·   | ·   | ·   | ·   | ·   | ·   | ·   | E   |
| Unspecified        | W   | W   | W   | W   | W   | ·   | W   | W   | W   | W   | W   | E   |
| Power input        | ·   | ·   | ·   | W   | ·   | ·   | W   | ·   | ·   | ·   | ·   | E   |
| Power output       | ·   | E   | W   | E   | ·   | ·   | W   | ·   | E   | E   | E   | E   |
| Open collector     | ·   | E   | ·   | W   | ·   | ·   | W   | ·   | E   | ·   | ·   | E   |
| Open emitter       | ·   | E   | W   | W   | ·   | ·   | W   | ·   | E   | ·   | ·   | E   |
| Not connected      | E   | E   | E   | E   | E   | E   | E   | E   | E   | E   | E   | E   |

Two open-collector outputs on one net are fine (a wired AND); two push-pull
outputs are an error, because they would short each other when they disagree.

**Reset to defaults** restores KiCad's matrix and severities; only settings
that differ from the defaults are stored in the project.

### Settings from KiCad

When you [import a KiCad project](./import-kicad), its ERC settings
(`erc.rule_severities` and `erc.pin_map` in the `.kicad_pro` file) are
taken over. Findings that were marked as excluded in KiCad are not imported:
the check reports them again, and the import report says how many there were.

## See also

- [Drawing schematics](./schematic-editor)
- [Engine API: `schematic.erc`](../reference/engine-api#schematic-erc)
