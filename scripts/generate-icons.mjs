#!/usr/bin/env node
/**
 * Generates every raster icon from the single SVG master
 * (assets/icon/icon.svg):
 *
 * - apps/desktop/build/icon.png           1024 px, full bleed; electron-builder
 *                                         derives the Windows .ico and Linux icons
 * - apps/desktop/build/icon-macos.png     1024 px on Apple's icon grid (824 px body
 *                                         with shadow); converted to .icns
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
const svg = readFileSync(master, 'utf8');

/**
 * macOS variant: the artwork shrinks to Apple's 824 px icon body and gets a
 * soft drop shadow, so it matches the size of other apps in the Dock.
 */
function macosVariant(source) {
  const inner = source.replace(/^[\s\S]*?<svg[^>]*>/u, '').replace(/<\/svg>\s*$/u, '');
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <defs>
    <filter id="shadow" x="-10%" y="-10%" width="120%" height="125%">
      <feDropShadow dx="0" dy="12" stdDeviation="14" flood-color="#000" flood-opacity="0.3" />
    </filter>
  </defs>
  <g filter="url(#shadow)">
    <svg x="100" y="100" width="824" height="824" viewBox="0 0 1024 1024">${inner}</svg>
  </g>
</svg>`;
}

function render(size, file, source = svg) {
  const png = new Resvg(source, { fitTo: { mode: 'width', value: size } }).render().asPng();
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
render(1024, join(build, 'icon-macos.png'), macosVariant(svg));
for (const size of [16, 24, 32, 48, 64, 128, 256, 512]) {
  render(size, join(build, 'icons', `${size}x${size}.png`));
}

const rendererPublic = join(root, 'apps', 'desktop', 'src', 'renderer', 'public');
copy(join(rendererPublic, 'favicon.svg'));
render(256, join(rendererPublic, 'favicon.png'));

const docsPublic = join(root, 'docs', 'public');
copy(join(docsPublic, 'icon.svg'));
render(256, join(docsPublic, 'icon-256.png'));
