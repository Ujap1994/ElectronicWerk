import { existsSync } from 'node:fs';
import { join } from 'node:path';

/**
 * File names the native engine may have in `directory`, best match first:
 * the exact platform/architecture build, then a macOS universal build.
 */
export function addonCandidates(
  directory: string,
  platform: string = process.platform,
  arch: string = process.arch,
): string[] {
  return [
    join(directory, `ew_node.${platform}-${arch}.node`),
    join(directory, `ew_node.${platform}-universal.node`),
  ];
}

/** Returns the first existing native engine file, or `undefined`. */
export function findAddon(directory: string): string | undefined {
  return addonCandidates(directory).find((candidate) => existsSync(candidate));
}
