# Design rules

The design rules say how close copper may come to other copper, how wide
tracks must be, how large vias and holes are, and how far copper stays from
the board edge. The router, the zone filler and the
[design rules check](./drc) all use the same rules. They live in the
project's **Design rules** document, so they travel with the project and
every board of it uses them.

Open them with the **Design rules** button in the board toolbar or
**Board: design rules and net classes** in the command palette. The dialog
has four tabs; **Save the rules** stores all of them as one undo step
(`design-rules`).

There are three levels, from general to specific:

1. **Board minimums** — what the board maker can manufacture. Nothing goes
   below them.
2. **Net classes** — sizes for groups of nets (signals, power, …).
3. **Custom rules** — conditions in a small query language for everything
   else (high-voltage spacing, an RF area, thin tracks under a fine-pitch
   part).

## Net classes

![Net classes with their sizes](/screenshots/layout-rules-classes.png)

Every net belongs to exactly one net class. A class sets:

| Field                    | Used for                                                                                    |
| ------------------------ | ------------------------------------------------------------------------------------------- |
| Clearance                | Copper-to-copper distance to other nets. Between two classes, the larger clearance applies. |
| Track width              | The width of new tracks of the class.                                                       |
| Via diameter, via drill  | The size of new vias of the class.                                                          |
| Diff. pair width and gap | Reserved for differential pairs (routed as two tracks for now).                             |

The first class, `Default`, holds every net that no other class claims; it
cannot be renamed or removed. **Add a net class** copies the `Default`
values into a new class. The **Nets** column counts the nets of each class
on the board (hover over it to see their names).

### Assigning nets

**Assign nets by name** lists patterns: a net whose name matches a pattern
belongs to its class, and the first matching pattern wins. `*` stands for
any text and `?` for one character:

| Pattern  | Matches                 |
| -------- | ----------------------- |
| `GND`    | `GND` only              |
| `+*V*`   | `+5V`, `+3V3`, `+12V`   |
| `USB_D?` | `USB_DP`, `USB_DN`      |
| `*`      | every net (put it last) |

A net class set directly on a board net (by an import, for example) wins
over the patterns.

## Board minimums

![The board minimums](/screenshots/layout-rules-minimums.png)

| Minimum          | Default | Checked by the DRC as |
| ---------------- | ------- | --------------------- |
| Clearance        | 0.2 mm  | `clearance`           |
| Track width      | 0.2 mm  | `track-width`         |
| Via diameter     | 0.5 mm  | `via-diameter`        |
| Via drill        | 0.3 mm  | `via-drill`           |
| Annular ring     | 0.13 mm | `annular-ring`        |
| Hole             | 0.3 mm  | `hole-size`           |
| Hole to hole     | 0.25 mm | `hole-to-hole`        |
| Copper to edge   | 0.3 mm  | `copper-to-edge`      |
| Silkscreen width | 0.15 mm | `silk-width`          |

The defaults suit common low-cost prototype services. A
[board preset](./board-setup#presets) sets them for other kinds of boards.
Net classes and custom rules never go below these values (except a custom
copper-to-edge distance, which may be smaller for edge connectors).

## Custom rules

![Two custom rules, one with a syntax error](/screenshots/layout-rules-custom.png)

A custom rule has a **name**, a **condition** and the **constraints** it
sets. Every constraint is optional: tick the ones the rule sets and enter
their values. Rules apply in order, and **later rules win** over earlier
ones for the same constraint. Switch a rule off with **On** to try a design
without it.

| Constraint               | Applies to                              |
| ------------------------ | --------------------------------------- |
| Clearance                | Pairs of copper items of different nets |
| Min. / max. track width  | Tracks                                  |
| Track width for routing  | New tracks drawn with the router        |
| Min. via diameter, drill | Vias                                    |
| Min. annular ring        | Vias and plated pads                    |
| Min. hole                | Pad holes                               |
| Min. hole to hole        | Pairs of holes                          |
| Copper to edge           | Copper items near the board edge        |

The condition is checked as you type: a mistake shows the position and what
is wrong, and the rule is skipped until it is fixed (the DRC also reports it
as `rule-error`).

### The condition language

A condition looks at the properties of **A**, the item being checked, and
for rules between two items (clearance, hole to hole) also of **B**, the
other item. A pairwise rule matches in both directions, so
`A.netclass == 'HV' && B.netclass != 'HV'` covers both orders.

| Property     | Value                                                                    |
| ------------ | ------------------------------------------------------------------------ |
| `type`       | `'pad'`, `'track'`, `'via'` or `'zone'`                                  |
| `net`        | Net name (`''` without a net)                                            |
| `netclass`   | Net class name                                                           |
| `layer`      | The copper layer being checked: `'cu.top'`, `'cu.in1'`, `'cu.bottom'`, … |
| `designator` | Designator of the footprint (pads)                                       |
| `footprint`  | Footprint name (pads)                                                    |
| `pad`        | Pad number                                                               |
| `side`       | `'top'` or `'bottom'` (pads)                                             |
| `width`      | Track width in mm                                                        |
| `diameter`   | Via diameter or pad size in mm                                           |
| `drill`      | Drill in mm                                                              |

| Syntax                             | Meaning                                                      |
| ---------------------------------- | ------------------------------------------------------------ |
| `==`, `!=`                         | Equal, not equal. Text compares with `*` and `?` wildcards.  |
| `<`, `<=`, `>`, `>=`               | Comparisons of numbers                                       |
| `&&`, `\|\|`, `!`, `( )`           | And, or, not, grouping                                       |
| `'text'` or `"text"`               | Text values                                                  |
| `0.2mm`, `8mil`, `200um`, `0.01in` | Numbers with units (mm without a unit)                       |
| `A.insideZone('RF')`               | True for items inside the zone (or keep-out area) named `RF` |
| `true`, `false`                    | Constants; an empty condition matches everything             |

Examples:

| Condition                                  | Use                                             |
| ------------------------------------------ | ----------------------------------------------- |
| `A.netclass == 'HV' && B.netclass != 'HV'` | Wider spacing between mains and low voltage     |
| `A.net == 'USB_D?'`                        | Fixed track width for the USB pair              |
| `A.type == 'via' && A.layer == 'cu.top'`   | Via sizes (vias are checked on their top layer) |
| `A.designator == 'U3' && A.type == 'pad'`  | Smaller clearance at a fine-pitch part          |
| `A.insideZone('Antenna')`                  | Different rules in a named area                 |
| `A.width < 0.3mm`                          | Narrow tracks only                              |

## Checks

![Severities of the checks](/screenshots/layout-rules-checks.png)

The **Checks** tab sets how serious each [DRC check](./drc#what-is-checked)
is: **Error**, **Warning** or **Ignore**. Only changes from the defaults are
stored.
