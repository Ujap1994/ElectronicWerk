# Design rules check

The design rules check (DRC) compares the board with its
[design rules](./design-rules) and with the schematic: copper too close to
other copper, shorts, tracks that are too thin, vias and holes that are too
small, copper near the board edge, items outside the board or in keep-out
areas, overlapping courtyards, silkscreen on pads, connections still to
route, and parts that differ from the schematic.

![The findings below the board](/screenshots/layout-drc-panel.png)

## Running the check

Click **Design rules check** (✓ in a circle) in the board toolbar or run
**Board: run the design rules check** from the command palette. The panel
opens below the board:

- The header counts **errors**, **warnings** and **unrouted** connections.
  The checkboxes show or hide errors, warnings and accepted findings.
- Every line names the problem with the items, nets and values involved,
  and the copper layer on the right. The values say where a limit comes
  from: `net class Power`, `rule HV spacing`, `board constraints`, `pad
clearance` or `zone clearance`.
- **Click a line** to zoom to the problem and select the objects concerned.
- Each finding is also marked on the board with a red (error) or yellow
  (warning) circle.

### Online check

With **Online** ticked (the default), the check runs again shortly after
every change: move a part, and the findings update a moment later. The
engine can check just the area of an edit and merge the result with the
previous report; on a board with 500 parts and 20,000 track segments this
takes about 20 ms. A full check of a board with 5,000 parts and 200,000
track segments takes about 1.5 seconds. Both are measured on every CI run.

## Accepting findings

Some findings are intended — a net tie, a test point under a part, a
deliberate copper bridge. Click the eye button at the start of the line to
**accept** the finding: it disappears from the list and the counts, but is
remembered with the board. **Accepted (n)** shows them again (dimmed), and
the eye button on an accepted finding reports it again.

A finding is recognised by its check, the objects involved and the layer, so
it stays accepted as long as these stay the same; when you move one of the
objects far enough to change the finding, it comes back.

## What is checked

| Code                   | Default | Reported when                                                                                            |
| ---------------------- | ------- | -------------------------------------------------------------------------------------------------------- |
| `short`                | Error   | Copper of two different nets touches.                                                                    |
| `clearance`            | Error   | Copper of different nets (or without a net) is closer than the clearance of the [rules](./design-rules). |
| `unrouted`             | Error   | Pads of one net are not connected by copper. One finding per missing connection of the ratsnest.         |
| `track-width`          | Error   | A track is thinner than its minimum, or wider than a custom maximum.                                     |
| `via-diameter`         | Error   | A via is smaller than the minimum diameter.                                                              |
| `via-drill`            | Error   | A via drill is smaller than the minimum.                                                                 |
| `annular-ring`         | Error   | The copper ring around a via or plated hole is narrower than the minimum.                                |
| `hole-size`            | Error   | A pad hole is smaller than the minimum.                                                                  |
| `hole-to-hole`         | Error   | Two holes are closer than the minimum distance.                                                          |
| `copper-to-edge`       | Error   | Copper comes closer to the board edge than allowed (edge-connector pads are left out).                   |
| `no-outline`           | Error   | The board has no outline.                                                                                |
| `outline-open`         | Error   | Outline lines do not close (ends more than 10 µm apart).                                                 |
| `outside-board`        | Error   | A footprint, track or via lies outside the board.                                                        |
| `keepout`              | Error   | An item lies in a keep-out area that forbids it.                                                         |
| `courtyard-overlap`    | Error   | The courtyards of two footprints on the same side overlap.                                               |
| `silk-over-pad`        | Warning | A silkscreen line lies on a pad with a solder mask opening; it would be printed onto solder.             |
| `silk-width`           | Warning | A silkscreen line is thinner than the minimum.                                                           |
| `dangling-track`       | Warning | A track end is connected to nothing.                                                                     |
| `dangling-via`         | Warning | A via connects fewer than two items.                                                                     |
| `zone-unfilled`        | Warning | A copper zone has no fill (fill it with Shift+B).                                                        |
| `duplicate-designator` | Error   | Two footprints have the same designator.                                                                 |
| `missing-footprint`    | Warning | A part of the schematic has no footprint on the board ([update the board](./schematic-to-board)).        |
| `extra-footprint`      | Warning | A footprint linked to the schematic has no part there any more.                                          |
| `net-mismatch`         | Error   | A pad is on another net than its pin in the schematic.                                                   |
| `rule-error`           | Error   | A custom rule has a syntax error and is skipped.                                                         |

The severities can be changed in the **Checks** tab of the
[design rules](./design-rules#checks); a check set to **Ignore** does not
run. The settings button in the panel header opens that tab.

## How distances are measured

Every piece of copper is described exactly: a via is a disc, a track a
stadium (a segment with round ends), a round-rectangle pad a rectangle with
rounded corners, a custom pad or zone fill a polygon. Distances between
these shapes are exact, not sampled. Arc tracks and curved outlines are
divided into short segments with an error below 1 µm.

Measured values within 0.5 µm of a limit count as passing, so rounding in
generated geometry (a freshly filled zone, for example) never appears as a
violation.

## Compared with KiCad

The check was compared with KiCad 10's DRC on the KiCad example project
used in the tests: both find the same shorts, clearance violations and
unconnected items. KiCad has some checks that ElectronicWerk does not have
yet: solder mask bridges between pads, hole clearance to copper of other
nets, starved thermal reliefs and footprints that differ from their library.
