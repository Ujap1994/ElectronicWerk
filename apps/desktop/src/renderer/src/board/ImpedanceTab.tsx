/**
 * The impedance tab of the design rules. In the beginner working mode the
 * user picks the interface a net class carries (USB, Ethernet, …) and the
 * app computes the track width and gap for every copper layer from the
 * stack-up and applies them; in the advanced mode the user edits impedance
 * profiles (target, tolerance, widths and gaps per layer), sees the
 * impedance they give and has a free calculator.
 */
import { useEffect, useState, type ReactNode } from 'react';

import { formatLength, type Units } from '@shared/project';
import type { WorkMode } from '@shared/settings';

import { LengthInput, NumberInput, SelectInput, TextInput } from '../libedit/fields';
import { useAppState } from '../state';
import { layerName } from './BoardToolbar';
import {
  applyLayouts,
  assignProfile,
  presetProfile,
  removeProfile,
  renameProfile,
  withinTolerance,
  type ImpedanceProfile,
  type ImpedanceTable,
  type Layout,
  type RulesProps,
  type Structure,
} from './rules-types';

type Translate = ReturnType<typeof useAppState>['t'];

interface TabProps {
  readonly doc: string;
  readonly units: Units;
  readonly mode: WorkMode;
  readonly rules: RulesProps;
  /** Changes the edited rules (applied to the newest state). */
  readonly update: (change: (rules: RulesProps) => RulesProps) => void;
}

/** The impedance view of the edited rules, recomputed as they change. */
function useImpedanceTable(doc: string, rules: RulesProps): ImpedanceTable | null {
  const { engine } = useAppState();
  const [table, setTable] = useState<ImpedanceTable | null>(null);
  useEffect(() => {
    let cancelled = false;
    const timer = window.setTimeout(() => {
      engine
        .call<ImpedanceTable>('pcb.impedance', { doc, rules })
        .then((next) => {
          if (!cancelled) {
            setTable(next);
          }
        })
        .catch(() => undefined);
    }, 120);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [engine, doc, rules]);
  return table;
}

function ohms(value: number): string {
  return `${value.toFixed(1)} Ω`;
}

/** A short description of a layer's structure. */
function structureText(
  t: Translate,
  structure: Structure | null,
  units: Units,
  language: string,
): string {
  if (structure === null) {
    return t('board.rules.impedance.noPlane');
  }
  const length = (nm: number): string => formatLength(nm, units, language);
  const epsilon = structure.epsilonR.toFixed(2);
  return structure.kind === 'microstrip'
    ? t('board.rules.impedance.microstrip', { height: length(structure.height), epsilon })
    : t('board.rules.impedance.stripline', {
        above: length(structure.above),
        below: length(structure.below),
        epsilon,
      });
}

/** Whether a layout meets its profile, as a short status. */
function status(
  t: Translate,
  profile: ImpedanceProfile,
  layout: Layout | null,
): { readonly text: string; readonly ok: boolean } {
  if (layout === null) {
    return { text: t('board.rules.impedance.unreachable'), ok: false };
  }
  if (!withinTolerance(profile, layout.impedance)) {
    return { text: t('board.rules.impedance.offTarget'), ok: false };
  }
  if (!layout.fits) {
    return { text: t('board.rules.impedance.belowMinimum'), ok: false };
  }
  return {
    text: layout.manual ? t('board.rules.impedance.manual') : t('board.rules.impedance.computed'),
    ok: true,
  };
}

/** The sizes of one profile on every copper layer. */
function LayerTable({
  units,
  editable,
  index,
  profile,
  table,
  update,
}: {
  readonly units: Units;
  readonly editable: boolean;
  readonly index: number;
  readonly profile: ImpedanceProfile;
  readonly table: ImpedanceTable;
  readonly update: TabProps['update'];
}): ReactNode {
  const { t, snapshot } = useAppState();
  const language = snapshot.settings.language;
  const row = table.profiles.find((p) => p.profile.name === profile.name);
  const differential = profile.differential === true;
  const length = (nm: number): string => formatLength(nm, units, language);
  const setLayer = (layer: string, width: number, gap: number): void => {
    update((rules) => ({
      ...rules,
      impedanceProfiles: (rules.impedanceProfiles ?? []).map((p) =>
        p.name === profile.name ? { ...p, layers: { ...p.layers, [layer]: { width, gap } } } : p,
      ),
    }));
  };
  const resetLayer = (layer: string): void => {
    update((rules) => ({
      ...rules,
      impedanceProfiles: (rules.impedanceProfiles ?? []).map((p) => {
        if (p.name !== profile.name) {
          return p;
        }
        const layers = Object.fromEntries(
          Object.entries(p.layers ?? {}).filter(([key]) => key !== layer),
        );
        return { ...p, layers };
      }),
    }));
  };
  return (
    <div className="board-table__scroll">
      <table
        className="board-table impedance-layers"
        data-testid={`impedance-layers-${index.toString()}`}
      >
        <thead>
          <tr>
            <th>{t('board.rules.impedance.layer')}</th>
            <th>{t('board.rules.impedance.structure')}</th>
            <th>{t('board.rules.impedance.width')}</th>
            {differential && <th>{t('board.rules.impedance.gap')}</th>}
            <th>{t('board.rules.impedance.result')}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {table.layers.map(({ layer, structure }, layerIndex) => {
            const layout = row?.layers.find((l) => l.layer === layer)?.layout ?? null;
            const state = structure === null ? null : status(t, profile, layout);
            const id = `${index.toString()}-${layerIndex.toString()}`;
            return (
              <tr key={layer}>
                <td>{layerName(t, layer)}</td>
                <td className="impedance-layers__structure">
                  {structureText(t, structure, units, language)}
                </td>
                <td>
                  {editable && structure !== null ? (
                    <LengthInput
                      bare
                      label={t('board.rules.impedance.width')}
                      value={layout?.width ?? 0}
                      units={units}
                      min={1}
                      testId={`impedance-width-${id}`}
                      onSave={(width) => {
                        setLayer(layer, width, layout?.gap ?? 0);
                      }}
                    />
                  ) : (
                    <span data-testid={`impedance-width-${id}`}>
                      {layout === null ? '—' : length(layout.width)}
                    </span>
                  )}
                </td>
                {differential && (
                  <td>
                    {editable && structure !== null ? (
                      <LengthInput
                        bare
                        label={t('board.rules.impedance.gap')}
                        value={layout?.gap ?? 0}
                        units={units}
                        min={1}
                        testId={`impedance-gap-${id}`}
                        onSave={(gap) => {
                          setLayer(layer, layout?.width ?? gap, gap);
                        }}
                      />
                    ) : (
                      <span data-testid={`impedance-gap-${id}`}>
                        {layout === null ? '—' : length(layout.gap)}
                      </span>
                    )}
                  </td>
                )}
                <td data-testid={`impedance-value-${id}`}>
                  {layout === null ? '—' : ohms(layout.impedance)}
                </td>
                <td>
                  {state !== null && (
                    <span
                      className={
                        state.ok ? 'impedance-status' : 'impedance-status impedance-status--bad'
                      }
                      data-testid={`impedance-status-${id}`}
                    >
                      {state.text}
                    </span>
                  )}
                  {editable && layout?.manual === true && (
                    <button
                      type="button"
                      className="button button--small button--quiet"
                      data-testid={`impedance-reset-${id}`}
                      onClick={() => {
                        resetLayer(layer);
                      }}
                    >
                      {t('board.rules.impedance.reset')}
                    </button>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/** Which profile (or interface) every net class follows. */
function ClassTable({
  doc,
  mode,
  rules,
  table,
  update,
}: {
  readonly doc: string;
  readonly mode: WorkMode;
  readonly rules: RulesProps;
  readonly table: ImpedanceTable;
  readonly update: TabProps['update'];
}): ReactNode {
  const { t, engine } = useAppState();
  const profiles = rules.impedanceProfiles ?? [];
  const choosePreset = (className: string, presetId: string): void => {
    const preset = table.presets.find((p) => p.id === presetId);
    let next: RulesProps = rules;
    if (preset === undefined) {
      next = assignProfile(rules, className, '');
    } else {
      const made = presetProfile(rules, preset);
      next = assignProfile(made.rules, className, made.name);
    }
    update(() => next);
    if (preset === undefined) {
      return;
    }
    // Apply the computed sizes to the class as well.
    engine
      .call<ImpedanceTable>('pcb.impedance', { doc, rules: next })
      .then((computed) => {
        update((current) => applyLayouts(current, computed, className));
      })
      .catch(() => undefined);
  };
  return (
    <div className="board-table__scroll">
      <table className="board-table" data-testid="impedance-classes">
        <thead>
          <tr>
            <th>{t('board.rules.className')}</th>
            <th>
              {mode === 'beginner'
                ? t('board.rules.impedance.interface')
                : t('board.rules.impedance.profile')}
            </th>
            <th>{t('board.rules.impedance.target')}</th>
          </tr>
        </thead>
        <tbody>
          {rules.netClasses.map((cls, index) => {
            const profile = profiles.find((p) => p.name === cls.impedance);
            const custom = profile !== undefined && (profile.preset ?? '') === '';
            return (
              <tr key={cls.name}>
                <td>{cls.name}</td>
                <td>
                  {mode === 'beginner' ? (
                    <SelectInput
                      bare
                      label={t('board.rules.impedance.interface')}
                      value={custom ? `custom:${profile.name}` : (profile?.preset ?? '')}
                      testId={`impedance-class-${index.toString()}`}
                      options={[
                        ['', t('board.rules.impedance.none')],
                        ...table.presets.map(
                          (p) =>
                            [
                              p.id,
                              t('board.rules.impedance.presetOption', {
                                name: p.name,
                                target: p.target,
                                kind: p.differential
                                  ? t('board.rules.impedance.differential')
                                  : t('board.rules.impedance.single'),
                              }),
                            ] as const,
                        ),
                        ...(custom
                          ? [
                              [
                                `custom:${profile.name}`,
                                t('board.rules.impedance.customOption', { name: profile.name }),
                              ] as const,
                            ]
                          : []),
                      ]}
                      onSave={(value) => {
                        if (!value.startsWith('custom:')) {
                          choosePreset(cls.name, value);
                        }
                      }}
                    />
                  ) : (
                    <SelectInput
                      bare
                      label={t('board.rules.impedance.profile')}
                      value={profile?.name ?? ''}
                      testId={`impedance-class-${index.toString()}`}
                      options={[
                        ['', t('board.rules.impedance.none')],
                        ...profiles.map((p) => [p.name, p.name] as const),
                      ]}
                      onSave={(value) => {
                        update((current) => assignProfile(current, cls.name, value));
                      }}
                    />
                  )}
                </td>
                <td className="board-table__nets">
                  {profile === undefined
                    ? ''
                    : t('board.rules.impedance.targetValue', {
                        target: profile.target,
                        tolerance: profile.tolerance,
                      })}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/** Profiles with their target, tolerance and kind (advanced mode). */
function ProfileTable({
  rules,
  update,
}: {
  readonly rules: RulesProps;
  readonly update: TabProps['update'];
}): ReactNode {
  const { t } = useAppState();
  const profiles = rules.impedanceProfiles ?? [];
  const change = (name: string, next: Partial<ImpedanceProfile>): void => {
    update((current) => ({
      ...current,
      impedanceProfiles: (current.impedanceProfiles ?? []).map((p) =>
        p.name === name ? { ...p, ...next } : p,
      ),
    }));
  };
  return (
    <>
      <div className="board-table__scroll">
        <table className="board-table" data-testid="impedance-profiles">
          <thead>
            <tr>
              <th>{t('board.rules.impedance.name')}</th>
              <th>{t('board.rules.impedance.kind')}</th>
              <th>{t('board.rules.impedance.target')}</th>
              <th>{t('board.rules.impedance.tolerance')}</th>
              <th>{t('board.rules.impedance.usedBy')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {profiles.map((profile, index) => (
              <tr key={profile.name}>
                <td>
                  <TextInput
                    bare
                    label={t('board.rules.impedance.name')}
                    value={profile.name}
                    testId={`profile-name-${index.toString()}`}
                    onSave={(name) => {
                      const trimmed = name.trim();
                      if (trimmed !== '' && !profiles.some((p) => p.name === trimmed)) {
                        update((current) => renameProfile(current, profile.name, trimmed));
                      }
                    }}
                  />
                </td>
                <td>
                  <SelectInput
                    bare
                    label={t('board.rules.impedance.kind')}
                    value={profile.differential === true ? 'differential' : 'single'}
                    testId={`profile-kind-${index.toString()}`}
                    options={[
                      ['single', t('board.rules.impedance.single')],
                      ['differential', t('board.rules.impedance.differential')],
                    ]}
                    onSave={(kind) => {
                      // The sizes of the other kind do not fit any more.
                      change(profile.name, {
                        differential: kind === 'differential',
                        preset: '',
                        layers: {},
                      });
                    }}
                  />
                </td>
                <td>
                  <NumberInput
                    bare
                    integer
                    label={t('board.rules.impedance.target')}
                    value={profile.target}
                    min={10}
                    max={500}
                    suffix="Ω"
                    testId={`profile-target-${index.toString()}`}
                    onSave={(target) => {
                      change(profile.name, { target, preset: '' });
                    }}
                  />
                </td>
                <td>
                  <NumberInput
                    bare
                    integer
                    label={t('board.rules.impedance.tolerance')}
                    value={profile.tolerance}
                    min={1}
                    max={50}
                    suffix="%"
                    testId={`profile-tolerance-${index.toString()}`}
                    onSave={(tolerance) => {
                      change(profile.name, { tolerance, preset: '' });
                    }}
                  />
                </td>
                <td className="board-table__nets">
                  {rules.netClasses
                    .filter((cls) => cls.impedance === profile.name)
                    .map((cls) => cls.name)
                    .join(', ')}
                </td>
                <td>
                  <button
                    type="button"
                    className="button button--small button--quiet"
                    aria-label={t('board.rules.remove')}
                    data-testid={`profile-remove-${index.toString()}`}
                    onClick={() => {
                      update((current) => removeProfile(current, profile.name));
                    }}
                  >
                    ×
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <button
        type="button"
        className="button button--small board-add"
        data-testid="profile-add"
        onClick={() => {
          update((current) => {
            const taken = new Set((current.impedanceProfiles ?? []).map((p) => p.name));
            let n = (current.impedanceProfiles ?? []).length + 1;
            while (taken.has(t('board.rules.impedance.newProfile', { n }))) {
              n += 1;
            }
            return {
              ...current,
              impedanceProfiles: [
                ...(current.impedanceProfiles ?? []),
                { name: t('board.rules.impedance.newProfile', { n }), target: 50, tolerance: 10 },
              ],
            };
          });
        }}
      >
        {t('board.rules.impedance.addProfile')}
      </button>
    </>
  );
}

/** Impedance of any width and gap on any layer (advanced mode). */
function Calculator({
  doc,
  units,
  table,
}: {
  readonly doc: string;
  readonly units: Units;
  readonly table: ImpedanceTable;
}): ReactNode {
  const { t, engine } = useAppState();
  const layers = table.layers.filter((l) => l.structure !== null);
  const [layer, setLayer] = useState(layers[0]?.layer ?? 'cu.top');
  const [differential, setDifferential] = useState(false);
  const [width, setWidth] = useState(200_000);
  const [gap, setGap] = useState(200_000);
  const [result, setResult] = useState<number | null>(null);
  useEffect(() => {
    let cancelled = false;
    engine
      .call<{ impedance: number }>('pcb.impedanceCalc', { doc, layer, differential, width, gap })
      .then((reply) => {
        if (!cancelled) {
          setResult(reply.impedance);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setResult(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [engine, doc, layer, differential, width, gap]);
  return (
    <div className="board-form board-form--grid" data-testid="impedance-calculator">
      <SelectInput
        label={t('board.rules.impedance.layer')}
        value={layer}
        testId="calc-layer"
        options={layers.map((l) => [l.layer, layerName(t, l.layer)] as const)}
        onSave={setLayer}
      />
      <SelectInput
        label={t('board.rules.impedance.kind')}
        value={differential ? 'differential' : 'single'}
        testId="calc-kind"
        options={[
          ['single', t('board.rules.impedance.single')],
          ['differential', t('board.rules.impedance.differential')],
        ]}
        onSave={(kind) => {
          setDifferential(kind === 'differential');
        }}
      />
      <LengthInput
        label={t('board.rules.impedance.width')}
        value={width}
        units={units}
        min={1}
        testId="calc-width"
        onSave={setWidth}
      />
      {differential && (
        <LengthInput
          label={t('board.rules.impedance.gap')}
          value={gap}
          units={units}
          min={1}
          testId="calc-gap"
          onSave={setGap}
        />
      )}
      <p className="impedance-result" data-testid="calc-result">
        {result === null ? '—' : ohms(result)}
      </p>
    </div>
  );
}

/** The impedance tab. */
export function ImpedanceTab({ doc, units, mode, rules, update }: TabProps): ReactNode {
  const { t } = useAppState();
  const table = useImpedanceTable(doc, rules);
  if (table === null) {
    return <p className="dialog__text dialog__text--muted">{t('board.rules.loading')}</p>;
  }
  const profiles = rules.impedanceProfiles ?? [];
  const shown =
    mode === 'beginner'
      ? profiles.filter((p) => rules.netClasses.some((cls) => cls.impedance === p.name))
      : profiles;
  return (
    <div className="impedance" data-testid={`impedance-${mode}`}>
      <p className="field__hint">
        {mode === 'beginner'
          ? t('board.rules.impedance.beginnerHint')
          : t('board.rules.impedance.advancedHint')}
      </p>
      {mode === 'advanced' && (
        <>
          <h3 className="dialog__section">{t('board.rules.impedance.profiles')}</h3>
          <ProfileTable rules={rules} update={update} />
        </>
      )}
      <h3 className="dialog__section">{t('board.rules.impedance.classes')}</h3>
      <ClassTable doc={doc} mode={mode} rules={rules} table={table} update={update} />
      {shown.map((profile, index) => (
        <section key={profile.name} className="impedance__profile">
          <h3 className="dialog__section">
            {t('board.rules.impedance.sizesOf', {
              name: profile.name,
              target: t('board.rules.impedance.targetValue', {
                target: profile.target,
                tolerance: profile.tolerance,
              }),
            })}
          </h3>
          <LayerTable
            units={units}
            editable={mode === 'advanced'}
            index={index}
            profile={profile}
            table={table}
            update={update}
          />
        </section>
      ))}
      {mode === 'beginner' && shown.length > 0 && (
        <p className="field__hint">{t('board.rules.impedance.stackupHint')}</p>
      )}
      {mode === 'advanced' && (
        <>
          <h3 className="dialog__section">{t('board.rules.impedance.calculator')}</h3>
          <Calculator doc={doc} units={units} table={table} />
        </>
      )}
    </div>
  );
}
