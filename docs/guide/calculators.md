# Calculators

Six calculators answer the questions that come up while drawing a
schematic and laying out a board:

| Calculator          | Answers                                                           |
| ------------------- | ----------------------------------------------------------------- |
| Voltage divider     | Which two standard resistors give 3.3 V from 12 V?                |
| Regulator resistors | Which feedback resistors set an adjustable regulator to 5 V?      |
| E series            | Which standard value is next to 4.6 kΩ, in every series?          |
| Track current       | How wide must a track be for 2 A — or how much does 0.5 mm carry? |
| Via current         | How much current does a via carry?                                |
| Fusing current      | Which current melts a track or a via in a second?                 |

Open them with **View → Calculators …**, the calculator button in the
project bar, or **Calculators** in the [command palette](./command-palette).
They need no project. The calculator you used last opens again.

Every calculator computes while you type. Numbers may use a comma or a
point, and prefixes as engineers write them: `4.7k`, `4k7`, `10 k`, `2M2`,
`4R7` (4.7 Ω), `50u` or `50µ` (50 µA), `1.2e-3`. A field with an invalid
value gets a red border and the result explains what is missing. Lengths
use the units of the open project (millimetres without one).

The engine does the maths, so the [command line](../reference/cli) and AI
agents get the same results (see [below](#from-scripts-and-ai-agents)).

## Voltage divider

![Voltage divider: 12 V to 3.3 V from E24](/screenshots/calculator-divider.png)

Enter the input and the output voltage, the [E series](#e-series) to take the
resistors from, and the range of the total resistance R1 + R2. R1 goes from
the input to the output, R2 from the output to ground.

The table lists the best pairs first, at most eight, by the error of their
output; among pairs with the same ratio, the one whose total is nearest to
the middle of the range wins (the geometric middle: 31.6 kΩ for 1 kΩ to
1 MΩ). Each pair shows the output it gives, its error in percent and the
current through the divider. The best pair is highlighted.

The range sets the current: a small total wastes current, a large one makes
the output sensitive to the load and to leakage (keep the load current well
below the divider current, or buffer the output).

## Regulator resistors

![Feedback resistors of an LM317 for 5 V](/screenshots/calculator-regulator.png)

An adjustable regulator keeps its feedback pin at a reference voltage; two
resistors set the output:

> Vout = Vref · (1 + R1/R2) + Iadj · R1

Enter the reference voltage and the current out of the adjust pin from the
datasheet:

| Regulator                  | Vref           | Iadj  |
| -------------------------- | -------------- | ----- |
| LM317, LM1117 (adjustable) | 1.25 V         | 50 µA |
| Many switching regulators  | 0.6 V or 0.8 V | 0     |

and the range of R2 (the resistor from the feedback pin to ground). The
table lists R1 and R2, the output they give, its error and the current
through R2. LM317 needs a load of a few milliamperes to regulate — R2 = 240 Ω
gives about 5 mA.

## E series

![The neighbours of 4.6 kΩ in every series](/screenshots/calculator-series.png)

Resistors and capacitors come in preferred values (IEC 60063), a series per
tolerance:

| Series | Values per decade | Tolerance |
| ------ | ----------------- | --------- |
| E3     | 3                 | > 20 %    |
| E6     | 6                 | 20 %      |
| E12    | 12                | 10 %      |
| E24    | 24                | 5 %       |
| E48    | 48                | 2 %       |
| E96    | 96                | 1 %       |
| E192   | 192               | 0.5 %     |

Enter a value: each series shows its nearest value, the values just below
and above, and the error of the nearest one. E3 to E24 use the historical
values (2.7, 3.3, 4.7 …), E48 to E192 the rounded powers of ten with the one
exception of E192 (9.20 instead of 9.19).

## Track current

![A 2 A track in 1 oz copper at 10 °C rise](/screenshots/calculator-track.png)

A track heats up with the current it carries. Choose:

- **Width for a current** (enter the current) or **Current of a width**
  (enter the width);
- the copper: 0.5 oz (17.5 µm), 1 oz (35 µm, the usual outer layer), 2 oz
  (70 µm) or 3 oz (105 µm);
- the track length, for its resistance, voltage drop and power loss;
- the allowed **temperature rise** above the ambient temperature (10 °C is a
  common choice; 20 °C to 30 °C for short high-current tracks);
- the **standard**.

### IPC-2152 and IPC-2221

**IPC-2152** (2009) replaced the old charts with measurements. ElectronicWerk
uses a published least-squares fit of its chart 5-2:

> A = (117.555 · ΔT^−0.913 + 1.15) · I^(0.84 · ΔT^−0.108 + 1.159)

with A in square mils, I in amperes and ΔT in °C. At 10 A and 20 °C it gives
513 mil², the chart 500 mil². The chart is for 3 oz copper on a polyimide
board 1.78 mm thick, in still air, with no copper planes and the traces far
apart; the standard's modifiers for other copper, boards and planes are not
applied. Copper planes and nearby copper carry heat away, so real tracks
usually run cooler. The fit covers 2 °C to 100 °C rise and up to 30 A;
outside, a note says that the value is extrapolated.

**IPC-2221** (and the older IPC-D-275) uses the formula

> I = k · ΔT^0.44 · A^0.725, with k = 0.048 on outer and 0.024 on inner layers

It is still in many calculators and shown for comparison; it is known to be
far too pessimistic for inner layers. With IPC-2221 the calculator asks for
the layer (outer or inner).

The resistance uses copper's resistivity 1.724 × 10⁻⁸ Ω·m at 20 °C and its
temperature coefficient 0.393 %/K, at the ambient temperature plus the rise.

## Via current

![The current of a 0.3 mm via](/screenshots/calculator-via.png)

The current of a via is carried by its plated barrel: a ring around the
finished hole, as thick as the plating (25 µm is usual, 20 µm the minimum of
IPC class 2). The calculator takes the ring's cross-section,

> A = π · (d + t) · t

with the finished hole d and the plating t, and applies the chosen standard
as for an outer-layer track. The board thickness is the length of the barrel
for its resistance. Several vias in parallel share the current — for a
power connection, use several vias rather than one large one.

## Fusing current

![The fusing current of a 0.25 mm track](/screenshots/calculator-fusing.png)

The current that melts a track (or a via barrel) within a time, from
Onderdonk's equation:

> I = A · √(log₁₀((1083 °C − Ta) / (234 + Ta) + 1) / (33 · t))

with A in circular mils, the ambient temperature Ta and the time t in
seconds. It assumes that no heat leaves the copper, which holds for short
pulses; for more than about 10 s the copper gives off heat and carries more
(a note says so). Use it to judge what a short circuit does to a track, not
to design a fuse: a track as a fuse needs a large margin, and tracks that
carry a high current for long belong to the
[track calculator](#track-current).

## From scripts and AI agents

Each calculator is an engine method (see the [engine API](../reference/engine-api#calculators)):

```sh
ewerk call calc.divider '{"vin": 12, "vout": 3.3, "series": "E24"}'
ewerk call calc.track '{"current": 2, "rise": 10, "thickness": 35000}'
ewerk call calc.eSeries '{"value": 4600, "series": "E96"}'
```

AI agents call them through the MCP tool `call_engine`. Lengths are
nanometres, as everywhere in the engine (`35000` is 35 µm).
