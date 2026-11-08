# Benchmarks and performance budgets

Performance is a feature of ElectronicWerk (see the roadmap's
[performance budgets](https://github.com/Ujap1994/ElectronicWerk/blob/main/ROADMAP.md#6-performance-budgets)).
Budgets are measured on **synthetic reference designs**, so the measurements
are reproducible and contain no private data.

## Synthetic reference designs

The generator lives in
[`crates/ew-synth`](https://github.com/Ujap1994/ElectronicWerk/tree/main/crates/ew-synth).
The same size and seed always produce the same project.

| Size | Parts  | Nets   | Track segments | Copper layers | Board        | Schematic sheets |
| ---- | ------ | ------ | -------------- | ------------- | ------------ | ---------------- |
| S    | 50     | 60     | 600            | 2             | 80 × 60 mm   | 1                |
| M    | 500    | 1,000  | 20,000         | 4             | 160 × 120 mm | 1 + 5            |
| L    | 5,000  | 10,000 | 200,000        | 8             | 400 × 300 mm | 1 + 50           |
| XL   | 20,000 | 40,000 | 800,000        | 8             | 800 × 600 mm | 1 + 100          |

A design contains:

- A project library with six part types (0603 resistor and capacitor, SOT-23
  transistor, SOIC-8 IC, QFP-64 microcontroller, 1×10 pin header): symbols
  with pins and footprints with pads, courtyard and designator text.
- Parts placed on a grid, with 10 % of the passives on the bottom side.
- Nets that connect neighbouring pads; staircase routes with exactly the
  target number of track segments, vias where a route leaves the top layer,
  and ground pours on every inner layer.
- A schematic hierarchy: a top sheet with one sheet symbol per child sheet,
  and on each child sheet the symbols with a wire stub and a global label per
  pin. Symbols and footprints are linked by component ids.

Generate one with the command line:

```sh
ewerk synth L ./synthetic-l
ewerk synth XL ./synthetic-xl --seed 42
```

The **Demo project** template in the new-project wizard is a synthetic S
design.

## Budgets checked in CI

| Metric           | Budget  | Typical (Apple M-series) | Test                                |
| ---------------- | ------- | ------------------------ | ----------------------------------- |
| Open project (L) | < 3 s   | ≈ 0.14 s                 | `crates/ew-format/tests/budgets.rs` |
| Save project (L) | < 1 s   | ≈ 0.31 s                 | `crates/ew-format/tests/budgets.rs` |
| Board scene (L)  | < 0.5 s | ≈ 0.04 s                 | `crates/ew-scene/tests/budgets.rs`  |

The L board file is about 55 MB. Saving writes every file (not only changed
ones), each through a synced temporary file, so the measurement includes the
full cost of an atomic save. Documents are encoded and written in parallel.

The CI job **Performance budgets** runs:

```sh
cargo test --release -p ew-format -p ew-scene --test budgets -- --ignored --nocapture
```

and prints the measured times. The board scene is everything the
[2D view](../guide/viewer) needs from the engine: about 590,000 primitives for
the L board, built in one pass and sorted for view culling.

## Viewer

The end-to-end test `apps/desktop/e2e/viewer-benchmark.spec.ts` opens a
project in the real app, pans and zooms its board for 240 frames each and
measures the frame rate and the CPU time per frame:

```sh
ewerk synth L /tmp/synth-l
npm run build -w @electronicwerk/desktop
cd apps/desktop
EW_PERF_PROJECT="/tmp/synth-l/Synthetic L.ewproj" EW_PERF_MIN_FPS=58 \
  npx playwright test e2e/viewer-benchmark.spec.ts --reporter=line
```

The results are written to `apps/desktop/test-results/viewer-benchmark.json`.

| Design | Primitives  | Open to first frame | Pan    | Zoom   | CPU per frame |
| ------ | ----------- | ------------------- | ------ | ------ | ------------- |
| L      | ≈ 590,000   | 0.20 s              | 60 fps | 60 fps | 0.16 ms       |
| XL     | ≈ 2,280,000 | 0.86 s              | 60 fps | 60 fps | 0.17 ms       |

Measured on an Apple M3 (integrated graphics, 60 Hz display, window
1600 × 1000). 60 fps is the refresh rate of the display, so the view is never
the bottleneck there. The CPU time stays tiny because the engine prepares the
primitives once and every frame only sets a few uniforms and issues one draw
call per visible run of chunks.

The CI job **Viewer benchmark** runs the same test on a macOS runner. Its
virtual GPU is much slower than real hardware, so CI checks a lower frame
rate (20 fps) and the CPU time per frame (below 4 ms); on software renderers
only the CPU time is checked. The roadmap budget of 60 fps on a mid-range
integrated GPU is verified on real hardware for every release.

## KiCad import

The KiCad import is measured locally with ignored tests, because real
projects and the official KiCad libraries are not part of the repository:

```sh
EW_KICAD_SAMPLE=/path/to/project.kicad_pro \
EW_KICAD_LIBRARIES=/path/to/kicad-libraries \
cargo test --release -p ew-kicad --test samples -- --ignored --nocapture
```

`EW_KICAD_SAMPLE` imports a project (with its KiCad netlist exported next to
it as `.net`), checks that the board connects exactly like the netlist and
that the pins of the placed symbols meet wires. `EW_KICAD_LIBRARIES` reads
every symbol and footprint library below a folder.

| Measurement (Apple M3, release build)                                | Result                        |
| -------------------------------------------------------------------- | ----------------------------- |
| A 4-layer, multi-sheet KiCad 10 project (480 nets, about 2,800 pins) | 0.08 s, 480 of 480 nets match |
| All official KiCad libraries (22,916 symbols, 15,470 footprints)     | 5.6 s, no errors              |

## Micro-benchmarks

[Criterion](https://github.com/bheisler/criterion.rs) benchmarks measure
encoding and decoding of the S, M and L boards and the generator:

```sh
cargo bench -p ew-format                  # everything (the L size takes a while)
cargo bench -p ew-format -- "/S"          # quick run
```

Criterion keeps the previous results in `target/criterion` and reports changes
against them, so you can check a change locally before and after.

## Quick measurement with `ewerk bench`

```console
$ ewerk bench --size L
L design: open 142 ms, save 304 ms, board scene 40 ms (587730 primitives) (best of 3)
```
