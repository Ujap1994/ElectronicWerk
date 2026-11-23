# Rooms

A room is a named area on the board for a part of the circuit: the power
supply, the input stage, the radio. Parts can be placed into a room in one
step, the parts of each schematic sheet can get a room of their own, a room
carries its parts along when it is moved, and [custom design rules](./design-rules#custom-rules)
can treat the copper inside a room differently.

![The room "Output" with the timer U1 placed into it](/screenshots/rooms.png)

Rooms are a placement aid: they are drawn on the screen (layer **Rooms** in
the layer list) but never in an output.

## Drawing a room

1. Choose the **room** tool (<kbd>Shift</kbd>+<kbd>O</kbd>).
2. Two clicks draw a rectangle. <kbd>/</kbd> switches to a polygon: click the
   corners, <kbd>Enter</kbd> or a double-click closes it.
3. Type the name of the room and press <kbd>Enter</kbd>. Names are unique on
   a board.

The select tool comes back, so the next clicks pick parts for the room.

![The properties of a room](/screenshots/room-properties.png)

The properties show the name, how many parts lie inside and whether the room
is locked. Rename a room there; custom rules that name it must then be
changed too.

## Placing parts into a room

Select the room and the parts (<kbd>Shift</kbd>+click adds to the
selection), then click **Place the … selected parts in the room** in the
properties. The parts are packed into the room in rows, in the order of
their designators, below the room's name and with a gap of 1 mm between
them. Locked parts stay where they are. If the room is too small, the parts that do not
fit are named in a message and stay where they were: make the room larger
and place them again.

## A room for every sheet

**Board: a room for the parts of each schematic sheet** in the
[command palette](./command-palette) makes one room per schematic sheet,
named after the sheet, and packs the sheet's parts into it. New rooms go
below the board, side by side, each about as wide as it is high. Parts that
are already on the board, already in a room or locked are left alone, so the
command can run again after new parts came from the schematic. A sheet whose
room exists already gets its new parts packed into that room.

This is a quick start for a new board: all parts of a circuit block end up
together, ready to be moved onto the board as a group.

## Moving a room

Moving a room moves the unlocked parts inside it along — drag the room by
its edge or select it and press <kbd>M</kbd>. Turning or flipping a room
changes only the room. Lock a room to keep it in place while its parts are
arranged.

## Rooms in design rules

In a custom rule, `A.inRoom('Power')` is true for copper inside the room
named `Power`. One point of the item counts — the centre of a via or round
pad, the start of a track, a corner of other pads — so draw rooms a little
larger than the parts in them (the same holds for `insideZone`):

| Condition                                 | Use                                 |
| ----------------------------------------- | ----------------------------------- |
| `A.inRoom('Power')`                       | Wider tracks in the power supply    |
| `A.inRoom('RF') && B.inRoom('RF')`        | Smaller clearance inside the radio  |
| `A.inRoom('Mains') && !B.inRoom('Mains')` | Creepage between mains and the rest |

See [design rules](./design-rules#the-condition-language) for the whole
language and [creepage](./creepage) for high-voltage spacing.

## Scripts

[`pcb.addRoom`, `pcb.placeInRoom` and `pcb.roomsFromSheets`](../reference/engine-api#dimensions-and-rooms)
do the same from a script or an AI agent.
