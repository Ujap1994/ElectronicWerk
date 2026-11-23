# Net inspector and net colours

Two tools help to keep an overview of the nets of a design:

- the **net inspector** lists every net of a board with its class, routed
  length, tracks, vias, pads and open connections;
- **net colours** give important nets — supplies, clocks, fast signals — a
  colour of their own, on the board and on the sheets.

## The net inspector

Click **Nets** in the layout editor's tool bar. A table opens below the
board:

![The net inspector below the board, VCC and /OUT coloured](/screenshots/net-inspector.png)

| Column | Meaning                                                                                |
| ------ | -------------------------------------------------------------------------------------- |
| Net    | The name, with a dot in the net's colour (if it has one)                               |
| Class  | The [net class](./design-rules#net-classes)                                            |
| Length | The routed length of the net's tracks (vias are not counted; see [lengths](./lengths)) |
| Tracks | Number of track segments                                                               |
| Vias   | Number of vias                                                                         |
| Pads   | Number of pads on the net                                                              |
| Open   | Connections still missing (the ratsnest lines of the net), in orange                   |

- **Sort** by clicking a column title; a second click reverses the order.
  Numbers sort largest first, names alphabetically.
- **Filter** with the field at the top: the words must appear in the name or
  the class (`gnd`, `power`, `/spi`).
- **Click a row** to highlight the net on the board — its copper is drawn
  brighter and a chip in the footer names it. Click the row again to end
  the highlight.
- The table follows every edit: route a track and its length and open
  connections change at once.

![The net inspector with one net ticked](/screenshots/net-inspector-panel.png)

### Changing several nets at once

Tick nets in the first column (the box in the title row ticks all nets
shown). A bar appears above the table:

- **Net class** gives every ticked net the chosen class — one undo step
  (**Change properties**). The classes and their rules are edited in the
  [design rules](./design-rules).
- **Colour** gives every ticked net a colour, or **None** removes it — one
  undo step (**Change project properties**).

The ticks stay after a change, so a class and a colour can be set one after
the other. Close the inspector with **×** or the **Nets** button.

## Net colours

A net with a colour is drawn in it:

- on **boards**: the copper of its tracks, vias, zones and pads, on every
  copper layer (silkscreen, mask and the other layers keep their colours);
- on **sheets**: its wires, buses, junctions and net labels.

![VCC (blue) and /OUT (orange) on the board](/screenshots/picture-board.png)

![The same nets on the sheet](/screenshots/net-colors-sheet.png)

Colours belong to the **net name** and are saved in the project, so a net
looks the same on its sheets and on its board.

### Giving a net a colour

- In the **properties panel**: select a track, via, zone or pad on the board,
  or a wire, bus, junction or label on a sheet. **Colour of net …** offers
  eight colours, a free colour (the small field after the eight) and
  **None**.
- In the **net inspector**: tick nets, then click a colour in the bar.

![The colour of the net of a track](/screenshots/net-color-picker.png)

The eight colours stand out on dark copper as well as on white paper. A
free colour is taken when its picker closes. Each change is one undo step.

Tip: avoid the colours of the copper layers for the nets you colour (the
top copper is red), or the net will not stand out.

### Showing and hiding them

**Show net colours** at the bottom of the layer panel switches all net
colours off and on, for every open sheet and board. It is shown when the
project has net colours, and is kept for the session. The colours stay in
the project; plots, Gerber files and PDF prints never use them.

### Where they are saved

Net colours are part of the project settings in the `.ewproj` file
(`settings.netColors`, net name → `#rrggbb`; see the
[file format](../reference/file-format#the-manifest-ewproj)). Scripts and AI
agents change them like the other project settings with
`project.setProperties`:

```json
{ "settings": { "netColors": { "VCC": "#1e88e5", "/OUT": null } } }
```

`null` removes a colour. `schematic.netColors` returns the objects of a
sheet that belong to coloured nets; `pcb.nets` returns the rows of the net
inspector (see the [engine API](../reference/engine-api)).
