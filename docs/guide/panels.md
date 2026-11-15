# Panels

A panel puts several copies of a board on one sheet. Board makers and
assembly services handle small boards in panels: the machines need a
minimum size, a stencil covers all copies at once, and a panel of eight
boards is assembled in one run instead of eight. After assembly the boards
are broken out of the panel.

ElectronicWerk makes the panel when it generates the outputs. You keep
designing one board; the [output job](./outputs) writes the Gerber, drill
and placement files for the whole panel.

![The panel settings of an output job with the preview](/screenshots/panel-settings.png)

## Making a panel

1. Open the output job (**Outputs** in the toolbar).
2. Select **Panel** below the list of outputs.
3. Switch on **Make the board outputs for a panel** (or tick the box next
   to **Panel** in the list).
4. Set the number of boards, the separation and the rails. The preview
   below the settings shows the panel and its size at once.
5. **Generate all**.

The list entry shows the panel in short, for example
**2 × 2, Mouse bites**, or **Single boards** when the panel is off. Switch
it off to make the files for one board again (switching it on again starts
from the default settings).

## What is made for the panel

| Output             | For the panel                                                                                                                                                          |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Gerber**         | All layers of the panel: the copies of the board, the rails, fiducials and tooling holes, and the outline of the whole panel. V-score lines go to a file of their own. |
| **Drill**          | The holes of all copies, the mouse bites and the tooling holes.                                                                                                        |
| **Drill map**      | The drill drawing of the panel.                                                                                                                                        |
| **Pick and place** | One line per part and copy, measured from the lower-left corner of the panel. Fiducials, tooling holes and mouse bites are not in it.                                  |
| everything else    | Unchanged, for one board: the bill of materials, prints, netlists, IPC-D-356, STEP and the [documentation](./documentation).                                           |

The files of a panel are named after the board with `_panel`, for example
`Sensor_Board_panel-Copper_Top.gtl` and `Sensor_Board_panel-placement.csv`,
so you can tell them from the files of a single board.

![The panel in the Gerber viewer: four boards, rails with fiducials and tooling holes, tabs with mouse bites](/screenshots/panel-gerber.png)

Check the panel in the [Gerber viewer](./gerber-viewer) before you send it.

## Settings

| Setting                | Default        | Meaning                                                                                                                                                                              |
| ---------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Columns**, **Rows**  | 2 × 2          | Copies side by side and above each other (1 to 50 each).                                                                                                                             |
| **Separation**         | Mouse bites    | How the boards are held and broken out (see below).                                                                                                                                  |
| **Gap between boards** | 2 mm           | Width of the milled gaps (mouse bites only). Ask your board maker for its smallest milling tool; 2 mm is common.                                                                     |
| **Tabs per edge**      | 2              | Tabs on each edge between two boards, or between a board and a rail (mouse bites only, 1 to 10).                                                                                     |
| **Tab width**          | 2 mm           | Width of a tab.                                                                                                                                                                      |
| **Rails**              | Top and bottom | Frame strips along the panel: **Top and bottom**, **Left and right**, **All four sides** or **None**. Assembly machines hold the panel at its rails, along the direction it travels. |
| **Rail width**         | 5 mm           | Width of each rail.                                                                                                                                                                  |
| **Fiducials**          | on             | Three fiducials on the rails, so the camera of a pick-and-place machine finds the panel's position and orientation.                                                                  |
| **Tooling holes**      | on             | A non-plated hole in each corner of the rails, to pin the panel on the machine or the stencil printer.                                                                               |

### Mouse bites

![A 2 × 2 panel with mouse bites](/screenshots/panel-preview-mouse-bites.png)

The boards sit apart, with milled gaps between them. Short **tabs** bridge
the gaps and hold the boards; where a tab meets a board it carries a row
of small holes (0.5 mm, every 0.8 mm), so you can snap it off by hand. The holes leave small bumps on the board edge, like a bite — hence
the name. Sand them off if the board has to fit tightly into a case.

Mouse bites work for any board outline. For a board that is not a
rectangle, the tabs are placed on the board's bounding box; the preview
says so, and you should check in the Gerber viewer that each tab meets the
outline.

### V-score

![A 2 × 2 panel with V-score lines and no rails](/screenshots/panel-preview-v-score.png)

The boards touch each other. The board maker cuts V-shaped grooves from
both sides along the edges (usually a third of the board thickness from
each side), and you snap the boards apart along them. V-score leaves clean, straight edges and
wastes no space between the boards.

A V-score line always runs across the whole panel, so this needs
**rectangular boards**. The lines are written to their own Gerber file
(`…-User_1.gbr`, on the layer _User 1_); tell the board maker that this file
holds the V-score lines when you order.

## Fiducials and tooling holes

- A **fiducial** is a round copper dot of 1 mm with a 2 mm opening in the
  solder mask. Three of them sit on the rails, 10 mm from three of the
  corners — the missing fourth makes the orientation unique.
- A **tooling hole** is a 2 mm non-plated hole, 5 mm from each corner of
  the rails.

Both need rails at least 3 mm wide. If there are no rails, or they are
too narrow, the preview says that they are left out:

| Hint                                                            | What to do                                                                      |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Tooling holes need rails at least 3 mm wide; they are left out. | Choose rails and a rail width of 3 mm or more, or switch the tooling holes off. |
| Fiducials need rails at least 3 mm wide; they are left out.     | The same for the fiducials.                                                     |
| The board is not a rectangle: the tabs sit on its bounding box. | Check the tabs in the Gerber viewer.                                            |
| V-score needs a rectangular board.                              | Use mouse bites for this outline.                                               |
| The board has no closed outline.                                | Draw the [board outline](./board-setup#board-outline) first.                    |

## Several boards

The panel is made of the board of each output: an output for a second
board (chosen under **Board** in its settings) makes a panel of that
board. In the preview, **Board** chooses which board to show.

## In the job file

The panel is part of the output job document (`.ewout`) under `panel`, with
the lengths in nanometres. See
[the file format](../reference/file-format#panels). Scripts can draw the
preview with the engine method
[`outputs.panelPreview`](../reference/engine-api#outputs-panelpreview).
