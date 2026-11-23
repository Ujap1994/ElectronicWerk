# Design blocks

A design block is a piece of a circuit that you use again and again — a
power supply, a USB input with its protection, a crystal oscillator, an
LED driver — saved **with its layout**. Placing it in another project brings
the schematic, the library parts and the routed board section in one step.

![The design blocks dialog with a saved block](/screenshots/design-blocks.png)

## Saving a block

1. Open the schematic sheet and select what belongs to the block: the
   parts, the wires, the labels, the power ports, texts and drawings
   (<kbd>Ctrl</kbd>/<kbd>Cmd</kbd>+<kbd>A</kbd> selects the whole sheet).
2. Click **Design blocks** in the tool bar of the schematic editor, or run
   **Schematic: design blocks** in the [command palette](./command-palette).
3. Enter a **name** and a **description** and click **Save block**.

The block takes along:

- **The schematic objects** you selected. Sheet symbols (hierarchy) are not
  included.
- **The library items** the parts use: their symbols and footprints from the
  project library, so the block works in a project that has never seen them.
- **The layout**, if the parts are on a board: their footprints with their
  positions, sides and silkscreen texts, the tracks and vias of nets that
  only these parts use, and of their other nets (GND, the supply) the tracks
  and vias within 1 mm of their pads.

A name that exists already shows **Replace block** instead: saving replaces
the old block.

## Placing a block

Open the **Design blocks** dialog on the sheet that should get the block and
click **Place** next to it. In one undo step:

![The block placed a second time, below the original circuit](/screenshots/design-block-schematic.png)

1. The **schematic** is pasted into a free area of the sheet — clear of the
   existing circuit and of the title block — and selected, ready to be
   moved. Parts whose designators are taken get the next free numbers
   (R1 → R5), as when pasting.
2. **Library items** the project lacks are added to its project library. If
   the project has an item of the same name already, its own is kept.
3. On the **board**, the block's footprints are added the way
   [updating the board](./schematic-to-board) adds them — with the nets of
   the schematic — but at the block's positions, 10 mm below the board. The
   block's tracks and vias are copied onto the new nets. Footprints, tracks
   and vias form one [group](./selecting-and-editing#groups) named after the
   block, so one click selects the whole section and it moves as one piece
   onto its place.

![The block's layout as a group below the board](/screenshots/design-block-board.png)

The message after placing says how many parts, footprints, tracks and vias
came. Tracks and vias on a layer the board does not have (a 4-layer block on
a 2-layer board) are left out and counted as skipped.

### Connecting the block

The block's nets join the project's nets by name, as everywhere in a
schematic: power ports (GND, +3V3) and global labels connect with the same
nets of the whole project, local labels with labels of the same name on that
sheet. Unnamed nets stay the block's own. When a block is placed twice on
one sheet, or next to a label of the same name, rename its local labels if
the nets should stay apart — the [ERC](./erc) and the net navigator show
what is connected. Then draw the wires from the block to the rest of the
circuit and [update the board](./schematic-to-board) as usual.

## Where blocks are kept

Blocks are files (`<name>.ewblock`) in the folder **blocks** next to the
[base library](./library) in the ElectronicWerk folder of your user account:

| System  | Folder                                                |
| ------- | ----------------------------------------------------- |
| macOS   | `~/Library/Application Support/ElectronicWerk/blocks` |
| Windows | `%APPDATA%\ElectronicWerk\blocks`                     |
| Linux   | `~/.config/ElectronicWerk/blocks`                     |

Every project on the computer sees the same blocks. To share a block, copy
its file into a colleague's blocks folder; to keep blocks with a team, put
the folder under version control. **Delete** in the dialog removes a block
after a second click.

The file is JSON (see the [file format](../reference/file-format#design-blocks)):
the schematic objects, the library items and the layout, readable and
mergeable like every other ElectronicWerk file.

## Scripts

`block.list`, `block.save`, `block.place` and `block.delete` in the
[engine API](../reference/engine-api#design-blocks) do the same from a
script or an AI agent — for example to place a tested power supply into
every new project.
