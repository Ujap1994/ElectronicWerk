import { describe, expect, it } from 'vitest';

import {
  applyLayouts,
  assignProfile,
  presetProfile,
  removeProfile,
  renameProfile,
  withinTolerance,
  type ImpedanceTable,
  type NetClass,
  type Preset,
  type RulesProps,
} from './rules-types';

function netClass(name: string, impedance?: string): NetClass {
  return {
    name,
    clearance: 200_000,
    trackWidth: 250_000,
    viaDiameter: 600_000,
    viaDrill: 300_000,
    microViaDiameter: 300_000,
    microViaDrill: 100_000,
    diffPairWidth: 200_000,
    diffPairGap: 250_000,
    ...(impedance === undefined ? {} : { impedance }),
  };
}

function rules(classes: NetClass[], profiles: RulesProps['impedanceProfiles'] = []): RulesProps {
  return {
    name: 'Rules',
    constraints: {
      minClearance: 150_000,
      minTrackWidth: 150_000,
      minViaDiameter: 500_000,
      minViaDrill: 300_000,
      minAnnularRing: 100_000,
      minHole: 300_000,
      minHoleToHole: 250_000,
      minCopperToEdge: 300_000,
      minSilkWidth: 120_000,
      minMicroViaDiameter: 200_000,
      minMicroViaDrill: 100_000,
    },
    netClasses: classes,
    assignments: [],
    erc: {},
    custom: [],
    drc: {},
    impedanceProfiles: profiles,
  };
}

const USB: Preset = {
  id: 'usb2',
  name: 'USB 2.0',
  differential: true,
  target: 90,
  tolerance: 10,
  skew: 1250,
};

describe('impedance profiles in the rules', () => {
  it('makes a profile for an interface once and reuses it', () => {
    const start = rules([netClass('Default'), netClass('USB'), netClass('USB2')]);
    const first = presetProfile(start, USB);
    expect(first.name).toBe('USB 2.0');
    expect(first.rules.impedanceProfiles).toEqual([
      { name: 'USB 2.0', target: 90, tolerance: 10, preset: 'usb2', differential: true },
    ]);
    const assigned = assignProfile(first.rules, 'USB', first.name);
    const again = presetProfile(assigned, USB);
    expect(again.rules).toBe(assigned);
    expect(again.name).toBe('USB 2.0');
  });

  it('gives a new profile a free name', () => {
    const start = rules([netClass('Default')], [{ name: 'USB 2.0', target: 85, tolerance: 5 }]);
    expect(presetProfile(start, USB).name).toBe('USB 2.0 2');
  });

  it('drops interface profiles nobody uses but keeps hand-made ones', () => {
    const start = rules(
      [netClass('Default'), netClass('USB', 'USB 2.0'), netClass('RF', 'Antenna')],
      [
        { name: 'USB 2.0', target: 90, tolerance: 10, preset: 'usb2', differential: true },
        { name: 'Antenna', target: 50, tolerance: 5 },
      ],
    );
    const cleared = assignProfile(assignProfile(start, 'USB', ''), 'RF', '');
    expect(cleared.netClasses.every((cls) => cls.impedance === undefined)).toBe(true);
    expect(cleared.impedanceProfiles?.map((p) => p.name)).toEqual(['Antenna']);
  });

  it('renames and removes a profile together with its classes', () => {
    const start = rules(
      [netClass('Default'), netClass('RF', 'Antenna')],
      [{ name: 'Antenna', target: 50, tolerance: 5 }],
    );
    const renamed = renameProfile(start, 'Antenna', '50 ohm');
    expect(renamed.netClasses[1]?.impedance).toBe('50 ohm');
    expect(renamed.impedanceProfiles?.[0]?.name).toBe('50 ohm');
    const removed = removeProfile(renamed, '50 ohm');
    expect(removed.netClasses[1]).not.toHaveProperty('impedance');
    expect(removed.impedanceProfiles).toEqual([]);
  });

  it('applies the top layer sizes to a class', () => {
    const profile = {
      name: 'USB 2.0',
      target: 90,
      tolerance: 10,
      differential: true,
      preset: 'usb2',
    };
    const start = rules([netClass('Default'), netClass('USB', 'USB 2.0')], [profile]);
    const table: ImpedanceTable = {
      presets: [USB],
      layers: [],
      profiles: [
        {
          profile,
          layers: [
            {
              layer: 'cu.top',
              layout: { width: 180_000, gap: 160_000, impedance: 90.2, manual: false, fits: true },
            },
          ],
        },
      ],
    };
    const applied = applyLayouts(start, table, 'USB');
    expect(applied.netClasses[1]).toMatchObject({
      trackWidth: 180_000,
      diffPairWidth: 180_000,
      diffPairGap: 160_000,
      maxSkew: 1_250_000,
    });
    expect(applied.netClasses[0]).toBe(start.netClasses[0]);
    // Nothing left to change: the same rules come back.
    expect(applyLayouts(applied, table)).toBe(applied);
  });

  it('checks the tolerance', () => {
    const profile = { name: 'P', target: 90, tolerance: 10 };
    expect(withinTolerance(profile, 99)).toBe(true);
    expect(withinTolerance(profile, 81)).toBe(true);
    expect(withinTolerance(profile, 99.5)).toBe(false);
  });
});
