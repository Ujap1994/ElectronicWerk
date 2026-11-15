# External autorouters (Specctra)

ElectronicWerk routes interactively and has a simple walkaround router for
scripts. For a full autorouter, hand the board to an external one such as
**Freerouting**: ElectronicWerk writes the board as a **Specctra DSN** file,
the router routes it and saves a **Specctra session (SES)**, and
ElectronicWerk reads the routing back.

![The autorouter dialog](/screenshots/autorouter-dialog.png)

## Routing a board

1. Open the board in the [layout editor](./layout-editor). Place every part
   and draw the board outline first; [lock](./layout-editor) tracks you want
   to keep (<kbd>L</kbd>).
2. Choose **Autorouter** in the toolbar (or **Board: external autorouter**
   in the [command palette](./command-palette)).
3. **Export DSN file…** saves the board as `<board>.dsn` (next to the
   project by default).
4. Open the DSN file in the router (in Freerouting: _File → Open_), let it
   route and save the result as a session file (_File → Export Specctra
   Session File_).
5. Back in ElectronicWerk, **Import session file…** and choose the `.ses`
   file.

The result line says how many tracks and vias came in and how many were
replaced. Nets of the session that the board does not have, and layers it
does not have, are named and skipped.

After the import, [fill the zones](./zones) and run the
[design rules check](./drc): routers keep to the rules of the DSN file, but
zones and your own [custom rules](./design-rules) are checked only by
ElectronicWerk.

## What the router gets

| In the DSN file        | From                                                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------------------------------- |
| Copper layers          | The stack-up (`F.Cu`, `In1.Cu`, …, `B.Cu`).                                                                   |
| Boundary and keep-outs | The board outline; cut-outs and [keep-out areas](./zones#keep-out-areas) (tracks, vias).                      |
| Planes                 | Copper pours with a net (the router treats them as connected areas).                                          |
| Parts and pads         | Every footprint as placed, its pads with their shape on each copper layer; holes without copper as keep-outs. |
| Nets and classes       | Every net with its pins; one class per [net class](./design-rules) with its track width, clearance and via.   |
| Existing routing       | Tracks and vias: the router may change them; **locked** ones are marked _protect_, so the router keeps them.  |

Coordinates are micrometres with the y axis up, like the board.

## What comes back

The routing of **every net in the session replaces the unlocked tracks and
vias of that net**; locked tracks stay as they are. Nets the session does
not mention keep their routing. Wires the router marks as _protect_ are
compared with the board's locked tracks: those are not added twice, every
other protected wire (for example one you fixed in the router) comes in
like the rest. Vias get the size of their padstack
(`Via[0-1]_600:300_um` is a 0.6 mm via with a 0.3 mm drill through both
layers).

The import is **one step to undo**: <kbd>Ctrl</kbd>+<kbd>Z</kbd> takes the
whole session back.

The session uses a 0.1 µm grid. Track ends land within 0.05 µm of the pad
centres, well within the pads.

## Freerouting

ElectronicWerk is tested with **Freerouting 2.5.0**: a recorded Freerouting
run on a test board is replayed in every build, and after the import and a
zone fill the board has no unrouted connection and no design rule error.

- The **desktop app** of Freerouting brings its own Java. The plain
  `freerouting-2.5.0.jar` needs Java 25 or newer.
- Planes: Freerouting connects the pads of a net with a copper pour (for
  example GND) to the pour with short tracks and vias. These connections are
  complete only once the zones are filled again.
- Freerouting routes without its window too; `-da` switches off its
  anonymous usage statistics:

```sh
freerouting -de sensor.dsn -do sensor.ses -da --gui.enabled=false
```

On macOS the launcher is
`/Applications/freerouting.app/Contents/MacOS/freerouting`. The
command-line build `freerouting-cli` of version 2.5.0 stops on boards with
keep-out areas (mounting holes, keep-out zones) with an
`UnsupportedFeatureError`; use the desktop app or the jar instead.

## From the command line

```sh
ewerk board export-dsn "Sensor Board" -o sensor.dsn
# … route sensor.dsn, for example with Freerouting …
ewerk board import-ses "Sensor Board" sensor.ses
```

See [`ewerk board export-dsn`](../reference/cli#ewerk-board-export-dsn-import-ses)
and the engine methods
[`pcb.exportSpecctra`](../reference/engine-api#pcb-exportspecctra) and
[`pcb.importSession`](../reference/engine-api#pcb-importsession).
