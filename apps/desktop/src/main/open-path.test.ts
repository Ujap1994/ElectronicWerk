import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { projectFromArgv } from './open-path';

describe('projectFromArgv', () => {
  it('finds an existing project file among the arguments', () => {
    const dir = mkdtempSync(join(tmpdir(), 'ew-argv-'));
    writeFileSync(join(dir, 'Board.EWPROJ'), '{}');
    expect(projectFromArgv(['electron', '--flag', 'Board.EWPROJ'], dir)).toBe(
      join(dir, 'Board.EWPROJ'),
    );
    expect(projectFromArgv(['electron', join(dir, 'Board.EWPROJ')], '/')).toBe(
      join(dir, 'Board.EWPROJ'),
    );
    expect(projectFromArgv(['electron', 'missing.ewproj'], dir)).toBeNull();
    expect(projectFromArgv(['electron', '.'], dir)).toBeNull();
    expect(projectFromArgv(['Board.ewproj'], dir)).toBeNull();
  });
});
