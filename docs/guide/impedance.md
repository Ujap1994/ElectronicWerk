# Controlled impedance

Fast signals do not simply flow through a track: they travel along it as a
wave, together with the return current in the plane below. The track and
its plane form a transmission line with a **characteristic impedance** (in
ohms, Ω). It depends on the track's width, its distance to the plane, the
material between them (its relative permittivity εr) and the copper
thickness; for a differential pair also on the gap between the two tracks.

When the impedance of a track differs from what the interface expects, part
of the signal is reflected at its ends. The result is ringing, data errors
and more radiated noise. Interfaces such as USB, Ethernet or HDMI therefore
specify the impedance of their lines, and RF circuits and antennas are
built for 50 Ω.

ElectronicWerk computes these impedances from the board's
[stack-up](./board-setup#stack-up) and sizes the tracks for you:

- **Impedance profiles** in the [design rules](./design-rules) hold a target
  impedance and a tolerance, for a single track or a differential pair.
- A **net class** follows a profile. The router takes the profile's width
  (and the gap of a pair) **on the layer it routes on**, so a route that
  changes from an outer to an inner layer changes its width too.
- The [design rules check](./drc) warns about tracks whose width misses the
  target impedance.

::: tip Do I need this?
Slow signals (LEDs, buttons, I²C, SPI and UART at usual speeds, power) do not
need a controlled impedance. You need it for high-speed interfaces (USB,
Ethernet, HDMI, PCI Express, LVDS and MIPI displays and cameras), for long
CAN or RS-485 lines on the board, and for everything that carries RF (antennas,
radio modules, SMA connectors).
:::

## Two working modes

How much ElectronicWerk decides for you is set by the **Working mode** in the
[settings](./settings#working-mode):

| Working mode           | What you do                                                                                                  | What ElectronicWerk does                                                                                 |
| ---------------------- | ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| **Beginner** (default) | Pick the interface a net class carries, for example **USB 2.0**.                                             | Makes the impedance profile, computes width and gap on every copper layer and applies them to the class. |
| **Advanced**           | Make profiles with your own target and tolerance, set widths and gaps per layer by hand, use the calculator. | Shows the impedance your values give and whether they meet the target.                                   |

![The working mode in the settings](/screenshots/impedance-work-mode.png)

You can switch at any time; both modes edit the same profiles. A profile you
changed in the advanced mode shows up in the beginner mode as
**(set in advanced mode)** and is left alone there.

## Beginner mode

1. Open **Design rules** in the layout editor's toolbar.
2. On the **Net classes** tab, add a class for the lines (for example `USB`)
   and assign its nets by name (for example the pattern `USB_D?` for
   `USB_DP` and `USB_DN`). See [assigning nets](./design-rules#assigning-nets).
3. Go to the **Impedance** tab and choose the **Interface** of the class.
4. **Save the rules**.

![The impedance tab in the beginner mode: USB 2.0 and a 50 Ω RF class](/screenshots/impedance-beginner.png)

Below the classes, a table for every interface in use shows its sizes on
every copper layer: the **Structure** of the layer (see
[how it is computed](#how-it-is-computed)), the **Width** and, for a pair,
the **Gap**, the **Impedance** these sizes give and a status. The sizes of
the top layer are also written into the class (**Track width**, **Diff. pair
width** and **Diff. pair gap** on the **Net classes** tab), so exports that
know only one width per class get them too, and for a pair interface the
usual largest **skew** ([length matching](./lengths)).

The interfaces:

| Interface                  | Kind              | Target | Tolerance |
| -------------------------- | ----------------- | ------ | --------- |
| USB 2.0                    | Differential pair | 90 Ω   | ± 10 %    |
| USB 3 and USB-C high speed | Differential pair | 90 Ω   | ± 7 %     |
| Ethernet                   | Differential pair | 100 Ω  | ± 10 %    |
| HDMI                       | Differential pair | 100 Ω  | ± 10 %    |
| LVDS and MIPI              | Differential pair | 100 Ω  | ± 10 %    |
| PCI Express                | Differential pair | 85 Ω   | ± 10 %    |
| CAN and RS-485             | Differential pair | 120 Ω  | ± 10 %    |
| RF and antennas (50 Ω)     | Single track      | 50 Ω   | ± 10 %    |

Choosing **None** removes the interface from the class; a profile that no
class uses any more is removed with it.

The values follow the stack-up: change a dielectric's thickness or material
in **Board setup → Stack-up** and they are computed again the next time the
router or the DRC needs them.

## Advanced mode

In the advanced mode the **Impedance** tab shows everything:

![The impedance tab in the advanced mode, with the RF width set by hand on the top layer](/screenshots/impedance-advanced.png)

**Impedance profiles** — one row per profile:

| Column      | Meaning                                                                                       |
| ----------- | --------------------------------------------------------------------------------------------- |
| Name        | The name net classes refer to. Renaming a profile renames it in its classes too.              |
| Kind        | **Single track** or **differential pair**. Changing it drops the sizes set by hand.           |
| Target      | The impedance to reach, in Ω.                                                                 |
| Tolerance   | How far the impedance may deviate, in % of the target.                                        |
| Net classes | The classes that follow the profile. **×** removes the profile; its classes follow none then. |

**Add a profile** adds one with 50 Ω ± 10 %.

**Net classes** — the **Profile** each class follows.

**The sizes of each profile** — one row per copper layer. The **Width** and
**Gap** fields show the computed values; type a value to set it by hand for
that layer. The impedance is computed again at once, and the status tells
how it compares:

| Status                          | Meaning                                                                                                                                        |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| **Computed**                    | Computed from the stack-up; meets the target.                                                                                                  |
| **Set by hand**                 | Your value; meets the target. **Compute** goes back to the computed value.                                                                     |
| **Outside the tolerance**       | Your value misses the target by more than the tolerance. The DRC will warn about such tracks.                                                  |
| **Below the board minimums**    | The width (or the gap) is smaller than the [board minimums](./design-rules#board-minimums) allow; ask your board maker or change the stack-up. |
| **No width reaches the target** | No width between 0.01 mm and 20 mm gives the target on this layer, for example 120 Ω over a thin prepreg.                                      |

A layer without a copper layer next to it (a single-layer board) shows
**No plane next to this layer**; no impedance can be computed there.

### Calculator

The **Calculator** at the bottom gives the impedance of any width (and gap)
on any layer of the board, without changing anything:

![The calculator: a 0.3 mm track on the first inner layer](/screenshots/impedance-calculator.png)

## Routing

A net whose class follows a profile is routed with the profile's sizes:

- A route starts with the width of the layer it starts on.
- When the route changes the layer (**Page Up**, **Page Down** or the layer
  selector, see [vias and layer changes](./routing#vias-and-layer-changes)),
  the following segments get the width of the new layer: on the
  [4-layer standard stack-up](./board-setup#presets) a USB 2.0 track is
  about 0.3 mm wide on the outer layers and 0.2 mm on the inner ones.
- **Route differential pair** (key **6**) routes both nets of a pair
  together with the profile's width and gap, see
  [differential pairs](./routing#differential-pairs).

## Design rules check

The `pair-gap` check (a **Warning** by default) measures the gap between
the coupled tracks of every [differential pair](./routing#differential-pairs)
whose class follows a differential profile: where the two tracks run side
by side, their gap must be the profile's gap on that layer within 10 % (at
least 10 µm). Where the tracks part to reach their pads they are not
checked.

The `impedance` check (a **Warning** by default) computes the impedance of
every track whose net class follows a profile, on its own layer, with the
gap of the profile. A track outside the tolerance is reported with its
width, its impedance and the width that would fit:

> Track width 0.8 mm gives 71 Ω, outside 90 Ω ± 10 % (USB 2.0); 0.504 mm
> would fit

Set the check to **Error** on the **Checks** tab of the design rules if
the board must not be ordered with such tracks.

## How it is computed

Every copper layer is one of two structures:

| Structure      | Where                                                     | Formula                                                                                                  |
| -------------- | --------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| **Microstrip** | An outer layer, over the plane of the next copper layer   | Hammerstad and Jensen, with their correction for the copper thickness                                    |
| **Stripline**  | An inner layer, between the copper layers above and below | Wheeler, with the copper thickness; a line closer to one plane as two centred lines in parallel (Wadell) |

Differential pairs use the IPC-2141 coupling factors. The distance to the
plane, εr and the copper thickness come from the stack-up; for a stripline
εr is the thickness-weighted mean of the dielectrics above and below.
Every neighbouring copper layer counts as a plane, so keep a solid ground
or supply zone under impedance-controlled tracks.

Widths and gaps are rounded to whole micrometres. For a pair the gap starts
at the class's clearance (at least the board minimum) and the width is
solved for the target; if that width would be narrower than the board's
minimum track width, the width stays at the minimum and the gap grows
instead.

These closed formulas are what common board calculators use and are
accurate to a few per cent. Solder mask and the trapezoid shape of etched
tracks are not taken into account. For tight tolerances (below about
± 10 %), order the board as **impedance controlled**: the board maker then
checks the stack-up with a field solver and adjusts the widths slightly.

## In the files

The profiles are stored in the project's design rules (`.ewrules`), see the
[file format](../reference/file-format):

```json
{
  "impedanceProfiles": [
    {
      "name": "USB 2.0",
      "differential": true,
      "target": 90,
      "tolerance": 10,
      "preset": "usb2"
    },
    {
      "name": "Antenna",
      "target": 50,
      "tolerance": 5,
      "layers": { "cu.top": { "width": 350000, "gap": 0 } }
    }
  ],
  "netClasses": [{ "name": "USB", "impedance": "USB 2.0", "…": "…" }]
}
```

Scripts and AI agents read the computed values with the engine methods
[`pcb.impedance` and `pcb.impedanceCalc`](../reference/engine-api#pcb-impedance-pcb-impedancecalc).
