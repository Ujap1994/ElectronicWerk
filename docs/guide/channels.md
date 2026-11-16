# Channels

Many designs repeat a block: four identical amplifiers, eight LED drivers,
a sensor input per channel. In the schematic you draw the block once on its
own sheet and use that sheet several times
([hierarchical sheets](./schematic-editor)); every use gets its own
designators (R1, R2, R3 …) and its own nets (`/Channel 1/OUT`,
`/Channel 2/OUT`, …). On the board each use is a **channel**: one group of
parts.

You place and route one channel by hand. **Channels** then copies its
layout to the others, so all channels look and behave alike.

![Before: channel 1 placed and routed, channels 2 and 3 only placed](/screenshots/channels-before.png)

## Copying the layout of a channel

1. Place and route one channel the way you want it.
2. Click **Channels** in the layout editor's tool bar (or **Board: copy the
   layout of a channel** in the [command palette](./command-palette)).
3. Choose the **Repeated sheet** (if the schematic has several), the use to
   **Copy from** and the uses to **Copy to** (all others by default). The
   **Parts** column shows the designators of every use.
4. Leave **Copy the tracks and vias too** on to copy the routing as well.
5. Click **Copy the layout**.

![The Channels dialog with the three uses of the RC filter sheet](/screenshots/channels-dialog.png)

![After: channels 2 and 3 have the layout of channel 1](/screenshots/channels-after.png)

What happens, for each target channel:

- **Placement** — every part goes where the same part of the source channel
  is, moved by the offset between the channels (the lower left corner of a
  channel's footprints stays where it was), with the same rotation, side and
  text positions. Parts correspond by their symbol on the shared sheet.
- **Routing** — the tracks and vias between the source channel's parts are
  copied with the nets of the target channel. The target channel's own
  unlocked tracks and vias of those nets inside its area are replaced. Nets
  shared by all channels (such as `GND`) stay what they are.

Copying is one undo step.

Parts are left as they are (and named in the message) when they have
another footprint than in the source channel or are locked. A source net
whose pads belong to two different nets in a target channel is not copied.

Move a whole channel first (select its parts and move them) to choose where
it goes; the copy keeps that place.

## Trying it out

The command-line tool makes a small example with a sheet used three times:

```console
ewerk synth channels my-channels
```

Open `my-channels/Channels.ewproj`, open the board and copy the layout of
channel 1.

## For scripts and AI agents

[`pcb.channels` and `pcb.copyChannel`](../reference/engine-api#pcb-channels-pcb-copychannel)
list the channels of a board and copy a layout.
