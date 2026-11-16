/**
 * The design rules as the engine sends them (`pcb.rules`) and takes them
 * back (`pcb.setRules`), and the impedance view of a board
 * (`pcb.impedance`). Lengths are in nanometres.
 */

export type Severity = 'error' | 'warning' | 'ignore';

export interface NetClass {
  name: string;
  description?: string;
  clearance: number;
  trackWidth: number;
  viaDiameter: number;
  viaDrill: number;
  microViaDiameter: number;
  microViaDrill: number;
  diffPairWidth: number;
  diffPairGap: number;
  /** The impedance profile its tracks follow (missing: none). */
  impedance?: string;
}

export interface Constraints {
  minClearance: number;
  minTrackWidth: number;
  minViaDiameter: number;
  minViaDrill: number;
  minAnnularRing: number;
  minHole: number;
  minHoleToHole: number;
  minCopperToEdge: number;
  minSilkWidth: number;
  minMicroViaDiameter: number;
  minMicroViaDrill: number;
  /** Missing when off. */
  allowBlindBuriedVias?: boolean;
  allowMicroVias?: boolean;
}

export type RuleConstraints = Partial<{
  clearance: number;
  trackWidthMin: number;
  trackWidthMax: number;
  trackWidth: number;
  viaDiameterMin: number;
  viaDrillMin: number;
  annularRingMin: number;
  holeMin: number;
  holeToHoleMin: number;
  edgeClearance: number;
}>;

export interface CustomRule {
  name: string;
  enabled?: boolean;
  condition?: string;
  constraints: RuleConstraints;
}

/** Width and gap of an impedance profile on one layer, set by hand. */
export interface TrackGeometry {
  width: number;
  gap: number;
}

/** A target impedance that net classes follow. */
export interface ImpedanceProfile {
  name: string;
  /** Missing for single tracks. */
  differential?: boolean;
  /** Ω. */
  target: number;
  /** %. */
  tolerance: number;
  /** The interface it was made from in the beginner mode (`usb2`, …). */
  preset?: string;
  /** Sizes set by hand, by copper layer (`cu.top`, …). */
  layers?: Record<string, TrackGeometry>;
}

export interface RulesProps {
  name: string;
  constraints: Constraints;
  netClasses: NetClass[];
  assignments: { pattern: string; class: string }[];
  erc: unknown;
  custom: CustomRule[];
  drc: { severities?: Record<string, Severity> };
  impedanceProfiles?: ImpedanceProfile[];
}

export interface RulesReply {
  readonly rules: RulesProps;
  readonly checks: readonly { code: string; default: Severity; severity: Severity }[];
  readonly nets: Readonly<Record<string, readonly string[]>>;
}

/** How a copper layer's tracks sit between their planes. */
export type Structure =
  | {
      readonly kind: 'microstrip';
      readonly height: number;
      readonly epsilonR: number;
      readonly thickness: number;
    }
  | {
      readonly kind: 'stripline';
      readonly above: number;
      readonly below: number;
      readonly epsilonR: number;
      readonly thickness: number;
    };

/** A profile on one layer. */
export interface Layout {
  readonly width: number;
  readonly gap: number;
  /** Ω. */
  readonly impedance: number;
  /** Set by hand (otherwise computed). */
  readonly manual: boolean;
  /** Within the board minimums. */
  readonly fits: boolean;
}

/** An interface of the beginner mode. */
export interface Preset {
  readonly id: string;
  readonly name: string;
  readonly differential: boolean;
  readonly target: number;
  readonly tolerance: number;
}

export interface ImpedanceTable {
  readonly presets: readonly Preset[];
  readonly layers: readonly { readonly layer: string; readonly structure: Structure | null }[];
  readonly profiles: readonly {
    readonly profile: ImpedanceProfile;
    readonly layers: readonly { readonly layer: string; readonly layout: Layout | null }[];
  }[];
}

/** `true` if an impedance is within the profile's tolerance. */
export function withinTolerance(profile: ImpedanceProfile, impedance: number): boolean {
  return Math.abs(impedance - profile.target) <= (profile.target * profile.tolerance) / 100;
}

/** A net class that follows no impedance profile. */
function withoutProfile(cls: NetClass): NetClass {
  const next = { ...cls };
  delete next.impedance;
  return next;
}

/**
 * Sets the impedance profile of a net class and drops the profiles the
 * beginner mode made that no class uses any more. Profiles made or changed
 * by hand stay.
 */
export function assignProfile(rules: RulesProps, className: string, profile: string): RulesProps {
  const netClasses = rules.netClasses.map((cls): NetClass => {
    if (cls.name !== className) {
      return cls;
    }
    if (profile === '') {
      return withoutProfile(cls);
    }
    return { ...cls, impedance: profile };
  });
  const used = new Set(netClasses.map((cls) => cls.impedance));
  const impedanceProfiles = (rules.impedanceProfiles ?? []).filter(
    (p) =>
      used.has(p.name) ||
      p.preset === undefined ||
      p.preset === '' ||
      Object.keys(p.layers ?? {}).length > 0,
  );
  return { ...rules, netClasses, impedanceProfiles };
}

/**
 * The profile of a beginner interface: the one made from it before, or a new
 * one named after it.
 */
export function presetProfile(
  rules: RulesProps,
  preset: Preset,
): { readonly rules: RulesProps; readonly name: string } {
  const profiles = rules.impedanceProfiles ?? [];
  const existing = profiles.find((p) => p.preset === preset.id);
  if (existing !== undefined) {
    return { rules, name: existing.name };
  }
  const taken = new Set(profiles.map((p) => p.name));
  let name = preset.name;
  for (let n = 2; taken.has(name); n += 1) {
    name = `${preset.name} ${n.toString()}`;
  }
  const profile: ImpedanceProfile = {
    name,
    target: preset.target,
    tolerance: preset.tolerance,
    preset: preset.id,
    ...(preset.differential ? { differential: true } : {}),
  };
  return { rules: { ...rules, impedanceProfiles: [...profiles, profile] }, name };
}

/** Renames a profile and the net classes that follow it. */
export function renameProfile(rules: RulesProps, from: string, to: string): RulesProps {
  return {
    ...rules,
    netClasses: rules.netClasses.map((cls) =>
      cls.impedance === from ? { ...cls, impedance: to } : cls,
    ),
    impedanceProfiles: (rules.impedanceProfiles ?? []).map((p) =>
      p.name === from ? { ...p, name: to } : p,
    ),
  };
}

/** Removes a profile; the classes that followed it follow none. */
export function removeProfile(rules: RulesProps, name: string): RulesProps {
  return {
    ...rules,
    netClasses: rules.netClasses.map((cls): NetClass => {
      return cls.impedance === name ? withoutProfile(cls) : cls;
    }),
    impedanceProfiles: (rules.impedanceProfiles ?? []).filter((p) => p.name !== name),
  };
}

/**
 * The beginner mode applies a profile's sizes on the top layer to a net
 * class (or every class with a profile), so exports and tools that know
 * only one width per class get them too.
 */
export function applyLayouts(rules: RulesProps, table: ImpedanceTable, only?: string): RulesProps {
  const netClasses = rules.netClasses.map((cls) => {
    if (only !== undefined && cls.name !== only) {
      return cls;
    }
    const row = table.profiles.find((p) => p.profile.name === cls.impedance);
    const top = row?.layers[0]?.layout ?? null;
    if (row === undefined || top === null) {
      return cls;
    }
    const next =
      row.profile.differential === true
        ? { ...cls, trackWidth: top.width, diffPairWidth: top.width, diffPairGap: top.gap }
        : { ...cls, trackWidth: top.width };
    if (
      next.trackWidth === cls.trackWidth &&
      next.diffPairWidth === cls.diffPairWidth &&
      next.diffPairGap === cls.diffPairGap
    ) {
      return cls;
    }
    return next;
  });
  const changed = netClasses.some((cls, index) => cls !== rules.netClasses[index]);
  return changed ? { ...rules, netClasses } : rules;
}
