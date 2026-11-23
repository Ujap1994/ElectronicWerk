# Creepage for high voltages

Boards that carry mains or other high voltages need two kinds of spacing
between the high-voltage copper and everything else:

- **Clearance** — the shortest distance **through the air**. ElectronicWerk
  checks it with every [clearance rule](./design-rules).
- **Creepage** — the shortest distance **along the surface** of the board.
  Dirt and moisture on the surface can conduct, so a path along the board
  must be long enough too. A path along the surface cannot jump over a
  milled slot, a cut-out, a non-plated hole or the board edge: it has to go
  around them. That is why a slot between mains and low voltage is a common
  way to gain creepage on a small board.

ElectronicWerk measures the real path along the surface, around every slot
and hole, and reports copper that is closer than the creepage distance you
ask for.

## Where the distances come from

The required distances depend on the working voltage, the pollution degree,
the material group of the board (its CTI) and the kind of insulation
(functional, basic or reinforced). They are listed in the safety standard
that applies to your product — for example **IEC 60664-1** (tables F.2 for
clearance and F.4 for creepage), **IEC 62368-1** for audio, video and IT
equipment, **IEC 60335-1** for household appliances or **UL 840**.

ElectronicWerk does not ship these tables: they belong to their publishers,
and the right line depends on decisions only you can make for your product.
Look the values up in your copy of the standard and enter them as rules.
Note where each value comes from in the rule's name, for example
`Mains–SELV reinforced (60664-1 F.4, PD2, IIIa)`.

## Setting up the rule

![A custom rule that asks for 8 mm creepage between VCC and GND](/screenshots/creepage-rule.png)

1. Put the high-voltage nets into a [net class](./design-rules#net-classes),
   for example **Mains**.
2. Open the **Design rules**, tab **Custom rules**, and add a rule.
3. Enter a condition that picks the pairs, for example
   `A.netclass == 'Mains' && B.netclass != 'Mains'` (a pairwise rule matches
   in both directions).
4. Tick **Min. creepage (outer layers, around slots)** and enter the
   distance. Tick **Clearance** too and
   enter the clearance from the standard, if it is larger than the class's
   clearance.

Other useful conditions:

| Condition                                        | Use                                                         |
| ------------------------------------------------ | ----------------------------------------------------------- |
| `A.netclass == 'Mains' && B.netclass != 'Mains'` | Mains to everything else                                    |
| `A.net == 'L' && B.net == 'N'`                   | Between the two mains conductors (functional insulation)    |
| `A.inRoom('Primary') && B.inRoom('Secondary')`   | Between the two sides of a transformer, by [rooms](./rooms) |

## What the check measures

![Creepage findings in the design rules check](/screenshots/creepage-findings.png)

For every pair of copper items that a creepage rule covers, on the top and
the bottom layer (the surfaces of the board; inner layers are inside the
insulating material and need only their clearance):

1. The straight distance between the two items. If it is already as long as
   the creepage distance, the pair is fine.
2. Otherwise the shortest path along the surface: from the nearest points of
   the two items, around the corners of every slot, cut-out and non-plated
   hole in the way, and inside the board outline. A milled slot is a cut-out
   drawn on the **outline** layer inside the board, or a non-plated slot of
   a pad.
3. If that path is shorter than the creepage distance, the check reports
   `creepage` with the length of the path, the two items and the rule.

The path is found on a visibility graph of the obstacle corners near the two
items, so it is the true shortest path around straight-edged cut-outs. Round
holes are followed as polygons within 5 µm of the circle, so the path is at
most a few micrometres longer than the exact one — on the safe side.

The check runs with the [online DRC](./drc#online-check) while you move
parts and draw tracks, so a slot can be shaped until the findings disappear.

## Tips

- A slot works only if it really separates the two sides: the path must not
  find a way around its ends. Make it long enough, or run it to the board
  edge.
- Coat or pot the board only if the standard allows it to reduce the
  distances; the check knows nothing about coatings.
- Keep the tracks of the mains side on the outer layers short and wide; the
  creepage of inner layers is not checked (they need their clearance).
