import { existsSync } from 'node:fs';
import { extname, isAbsolute, resolve } from 'node:path';

/** The first `.ewproj` file among command-line arguments, if it exists. */
export function projectFromArgv(argv: readonly string[], cwd: string): string | null {
  for (const argument of argv.slice(1)) {
    if (argument.startsWith('-') || extname(argument).toLowerCase() !== '.ewproj') {
      continue;
    }
    const path = isAbsolute(argument) ? argument : resolve(cwd, argument);
    if (existsSync(path)) {
      return path;
    }
  }
  return null;
}
