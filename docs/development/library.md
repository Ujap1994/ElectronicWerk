# Building the base library

The [base library](../guide/library) that the app downloads is built from
the official KiCad libraries by the **Library** workflow and published as a
separate GitHub release. This page explains the pipeline, the pack format
and how to build and test the library locally. The design is recorded in
[ADR-0005: Base library and deduplication](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0005-base-library.md).

## The pipeline

```text
KiCad libraries (GitLab, tag 10.0.x)
  symbols/  footprints/  3dmodels/*.3dshapes/*.step
        │                          │
        │                          ▼  apps/desktop/scripts/tessellate.ts
        │                    meshes/*.3dshapes/*.glb      (OpenCASCADE, WebAssembly)
        ▼                          │
  ewerk library build ◄────────────┘
  library/  library.sqlite, dedup-report.*, LICENSE.md, meshes/<hash>.glb
        │
        ▼  ewerk library pack
  dist/  …-index.zip, …-meshes-<3D library>.zip (105×), manifest.json
        │
        ▼  gh release create library-kicad-<version>-r<run> (not "latest")
```

1. **Download.** The workflow clones the three KiCad library repositories at
   the chosen tag from <https://gitlab.com/kicad/libraries> (the official
   source; the repositories on GitHub are no longer updated).
2. **Tessellate.** `apps/desktop/scripts/tessellate.ts` turns every STEP model
   into a GLB mesh.
3. **Build.** `ewerk library build` reads symbols, footprints and models,
   fingerprints them, finds duplicates, writes the index and copies the
   meshes into the mesh cache (named by content hash).
4. **Pack.** `ewerk library pack` writes the packs and `manifest.json`.
5. **Publish.** With **publish** ticked, the workflow creates the release
   `library-kicad-<version>-r<run number>` with every pack, never marked as
   the latest release (so the app's update check keeps pointing at app
   releases). The app finds the newest `library-…` release through the
   GitHub API.

Run the workflow from the **Actions** tab (**Library** → **Run workflow**),
with the KiCad library tag (for example `10.0.8`). Without **publish** it
builds everything and keeps the reports (manifest, deduplication report,
tessellation report) as a workflow artifact.

## Tessellating the 3D models

```sh
node apps/desktop/scripts/tessellate.ts <models> <out> [--jobs N] [--filter TEXT]
```

The script runs directly with Node.js 24 (TypeScript type stripping). Every
worker thread loads OpenCASCADE once (`occt-import-js`, a WebAssembly build,
LGPL-2.1) and converts STEP files with the same settings as the app
(`OCCT_PARAMS` in `apps/desktop/src/shared/mesh.ts`): a linear deflection of
0.1 % of the bounding box and an angular deflection of 0.5 rad. The GLB
files are written in metres with y up, as glTF requires, with one primitive
per colour and normals quantized to bytes (`KHR_mesh_quantization`).

- Files that are newer than their STEP file are kept, so a second run only
  converts what changed.
- Workers restart after 100 files (WebAssembly memory never shrinks) and give
  up on a file after three minutes.
- `<out>/tessellate-report.json` lists the number of models, failures,
  triangles, bytes, the time and the ten slowest files.

The same module tessellates STEP files on demand inside the app (a worker
thread of the engine host, `mesh.tessellate`), for libraries built from
local folders.

## Packs

| File                               | Content                                                                                |
| ---------------------------------- | -------------------------------------------------------------------------------------- |
| `<prefix>-index.zip`               | `library.sqlite`, `dedup-report.json`, `dedup-report.md`, `LICENSE.md`                 |
| `<prefix>-meshes-<3D library>.zip` | `meshes/<hash>.glb` for every model of one 3D library (`Resistor_SMD`, …)              |
| `manifest.json`                    | Name, version, sources, entry counts and every pack with its size and SHA-256 checksum |

The index stores every definition compressed (deflate) and without its ids,
which are derived again from the name when an entry is read; it is about
100 MB, 42 MB as a pack. The 105 mesh packs hold about 650 MB of GLB data,
about 220 MB compressed. Mesh files are named by the first 128 bits of their
SHA-256 hash, so identical models are stored once and every file is checked
when a pack is installed.

The app downloads the index pack when the library is set up, and a mesh pack
the first time the user looks at a model of that 3D library. Every download
is checked against the manifest before the engine installs it
(`library.install`).

## Testing

| Test                                 | What it checks                                                                                                                        |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/ew-library` unit tests       | Fingerprints (origin, rotation, names, tolerance), mesh hashes, duplicate classes, decisions                                          |
| `crates/ew-library/tests/fixture.rs` | The synthetic library: groups of every class, search, details, decisions after a rebuild, reproducible report, packs                  |
| `crates/ew-core/tests/library.rs`    | The `library.*` methods: background build with events, previews, decisions, adding to a project                                       |
| `crates/ew-library/tests/budgets.rs` | Search over 50,000 synthetic entries within the CI budget (runs in the **Performance budgets** job)                                   |
| `apps/desktop/e2e/library.spec.ts`   | The library manager: local build, search, 2D and 3D previews (on-demand tessellation), review, adding to a project, downloading packs |
| `crates/ew-library/tests/samples.rs` | Locally, with the real KiCad libraries: every search under 50 ms and a reproducible report                                            |

The synthetic library in `crates/ew-library/tests/fixtures` is generated by
`scripts/make-library-fixture.py` (KiCad files and STEP boxes made from
scratch); its meshes come from the tessellator:

```sh
python3 scripts/make-library-fixture.py
node apps/desktop/scripts/tessellate.ts crates/ew-library/tests/fixtures/kicad/3dmodels \
  crates/ew-library/tests/fixtures/meshes
rm crates/ew-library/tests/fixtures/meshes/tessellate-report.json
```

To check the acceptance criteria against the real libraries:

```sh
node apps/desktop/scripts/tessellate.ts /path/to/kicad/3dmodels /tmp/meshes
EW_KICAD_LIBRARIES=/path/to/kicad EW_KICAD_MESHES=/tmp/meshes \
  cargo test --release -p ew-library --test samples -- --ignored --nocapture
```

See [Benchmarks](./benchmarks#base-library) for the numbers.
