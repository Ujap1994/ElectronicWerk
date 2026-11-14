//! One-line summaries of the engine methods, for `ewerk methods --json`,
//! the MCP server and other self-describing clients. The details are in
//! the engine API reference (`docs/reference/engine-api.md`).

use serde_json::{Value, json};

/// The address of the engine API reference.
pub const REFERENCE: &str = "https://ujap1994.github.io/ElectronicWerk/reference/engine-api";

/// Every method with its summary, in the order of [`crate::rpc::METHODS`].
pub const SUMMARIES: &[(&str, &str)] = &[
    (
        "board3d.export",
        "`board3d.export {\"doc\", \"path\", \"format\"?, \"origin\"?, \"includeDnp\"?, \"units\"?}` writes the board as a 3D model to `path` (absolute): `format` `step` (default), `glb` or `vrml`; `origin` `drill` (default), `center` or `absolute`; `units` (VRML) `mm` (default), `m` or `inch`. The reply:",
    ),
    (
        "board3d.parts",
        "`board3d.parts {\"doc\"}` → `{\"parts\", \"models\"}`: only the parts, for live updates while parts move (about 7 ms for 5,000 parts). Every part:",
    ),
    (
        "board3d.resolve",
        "`board3d.resolve {\"path\"}` → `{\"path\", \"file\"?, \"format\"?, \"entry\"?, \"mesh\"?}`: where a model path was found — a `file` on this computer (`format` `step` or `vrml`), the base library's model `entry` with the hash of its `mesh` in the mesh cache, both, or neither. See Parts and their 3D models for the search order.",
    ),
    (
        "board3d.scene",
        "`board3d.scene {\"doc\", \"inner\"?}` builds the 3D board of a board document: the laminate (outline minus holes), the copper, solder mask, silkscreen and paste of each side, the plated walls of holes and, with `inner: true`, the inner copper layers. The reply:",
    ),
    (
        "design.arrange",
        "{\"designators\"?, \"gap\"? (mm, default 2), \"board\"?}. Puts footprints in rows inside the board outline, in designator order (default: those not on the board).",
    ),
    (
        "design.board",
        "{\"board\"?}. The board: outline (mm), copper layers, footprints with position (mm), rotation (degrees), side and whether they are on the board, track and via counts, unrouted connections.",
    ),
    (
        "design.components",
        "The parts of the schematic by designator: value, symbol, footprint, sheet and every pin with its number, name and net. No ids.",
    ),
    (
        "design.connect",
        "{\"net\", \"pins\": [\"R1.2\", \"U1.VCC\"], \"style\"?: \"auto\" | \"label\" | \"global\" | \"power\"}. Connects pins (by number or name) to a net with a short wire and a label (global when the net spans sheets) or a power port. Pins on the net already are skipped; pins on another named net fail.",
    ),
    (
        "design.footprint",
        "{\"name\"}. Makes sure a footprint is in the project library: as it is, from a footprint wizard preset (design.presets) or from the base library. Returns its name.",
    ),
    (
        "design.nets",
        "The nets with their name and their pins as designator.pin (for example R1.2). Local nets carry the sheet path (/LED_A).",
    ),
    (
        "design.noConnect",
        "{\"pins\": [\"U1.7\"]}. Puts no-connect flags on pins that stay open on purpose.",
    ),
    (
        "design.place",
        "{\"symbol\" | \"component\", \"designator\"?, \"value\"?, \"footprint\"?, \"sheet\"?, \"at\"?: [x, y] mm, \"rotation\"? (degrees), \"mirror\"?, \"unit\"?}. Places a symbol; a symbol missing in the project is taken from the base library (Device:R), a footprint may be a wizard preset (0805, SOIC-8). Without \"at\", a free place on the sheet. Returns designator, value, footprint, at and pins.",
    ),
    (
        "design.placeFootprint",
        "{\"designator\", \"at\": [x, y] mm, \"rotation\"? (degrees), \"side\"?: \"top\" | \"bottom\", \"board\"?}. Puts a footprint at an exact place.",
    ),
    (
        "design.presets",
        "The names of the footprint wizard presets usable as footprints (0402 … 2512, SOIC-8, TSSOP-20, QFN-32 5x5, pin headers, …).",
    ),
    (
        "design.route",
        "{\"nets\"?, \"layers\"?: [\"top\", \"bottom\"], \"width\"? (mm), \"board\"?}. Routes the unrouted connections one by one with the walkaround router (45° bends, one layer per connection, no vias). One undo step. Returns routed, failed and unrouted.",
    ),
    (
        "design.set",
        "{\"designator\", \"value\"?, \"footprint\"?, \"rename\"?, \"properties\"?: {key: text}}. Changes a part on every sheet (all units); an empty property text removes it.",
    ),
    (
        "design.updateBoard",
        "{\"board\"?}. Applies every change of the schematic to the board (new footprints below the board, nets, values, removed parts). Returns the changes.",
    ),
    (
        "document.add",
        "`{\"kind\": \"schematic\", \"name\": \"Power Supply\", \"parent\": \"…\"}`. Adds an empty document. With `parent` (a schematic), a sheet symbol for the new sheet is placed on the parent: at `at` (its top-left corner) with `size`, or in the next free slot of a 6-column grid if they are missing. Returns `id` and `path`.",
    ),
    (
        "document.get",
        "`{\"doc\": \"…\"}`. Returns the complete document, as in its file, plus `path`.",
    ),
    (
        "document.object",
        "`{\"doc\": \"…\", \"id\": \"…\"}`. One object of a document, with its `type`, in the same form as `edit.apply` takes it. Unknown objects fail with `notFound`.",
    ),
    (
        "document.properties",
        "`document.properties` returns the editable properties of a document (`name`, `page` and `titleBlock` of a sheet, `stackup` and `origin` of a board, the whole content of rules and output jobs) plus `docKind` and `path`. `document.setProperties` changes them with a JSON merge patch:",
    ),
    (
        "document.remove",
        "`{\"doc\": \"…\"}`. Removes the document and the sheet symbols that place it; the file is deleted on the next save. One undo step.",
    ),
    (
        "document.rename",
        "`{\"doc\": \"…\", \"name\": \"…\"}`. One undo step.",
    ),
    (
        "document.setProperties",
        "`document.properties` returns the editable properties of a document (`name`, `page` and `titleBlock` of a sheet, `stackup` and `origin` of a board, the whole content of rules and output jobs) plus `docKind` and `path`. `document.setProperties` changes them with a JSON merge patch:",
    ),
    (
        "edit.apply",
        "Applies a list of changes as one transaction: either all succeed or nothing changes.",
    ),
    (
        "edit.history",
        "Returns `state` (the history state), `undo` and `redo` (labels, next step first) and `bytes` (memory used by the history).",
    ),
    (
        "edit.redo",
        "Undo or redo one transaction. Result: `applied` (`false` if there was nothing to undo/redo), `label` and `history`.",
    ),
    (
        "edit.undo",
        "Undo or redo one transaction. Result: `applied` (`false` if there was nothing to undo/redo), `label` and `history`.",
    ),
    (
        "engine.info",
        "Information about the engine build: name, version, operating system, architecture, threads, debug or release.",
    ),
    (
        "engine.methods",
        "Returns the names of all methods, sorted alphabetically.",
    ),
    (
        "engine.ping",
        "Returns `{\"pong\": true, \"echo\": <params>}`.",
    ),
    (
        "gerber.scene",
        "`gerber.scene {\"paths\", \"name\"?, \"encoding\"?}` reads Gerber and Excellon files (absolute, or relative to the project folder) and returns a scene like `view.scene`, one layer per file, plus `\"files\": [{\"file\", \"function\"}]` (the X2 file function, or one guessed from the extension) and `\"problems\":",
    ),
    (
        "import.archive",
        "Imports the download as one undoable step (`Import part download`): the symbols and footprints the library does not have yet, the used 3D models copied into the project folder. Returns `{\"symbols\", \"footprints\", \"models\", \"replaced\", \"skipped\", \"nickname\", \"problems\", \"history\"}`.",
    ),
    (
        "import.archivePreview",
        "Reads a part download of an online part library without changing anything. Works without an open project. The reply:",
    ),
    (
        "import.kicad",
        "Imports the KiCad project into a new project, opens it and returns `{\"project\", \"report\", \"reportFile\"}`: the project snapshot, the import report and the path of the Markdown report inside the project (`kicad-import-report.md`). No project may be open.",
    ),
    (
        "import.kicadProbe",
        "`{\"source\": \"…\"}`. Checks a KiCad project without importing it: the `.kicad_pro` file, its schematic or board, or a folder with exactly one KiCad project. Returns `{\"name\", \"folder\", \"project\", \"schematic\", \"board\"}` (the last three tell which files exist). Fails with `invalidParams` if `source` is not a KiCad project.",
    ),
    (
        "import.library",
        "Adds the symbols and footprints that the library does not have yet, as one undoable step (`Import library`). Returns `{\"symbols\", \"footprints\", \"report\", \"history\"}`: the numbers added, a report (with `already-in-library` for skipped names) and the history state.",
    ),
    ("import.netlist", "Returns:"),
    (
        "libedit.check",
        "`{\"doc\", \"kind\"?, \"name\"?}`. The library checks of a whole library, one kind or one item: `[{\"kind\", \"name\", \"findings\"}]` for the items with findings. A finding has `code`, `severity` (`error`, `warning`, `info`), `message` (English), `args` (for translations) and `target` (`{\"kind\":",
    ),
    (
        "libedit.create",
        "`{\"doc\", \"kind\", \"name\", \"copyOf\"?, \"def\"?, \"wizard\"?}` (`create-library-item`). An empty item, a copy of another item of the library (`copyOf`), a given definition (`def`) or a footprint from a wizard (see `libedit.wizard`). Returns `{\"name\"}`; a name in use fails with `conflict`.",
    ),
    (
        "libedit.delete",
        "`{\"doc\", \"kind\", \"name\", \"force\"?}` (`delete-library-item`). An item in use fails with `conflict` unless `force` is set. Returns `{\"usage\"}`.",
    ),
    (
        "libedit.footprint",
        "`{\"doc\", \"name\", \"edit\"}` (`edit-footprint`). One edit of a footprint; returns `{\"select\"}`. Besides `move`, `duplicate`, `delete`, `orient`, `addGraphic`, `addText`, `updateGraphic`, `updateText` and `renumber` (pads), which work as for symbols:",
    ),
    (
        "libedit.item",
        "`{\"doc\", \"kind\", \"name\"}`. `{\"def\", \"usage\", \"findings\"}`: the definition in the file format, where it is used (`{\"placed\", \"board\", \"symbols\", \"components\"}`) and the check findings (see `libedit.check`).",
    ),
    (
        "libedit.list",
        "`{\"doc\"}`. The items of a library: `{\"name\", \"symbols\", \"footprints\", \"components\"}`, each sorted by name, with `name`, `description` and `checks` (`{\"errors\", \"warnings\", \"infos\"}`); symbols also `units`, `pins`, `alternate` and `power`, footprints `pads` and `models`, components `symbol`, `footprint` and `mpn`.",
    ),
    (
        "libedit.presets",
        "`libedit.presets` returns the wizard presets: `[{\"name\", \"wizard\"}]`. `libedit.wizard` (`{\"wizard\", \"name\"?}`) computes a footprint without adding it: `{\"name\", \"def\", \"findings\"}`, with the suggested name unless `name` is given. A wizard is one of:",
    ),
    (
        "libedit.rename",
        "`{\"doc\", \"kind\", \"name\", \"new\"}` (`rename-library-item`). Renames the item and every reference: placed symbols, board footprints, default footprints of symbols, symbols and footprints of components. Returns `{\"name\", \"changes\"}`.",
    ),
    (
        "libedit.setProps",
        "`{\"doc\", \"kind\", \"name\", \"props\"}` (`edit-library-item`). Changes the item's own properties as a JSON merge patch: `{\"description\": \"…\", \"pinText\": {\"hideNumbers\": true}, \"properties\": {\"Tolerance\": \"1%\", \"Old\": null}}`.",
    ),
    (
        "libedit.symbol",
        "`{\"doc\", \"name\", \"body\"?: {\"unit\", \"alternate\"}, \"edit\"}` (`edit-symbol`). One edit of a symbol body (default: unit 1, normal). Returns `{\"body\", \"select\"}`: the body to show afterwards (a new unit, the alternate body) and the items to select (`[{\"kind\": \"pin\", \"index\": 3}]`). Edits (`op`):",
    ),
    (
        "libedit.wizard",
        "`libedit.presets` returns the wizard presets: `[{\"name\", \"wizard\"}]`. `libedit.wizard` (`{\"wizard\", \"name\"?}`) computes a footprint without adding it: `{\"name\", \"def\", \"findings\"}`, with the suggested name unless `name` is given. A wizard is one of:",
    ),
    (
        "library.add",
        "`{\"ids\": [42, 43], \"doc\": \"…\", \"footprints\": true}`. Copies symbols and footprints into a library of the open project (default: the first one), as one undoable step (`add-from-library`). With `footprints` (default), the default footprints of the symbols are copied too, if the base library has them.",
    ),
    (
        "library.build",
        "A background build returns `{\"started\": true}` at once and reports through events (see below); searches keep using the old index until the new one is ready. With `wait`, the result is `{\"status\", \"problems\"}`. Review decisions are kept.",
    ),
    (
        "library.decide",
        "`{\"key\": \"…\", \"decision\": \"merge\", \"canonical\": 42}`. Records a review decision (also in `decisions.json`) and returns the group. `canonical` sets the shown entry. Emits a `changed` event.",
    ),
    (
        "library.detect",
        "Lists KiCad installations of this computer with libraries: `[{\"path\", \"symbols\", \"footprints\", \"models\"}]`.",
    ),
    (
        "library.facets",
        "`{\"kind\": \"footprint\"}`. The libraries of a kind with their number of visible entries: `[{\"library\", \"count\"}]`.",
    ),
    (
        "library.get",
        "`{\"id\": 42}`. Everything about an entry: the search fields plus `keywords`, `mpn`, `manufacturer`, `datasheet`, `params`, `units`, `size` (mm), `model` (the model key of a footprint, `Library/Name`), `modelId`, `file`, `sourceFile` (the STEP file on this computer, if known), `mesh` (the GLB content hash of a model), `a…",
    ),
    (
        "library.groups",
        "Returns `{\"total\", \"groups\"}`; each group has `key`, `kind`, `class`, `reason`, `decision`, `canonical` (the id of the shown entry) and `members` (`id`, `name`), best first.",
    ),
    (
        "library.install",
        "`{\"file\": \"…\", \"release\": \"library-…\", \"remove\": true}`. Installs a pack (see Building the base library). `release` is stored with an index pack; `remove` deletes the pack afterwards. Returns `{\"installed\": {\"index\", \"meshes\"}, \"status\"}` and emits `changed`.",
    ),
    (
        "library.mesh",
        "`{\"hash\": \"…\"}`. Where the GLB file of a mesh hash is cached: `{\"path\", \"exists\"}`.",
    ),
    (
        "library.model",
        "`{\"path\": \"${KICAD9_3DMODEL_DIR}/Package_SO.3dshapes/SOIC-8….step\"}`. The base library entry of a footprint's 3D model path:",
    ),
    (
        "library.open",
        "`{\"dir\": \"…\"}`. Uses another library folder (closes the current index).",
    ),
    (
        "library.preview",
        "`{\"id\": 42, \"unit\": 1}`. The 2D scene of a symbol (one unit) or footprint, in the same form as `view.scene` (the desktop app receives the buffer separately). 3D models have no 2D preview (`invalidParams`).",
    ),
    (
        "library.report",
        "`{\"format\": \"markdown\"}` or JSON (default). The deduplication report as a string.",
    ),
    (
        "library.search",
        "Returns `{\"total\", \"items\", \"elapsedMs\"}`. Each item: `id`, `kind`, `library`, `name`, `description`, `value`, `package`, `pins`, `mounting`, `mergedInto` (the id of the entry it is merged into, or `null`) and `groups` (the number of duplicate groups it belongs to).",
    ),
    (
        "library.status",
        "Returns `{\"dir\", \"installed\", \"building\", \"status\"}`. `building` is `{\"stage\", \"done\", \"total\"}` while a build runs (stages `symbols`, `footprints`, `models`, `index`), else `null`. `status` describes the index:",
    ),
    (
        "outputs.applyPreset",
        "`outputs.applyPreset {\"job\", \"preset\"}` (`output-preset`) replaces the outputs of a job with the outputs of a preset.",
    ),
    (
        "outputs.bom",
        "`outputs.bom {\"source\"?, \"options\"?}` → `{\"headers\", \"rows\", \"parts\", \"fields\"}`: the bill of materials for the given BOM options (columns, grouping, …) without writing a file — the preview of the editor. `fields` lists the fields that columns can show.",
    ),
    (
        "outputs.create",
        "`outputs.create {\"name\", \"preset\"?}` (`add-output-job`) adds an output job with the outputs of a preset (default `generic`) → `{\"id\"}`.",
    ),
    (
        "outputs.files",
        "`outputs.files {\"job\"}` → `{\"directory\", \"files\"}`: the Gerber and drill files in the job's output folder (absolute paths), for the Gerber viewer.",
    ),
    (
        "outputs.presets",
        "Lists the presets for board makers: `[{\"id\", \"name\", \"description\", \"outputs\": [[name, kind, options], ...]}]`. Works without an open project.",
    ),
    (
        "outputs.run",
        "`outputs.run {\"job\", \"outputs\"?, \"labels\"?}` generates the outputs of a job (or only the listed output ids) and writes them into the job's folder inside the project. `labels` translates the title block of schematic prints, as in `view.scene`. The reply:",
    ),
    (
        "pcb.addTracks",
        "`pcb.routeStart {\"doc\", \"at\", \"layer\"}` → `{\"at\", \"net\", \"netName\", \"width\", \"viaDiameter\", \"viaDrill\", \"clearance\", \"previous\"}`: where a route that begins at `at` starts (snapped to a pad or via centre or a track end) and the sizes from the rules.",
    ),
    (
        "pcb.applyPreset",
        "`pcb.presets` lists the board presets. `pcb.applyPreset {\"doc\", \"preset\": \"standard-4l\"}` (`board-preset`) sets the stack-up of the board and the minimums and `Default` class of the rules.",
    ),
    (
        "pcb.backApply",
        "The same for back annotation: `setDesignator`, `setValue` and `setFootprint` changes from the board to the schematic (`update-schematic`).",
    ),
    (
        "pcb.backPreview",
        "The same for back annotation: `setDesignator`, `setValue` and `setFootprint` changes from the board to the schematic (`update-schematic`).",
    ),
    (
        "pcb.crossProbe",
        "`{\"doc\", \"symbols\"?: [\"…\"], \"footprints\"?: [\"…\"]}` → `{\"footprints\": [\"…\"], \"symbols\": [{\"doc\", \"id\"}]}`: the footprints of placed symbols and the placed symbols (with their sheets) of footprints.",
    ),
    (
        "pcb.drc",
        "`{\"doc\", \"region\"?: [x0, y0, x1, y1]}`. Runs the design rules check. With `region`, only the items in that area (grown by the largest clearance) are checked and the result is merged with the last report of the board; without, the whole board is checked. Returns:",
    ),
    (
        "pcb.dxfPreview",
        "`pcb.dxfPreview {\"path\", \"scale\"?}` → `{\"layers\", \"units\", \"count\", \"bounds\", \"skipped\"}`. `pcb.importDxf {\"doc\", \"path\", \"layer\", \"layers\"?, \"replace\"?, \"scale\"?, \"origin\"?, \"width\"?}` (`import-dxf`) imports the shapes of the chosen DXF layers onto a board layer (`outline`, `mech.1`, …); `scale` is nanometres per draw…",
    ),
    (
        "pcb.edit",
        "`{\"doc\", \"op\", …}`. Operations (label in brackets):",
    ),
    (
        "pcb.exclude",
        "`{\"doc\", \"keys\": [\"…\"], \"excluded\"?: true}` (`drc-exclusions`). Accepts findings (or reports them again with `\"excluded\": false`). The keys are stored in the board.",
    ),
    (
        "pcb.fillZones",
        "`{\"doc\", \"zones\"?: [\"…\"]}` (`fill-zones` / `unfill-zones`). Fills (or empties) all zones of the board, or the given ones. Returns `{\"zones\", \"areas\"}`: the number of changed zones and of filled areas.",
    ),
    (
        "pcb.importDxf",
        "`pcb.dxfPreview {\"path\", \"scale\"?}` → `{\"layers\", \"units\", \"count\", \"bounds\", \"skipped\"}`. `pcb.importDxf {\"doc\", \"path\", \"layer\", \"layers\"?, \"replace\"?, \"scale\"?, \"origin\"?, \"width\"?}` (`import-dxf`) imports the shapes of the chosen DXF layers onto a board layer (`outline`, `mech.1`, …); `scale` is nanometres per draw…",
    ),
    (
        "pcb.place",
        "`{\"doc\", \"footprint\": \"SOIC-8\", \"at\", \"rotation\"?, \"side\"?, \"designator\"?, \"value\"?}` (`place-footprint`). Places a footprint of the project libraries. Without a designator it gets the next free `X<n>`.",
    ),
    (
        "pcb.presets",
        "`pcb.presets` lists the board presets. `pcb.applyPreset {\"doc\", \"preset\": \"standard-4l\"}` (`board-preset`) sets the stack-up of the board and the minimums and `Default` class of the rules.",
    ),
    (
        "pcb.ratsnest",
        "`{\"doc\"}` → `{\"airwires\": [...], \"unrouted\": 3}`: the unrouted connections without running the checks.",
    ),
    (
        "pcb.route",
        "`pcb.routeStart {\"doc\", \"at\", \"layer\"}` → `{\"at\", \"net\", \"netName\", \"width\", \"viaDiameter\", \"viaDrill\", \"clearance\", \"previous\"}`: where a route that begins at `at` starts (snapped to a pad or via centre or a track end) and the sizes from the rules.",
    ),
    (
        "pcb.routeStart",
        "`pcb.routeStart {\"doc\", \"at\", \"layer\"}` → `{\"at\", \"net\", \"netName\", \"width\", \"viaDiameter\", \"viaDrill\", \"clearance\", \"previous\"}`: where a route that begins at `at` starts (snapped to a pad or via centre or a track end) and the sizes from the rules.",
    ),
    (
        "pcb.rules",
        "`pcb.rules {\"doc\"?}` returns the project's design rules (`rules`: the properties of the rules document), the DRC checks with their default and current severity (`checks`), the nets of each net class on the board `doc` (`nets`) and syntax errors of custom rules (`errors`).",
    ),
    (
        "pcb.setRules",
        "`pcb.rules {\"doc\"?}` returns the project's design rules (`rules`: the properties of the rules document), the DRC checks with their default and current severity (`checks`), the nets of each net class on the board `doc` (`nets`) and syntax errors of custom rules (`errors`).",
    ),
    (
        "pcb.syncApply",
        "`pcb.syncPreview {\"doc\"}` compares the schematic with the board and returns `{\"changes\", \"problems\"}`. Each change has an `index`, a `kind` (`addNet`, `renameNet`, `removeNet`, `addFootprint`, `removeFootprint`, `changeFootprint`, `setDesignator`, `setValue`, `link`, `setPadNet`), its fields and an English `description…",
    ),
    (
        "pcb.syncPreview",
        "`pcb.syncPreview {\"doc\"}` compares the schematic with the board and returns `{\"changes\", \"problems\"}`. Each change has an `index`, a `kind` (`addNet`, `renameNet`, `removeNet`, `addFootprint`, `removeFootprint`, `changeFootprint`, `setDesignator`, `setValue`, `link`, `setPadNet`), its fields and an English `description…",
    ),
    (
        "pcb.unfillZones",
        "`{\"doc\", \"zones\"?: [\"…\"]}` (`fill-zones` / `unfill-zones`). Fills (or empties) all zones of the board, or the given ones. Returns `{\"zones\", \"areas\"}`: the number of changed zones and of filled areas.",
    ),
    (
        "pcb.validateRule",
        "`pcb.rules {\"doc\"?}` returns the project's design rules (`rules`: the properties of the rules document), the DRC checks with their default and current severity (`checks`), the nets of each net class on the board `doc` (`nets`) and syntax errors of custom rules (`errors`).",
    ),
    (
        "project.archive",
        "`{\"path\": \"….zip\"}`. Writes a ZIP archive of the current state. Returns `path`, `files`, `bytes`.",
    ),
    (
        "project.close",
        "`{\"discard\": false}`. Closes the project. With unsaved changes this fails with `unsavedChanges` unless `discard` is `true`.",
    ),
    (
        "project.create",
        "Creates the project on disk, opens it and returns the project snapshot.",
    ),
    (
        "project.discardRecovery",
        "Restores or discards the unsaved work of a crashed session (see Undo, autosave and recovery). Both return the snapshot. `project.recover` fails with `conflict` (and changes nothing) if the journal no longer fits the saved files.",
    ),
    (
        "project.files",
        "Lists the files in the project folder: `path`, `size`, `category` (`manifest`, `document`, `generated`, `other`).",
    ),
    ("project.info", "Returns the snapshot of the open project."),
    (
        "project.open",
        "`{\"path\": \"…\"}` (folder or `.ewproj` file). Opens the project and returns the snapshot. If unsaved work from a crashed session exists, `recovery` describes it; call `project.recover` or `project.discardRecovery` before editing.",
    ),
    (
        "project.probe",
        "`{\"path\": \"…\"}`. A quick check without opening: `{\"exists\", \"valid\", \"name\", \"path\", \"manifestPath\", \"hasRecovery\"}`.",
    ),
    (
        "project.recover",
        "Restores or discards the unsaved work of a crashed session (see Undo, autosave and recovery). Both return the snapshot. `project.recover` fails with `conflict` (and changes nothing) if the journal no longer fits the saved files.",
    ),
    (
        "project.resolveText",
        "`{\"text\": \"${PROJECT_NAME} rev ${REVISION}\", \"doc\": \"…\"}` → `{\"text\": \"LED Blinker rev B\"}`. `doc` provides `SHEET_NAME` and `FILENAME`.",
    ),
    (
        "project.save",
        "`{\"all\": false}`. Writes changed files (every file with `all`), deletes files of removed documents and resets the autosave journal. Returns `written`, `deleted` (project-relative paths) and `history`.",
    ),
    (
        "project.saveAs",
        "`{\"path\": \"…\"}` (a new or empty folder outside the project). `saveAs` writes the project there and continues working in the new folder (returns the snapshot); `saveCopy` only writes a copy (returns its `manifestPath`). Other files of the project folder are copied too.",
    ),
    (
        "project.saveCopy",
        "`{\"path\": \"…\"}` (a new or empty folder outside the project). `saveAs` writes the project there and continues working in the new folder (returns the snapshot); `saveCopy` only writes a copy (returns its `manifestPath`). Other files of the project folder are copied too.",
    ),
    (
        "project.setProperties",
        "Changes project properties with a JSON merge patch: `name`, `revision`, `description`, `variables` (`null` removes a variable), `rootSchematic`, `settings` (`units`, `schematicGrid`, `boardGrid`). One undo step. Returns the history state.",
    ),
    (
        "project.templates",
        "Lists the templates: `id`, default `board` settings (`width`, `height`, `layers`, `thickness` in nanometres, or `null`), `fixedSize`, `example`.",
    ),
    (
        "project.validate",
        "`{\"path\": \"…\"}` checks a project folder without opening it; without `path` the open project is checked. Returns `issues` (`severity`, `code`, `message`, `path`, `object`, `params`), `errors`, `warnings`, `documents`, `objects`. `message` is English; `params` holds the values in it (for example `{\"name\":",
    ),
    (
        "schematic.add",
        "`{\"doc\", \"item\", \"at\", \"rotation\"?}` with `item` one of `{\"kind\": \"label\", \"label\": \"local\" | \"global\" | \"hierarchical\", \"text\"}`, `{\"kind\": \"powerPort\", \"net\", \"style\"?}`, `{\"kind\": \"noConnect\"}`, `{\"kind\": \"junction\"}` or `{\"kind\":",
    ),
    (
        "schematic.annotate",
        "`{\"sheet\"?, \"reset\"?, \"order\"?, \"bySheet\"?}` (`annotate`). Numbers the symbols whose designator ends in `?` (with `reset`: all of them) on all sheets or only on `sheet`, sheet by sheet in hierarchy order and by position (`order`: `rows` or `columns`), from 1 or (with `bySheet`) from the sheet number × 100 + 1.",
    ),
    (
        "schematic.bom",
        "`schematic.netlist` (`{\"format\": \"kicad\" | \"csv\"}`) and `schematic.bom` return `{\"text\"}`: a KiCad netlist (version E), a CSV netlist (`Net,Designator,Pin`) or the bill of materials as CSV. See Schematic exports.",
    ),
    (
        "schematic.components",
        "The placed parts of all sheets: designator, value, footprint, symbol, sheet and pins (with ids, for editors; design.components is the simpler form).",
    ),
    (
        "schematic.copy",
        "`schematic.copy` (`{\"doc\", \"ids\"}`) returns `{\"objects\"}`, the objects as stored. `schematic.paste` (`{\"doc\", \"objects\", \"offset\", \"annotate\"?}`, `paste`) inserts them on any sheet with new ids, moved by `offset`; symbols keep their designator if it is free and are numbered again otherwise (`annotate`, default `true`)…",
    ),
    ("schematic.delete", "`{\"doc\", \"ids\"}` (`delete`)."),
    (
        "schematic.dragInfo",
        "`{\"doc\", \"ids\"}`. What a drag of these objects drags along, for previews: `{\"wires\": [{\"id\", \"start\", \"end\", \"startMoves\", \"endMoves\"}], \"followers\": [ids]}`.",
    ),
    (
        "schematic.erc",
        "The electrical rules check of the whole project, with the settings of the rules document. Returns `{\"issues\", \"errors\", \"warnings\", \"nets\", \"pins\"}`; each issue has `code`, `severity` (`error` or `warning`), `message` (English), `args` (values for a translated message:",
    ),
    (
        "schematic.ercSettings",
        "`schematic.ercSettings` returns `{\"checks\": [{\"code\", \"default\", \"value\"}], \"kinds\", \"defaultMatrix\", \"matrix\", \"rules\"}`: the severity of every check, the pin types in matrix order and the 12 × 12 pin type matrix (`error`, `warning` or `ignore` per cell), with the defaults, and the rules document that stores them.",
    ),
    (
        "schematic.find",
        "`schematic.find` takes `{\"query\", \"matchCase\"?, \"wholeWord\"?, \"sheet\"?}` and returns `[{\"doc\", \"id\", \"field\", \"text\", \"at\"}]` with `field` one of `designator`, `value`, `footprint`, `property:<name>`, `label`, `power`, `text` and `sheet`. `schematic.replace` takes the same plus `\"with\"` and, optionally, `\"only\":",
    ),
    (
        "schematic.move",
        "`{\"doc\", \"ids\", \"delta\", \"drag\"?}` (`move`, `drag`). With `drag`, wires that end on a connection point of a moved object follow: horizontal and vertical wires stay straight and get a corner wire, others stretch; junctions, labels, power ports and flags on those points move along.",
    ),
    (
        "schematic.net",
        "`{\"doc\", \"id\"?, \"pin\"?: {\"symbol\", \"index\"}, \"at\"?}`. The net of a wire, label or other object, of a pin (its index in the unit's pin list) or of a point, and what belongs to it on each sheet: `{\"net\": \"/DIS\", \"sheets\": {\"<doc>\": {\"items\": [ids], \"pins\": [\"<symbol id>/pin/<n>\", \"<sheet id>/sheetPin/<n>\"]}}, \"places\":",
    ),
    (
        "schematic.netlist",
        "`schematic.netlist` (`{\"format\": \"kicad\" | \"csv\"}`) and `schematic.bom` return `{\"text\"}`: a KiCad netlist (version E), a CSV netlist (`Net,Designator,Pin`) or the bill of materials as CSV. See Schematic exports.",
    ),
    (
        "schematic.orient",
        "`{\"doc\", \"ids\", \"orientation\", \"center\"?}` with `orientation` `left` (counterclockwise), `right`, `horizontal` (mirror left to right) or `vertical` (top to bottom) (`rotate`, `mirror`). One object turns about its anchor, several about the centre of their anchors on the grid.",
    ),
    (
        "schematic.paste",
        "`schematic.copy` (`{\"doc\", \"ids\"}`) returns `{\"objects\"}`, the objects as stored. `schematic.paste` (`{\"doc\", \"objects\", \"offset\", \"annotate\"?}`, `paste`) inserts them on any sheet with new ids, moved by `offset`; symbols keep their designator if it is free and are numbered again otherwise (`annotate`, default `true`)…",
    ),
    (
        "schematic.place",
        "`{\"doc\", \"symbol\", \"at\", \"rotation\"?, \"mirror\"?, \"unit\"?, \"annotate\"?}`. Places a symbol (`place-symbol`). With `annotate`, it gets the next free designator of its prefix across the project; otherwise `R?`.",
    ),
    (
        "schematic.replace",
        "`schematic.find` takes `{\"query\", \"matchCase\"?, \"wholeWord\"?, \"sheet\"?}` and returns `[{\"doc\", \"id\", \"field\", \"text\", \"at\"}]` with `field` one of `designator`, `value`, `footprint`, `property:<name>`, `label`, `power`, `text` and `sheet`. `schematic.replace` takes the same plus `\"with\"` and, optionally, `\"only\":",
    ),
    (
        "schematic.setErcSettings",
        "`schematic.ercSettings` returns `{\"checks\": [{\"code\", \"default\", \"value\"}], \"kinds\", \"defaultMatrix\", \"matrix\", \"rules\"}`: the severity of every check, the pin types in matrix order and the 12 × 12 pin type matrix (`error`, `warning` or `ignore` per cell), with the defaults, and the rules document that stores them.",
    ),
    (
        "schematic.snapToGrid",
        "`{\"doc\", \"ids\"?}` (`snap-to-grid`). Moves wire ends, symbols, labels, junctions, flags, power ports, texts and sheet symbols of the sheet (or only `ids`) to the nearest grid point.",
    ),
    (
        "schematic.symbol",
        "`schematic.symbols` lists the symbols of the project libraries: `name`, `description`, `keywords`, `prefix`, `value`, `footprint`, `power`, `units`, `pins` and `library`, sorted by name. `schematic.symbol` (`{\"name\"}`) returns one symbol definition.",
    ),
    (
        "schematic.symbols",
        "`schematic.symbols` lists the symbols of the project libraries: `name`, `description`, `keywords`, `prefix`, `value`, `footprint`, `power`, `units`, `pins` and `library`, sorted by name. `schematic.symbol` (`{\"name\"}`) returns one symbol definition.",
    ),
    (
        "schematic.syncSheetPins",
        "`{\"doc\", \"id\", \"remove\"?}` (`sync-sheet-pins`). Gives the sheet symbol `id` a pin for every hierarchical label of its sheet that has none yet (down the left edge, growing the symbol if needed); with `remove`, also removes pins without a label.",
    ),
    (
        "schematic.wire",
        "`{\"doc\", \"points\": [[x, y], …], \"bus\"?}`. A wire (or bus) through the points (`draw-wire`). Repeated points are dropped and straight runs merged; inner corners count as connection points.",
    ),
    (
        "view.item",
        "`{\"doc\", \"kind\": \"symbol\" | \"footprint\", \"name\", \"unit\"?, \"alternate\"?, \"encoding\"?}`. The scene of the library editor: one body of a symbol (unit, 1-based, normal or alternate) or a footprint, in the same form as `view.scene`. Every pin, pad, graphic and text is an object of its own; its id encodes the item:",
    ),
    (
        "view.object",
        "`{\"doc\": \"…\", \"id\": \"…\", \"child\": {\"kind\": \"pad\", \"index\": 0}}` (`child` only for pads, footprint texts, pins, fields and sheet pins). Returns the properties shown in the inspector:",
    ),
    (
        "view.scene",
        "Returns everything the view draws for the document. The metadata is JSON; the primitives are one little-endian binary buffer (`data`). The desktop app receives the buffer as raw bytes instead of base64.",
    ),
];

/// The summary of a method.
#[must_use]
pub fn summary(method: &str) -> Option<&'static str> {
    SUMMARIES
        .iter()
        .find(|(name, _)| *name == method)
        .map(|(_, text)| *text)
}

/// `[{"name", "summary"}]` for every method, plus the reference address.
#[must_use]
pub fn methods() -> Value {
    let list: Vec<Value> = crate::rpc::METHODS
        .iter()
        .map(|name| json!({ "name": name, "summary": summary(name).unwrap_or("") }))
        .collect();
    json!({ "reference": REFERENCE, "methods": list })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_method_has_a_summary() {
        for method in crate::rpc::METHODS {
            assert!(
                super::summary(method).is_some_and(|text| !text.is_empty()),
                "{method} has no summary"
            );
        }
        assert_eq!(super::SUMMARIES.len(), crate::rpc::METHODS.len());
    }
}
