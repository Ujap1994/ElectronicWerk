# The assembly assistant

The assembly assistant helps you solder a board by hand. It shows the
board, picks the next parts to place, highlights where they go and keeps a
checklist of what you have done. You place one group of equal parts at a
time: first all 10 kΩ resistors, then all 100 nF capacitors, and so on.
Each group is one trip to the parts drawer.

![The assembly assistant: board on the left, the current step and the checklist on the right](/screenshots/assembly.png)

## Opening the assistant

- **Assembly assistant** on the details panel of a board (select the board
  in the [project tree](./project-tree)):

  ![The button on the board details](/screenshots/assembly-button.png)

- **Assembly assistant: place parts by hand** in the
  [command palette](./command-palette) opens it for the first board of the
  project.

The assistant opens in a tab of its own, next to the documents. It reads
the board as it is: changes in the layout editor show up at once.

## Steps

The assistant groups the parts of the board. Parts with the same **side**,
**mounting type**, **value**, **footprint** and **part number** form one
group, so every part of a group comes out of the same reel or bag.

The groups come in this order:

1. **SMD parts on the first side**, then **SMD parts on the other side**.
2. **Through-hole parts**, starting with the first side again. They come
   last because they keep the board from lying flat.
3. Within each of these, the order you choose under **Order**:

| Order                           | Meaning                                                                                                                |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Small parts first** (default) | By footprint size: 0402 before 0603, chips before ICs. Small parts are hard to reach once tall parts sit next to them. |
| **By value**                    | Alphabetically by value, then footprint (`1k` before `10k`, numbers in their natural order).                           |
| **Most parts first**            | The largest groups first.                                                                                              |
| **By designator**               | By the first designator of each group (`C1`, `R1`, `U1`).                                                              |

**Start with** chooses the first side: **Top side** (default) or **Bottom
side**.

Parts marked **Do not populate** are left out and listed under the steps
(“Not populated: R4”). Parts of the schematic that have no footprint on
the board yet are listed as “Not on the board”:
[update the board from the schematic](./schematic-to-board) first if you
see this.

## The current step

![The card of the current step](/screenshots/assembly-current.png)

The card at the top of the panel shows the step you are on:

- the **step number**, the **side** and the **mounting type**,
- the **value** in large letters,
- the **footprint**, **quantity** (placed / all), **part number** and
  **manufacturer** when the part has them,
- one button per **designator**. Click a designator to tick off a single
  part (or untick it again).

The board view follows the step:

- the parts of the current step that are still to be placed are
  **highlighted**,
- parts of later steps are **dimmed**, so you see where the current parts
  go and which spots are still free,
- only the layers of the current side are shown: silkscreen, fabrication
  and courtyard drawings, and the copper faintly underneath.

Steps on the bottom side turn the board over: the view shows the board from
below, the way you hold it, with the note **Seen from the bottom**:

![A step on the bottom side, seen from below](/screenshots/assembly-bottom.png)

Zoom and pan as in the [viewer](./viewer); **Home** fits the board again.

## Ticking off

| Button            | Key                                  | What it does                                                                                      |
| ----------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------- |
| **Placed – next** | <kbd>Space</kbd> or <kbd>Enter</kbd> | Ticks off every part of the current step and goes to the next step that still has parts to place. |
| **Skip**          | <kbd>→</kbd> or <kbd>↓</kbd>         | Goes to the next step without ticking anything off.                                               |
| **Back**          | <kbd>←</kbd> or <kbd>↑</kbd>         | Goes to the previous step.                                                                        |

The **Steps** list below the card shows every step with its value,
footprint, progress (`2/3`) and side. Click a step to go to it. A step whose
parts are all placed gets a tick. **Hide placed** shortens the list to the
steps that still have work.

The progress bar at the top counts the placed parts of the whole board.
When every part is placed, the card says **All parts are placed**.

## Several boards

Building a batch? **Board finished – next board** counts the board as
finished and starts the next one with an empty checklist. The heading shows
the number of the board you are on (**Board 2**).

**Reset** unticks every part of the current board (click twice to confirm).
The count of finished boards stays.

## At the workbench

- **Workbench mode** makes the text and the buttons large enough to read
  and hit from a distance, with gloves or tweezers in your hand.
- **Own window** moves the assistant into a window of its own, for a second
  monitor or a tablet-sized screen above the bench. **Full screen** (or
  <kbd>F11</kbd>) fills that screen.

![The assistant in its own window, in workbench mode](/screenshots/assembly-workbench.png)

The main window and the assistant's window show the same progress: a tick
in one appears in the other at once.

## Where the progress is saved

The progress is saved at once in `assembly.json` in the project folder —
ticking off a part is not a design change, so there is nothing to undo and
nothing to save. The file keeps, per board, the designators that are placed
and the number of finished boards. It survives closing the project and
works with [source control](./github): commit it to share the state of a
build, or add it to `.gitignore` to keep it on your computer. The format is
described in the [file format reference](../reference/file-format#assembly-progress-assembly-json).

## From the command line

`ewerk assembly` prints the steps with the saved progress, for a printed
checklist or a script:

```sh
ewerk assembly "Sensor Board" --order size --first-side top
```

```text
Sensor Board: 0 of 50 parts placed
  1. top    10k          R_0603                       [ ] R1  [ ] R3  [ ] R4  …
  2. top    100n         C_0603                       [ ] C1  [ ] C2  [ ] C4  …
  3. top    NPN          SOT-23                       [ ] Q1  [ ] Q2  [ ] Q3
  …
  8. top    Header       PinHeader_1x10_P2.54mm       [ ] J1  [ ] J2
```

See [`ewerk assembly`](../reference/cli#ewerk-assembly). Scripts and AI
agents tick parts off with the engine methods
[`assembly.plan`, `assembly.mark` and `assembly.reset`](../reference/engine-api#assembly).
