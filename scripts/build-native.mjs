#!/usr/bin/env node
/**
 * Builds the native engine addon (crates/ew-node) and copies it to
 * apps/desktop/native/ew_node.<platform>-<arch>.node.
 *
 * Usage:
 *   node scripts/build-native.mjs [--release] [--target <rust-triple>] [--universal]
 *
 * --universal (macOS only) builds x64 and arm64 and merges them with lipo into
 * ew_node.darwin-universal.node.
 */
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';

const root = resolve(import.meta.dirname, '..');
const outDir = join(root, 'apps', 'desktop', 'native');

const { values } = parseArgs({
  options: {
    release: { type: 'boolean', default: false },
    target: { type: 'string' },
    universal: { type: 'boolean', default: false },
  },
});

/** Rust target triple → Node platform/arch. */
const TARGETS = {
  'aarch64-apple-darwin': ['darwin', 'arm64'],
  'x86_64-apple-darwin': ['darwin', 'x64'],
  'x86_64-pc-windows-msvc': ['win32', 'x64'],
  'aarch64-pc-windows-msvc': ['win32', 'arm64'],
  'x86_64-unknown-linux-gnu': ['linux', 'x64'],
  'aarch64-unknown-linux-gnu': ['linux', 'arm64'],
};

function hostTarget() {
  const output = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
  const line = output.split('\n').find((entry) => entry.startsWith('host: '));
  if (line === undefined) {
    throw new Error('could not determine the Rust host target');
  }
  return line.slice('host: '.length).trim();
}

function libraryFileName(platform) {
  if (platform === 'win32') return 'ew_node.dll';
  if (platform === 'darwin') return 'libew_node.dylib';
  return 'libew_node.so';
}

function build(target) {
  const mapping = TARGETS[target];
  if (mapping === undefined) {
    throw new Error(`unsupported target: ${target}`);
  }
  const [platform, arch] = mapping;
  const profile = values.release ? 'release' : 'debug';
  const args = ['build', '-p', 'ew-node', '--target', target];
  if (values.release) args.push('--release');
  console.log(`> cargo ${args.join(' ')}`);
  execFileSync('cargo', args, { cwd: root, stdio: 'inherit' });
  const built = join(root, 'target', target, profile, libraryFileName(platform));
  return { built, platform, arch };
}

mkdirSync(outDir, { recursive: true });

if (values.universal) {
  if (process.platform !== 'darwin') {
    throw new Error('--universal is only supported on macOS');
  }
  const parts = ['aarch64-apple-darwin', 'x86_64-apple-darwin'].map(
    (target) => build(target).built,
  );
  const output = join(outDir, 'ew_node.darwin-universal.node');
  rmSync(output, { force: true });
  execFileSync('lipo', ['-create', '-output', output, ...parts], { stdio: 'inherit' });
  console.log(`✓ ${output}`);
} else {
  const { built, platform, arch } = build(values.target ?? hostTarget());
  const output = join(outDir, `ew_node.${platform}-${arch}.node`);
  copyFileSync(built, output);
  console.log(`✓ ${output}`);
}
