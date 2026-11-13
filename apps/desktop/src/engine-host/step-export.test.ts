/**
 * Reads the STEP assembly that the Rust exporter writes for the blinky
 * fixture (`crates/ew-board3d/tests/fixtures/blinky-assembly.step`, kept in
 * sync by the Rust test `step_export`) with OpenCASCADE, the kernel most
 * mechanical CAD tools build on, and checks what it finds: the board body
 * with its holes and every placed part at its place.
 */
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

import { describe, expect, it } from 'vitest';

import { OCCT_PARAMS, type OcctResult } from '@shared/mesh';

interface OcctNode {
  readonly name?: string;
  readonly meshes: readonly number[];
  readonly children: readonly OcctNode[];
}

interface Occt {
  ReadStepFile: (data: Uint8Array, params: unknown) => OcctResult & { root: OcctNode };
}

function bounds(array: ArrayLike<number>): { min: number[]; max: number[] } {
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i + 2 < array.length; i += 3) {
    for (let axis = 0; axis < 3; axis++) {
      const value = array[i + axis] ?? 0;
      min[axis] = Math.min(min[axis] ?? Infinity, value);
      max[axis] = Math.max(max[axis] ?? -Infinity, value);
    }
  }
  return { min, max };
}

describe('STEP export read back with OpenCASCADE', () => {
  it('finds the board body and the parts where they belong', async () => {
    const require = createRequire(import.meta.url);
    const factory = require('occt-import-js') as () => Promise<Occt>;
    const occt = await factory();
    const file = new URL(
      '../../../../crates/ew-board3d/tests/fixtures/blinky-assembly.step',
      import.meta.url,
    );
    const result = occt.ReadStepFile(new Uint8Array(readFileSync(file)), OCCT_PARAMS);
    expect(result.success).toBe(true);

    const byName = new Map<string, { min: number[]; max: number[] }[]>();
    for (const mesh of result.meshes) {
      const name = mesh.name ?? '';
      const list = byName.get(name) ?? [];
      list.push(bounds(mesh.attributes.position.array));
      byName.set(name, list);
    }
    // The body: 40 × 30 mm at the drill origin, 1.5558 mm of laminate.
    const body = byName.get('blinky_PCB')?.[0];
    expect(body?.min[0]).toBeCloseTo(0, 3);
    expect(body?.min[1]).toBeCloseTo(0, 3);
    expect(body?.max[0]).toBeCloseTo(40, 3);
    expect(body?.max[1]).toBeCloseTo(30, 3);
    expect(body?.max[2]).toBeCloseTo(1.5558, 3);
    // Merged STEP boxes for the three resistors, rotated parts included.
    const resistors = byName.get('R_0805') ?? [];
    expect(resistors).toHaveLength(3);
    for (const box of resistors) {
      // They stand on the top copper (1.5558 + 0.035 mm).
      expect(box.min[2]).toBeCloseTo(1.5908, 3);
    }
    // The capacitors come from a mesh; one of them hangs under the board.
    const capacitors = byName.get('C_0805_2012Metric') ?? [];
    expect(capacitors).toHaveLength(2);
    expect(Math.min(...capacitors.map((box) => box.max[2] ?? 0))).toBeCloseTo(-0.035, 3);
    // The SOIC box sits around the footprint origin (120, 80 + 18).
    const soic = byName.get('SOIC-8')?.[0];
    expect(((soic?.min[0] ?? 0) + (soic?.max[0] ?? 0)) / 2).toBeCloseTo(20, 2);
    expect(((soic?.min[1] ?? 0) + (soic?.max[1] ?? 0)) / 2).toBeCloseTo(18, 2);
    // One assembly with the body and six parts.
    const top = result.root.children[0];
    expect(top?.name).toBe('blinky');
    expect(top?.children).toHaveLength(7);
  });
});
