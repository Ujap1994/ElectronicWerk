# ADR-0021: Everyday editing

- **Status:** accepted
- **Date:** 2026-11-23
- **Milestone:** M20

## Context

After M19 the maintainer chose the everyday tools of Altium Designer and
KiCad for M20: a selection filter, select similar and editing many objects
at once, a net inspector, net colours, groups, custom shortcuts,
calculators, and logos and QR codes on the board. Several of them needed a
decision where their data lives and which part of the program does the
work.

## Decision

### Net colours belong to the project, by net name

A net colour is a project setting (`settings.netColors` in the `.ewproj`
file: net name → `#rrggbb`), not a property of a board net or of a sheet.

- **Why the project:** a net should look the same on its sheets and on its
  board. Sheets have no net objects (nets follow from the wires), boards
  have them; only the name is common to both. KiCad keeps net colours in its
  project file for the same reason.
- **Why not the app settings:** colours describe a design (supplies, clocks)
  and go with it to other people and machines; the switch **Show net
  colours** is a view setting and stays in the session.
- Changing a colour is a project-property patch (`project.setProperties`),
  so it is one undo step and needs no new engine method.

The renderer draws them: a second per-object texture holds the colour of
every object (alpha 0: none), and the shaders take it instead of the layer
colour on copper layers and on the wire, bus, junction and label layers of
sheets. Boards colour by the net that the scene stores for every object;
sheets ask the engine for the members of the coloured nets
(`schematic.netColors`), as for highlighting.

### Groups are board objects

A group is an object of the board (`groups`: `id`, `name`, `members`), so
it is saved, undone and redone like any other object. A member belongs to
at most one group. The selection expands a hit to its group in the viewer,
so every tool that works on the selection — move, rotate, flip, lock,
delete, the properties panel — works on groups without knowing them.
Edits that delete members or regroup objects keep the groups consistent;
groups of fewer than two members go.

### Shortcuts are app settings; the editors look up commands

The keys of editor commands are a setting of the user (`shortcuts` in
`settings.json`: only the changed keys, command id → key), not of the
project. A table in the shared code lists every command with its context
(schematic or board) and default key; the editors ask which command a key
press starts instead of testing letters. Keys are written in one normal
form (`ctrl+shift+g`); `ctrl` is ⌘ on macOS, and Shift counts only for
letters and named keys, because signs such as `/` need Shift on some
keyboards (German: Shift+7) and not on others. Clipboard, undo, saving and
the keys that end or confirm an action are reserved.

### Calculators run in the engine

The calculators are engine methods (`calc.*`) rather than code of the
window. They need no project, so `ewerk call` and AI agents (MCP
`call_engine`) get exactly the values of the app, and tests check them
against published reference values. For the current of tracks the app
uses a published least-squares fit of IPC-2152 chart 5-2 and shows the
conditions of that chart; IPC-2221 is offered for comparison.

### Pictures are traced in the engine, decoded in the window

The window decodes the picture (the browser reads PNG, JPEG, GIF, BMP,
WebP and SVG) and turns it into black and white pixels with the threshold
the user sets; the engine gets rows of `#` and `.` (`pcb.addImage`). The
engine follows the outlines of the pixel regions and, because board
polygons have no holes, joins every hole to its outline with a cut of zero
width — as Gerber regions describe holes. The engine needs no image
library, the method is usable from scripts, and the tracing is tested on
random pictures (the area of the polygons equals the number of set pixels).
QR codes come from the `qrcodegen` crate (MIT).

Shapes on copper layers became copper items without a net, so the checks,
the zone filler and the router keep their clearance to them; before, drawn
copper was invisible to all three.

## Consequences

- New engine methods: `pcb.nets`, `document.similar`, `schematic.netColors`,
  `calc.eSeries`, `calc.divider`, `calc.regulator`, `calc.track`,
  `calc.via`, `calc.fusing`, `pcb.addImage`, `pcb.addQrCode`, `pcb.qrCode`;
  `pcb.edit` gains `group` and `ungroup`.
- File format: `groups` in boards, `netColors` in the project settings;
  both are optional, so older files read unchanged.
- The selection filter and the switch for net colours are session state;
  shortcut keys are user settings.
- Limits: groups exist on boards only; net colours do not reach plots and
  Gerber files; pictures keep their pixel steps (no curve fitting).
