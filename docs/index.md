---
layout: home

hero:
  name: ElectronicWerk
  text: Open-source ECAD for makers and hobbyists
  tagline: Schematic, PCB layout, live 3D and manufacturing outputs in one project, on Windows, macOS and Linux.
  image:
    src: /icon.svg
    alt: ElectronicWerk icon
  actions:
    - theme: brand
      text: Get started
      link: /guide/introduction
    - theme: alt
      text: Download
      link: https://github.com/Ujap1994/ElectronicWerk/releases/latest
    - theme: alt
      text: View on GitHub
      link: https://github.com/Ujap1994/ElectronicWerk

features:
  - icon: 📁
    title: One project for everything
    details: Like Altium Designer and KiCad, a project holds schematics, PCB, libraries, rules and output jobs, organised in a project tree.
  - icon: ⚡
    title: Fast and stable engine
    details: The design engine is written in Rust and runs in its own process. If it ever crashes, it restarts on its own and the window stays open.
  - icon: 🔍
    title: Smooth 2D view
    details: Sheets and boards are drawn by the graphics card. A board with 5,000 parts pans and zooms at 60 frames per second, with layers, selection and an inspector.
  - icon: 🛟
    title: Never lose work
    details: Unlimited undo, an autosave journal for every change, crash recovery and atomic saves.
  - icon: 📥
    title: KiCad import
    details: Turn a KiCad 6 to 10 project into an ElectronicWerk project in one step, with a detailed report. Compare and apply netlists from KiCad and CSV.
  - icon: 🔁
    title: Open, git-friendly files
    details: Readable JSON with one object per line, so changes diff cleanly. Open standard formats follow.
  - icon: 🌐
    title: German and English
    details: The user interface follows your system language and can be switched at any time.
  - icon: ⌨️
    title: Keyboard first
    details: A command palette (Cmd/Ctrl+K) reaches every action, with search in both languages.
  - icon: 🤖
    title: Scriptable
    details: The ewerk command-line tool uses the same engine as the app, so scripts and AI agents can drive it.
---

::: warning Early development
ElectronicWerk is at an early stage. Version 0.4 imports KiCad projects,
libraries and netlists, on top of the 2D view and projects with the native
file format, undo, autosave and crash recovery. The editors follow milestone
by milestone. See the
[roadmap](https://github.com/Ujap1994/ElectronicWerk/blob/main/ROADMAP.md).
:::
