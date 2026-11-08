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

| Metric           | Budget | Typical (Apple M-series) | Test                                |
| ---------------- | ------ | ------------------------ | ----------------------------------- |
| Open project (L) | < 3 s  | ≈ 0.14 s                 | `crates/ew-format/tests/budgets.rs` |
| Save project (L) | < 1 s  | ≈ 0.31 s                 | `crates/ew-format/tests/budgets.rs` |

The L board file is about 55 MB. Saving writes every file (not only changed
ones), each through a synced temporary file, so the measurement includes the
full cost of an atomic save. Documents are encoded and written in parallel.

The CI job **Performance budgets** runs:

```sh
cargo test --release -p ew-format --test budgets -- --ignored --nocapture
```

and prints the measured times. The rendering budgets (frame rate, input
latency) follow with the 2D engine in milestone M2.

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
L design: open 137 ms, save 310 ms (best of 3)
```
