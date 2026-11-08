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
  - icon: 🛟
    title: Never lose work
    details: Unlimited undo, an autosave journal for every change, crash recovery and atomic saves.
  - icon: 🔁
    title: Open, git-friendly files
    details: Readable JSON with one object per line, so changes diff cleanly. KiCad import and open standard formats follow.
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
ElectronicWerk is at an early stage. Version 0.2 provides projects on top of
the application foundation: the project wizard and tree, the native file
format, undo, autosave and crash recovery. The editors follow milestone by
milestone. See the
[roadmap](https://github.com/Ujap1994/ElectronicWerk/blob/main/ROADMAP.md).
:::
