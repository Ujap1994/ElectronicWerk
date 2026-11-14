# ADR-0012: Automation and AI agents

- **Status:** accepted
- **Date:** 2026-11-14
- **Milestone:** M12

## Context

M12 completes the command-line tool. The roadmap asks for:

- every engine command in the CLI: create projects, place components,
  connect nets, place footprints, route, run ERC/DRC, generate outputs;
- JSON input and output, batch scripts and exit codes for CI;
- a JSON-RPC server mode and an **MCP server**, so AI agents can use
  ElectronicWerk directly;
- a scripting cookbook.

Every engine method was reachable already (`ewerk call`, `ewerk rpc`), but
the methods are made for the editors: they take object ids, document ids and
nanometres, and an edit like "connect these two pins" is several calls
(geometry of the pins, a wire, a label). Scripts and language models do much
better with the names a designer uses.

## Decision

### A design layer in the engine

A new module `ew-core::design` adds `design.*` methods that take
designators, pins as `R1.2` or `U1.VCC`, sheet, board and net names,
millimetres and degrees. They compose the existing edits (placing symbols,
wires, labels, power ports, board edits, synchronisation, the router), so
they behave exactly like the editors. The command line, scripts and the MCP
server use them; nothing is implemented twice.

- **One undo step per call.** Multi-step operations are staged on a copy of
  the project (each step sees the previous ones) and committed together.
- **Connections are labels.** `design.connect` draws a short wire from each
  pin and a net label (global across sheets) or a power port. This needs no
  wire routing on the sheet, is always readable and is what experienced
  designers do with dense parts.
- **Parts and footprints without a library.** Symbols come from the project
  library or the base library (copied in); footprints may be wizard presets
  (`0805`, `SOIC-8`), so complete designs work without installing anything.
- **Routing.** `design.route` routes the unrouted connections one by one with
  the walkaround router of the layout editor (45° bends, one layer per
  connection, no vias) and reports what it could not route. A real
  autorouter (push-and-shove, vias, rip-up) stays in M15 and Specctra export
  in M14.
- **Self-description.** `engine.methods` stays a list of names; a table of
  one-line summaries (`ew-core::docs`, checked by a test to cover every
  method) feeds `ewerk methods --json` and the MCP server.

### Front-ends

- `ewerk schematic …` and `ewerk board …` wrap the design methods: open,
  change, save, close. `--json` everywhere.
- `ewerk script` runs JSON or JSON Lines steps in one session.
- `ewerk rpc` accepts **JSON-RPC 2.0** (numeric error codes, the engine's
  code in `data`, notifications, batches) next to the existing short form,
  which stays compatible.
- `ewerk mcp` is an MCP server over standard input and output, written
  directly on `serde_json` (no SDK dependency): `initialize` with version
  negotiation, `tools/list`, `tools/call`, `ping`. 26 tools with JSON
  schemas; tool failures are results with `isError`, so agents can correct
  themselves. Changes are saved after every tool call (agents forget to
  save), unless `--no-autosave`.

Stdio only: a TCP server would let any local process change files.

### Exit codes

`0` success, `1` a check found errors (validation, ERC, DRC, outputs, script
steps), `2` usage, `3` the command could not run. Before, `1` also meant
"could not run", so CI could not tell a broken setup from a design error.

### Two writers

Agents work on the files while the project may be open in the app. Saving
now checks every file it would write or delete against the content it read
or saved (FNV-1a hashes the engine keeps anyway) and fails with the new error
code `changedOnDisk` if another program changed it. Nothing is ever
overwritten silently; the app explains what to do (save a copy, reopen).

## Consequences

- Scripts and agents get the same results as the editors, with undo, and the
  examples of the cookbook run as tests on every platform.
- The design layer is deliberately small; everything else stays reachable
  through `call_engine` / `ewerk call` with the full engine API.
- `design.route` is simple: dense boards need the app (or M15).
- The app does not reload a project changed by another program by itself
  yet; the user reopens it. A file watcher can follow.
