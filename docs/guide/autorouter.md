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
| Existing routing       | Tracks and vias; **locked** ones are marked _protect_, so the router keeps them.                              |

Coordinates are micrometres with the y axis up, like the board.

## What comes back

The routing of **every net in the session replaces the unlocked tracks and
vias of that net**; locked tracks stay as they are. Nets the session does
not mention keep their routing. Vias get the size of their padstack
(`Via[0-1]_600:300_um` is a 0.6 mm via with a 0.3 mm drill through both
layers).

The import is **one step to undo**: <kbd>Ctrl</kbd>+<kbd>Z</kbd> takes the
whole session back.

The session uses a 0.1 µm grid. Track ends land within 0.05 µm of the pad
centres, well within the pads.

## From the command line

```sh
ewerk board export-dsn "Sensor Board" -o sensor.dsn
# … route sensor.dsn, for example with Freerouting …
ewerk board import-ses "Sensor Board" sensor.ses
```

See [`ewerk board export-dsn`](../reference/cli#ewerk-board-export-dsn--import-ses)
and the engine methods
[`pcb.exportSpecctra`](../reference/engine-api#pcb-exportspecctra) and
[`pcb.importSession`](../reference/engine-api#pcb-importsession).
