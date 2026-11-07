#!/usr/bin/env node
/**
 * Generates every raster icon from the single SVG master
 * (assets/icon/icon.svg):
 *
 * - apps/desktop/build/icon.png           1024 px, electron-builder derives
 *                                         .icns (macOS) and .ico (Windows)
 * - apps/desktop/build/icons/<size>.png   Linux icon theme sizes
 * - apps/desktop/src/renderer/public/favicon.svg / favicon.png  UI and window icon
 * - docs/public/icon.svg / icon-256.png   documentation and README
 *
 * Usage: node scripts/generate-icons.mjs
 */
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

import { Resvg } from '@resvg/resvg-js';

const root = resolve(import.meta.dirname, '..');
const master = join(root, 'assets', 'icon', 'icon.svg');
const svg = readFileSync(master);

function render(size, file) {
  const png = new Resvg(svg, { fitTo: { mode: 'width', value: size } }).render().asPng();
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, png);
  console.log(`✓ ${file.slice(root.length + 1)} (${size}px)`);
}

function copy(file) {
  mkdirSync(dirname(file), { recursive: true });
  copyFileSync(master, file);
  console.log(`✓ ${file.slice(root.length + 1)}`);
}

const build = join(root, 'apps', 'desktop', 'build');
render(1024, join(build, 'icon.png'));
for (const size of [16, 24, 32, 48, 64, 128, 256, 512]) {
  render(size, join(build, 'icons', `${size}x${size}.png`));
}

const rendererPublic = join(root, 'apps', 'desktop', 'src', 'renderer', 'public');
copy(join(rendererPublic, 'favicon.svg'));
render(256, join(rendererPublic, 'favicon.png'));

const docsPublic = join(root, 'docs', 'public');
copy(join(docsPublic, 'icon.svg'));
render(256, join(docsPublic, 'icon-256.png'));
