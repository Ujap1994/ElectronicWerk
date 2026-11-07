#!/usr/bin/env node
/**
 * Keeps the version in sync across package.json, apps/desktop/package.json
 * and the Rust workspace (Cargo.toml).
 *
 * Usage:
 *   node scripts/version.mjs check [expected]   fail if versions differ (or differ from `expected`)
 *   node scripts/version.mjs set <version>      write a new version everywhere
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u;
const packageFiles = ['package.json', 'apps/desktop/package.json'];
const cargoFile = 'Cargo.toml';
const cargoPattern = /(\[workspace\.package\][^[]*?\nversion = ")([^"]+)(")/u;

function readVersions() {
  const versions = {};
  for (const file of packageFiles) {
    versions[file] = JSON.parse(readFileSync(join(root, file), 'utf8')).version;
  }
  const cargo = readFileSync(join(root, cargoFile), 'utf8').match(cargoPattern);
  versions[cargoFile] = cargo?.[2];
  return versions;
}

const [command, argument] = process.argv.slice(2);

if (command === 'check') {
  const versions = readVersions();
  const expected = argument?.replace(/^v/u, '') ?? versions['package.json'];
  const wrong = Object.entries(versions).filter(([, version]) => version !== expected);
  if (wrong.length > 0) {
    console.error(`Expected version ${expected}, but found:`);
    for (const [file, version] of wrong) console.error(`  ${file}: ${String(version)}`);
    process.exit(1);
  }
  console.log(`✓ all versions are ${expected}`);
} else if (command === 'set' && argument !== undefined && SEMVER.test(argument)) {
  for (const file of packageFiles) {
    const path = join(root, file);
    const json = JSON.parse(readFileSync(path, 'utf8'));
    json.version = argument;
    writeFileSync(path, `${JSON.stringify(json, null, 2)}\n`);
  }
  const path = join(root, cargoFile);
  writeFileSync(path, readFileSync(path, 'utf8').replace(cargoPattern, `$1${argument}$3`));
  console.log(
    `✓ version set to ${argument}; run "npm install" and "cargo check" to update the lock files`,
  );
} else {
  console.error('usage: version.mjs check [expected] | set <version>');
  process.exit(2);
}
