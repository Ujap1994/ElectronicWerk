/** Board stack-ups as the engine stores them, and the standard stack-up. */

export type StackupLayer =
  | { type: 'silkscreen'; side: string; color?: string }
  | { type: 'solderMask'; side: string; thickness: number; color?: string }
  | { type: 'copper'; layer: string; name?: string; thickness: number }
  | {
      type: 'dielectric';
      kind: 'core' | 'prepreg';
      thickness: number;
      material: string;
      epsilonR: number;
      lossTangent: number;
    };

export interface Stackup {
  layers: StackupLayer[];
  finish?: string;
}

/** Copper layer keys of a stack-up of `count` layers. */
function copperKeys(count: number): string[] {
  if (count <= 1) {
    return ['cu.top'];
  }
  return [
    'cu.top',
    ...Array.from({ length: count - 2 }, (_, i) => `cu.in${(i + 1).toString()}`),
    'cu.bottom',
  ];
}

/** A standard stack-up like the engine builds it (`Stackup::standard`). */
export function standardStackup(count: number, thickness: number, finish = 'HASL'): Stackup {
  const copper = copperKeys(count);
  const dielectrics = Math.max(copper.length - 1, 1);
  const masks = copper.length > 1 ? 2 : 1;
  const fixed = copper.length * 35_000 + masks * 10_000;
  const available = Math.max(thickness - fixed, dielectrics * 50_000);
  const each = Math.floor(available / dielectrics);
  const first = available - each * dielectrics;
  const layers: StackupLayer[] = [
    { type: 'silkscreen', side: 'top', color: 'white' },
    { type: 'solderMask', side: 'top', thickness: 10_000, color: 'green' },
  ];
  copper.forEach((layer, index) => {
    layers.push({ type: 'copper', layer, thickness: 35_000 });
    if (index + 1 < copper.length || copper.length === 1) {
      layers.push({
        type: 'dielectric',
        kind: copper.length <= 2 || index % 2 === 1 ? 'core' : 'prepreg',
        thickness: each + (index === 0 ? first : 0),
        material: 'FR4',
        epsilonR: 4.5,
        lossTangent: 0.02,
      });
    }
  });
  if (copper.length > 1) {
    layers.push(
      { type: 'solderMask', side: 'bottom', thickness: 10_000, color: 'green' },
      { type: 'silkscreen', side: 'bottom', color: 'white' },
    );
  }
  return { layers, finish };
}

export function thicknessOf(stackup: Stackup): number {
  return stackup.layers.reduce(
    (sum, layer) => sum + ('thickness' in layer ? layer.thickness : 0),
    0,
  );
}
