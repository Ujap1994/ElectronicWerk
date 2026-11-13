# The 3D view

The 3D view shows a board as it will come back from the board maker and the
assembly house: the laminate with its holes and cut-outs, copper with its
surface finish, the solder mask, the silkscreen and every part with its 3D
model. It follows the layout editor live: move or turn a part and it moves
in 3D at once.

![The 3D view of the Blinky example](/screenshots/view3d.png)

## Opening the 3D view

- **Show the board in 3D** (the cube) in the toolbar of the layout editor.
- **3D view** in the details of a board in the [project tree](./project-tree).
- **Show the 3D view** in the [command palette](./command-palette) (the
  first board of the project).

The 3D view opens in a tab next to the board, named after the board with
**(3D)**. Close it like every other tab.

### In its own window

**Show the 3D view in its own window** (the window symbol, at the right end
of the toolbar) opens the 3D view in a separate window. Put it on a second
monitor and keep the layout editor on the first one: every edit shows up in
3D while you work. The window closes when the project closes or the main
window closes; opening it again for the same board brings the open window to
the front.

![The 3D view in its own window](/screenshots/view3d-window.png)

## Moving around

| Action              | Mouse and keys                                                |
| ------------------- | ------------------------------------------------------------- |
| Turn the view       | Drag with the left mouse button                               |
| Pan                 | Drag with the right mouse button (or Shift + left button)     |
| Zoom                | Mouse wheel (zooms towards the pointer)                       |
| Fit the board       | **Fit the board** in the toolbar, or **Home** in the view     |
| Select a part       | Click it; Shift-click (Ctrl/⌘-click) adds it to the selection |
| Clear the selection | Click the background                                          |
| Stop measuring      | **Esc**                                                       |

### Views and projection

**View from the isometric side** (the cube) turns the board back to the view
it opens with. **Views** opens the other directions: **Top**, **Bottom**,
**Front**, **Back**, **Left** and **Right**. Each one fits the board.

![The views menu](/screenshots/view3d-views.png)

**Perspective** switches between a perspective view (things further away
look smaller) and an orthographic view without perspective, which is better
for comparing heights and for checking alignments from the top or the side.

| Top, orthographic                                      | Bottom, orthographic                                         |
| ------------------------------------------------------ | ------------------------------------------------------------ |
| ![The board from the top](/screenshots/view3d-top.png) | ![The board from the bottom](/screenshots/view3d-bottom.png) |

The bottom view shows the board as you see it when you turn it over: the
bottom silkscreen reads correctly and the parts on the bottom face you.

## What is shown

**Show** (the layers symbol) switches each part of the model on and off:

| Switch                           | Shows                                                                                                     |
| -------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Parts on the top / on the bottom | The 3D models of the parts on each side                                                                   |
| Parts marked “do not populate”   | Parts with the DNP attribute (off by default, like on the assembled board)                                |
| Silkscreen                       | The legend of both sides, cut at the mask openings and the board edge                                     |
| Solder mask                      | The mask of both sides, in the colour of the [stack-up](./board-setup), open over the pads                |
| Copper                           | The outer copper (tracks, pads, zones) and the plated walls of holes, in the colour of the surface finish |
| Solder paste                     | The paste on the pads (off by default)                                                                    |
| Board                            | The laminate                                                                                              |
| Inner copper layers              | The copper of inner layers, for [section views](#sections)                                                |

The switches are remembered for all boards. Without mask and silkscreen the
copper underneath becomes visible:

![The board without solder mask and silkscreen](/screenshots/view3d-show.png)

### Colours and heights

The 3D view takes everything from the board:

- the **mask and silkscreen colours** from the [stack-up](./board-setup)
  (green, red, blue, black, white, … or a hex colour such as `#1a3c6e`),
- the **surface finish** colour of exposed copper: gold for ENIG and ENEPIG,
  bright copper for OSP and bare copper, silver for HASL and the others,
- the **thicknesses** of the stack-up: the laminate spans the dielectrics and
  the inner copper layers, the outer copper lies on its faces, the mask on
  the copper, the silkscreen on the mask.

Parts stand on the outer copper of their side: a model's z = 0 is the copper
surface, as in KiCad's 3D library.

## Parts and their 3D models

Footprints name their 3D models like KiCad does, for example
`${KICAD10_3DMODEL_DIR}/Package_SO.3dshapes/SOIC-8_3.9x4.9mm_P1.27mm.step`
or `${KIPRJMOD}/3d/relay.step`, with an offset, a rotation and a scale. Edit
them in the [footprint editor](./footprint-editor#3d-models). The 3D view
looks for each model in this order:

1. **A file on this computer.** `${KIPRJMOD}` and relative paths point into
   the project folder, other variables are read from the environment, and
   KiCad's model variables (`KICAD10_3DMODEL_DIR`, `KICAD9_3DMODEL_DIR`, …)
   also look in the 3D model folder of a KiCad installation (on macOS in
   `/Applications/KiCad`, on Windows in `C:\Program Files\KiCad\<version>`,
   on Linux in `/usr/share/kicad` and the KiCad Flatpak). A `.wrl` path also
   finds a `.step` file of the same name, as in KiCad. The environment
   variable `EW_KICAD_3DMODEL_DIRS` (a list of folders, like `PATH`)
   replaces the search for KiCad installations; set it empty to use none.
2. **The [base library](./library).** A model of the library is shown from
   the mesh cache; if it is not downloaded yet, its mesh pack is downloaded
   the first time.

STEP files are turned into triangle meshes once with OpenCASCADE and kept in
the mesh cache (named by the file's content, so a changed file is converted
again). A large model can take a few seconds the first time; after that the
board opens at once.

The status line at the bottom left counts the models: **Loading models 3 of
8…** while they load, then **8 of 8 models**. Models that were not found are
listed under **… not found**, with hints where to put them. Parts without a
model are still on the board: select them with a click on their pads.

::: tip Models of an imported KiCad project
A KiCad project that keeps its own models in its folder (`${KIPRJMOD}/…`)
needs them in the ElectronicWerk project folder too: copy the model folder
next to the `.ewproj` file, keeping the same relative path.
:::

## Selection

The selection is shared with the layout editor and the schematic, in every
window: a part selected in the layout editor is framed in the 3D view, and a
part clicked in 3D is selected in the layout editor (and, with
[cross-probing](./schematic-to-board#cross-probing) on, its symbol in the
schematic). The status line names the selected part with its value and
footprint.

![U1 selected in the layout editor and in 3D](/screenshots/view3d-selection.png)

## Live updates

The 3D view follows every change of its board without a reload: a moved,
turned or flipped part is placed again within a fraction of a second (the
parts come from a quick update of their placements, about 7 ms for a board
with 5,000 parts); the copper, mask and silkscreen follow as soon as the
edits pause. Undo and redo work the same way. Building the 3D board takes
about 60 ms for a board with 500 parts and 20,000 track segments and about
0.6 s for 5,000 parts and 200,000 segments.

## Sections

**Section** cuts the board open along a plane: **Along x**, **Along y** or
**Horizontal**, with the **Position** slider across the board. **Keep the
other side** shows the other half. Use it to look at the pins of a
through-hole part in its holes, at the stand-off of a part over the board, or
(with **Inner copper layers** switched on) at the planes inside the board.

![A section through the SOIC and the pin header](/screenshots/view3d-section.png)

## Measuring

**Measure** (the ruler) measures between two points on any surface: click
the first point, then the second. The label shows the distance and its x, y
and z parts. A third click starts a new measurement; **Esc** or **Measure**
again stops. Measure in the orthographic top or side view for clean
distances in one plane.

![A measurement in the top view](/screenshots/view3d-measure.png)

## Screenshots and export

**Screenshot** (the camera) saves the view as a PNG image, exactly as shown.

**Export** saves the board as a 3D model for other programs: **STEP model…**
for mechanical CAD (enclosures, mounting), **GLB model…** for viewers,
renderers and web pages, **VRML model…** for older tools. The export uses
the drill and placement origin of the board. See
[3D models: STEP, GLB and VRML](./export-3d) for what each file contains;
the same files can be generated by an [output job](./outputs) or on the
command line.

![The export menu of the 3D view](/screenshots/view3d-export.png)
