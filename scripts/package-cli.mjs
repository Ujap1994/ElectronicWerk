#!/usr/bin/env node
/**
 * Builds the `ewerk` command-line tool for one target and packs it into
 * apps/desktop/release/ewerk-<version>-<os>-<arch>.(tar.gz|zip).
 *
 * Usage: node scripts/package-cli.mjs --target <rust-triple>
 */
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';

const root = resolve(import.meta.dirname, '..');
const { values } = parseArgs({ options: { target: { type: 'string' } } });
if (values.target === undefined) {
  console.error('usage: package-cli.mjs --target <rust-triple>');
  process.exit(2);
}

/** Rust target triple → release file naming (matches electron-builder). */
const NAMES = {
  'aarch64-apple-darwin': ['mac', 'arm64'],
  'x86_64-apple-darwin': ['mac', 'x64'],
  'x86_64-pc-windows-msvc': ['win', 'x64'],
  'aarch64-pc-windows-msvc': ['win', 'arm64'],
  'x86_64-unknown-linux-gnu': ['linux', 'x64'],
  'aarch64-unknown-linux-gnu': ['linux', 'arm64'],
};
const naming = NAMES[values.target];
if (naming === undefined) {
  console.error(`unsupported target: ${values.target}`);
  process.exit(2);
}
const [os, arch] = naming;
const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
const windows = os === 'win';
const binary = windows ? 'ewerk.exe' : 'ewerk';

execFileSync('cargo', ['build', '-p', 'ew-cli', '--release', '--target', values.target], {
  cwd: root,
  stdio: 'inherit',
});

const name = `ewerk-${version}-${os}-${arch}`;
const releaseDir = join(root, 'apps', 'desktop', 'release');
const staging = join(releaseDir, name);
rmSync(staging, { recursive: true, force: true });
mkdirSync(staging, { recursive: true });
copyFileSync(join(root, 'target', values.target, 'release', binary), join(staging, binary));
copyFileSync(join(root, 'LICENSE'), join(staging, 'LICENSE'));

const archive = join(releaseDir, windows ? `${name}.zip` : `${name}.tar.gz`);
// bsdtar (Windows 10+) writes zip archives with -a; GNU/BSD tar writes tar.gz.
const args = windows ? ['-a', '-cf', archive, name] : ['-czf', archive, name];
execFileSync('tar', args, { cwd: releaseDir, stdio: 'inherit' });
rmSync(staging, { recursive: true, force: true });
console.log(`✓ ${archive}`);
