# Drawing footprints

A footprint is what a part needs on the board: **pads** to solder it to,
outlines on the **silkscreen** and **fabrication** layers, a **courtyard**
that keeps other parts away, texts for the designator and value, and a
**3D model**. Open the [library editor](./library-editor), choose
**Footprints** and click a footprint, **New** or **Wizard**.

For standard packages, start with the [footprint wizard](./footprint-wizard):
it computes the pads from the datasheet dimensions. The footprint editor is
for everything else and for the finishing touches.

![The footprint editor: a SOIC-8 from the wizard with pad 1 selected](/screenshots/libedit-footprint.png)

Footprints are drawn as seen from the top of the board. The small cross is
the footprint's origin: the point a placed footprint sits on, usually the
centre of the part.

## Drawing tools

The tools are the same as in the [symbol editor](./symbol-editor#drawing-tools),
with the **pad** tool instead of the pin tool:

| Tool            | Key            | What it does                                                                           |
| --------------- | -------------- | -------------------------------------------------------------------------------------- |
| Select and move | <kbd>Esc</kbd> | Select, drag a box, move, rotate, mirror, duplicate and delete pads and graphics.      |
| Pad             | <kbd>P</kbd>   | Each click places a pad like the selected pad, or like the last one. Numbers count up. |
| Line            | <kbd>L</kbd>   | Chained lines on the chosen layer.                                                     |
| Rectangle       | <kbd>B</kbd>   | Two opposite corners.                                                                  |
| Circle          | <kbd>C</kbd>   | Centre, then a point on the circle.                                                    |
| Polyline        | <kbd>K</kbd>   | Points; <kbd>Enter</kbd> or a double-click ends it.                                    |
| Text            | <kbd>T</kbd>   | A text on the chosen layer (1 mm high, 0.15 mm stroke).                                |

**Layer.** The **Layer** list in the options row sets where new lines,
rectangles, circles, polylines and texts go: silkscreen, fabrication or
courtyard, top or bottom, or copper. New graphics get the usual line width of
their layer:

| Layer       | Width   | Used for                                                  |
| ----------- | ------- | --------------------------------------------------------- |
| Silkscreen  | 0.12 mm | What is printed on the board: outline, pin 1 mark         |
| Fabrication | 0.10 mm | The real body outline for assembly drawings               |
| Courtyard   | 0.05 mm | The area the part needs; other parts must stay out        |
| Copper      | 0.20 mm | Copper drawn in the footprint, for example a thermal area |

Change a graphic's layer, points, width and **Filled** state in the side
panel after selecting it.

**Grid.** Pads need a fine grid: 0.05 mm by default, down to 0.01 mm. For
connectors on a 2.54 mm pitch choose 1.27 mm.

## Pads

A pad has a **number** (it connects to the symbol pin with the same number),
a **kind**, a **shape**, a **size**, a **position** and a **rotation**,
**layers**, and through-hole pads a **drill**.

| Kind            | Meaning                                                                       |
| --------------- | ----------------------------------------------------------------------------- |
| SMD             | A surface-mount pad on the top copper with paste and solder mask openings     |
| Through-hole    | A plated hole: copper on all layers, mask openings on both sides, and a drill |
| Non-plated hole | A mounting hole without copper connection (no number needed)                  |
| Edge connector  | A contact finger at the board edge: copper and mask, no paste                 |

Changing the kind keeps the pad consistent: a pad that becomes through-hole
gets all copper and mask layers and, if it had none, a drill of half its
smaller side (at least 0.3 mm); a pad that becomes SMD loses its drill.

| Shape             | Notes                                                                                  |
| ----------------- | -------------------------------------------------------------------------------------- |
| Circle            | The width is the diameter                                                              |
| Rectangle         | Pad 1 of many packages, to mark it                                                     |
| Oval              | A rectangle with fully rounded ends (stadium)                                          |
| Rounded rectangle | The default for SMD pads; the corner radius is a percentage of the shorter side (25 %) |
| Trapezoid         | A rectangle with opposite edges shortened by a delta                                   |
| Custom            | A polygon around the pad centre; starts as the pad's rectangle                         |

## The pad table

The **Pads** tab lists every pad with its number, kind, shape, width,
height, drill, rotation, position and layers (`↑` top, `↓` bottom, `*` all).

![The pad table](/screenshots/libedit-pad-table.png)

It works like the [pin table](./symbol-editor#the-pin-table):

- The table and the drawing share the selection; <kbd>Ctrl</kbd>/<kbd>⌘</kbd>
  and <kbd>Shift</kbd> select several rows, the header box selects all.
- A change of **kind**, **shape**, **width**, **height**, **drill** or
  **rotation** in a selected row applies to **all selected pads**: select all
  pads and make them 0.6 mm wide in one step. Numbers and positions change
  one pad at a time.
- **Renumber** numbers the selected pads from top to bottom, starting at the
  number you type (`1`, `A1`), with a step.
- **Delete** removes the selected pads.

### Pad arrays

**Pad array** adds a grid of pads in one step: the rows of a connector, the
balls of a BGA you draw by hand, a row of test points.

![Adding a pad array](/screenshots/libedit-pad-array.png)

The new pads copy the selected pad (or the last one, or a default SMD pad).
Enter the number of **columns** and **rows**, the **column pitch** and the
**row pitch** (positive pitches go right and down), the **first number** and
where the first pad goes. The pads are numbered row by row: `1, 2, 3 …`, or
`A1, A2 …` when the first number has a letter.

## Pad properties

Select one pad to edit all of its properties in the side panel:

![The properties of pad 1](/screenshots/libedit-pad-properties.png)

- **Number**, **kind** and **shape**; for rounded rectangles the **corner
  radius** in percent of the shorter side (0 to 50 %).
- **Size** (width and height) and **rotation** in degrees.
- **Drill** and **slot length** for through-hole pads: a slot length makes an
  oblong hole.
- **Position** relative to the footprint origin.
- **Layers**: top and bottom copper, all copper (`cu.*`), solder mask and
  paste openings on either side. An SMD pad without a paste opening gets no
  solder paste: useful for pads you solder by hand.
- **Solder mask margin** and **paste margin**: positive values enlarge the
  opening, negative values shrink it (`0` uses the board's rules).

## Texts

Every footprint has a **designator** text (`REF**`, replaced by `R1`, `U3` …
on the board) and a **value** text. The wizard puts the designator on the
silkscreen above the part and the value on the fabrication layer below it.
Select a text to change its layer, position, size, stroke, rotation and
visibility. Texts you add with the text tool are plain texts.

## Courtyard

The courtyard is the area the part claims on the board. The design rule
check of the PCB editor (a later version) keeps courtyards apart; the
[library checks](./library-checks) already report footprints without one and
pads that reach outside it.

Under **Courtyard** in the side panel, set a **margin** (0.25 mm by default,
IPC-7351 nominal) and click **Draw a courtyard around pads and body**. It
replaces the top courtyard by a rectangle around all pads and the
fabrication outline, the margin away, rounded outwards to 0.01 mm. Draw a
courtyard by hand on the courtyard layer for parts that are not rectangular.

## 3D models

A footprint can reference one or more 3D models (STEP or WRL files). The 3D
board view of a later version shows them; the library checks note footprints
without one.

![The 3D models of a footprint](/screenshots/libedit-models.png)

1. Type the model file into **Model file** and click **Add model**. Use a
   path relative to the project, or one with a library variable such as
   `${KICAD9_3DMODEL_DIR}/Package_SO.3dshapes/SOIC-8_3.9x4.9mm_P1.27mm.step`,
   like KiCad footprints do.
2. Align the model with **Offset X / Y / Z** (in the project's units, Z up
   from the board), **Rotation X / Y / Z** in degrees and **Scale X / Y / Z**.
3. **Hidden** keeps a model in the footprint without showing it; the trash
   button removes it.

**3D preview.** If the model is part of the [base library](./library),
**3D preview** shows it placed on a board as large as the footprint, with the
offset, rotation and scale you entered: every change of a value updates the
preview, so you can align the model step by step. Drag to turn the view.
Models that are not in the base library have no preview yet.

## Properties

| Property                            | Meaning                                                                                  |
| ----------------------------------- | ---------------------------------------------------------------------------------------- |
| **Name**                            | Change it with **Rename**; boards, symbols and components that use the footprint follow. |
| **Description**, **Keywords**       | Shown and searched in lists.                                                             |
| **Mounting**                        | Surface mount, through-hole or other; used by the placement file.                        |
| **Leave out of the BOM**            | For footprints that are not parts: logos, mounting holes, test points.                   |
| **Leave out of the placement file** | For parts the assembler does not place.                                                  |
| **Do not populate**                 | The part is on the board but not fitted.                                                 |

![The footprint editor in the dark theme](/screenshots/libedit-footprint-dark.png)
