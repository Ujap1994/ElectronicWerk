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
    details: Like Altium Designer and KiCad, a project holds schematics, PCB, libraries, rules and output jobs. (Planned for v0.2.)
  - icon: ⚡
    title: Fast and stable engine
    details: The design engine is written in Rust and runs in its own process. If it ever crashes, it restarts on its own and the window stays open.
  - icon: 🔁
    title: Open formats and KiCad import
    details: Text-based, git-friendly project files, plus import of KiCad projects and open standard formats. (Planned.)
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
ElectronicWerk is at an early stage. Version 0.1 provides the application
foundation: the secure app shell, the engine, settings, the update notice and
the command palette. The editors follow milestone by milestone. See the
[roadmap](https://github.com/Ujap1994/ElectronicWerk/blob/main/ROADMAP.md).
:::
