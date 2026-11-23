# Logos and QR codes

A company logo on the silkscreen, a symbol in copper, a QR code that leads
to the documentation or names the serial number: the layout editor turns
pictures and texts into filled shapes on any layer.

![A logo and a QR code on the top silkscreen](/screenshots/picture-board.png)

Click **Logo or QR code** in the layout editor's tool bar, or use
**Board: add a logo or picture** and **Board: add a QR code** in the
[command palette](./command-palette).

## A logo or picture

![A logo in the dialog: the preview shows the pixels for the board](/screenshots/picture-logo.png)

1. **Choose a picture …** — PNG, JPEG, GIF, BMP, WebP or SVG. Black and white
   pictures work best; photos rarely look good in silkscreen.
2. The picture becomes black and white pixels. Dark parts are printed; the
   **threshold** sets what counts as dark (lower: only the darkest parts).
   **Invert** prints the light parts instead — for a white logo on a dark
   picture, or for a logo cut out of a copper area.
3. **Detail** sets the size of a pixel on the board: fine (0.025 mm), normal
   (0.05 mm) or coarse (0.1 mm). A picture is at most 1,000 pixels wide; a
   wide picture with fine detail uses larger pixels.
4. Choose the **layer** and the **width** on the board; the height follows
   from the picture.
5. **Add to the board**.

The preview on the right shows exactly the pixels that go onto the board,
with the size and the number of pixels below it. Transparent parts of a
picture count as white.

The picture is placed in the middle of the view and selected; move it with
the mouse or <kbd>M</kbd>. Its shapes form a [group](./selecting-and-editing#groups),
so one click selects all of them.

## A QR code

![A QR code with a light border, as printed on silkscreen](/screenshots/picture-qr.png)

1. Switch to **QR code** and enter the text: a web address, a serial number
   or any text.
2. Choose the **error correction**. More correction makes the code larger
   (more modules) but keeps it readable when part of it is worn or covered:
   low 7 %, medium 15 %, quartile 25 %, high 30 %.
3. **Print the light modules and a border** (on by default): silkscreen is
   white on a dark board, so printing the light modules and a border of two
   modules gives a dark code on a light ground, as scanners expect. Without
   it the dark modules are printed — right for copper under a light solder
   mask, or for a dark ink on a light board.
4. Choose the layer and the width of the whole code (border included).

### Size and readability

- A module should be at least 0.3 mm on silkscreen (about three times the
  smallest silkscreen line), so a short web address (version 2, 25 modules
  plus the border) needs about 9 mm. The size line under the preview shows
  the number of modules.
- Short texts make small codes: `https://example.com/b/42` is better than a
  long address with parameters.
- Check the code with a phone on the first boards — inks, mask colours and
  finishes differ between manufacturers.

## Layers

| Layer                    | Result                                                                                       |
| ------------------------ | -------------------------------------------------------------------------------------------- |
| Top / bottom silkscreen  | Printed in the silkscreen colour (usually white)                                             |
| Top / bottom copper      | Copper shapes without a net; under solder mask they show as a raised, slightly brighter area |
| Top / bottom solder mask | Openings in the mask: the copper or board below shows through                                |

On **bottom** layers the picture is mirrored, so that it reads right when
the board is turned over; the dialog says so.

Copper shapes have no net. They count as copper everywhere: the
[design rules check](./drc) reports a "copper shape" that comes too close to
copper of a net, [copper zones](./zones) keep their clearance around it
(a logo inside a ground plane stays an island of its own), and the router
routes around it.

## How a picture becomes shapes

The outline of every region of set pixels is followed along the pixel
edges; pixels that only touch at a corner count as separate. A board
polygon has no holes, so each hole (the inside of an "O", the light parts
of a QR finder pattern) is joined to the outline around it by a cut of zero
width — the way Gerber regions describe holes. The shapes keep the pixel
steps; at the pixel sizes used they are finer than silkscreen and etching
can show.

Each addition is one undo step (**Add picture**, **Add QR code**).

## From scripts and AI agents

`pcb.addImage` takes the picture as rows of `#` (set) and `.` (clear),
`pcb.addQrCode` a text; `pcb.qrCode` returns the modules of a code without
changing the board (see the [engine API](../reference/engine-api#logos-and-qr-codes)):

```json
{
  "method": "pcb.addQrCode",
  "params": {
    "doc": "<board id>",
    "layer": "silk.top",
    "text": "https://example.com/b/42",
    "size": 9000000,
    "center": [20000000, 15000000],
    "invert": true,
    "quiet": 2
  }
}
```

Lengths are nanometres (`9000000` is 9 mm). The method needs an open
project: run it in a script with
[`ewerk script --project`](../reference/cli#ewerk-script) or through the MCP
tool `call_engine`; `pcb.qrCode` works without a project
(`ewerk call pcb.qrCode '{"text": "42"}'`).
