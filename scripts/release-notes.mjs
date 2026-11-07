#!/usr/bin/env node
/**
 * Prints the release notes for one version: the matching CHANGELOG.md
 * section plus a download guide. The in-app update notice shows these notes.
 *
 * Usage: node scripts/release-notes.mjs <version> [--out <file>] [--allow-unreleased]
 *
 * --allow-unreleased falls back to the "Unreleased" section (test builds).
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';

const root = resolve(import.meta.dirname, '..');

/** Returns the body of `## [version]` in a Keep-a-Changelog file, or null. */
export function changelogSection(changelog, version) {
  const lines = changelog.split(/\r?\n/u);
  const escaped = version.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
  const header = new RegExp(`^## \\[${escaped}\\]`, 'u');
  const start = lines.findIndex((line) => header.test(line));
  if (start === -1) {
    return null;
  }
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => /^## /u.test(line) || /^\[[^\]]+\]:/u.test(line));
  return (end === -1 ? rest : rest.slice(0, end)).join('\n').trim();
}

/** Download guide appended to every release. */
export function downloadGuide(version) {
  const file = (suffix) => `\`ElectronicWerk-${version}-${suffix}\``;
  return `## Downloads

| System | File |
| --- | --- |
| Windows (installer) | ${file('win-x64-setup.exe')} · ${file('win-arm64-setup.exe')} |
| Windows (portable, no installation) | ${file('win-x64-portable.exe')} · ${file('win-arm64-portable.exe')} |
| macOS (Apple Silicon and Intel) | ${file('mac-universal.dmg')} |
| macOS (Apple Silicon only / Intel only) | ${file('mac-arm64.dmg')} · ${file('mac-x64.dmg')} |
| Ubuntu, Debian, Linux Mint | ${file('linux-x64.deb')} · ${file('linux-arm64.deb')} |
| Fedora, openSUSE, RHEL | ${file('linux-x64.rpm')} · ${file('linux-arm64.rpm')} |
| Arch Linux, CachyOS, Manjaro | ${file('linux-x64.pacman')} · ${file('linux-arm64.pacman')} |
| Any Linux (AppImage) | ${file('linux-x64.AppImage')} · ${file('linux-arm64.AppImage')} |
| Flatpak / Snap | ${file('linux-x64.flatpak')} · ${file('linux-x64.snap')} (and arm64) |
| Command-line tool \`ewerk\` | \`ewerk-${version}-<system>-<architecture>\` archives |

ElectronicWerk is open source and **not code-signed**. macOS and Windows show a
warning on first start. The [installation guide](https://ujap1994.github.io/ElectronicWerk/guide/installation)
explains how to open the app safely. Verify downloads with \`SHA256SUMS.txt\`.`;
}

const isMain = process.argv[1] !== undefined && resolve(process.argv[1]) === import.meta.filename;
if (isMain) {
  const { positionals, values } = parseArgs({
    allowPositionals: true,
    options: { out: { type: 'string' }, 'allow-unreleased': { type: 'boolean', default: false } },
  });
  const version = positionals[0]?.replace(/^v/u, '');
  if (version === undefined || version === '') {
    console.error('usage: release-notes.mjs <version> [--out <file>]');
    process.exit(2);
  }
  const changelog = readFileSync(join(root, 'CHANGELOG.md'), 'utf8');
  const section =
    changelogSection(changelog, version) ??
    (values['allow-unreleased'] ? changelogSection(changelog, 'Unreleased') : null);
  if (section === null) {
    console.error(`CHANGELOG.md has no section for ${version}`);
    process.exit(1);
  }
  const notes = `${section}\n\n${downloadGuide(version)}\n`;
  if (values.out === undefined) {
    process.stdout.write(notes);
  } else {
    writeFileSync(values.out, notes);
  }
}
