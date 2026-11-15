# Blind, buried and microvias

A normal via is drilled through the whole board and connects every copper
layer. Boards with four or more layers can also use vias that connect only
some layers:

| Via type    | Connects                                                       | Made by                                                                         |
| ----------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Through via | Every layer, from the top to the bottom                        | Drilling the finished board                                                     |
| Blind via   | An outer layer with one or more inner layers                   | Drilling part of the layers before they are pressed together, or depth-drilling |
| Buried via  | Inner layers only; it cannot be seen from outside              | Drilling inner layers before the outer layers are pressed on                    |
| Microvia    | Two neighbouring layers, with a very small hole (about 0.1 mm) | Laser drilling                                                                  |

They free space on the layers they do not reach: a blind via from the top to
the first inner layer leaves the bottom layer and the second inner layer
free under it. Dense boards with fine-pitch parts (BGAs, small QFNs) often
need them.

::: warning Ask your board maker first
Not every board maker makes blind, buried or microvias, and they cost extra
(more drilling and pressing steps). Most low-cost prototype services offer
through vias only. Check which layer pairs and sizes your board maker
supports before you use them.
:::

## Allowing them

Blind, buried and microvias are off until the design rules allow them:

1. Open **Design rules** in the layout editor's toolbar and go to **Board
   minimums**.
2. Switch on **Allow blind and buried vias**, **Allow microvias**, or both.
3. Check the **Microvia diameter** and **Microvia drill** minimums (0.2 mm
   and 0.1 mm by default).
4. **Save the rules**.

![The design rules with blind, buried and microvias allowed](/screenshots/vias-rules.png)

The net classes have a **Microvia diameter** and **Microvia drill** besides
the normal via sizes (0.3 mm and 0.1 mm by default): new microvias of a
class get these sizes, never less than the board minimums. See
[Design rules](./design-rules).

## Placing them while routing

The **Via type** in the layout editor's toolbar chooses the via that a layer
change places while [routing](./routing#vias-and-layer-changes):

1. Choose **Blind / buried via** or **Microvia** under **Via type**.
2. Start a route with **X** and click to fix its segments.
3. Change the layer with the layer selector, **Page Up** or **Page Down**:
   the via goes from the layer the route leaves to the layer it continues
   on. A blind via starts or ends on an outer layer, a buried via has both
   ends inside.
4. Go on routing on the new layer and finish as usual.

![A route from the top layer that continues on the second inner layer through a blind via](/screenshots/vias-routing.png)

| Via type              | What a layer change does                                                                                     |
| --------------------- | ------------------------------------------------------------------------------------------------------------ |
| Through via (default) | Places a via from the top to the bottom layer.                                                               |
| Blind / buried via    | Places a via between the two layers. A change between the top and the bottom layer places a through via.     |
| Microvia              | Places a microvia between neighbouring layers only; choose the next layer with **Page Up** or **Page Down**. |

A blind, buried or microvia only needs room on its own layers: copper on the
layers it does not reach is no obstacle. The status line tells you when the
design rules do not allow the chosen type, when a microvia would skip a
layer, or when no via fits at the end of the route.

## Changing a via

Select a via to see and change its layers in the properties panel:

| Field                    | Meaning                                                                                           |
| ------------------------ | ------------------------------------------------------------------------------------------------- |
| First layer, last layer  | The copper layers the via connects (every layer between them too).                                |
| Microvia (laser-drilled) | Marks a microvia: it is measured against the microvia minimums and must join neighbouring layers. |
| Type                     | Through, blind / buried or microvia, from the layers and the microvia mark.                       |

![A blind via in the properties panel](/screenshots/vias-properties.png)

## What the checks look for

The [design rules check](./drc) reports:

| Check                       | When                                                                                                                           |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `via-type`                  | A blind, buried or microvia while the design rules do not allow its type.                                                      |
| `via-layers`                | A via ends on a layer the board does not have, connects a layer with itself, or is a microvia whose layers are not neighbours. |
| `via-diameter`, `via-drill` | A via is smaller than the minimums: the microvia minimums for microvias, the via minimums for every other via.                 |

[Custom rules](./design-rules#custom-rules) can tell the via types apart
with `A.viaType` (`'through'`, `'blind'`, `'buried'` or `'micro'`), for
example `A.viaType == 'micro'` for a larger annular ring on microvias.

## Outputs and other programs

- **Drill files:** the [drill output](./gerber-drill#drill-files) writes one
  file per pair of copper layers (`<board>-L1-L2.drl`, …) besides the
  through-hole files, as board makers expect.
- **Gerber files:** the via pads appear only on the copper layers the via
  connects.
- **3D view:** blind and buried holes end inside the board, and their copper
  sleeves reach only their layers; the [STEP export](./export-3d) keeps the
  board body closed where a blind or buried via ends.
- **External autorouters:** the [DSN file](./autorouter) offers the
  autorouter every via the rules allow: blind and buried vias between any
  two layers, microvias between neighbouring layers (named
  `uVia[0-1]_300:100_um`). The session brings them back with their layers.
- **KiCad:** the [KiCad export](./export-kicad) writes blind and buried vias
  as `blind` vias and microvias as `micro` vias with their layers, and the
  rules switches as KiCad's _Allow blind/buried vias_ and _Allow
  micro vias_. KiCad 8 has one type for blind and buried vias; KiCad 10
  shows a via between inner layers as a blind via with the same layers.
  The [KiCad](./import-kicad) and [Eagle](./import-eagle) imports read the
  via layers of their files.

## From scripts

[`pcb.addTracks`](../reference/engine-api#pcb-routestart-pcb-route-pcb-addtracks)
takes `from`, `to` and `micro` for each via, and
[`pcb.viaFits`](../reference/engine-api#pcb-viafits) tells whether a via of
a given size and layers fits at a point.
