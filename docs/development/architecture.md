# Architecture

This page summarizes how ElectronicWerk is built. The binding decisions are
recorded as Architecture Decision Records (ADRs) in the repository, starting
with [ADR-0001: Application architecture](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0001-architecture.md).

## Overview

```text
┌──────────────────────── ElectronicWerk desktop app ────────────────────────┐
│                                                                            │
│  Renderer (React UI, sandboxed) ◄──── MessagePort (JSON) ────►  Engine host │
│        ▲                                                       (utility    │
│        │ IPC: settings, status, updates                         process)   │
│        ▼                                                          │        │
│  Main process (Electron): windows, menus, settings,           Node-API     │
│  single instance, security, update check, engine lifecycle        │        │
│                                                                   ▼        │
│                                                   Rust engine (ew-core)    │
└────────────────────────────────────────────────────────────────────────────┘
                     ewerk CLI ──────────► Rust engine (ew-core)
```

| Part           | Technology                | Responsibility                                                                              |
| -------------- | ------------------------- | ------------------------------------------------------------------------------------------- |
| Main process   | Electron, TypeScript      | Windows, menus, settings, security policy, update check, starting and restarting the engine |
| Renderer       | React, TypeScript         | The user interface. It has no access to Node.js or the file system                          |
| Preload script | TypeScript                | The only bridge: a small, typed API (`window.ew`)                                           |
| Engine host    | Electron `utilityProcess` | Loads the native engine and serves requests                                                 |
| Engine         | Rust (`crates/ew-core`)   | Data model, commands and, later, all design algorithms                                      |
| CLI            | Rust (`crates/ew-cli`)    | The same engine for terminals, scripts and AI agents                                        |

## Why a separate engine process?

- **Stability:** if native code crashes, only the engine process ends. The main
  process starts a new one (at most three times within a minute), the window
  stays open and shows a notice.
- **Performance:** the engine is native Rust and can use every processor core.
  The renderer talks to it directly over a `MessagePort`, without a detour
  through the main process.
- **One API:** the app and the CLI call the same engine methods, so every
  feature is scriptable ([Engine API](../reference/engine-api)).

## Security

- The renderer runs in Chromium's sandbox with context isolation and without
  Node.js integration.
- The user interface is loaded from a custom `app://` scheme with a strict
  Content Security Policy instead of from `file://`.
- Every IPC handler checks that the request comes from ElectronicWerk's own
  user interface.
- New windows, embedded web views, navigation to other sites and permission
  requests (camera, microphone, location, …) are denied.
- Links open in the browser only if they point to this project on GitHub.
- Packaged apps use hardened Electron fuses (for example, no "run as Node.js"
  mode and integrity checks for the application archive).

## Code quality

Every change passes the same gates locally (Git hooks) and in CI on Windows,
macOS and Linux:

- **Lint with zero warnings:** ESLint (strict, type-checked), TypeScript strict
  mode, Stylelint, markdownlint, Prettier, rustfmt, Clippy (pedantic),
  cargo-deny, actionlint and commitlint. A custom rule forbids hard-coded UI
  text, so every text exists in German and English.
- **Tests:** Rust unit tests, Vitest unit and integration tests (including the
  real native engine) and Playwright end-to-end tests of the running app.
