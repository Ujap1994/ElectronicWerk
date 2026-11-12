# Footprint wizard

The footprint wizard computes a complete footprint from the package
dimensions in a datasheet: pads following the IPC-7351 land pattern
formulas, a fabrication outline with a pin 1 mark, a silkscreen that keeps
clear of the pads, a courtyard, and designator and value texts. Open the
[library editor](./library-editor), choose **Footprints** and click
**Wizard**.

![The wizard with the QFN-32 preset: 32 terminals and an exposed pad](/screenshots/libedit-wizard-qfn.png)

## Using the wizard

1. **Preset**: start from a common package (see the list below). The values
   of the preset fill the form.
2. **Package family**: switching the family loads its first preset.
3. Adjust the **dimensions** to your datasheet. Every change updates the
   **preview** at once: pads in red (through-hole pads in gold), the
   silkscreen in yellow, the fabrication outline in grey and the courtyard in
   magenta, with the pad numbers.
4. Choose the **density level** (see below).
5. Check the **name**. The wizard suggests one in KiCad's naming scheme, such
   as `SOIC-8_3.9x4.9mm_P1.27mm` or `QFN-32-1EP_5x5mm_P0.5mm_EP3.45x3.45mm`;
   type your own name to keep it. A name that is already taken is marked.
6. **Create footprint** adds it to the library and opens it in the
   [footprint editor](./footprint-editor) for any finishing touches (a 3D
   model, a special silkscreen).

Below the preview, the wizard shows the number of pads, the size of the
courtyard and the [library check](./library-checks) findings of the result.
A new footprint has no 3D model yet, so that note is expected. Values that
cannot make a footprint (terminals longer than the body, pads that would
overlap, a pin count that does not fit the sides) are explained in a warning
instead of a preview.

![The SOIC-8 preset at the "most" density level, renamed for hand soldering](/screenshots/libedit-wizard.png)

## Package families

| Family              | Inputs                                                                                              | Presets                                                                         |
| ------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| **Chip**            | Body length and width, terminal length                                                              | 0402, 0603, 0805, 1206, 1210, 2010, 2512                                        |
| **Gull-wing leads** | Pins, leads on four sides (QFP), pitch, lead span tip to tip, lead foot length and width, body size | SOIC-8, SOIC-14, SOIC-16, TSSOP-14, TSSOP-20, SSOP-20, LQFP-32 7x7, LQFP-48 7x7 |
| **No leads**        | Pins, four sides (QFN) or two (DFN), pitch, body size, terminal length and width, exposed pad       | DFN-8 3x3, QFN-16 3x3, QFN-20 4x4, QFN-32 5x5, QFN-48 7x7                       |
| **Ball grid array** | Rows, columns, pitch, ball diameter, body size                                                      | BGA-64 8x8 P0.8, BGA-100 10x10 P1.0, BGA-256 16x16 P1.0                         |
| **Pin header**      | Rows (one or two), pins per row, pitch, drill, pad diameter                                         | 1×2 … 1×10 and 2×5 … 2×20, 2.54 mm                                              |

Where the datasheet gives a range (minimum, nominal, maximum), use the
nominal value.

**Pin numbering** follows the usual conventions:

- Chips: pad 1 left, pad 2 right.
- Gull-wing and no-lead packages: pin 1 at the top of the left side, counting
  counterclockwise (down the left side, along the bottom, up the right side,
  along the top). On gull-wing packages pin 1 has a rectangular pad, the
  others rounded ones; no-lead terminals are all rounded. The exposed pad
  gets the next number (`33` on a QFN-32) and a smaller paste opening (about
  65 % of its area) against voids under the part.
- BGAs: rows `A, B, C …` from the top, columns `1, 2, 3 …` from the left. The
  letters I, O, Q, S, X and Z are skipped as in JEDEC packages; after `Y`
  come `AA`, `AB` …. Ball A1 has a rectangular land, the others round ones.
- Headers: pin 1 at the origin with a square pad; on two-row headers the
  numbers go across the rows (`1 2 / 3 4 / …`), like KiCad's headers.

## How the pads are computed

The wizard uses the IPC-7351 land pattern equations without the tolerance
terms. For a pair of opposite terminals:

- outer extent of the lands: `Z = L + 2 · toe`,
- gap between the lands: `G = L − 2 · T − 2 · heel`,
- land width: `X = W + 2 · side`,

where `L` is the lead span (or body length of a chip), `T` the terminal or
foot length and `W` the terminal width. The pad length is `(Z − G) / 2` and
the pad centre lies at `(Z + G) / 4` from the middle. Results are rounded to
1 µm; courtyards are rounded outwards to 0.01 mm.

The **toe**, **heel** and **side** fillets depend on the density level:

| Family (IPC-7351B table)      | Fillet                 | Most | Nominal | Least | Courtyard excess (M / N / L) |
| ----------------------------- | ---------------------- | ---- | ------- | ----- | ---------------------------- |
| Chips 0603 and larger (3-5)   | toe                    | 0.55 | 0.35    | 0.15  | 0.5 / 0.25 / 0.1 mm          |
|                               | side                   | 0.05 | 0       | −0.05 |                              |
| Chips smaller than 0603 (3-6) | toe                    | 0.25 | 0.15    | 0.05  | 0.2 / 0.15 / 0.1 mm          |
|                               | side                   | 0.05 | 0       | −0.05 |                              |
| Gull-wing leads (3-2)         | toe                    | 0.55 | 0.35    | 0.15  | 0.5 / 0.25 / 0.1 mm          |
|                               | heel                   | 0.45 | 0.35    | 0.25  |                              |
|                               | side, pitch > 0.625 mm | 0.05 | 0.03    | 0.01  |                              |
|                               | side, finer pitch      | 0.01 | −0.02   | −0.04 |                              |
| No leads (QFN, DFN)           | toe                    | 0.4  | 0.3     | 0.2   | 0.5 / 0.25 / 0.1 mm          |
|                               | heel                   | 0.1  | 0       | −0.05 |                              |
|                               | side                   | 0.05 | 0       | −0.05 |                              |
| BGA                           | land / ball            | 85 % | 80 %    | 75 %  | 2 / 1 / 0.5 mm               |

All values in millimetres. Choose the level by how the board is made:

- **Most**: the largest lands and the widest courtyard. Good for hand
  soldering, prototypes and boards that see vibration.
- **Nominal**: the usual choice for machine assembly.
- **Least**: the smallest lands, for dense boards (phones, small modules).

Pin headers have no density level: their pads are the diameter you enter.

## What else the wizard draws

- **Fabrication outline**: the body outline on the top fabrication layer,
  with the pin 1 corner bevelled.
- **Silkscreen**: the visible parts of the body outline, kept at least
  0.1 mm (plus half the line width) away from every pad, and a pin 1 mark.
- **Courtyard**: a rectangle around pads and body, the courtyard excess of
  the density level away.
- **Texts**: the designator (`REF**`) on the silkscreen above the courtyard
  and the value (the footprint name) on the fabrication layer below it.
- **Attributes**: surface mount for chips, gull-wing, no-lead and BGA
  packages; through-hole for headers.
