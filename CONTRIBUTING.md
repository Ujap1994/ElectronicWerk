# Contributing to ElectronicWerk

Thank you for helping. This guide explains how to set up a development
environment and which rules every change must follow.

## Requirements

| Tool    | Version                                            |
| ------- | -------------------------------------------------- |
| Node.js | 22.12 or newer (CI uses 24)                        |
| npm     | 10 or newer                                        |
| Rust    | stable, installed with [rustup](https://rustup.rs) |
| Git     | any recent version                                 |

On Linux, the end-to-end tests need a display server. CI uses `xvfb-run`.

## Getting started

```sh
git clone https://github.com/Ujap1994/ElectronicWerk.git
cd ElectronicWerk
npm install          # also installs the Git hooks (lefthook)
npm run dev          # builds the Rust engine and starts the app with hot reload
```

## Project layout

| Path                           | Content                                                    |
| ------------------------------ | ---------------------------------------------------------- |
| `crates/ew-core`               | Rust engine: data model, commands, RPC dispatch            |
| `crates/ew-cli`                | `ewerk` command-line interface                             |
| `crates/ew-node`               | Node-API bridge that loads the engine into the desktop app |
| `apps/desktop/src/main`        | Electron main process                                      |
| `apps/desktop/src/preload`     | Preload script (`window.ew` API)                           |
| `apps/desktop/src/engine-host` | Engine host (`utilityProcess`)                             |
| `apps/desktop/src/renderer`    | React user interface                                       |
| `apps/desktop/src/shared`      | Code shared by all processes (i18n, settings, types)       |
| `apps/desktop/e2e`             | Playwright end-to-end tests                                |
| `docs/`                        | Documentation and architecture decision records (`adr/`)   |
| `tools/`                       | Project tooling, for example the custom ESLint rules       |

## Commands

| Command                | What it does                                   |
| ---------------------- | ---------------------------------------------- |
| `npm run dev`          | Start the app in development mode              |
| `npm run build`        | Release build of the engine and the app        |
| `npm run build:native` | Build only the Rust engine addon               |
| `npm run lint`         | Every lint check (must report zero warnings)   |
| `npm run format`       | Auto-format TypeScript, CSS, Markdown and Rust |
| `npm test`             | Rust, tooling, unit and integration tests      |
| `npm run test:e2e`     | End-to-end tests (build the app first)         |

## Rules for every change

1. **Lint is 100 % clean.** No new warnings, and no `eslint-disable` or
   `#[allow]` without a written reason.
2. **Tests.** New behaviour comes with unit tests and, where it is visible in
   the UI, an end-to-end test.
3. **Both languages.** Every UI text goes into `apps/desktop/src/shared/i18n`
   in English and German. Hard-coded text in JSX fails the lint.
4. **Documentation.** Every user-facing feature gets a documentation page with
   screenshots. Screenshots only show synthetic, anonymized demo data.
5. **Conventional Commits.** For example `feat: add layer panel` or
   `fix(engine): handle empty params`. The changelog is generated from them.
6. **Roadmap.** Update [ROADMAP.md](ROADMAP.md) when a feature changes status.

## Privacy

Never commit real customer or personal projects. Test fixtures and
documentation examples must be synthetic.
