<p align="center">
  <img src="assets/icon/icon.svg" width="128" height="128" alt="ElectronicWerk icon">
</p>

# ElectronicWerk

An open-source ECAD suite for beginners and advanced hobbyists. It covers
schematic, symbol, footprint and layout editing, a live 3D view, ERC/DRC,
manufacturing outputs, online part search, GitHub integration and a full CLI.

> **Status:** early development. Version 0.1 provides the application
> foundation; the editors follow milestone by milestone. See the
> [roadmap](ROADMAP.md) for milestones and progress.

**[Documentation](https://ujap1994.github.io/ElectronicWerk/)** ·
**[Download](https://github.com/Ujap1994/ElectronicWerk/releases/latest)** ·
**[Changelog](CHANGELOG.md)** · **[Contributing](CONTRIBUTING.md)**

## Planned highlights

- An Altium/KiCad-style **project** that holds schematics, PCB, libraries,
  rules and output jobs
- **KiCad import** (projects and libraries) and open standard formats
  (netlists, DXF, Gerber, STEP, …)
- A **unified base library** with deduplication, and **online part search**
- A **live 3D view**, optionally in its own window
- **ERC, DRC** and a rule system
- Manufacturing outputs: Gerber X2, Excellon, BOM, pick-and-place, PDF, STEP
- **GitHub integration**: save a project straight to a new or existing
  repository
- A **CLI** for scripting and AI agents
- An **assembly assistant**
- A German and English UI
- Builds for Windows, macOS and Linux

## License

The application code is licensed under the [MIT License](LICENSE). Library
data derived from third-party libraries keeps its original license (for
example, the KiCad libraries are CC-BY-SA 4.0 with a design exception).
