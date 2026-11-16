# Length matching

Some signals must arrive at the same time. The two nets of a
[differential pair](./routing#differential-pairs) carry one signal with
opposite polarity: when one is longer than the other (the **skew**), the
pair turns into a little antenna and the receiver sees a distorted edge.
The data lines of a parallel bus (memory, cameras, displays) are latched by
one clock: when one line is much longer, its bit arrives too late.

ElectronicWerk measures the routed lengths, checks them against two kinds
of rules and makes nets that are too short longer with **meanders** (a
serpentine track with 45° corners).

| Rule             | Where it is set                          | What it checks                                                                    |
| ---------------- | ---------------------------------------- | --------------------------------------------------------------------------------- |
| **Max. skew**    | Per net class, in the design rules       | The length difference of the two nets of every differential pair in the class.    |
| **Length group** | In the design rules, nets chosen by name | Every net of the group is at most the **tolerance** shorter than its longest net. |

The length of a net is the length of its tracks (arcs along the arc). Vias
are not counted: on a 1.6 mm board a via adds about 1.6 mm to the path of
the signal, so route the nets of a pair or group through the same number
of vias.

## Setting the rules

Open **Design rules** and go to the **Lengths** tab.

![The Lengths tab: the largest skew per net class and a length group](/screenshots/lengths-rules.png)

**Skew of differential pairs** — the largest length difference of the two
nets of a pair, per net class. `0` means not checked. In the
[beginner working mode](./impedance#beginner-mode), choosing the interface
of a class on the **Impedance** tab also sets its usual skew:

| Interface                  | Max. skew        |
| -------------------------- | ---------------- |
| USB 2.0                    | 1.25 mm (50 mil) |
| USB 3 and USB-C high speed | 0.125 mm (5 mil) |
| Ethernet                   | 1.25 mm (50 mil) |
| HDMI                       | 0.125 mm (5 mil) |
| LVDS and MIPI              | 0.125 mm (5 mil) |
| PCI Express                | 0.125 mm (5 mil) |
| CAN and RS-485             | not checked      |

These are the values common design guides give; the data sheet of your
chip may ask for less.

**Length groups** — **Add a length group** adds a row with a **Name**, the
**Nets** (name patterns with `*` for any text and `?` for one character,
separated by spaces or commas, for example `DQ0 DQ1 DQ2 DQ3` or `DQ?`) and
a **Tolerance**: how much shorter than the longest net of the group a net
may be. **×** removes the group.

**Save the rules** stores them with the other design rules.

## The lengths of a board

**Lengths** in the layout editor's tool bar (or **Board: lengths and length
matching** in the [command palette](./command-palette)) shows the lengths of
the board:

![The Lengths dialog: the USB pair has too much skew](/screenshots/lengths-report.png)

- **Differential pairs** — every pair with the length of both nets, their
  **Skew**, the **Allowed** skew of the class and a status: **OK**, **Too
  much skew**, or **not checked** when the class has no limit.
- **Length groups** — every group with the length of each net, how much
  **Shorter** it is than the longest one and a status: **OK** or **Too
  short**.

## Matching lengths

**Match lengths** makes every net that breaks a rule as long as its partner
(pairs) or as the longest net of its group, as one undo step. It adds
meanders to the net's longest straight tracks:

![After Match lengths: the skew is gone](/screenshots/lengths-matched.png)

![The meanders on the shorter net of the pair, between R2 and C1](/screenshots/lengths-meanders.png)

How the meanders are made:

- Each bump has two parallel legs and 45° corners only (no right angles).
  The legs are three track widths apart (at least the track width plus
  twice the clearance), and the bumps are at most three leg spacings high.
- The height of the bumps is chosen so the net gets exactly the length it
  needs; fewer, higher bumps come first.
- The bumps go to the side of the track with room. They keep the clearance
  to everything else, also to the meanders of other nets made in the same
  step. A track of a differential pair gets its bumps on the side away from
  its partner.
- Locked and curved tracks get no meanders. When one track has no room for
  all, the next longest straight track of the net takes the rest.

When a net has no room for the length it needs, a message names it. Make
room next to one of its straight tracks, or route it a little longer by
hand, and match again.

### Advanced: the shape and single tracks

In the [advanced working mode](./settings#working-mode) the dialog has two
more sections:

- **Meanders** — **Set the meander shape** lets you choose the **Spacing of
  the legs** (centre to centre) and the **Highest bump** for **Match
  lengths** and for single tracks.
- **Tune the selected track** — select one straight track before you open
  the dialog: it shows the net and its length. Enter the **Length of the
  net** you want (it starts at the length of the partner or the longest net
  of the group) and click **Add meanders**. Only this track gets meanders.

## Design rules check

Two checks of the [DRC](./drc) report broken length rules, both
**Warnings** by default:

| Code           | Reported when                                                                    |
| -------------- | -------------------------------------------------------------------------------- |
| `pair-skew`    | The nets of a differential pair differ by more than the max. skew of the class.  |
| `length-match` | A net of a length group is more than the tolerance shorter than its longest net. |

The finding points at the longest track of the net that is too short:
that is where meanders fit best.

## For scripts and AI agents

The engine methods [`pcb.lengths`, `pcb.tuneLength` and
`pcb.matchLengths`](../reference/engine-api#pcb-lengths-pcb-tunelength-pcb-matchlengths)
report the lengths, add meanders to one track and match all nets.
