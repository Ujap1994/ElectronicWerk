# Changelog

All notable changes to ElectronicWerk are documented in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Desktop app shell (Electron) with secure defaults: sandboxed renderer,
  context isolation, a strict Content Security Policy and a custom `app://`
  scheme.
- Single-instance lock; closing the window quits the app on every platform.
- Rust engine (`ew-core`) running in a crash-isolated `utilityProcess`, with
  automatic restart and a direct MessagePort channel to the UI.
- `ewerk` command-line interface with `info`, `methods` and `call`.
- Start screen, status bar with live engine status, and a settings dialog
  (language and appearance).
- German and English UI, following the system language by default and
  switchable at runtime.
- Light and dark appearance, following the system by default.
- Lint gates with zero warnings (ESLint strict type-checked, TypeScript strict,
  Stylelint, markdownlint, Prettier, rustfmt, Clippy pedantic, cargo-deny,
  actionlint, commitlint) and a custom rule against hard-coded UI text.
- Test stack: Rust unit tests, Vitest unit and integration tests, and
  Playwright end-to-end tests for Electron.
- CI on Windows, macOS and Linux.
