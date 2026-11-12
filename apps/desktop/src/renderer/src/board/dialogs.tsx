/**
 * Dialogs of the layout editor: zone and keep-out settings, the footprint
 * chooser, updating the board from the schematic (and back) and the DXF
 * import.
 */
import { useEffect, useMemo, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import type { Units } from '@shared/project';

import { Modal } from '../components/Modal';
import { CheckInput, LengthInput, NumberInput, SelectInput, TextInput } from '../libedit/fields';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { layerName } from './BoardToolbar';
import type { Point } from './editor';

type Call = <T>(method: string, params?: unknown) => Promise<T>;

function useCall(): { call: Call; fail: (error: unknown) => void } {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  return useMemo(
    () => ({
      call: (method, params) => engine.call(method, params),
      fail: (error: unknown) => {
        notify(errorMessage(t, error), 'error');
      },
    }),
    [engine, notify, t],
  );
}

// --- Zones ------------------------------------------------------------------

interface ZoneFill {
  clearance: number;
  minWidth: number;
  padConnection: 'thermal' | 'solid' | 'none';
  thermalGap: number;
  thermalSpokeWidth: number;
  hatch?: readonly [number, number] | null;
  islandRemoval: 'always' | 'never' | { belowArea: number };
}

interface Keepout {
  tracks: boolean;
  vias: boolean;
  pads: boolean;
  pours: boolean;
  footprints: boolean;
}

/** A zone as the engine stores it. */
export interface ZoneJson {
  type?: 'zone';
  id: string;
  name?: string;
  net?: string | null;
  layers: string[];
  outline: Point[];
  holes?: Point[][];
  priority?: number;
  keepout?: Keepout | null;
  fill?: Partial<ZoneFill>;
  filled?: unknown[];
}

const DEFAULT_FILL: ZoneFill = {
  clearance: 300_000,
  minWidth: 250_000,
  padConnection: 'thermal',
  thermalGap: 500_000,
  thermalSpokeWidth: 500_000,
  hatch: null,
  islandRemoval: 'always',
};

interface NetChoice {
  readonly id: string;
  readonly name: string;
}

/** Settings of a new or existing zone or keep-out area. */
export function ZoneDialog({
  doc,
  zone,
  layers,
  units,
  isNew,
  onClose,
}: {
  readonly doc: string;
  readonly zone: ZoneJson;
  readonly layers: readonly string[];
  readonly units: Units;
  readonly isNew: boolean;
  readonly onClose: () => void;
}): ReactNode {
  const { t } = useAppState();
  const { call, fail } = useCall();
  const [draft, setDraft] = useState<ZoneJson>(zone);
  const [nets, setNets] = useState<readonly NetChoice[]>([]);
  const fill: ZoneFill = { ...DEFAULT_FILL, ...draft.fill };
  const keepout = draft.keepout ?? null;

  useEffect(() => {
    let cancelled = false;
    call<{ nets?: { id: string; name: string }[] }>('document.get', { doc })
      .then((board) => {
        if (!cancelled) {
          const list = (board.nets ?? []).map((n) => ({ id: n.id, name: n.name }));
          list.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
          setNets(list);
        }
      })
      .catch(fail);
    return () => {
      cancelled = true;
    };
  }, [call, fail, doc]);

  const setFill = (patch: Partial<ZoneFill>): void => {
    setDraft({ ...draft, fill: { ...fill, ...patch } });
  };
  const toggleLayer = (layer: string, on: boolean): void => {
    const next = on ? [...draft.layers, layer] : draft.layers.filter((l) => l !== layer);
    setDraft({ ...draft, layers: layers.filter((l) => next.includes(l)) });
  };
  const save = (): void => {
    const object = {
      ...draft,
      type: 'zone' as const,
      fill,
      filled: keepout === null ? (draft.filled ?? []) : [],
    };
    call('edit.apply', {
      label: isNew ? (keepout === null ? 'add-zone' : 'add-keepout') : 'edit-zone',
      changes: [{ op: isNew ? 'insert' : 'update', doc, object }],
    })
      .then(() =>
        keepout === null ? call('pcb.fillZones', { doc, zones: [draft.id] }) : undefined,
      )
      .then(onClose)
      .catch(fail);
  };
  const title = keepout === null ? t('board.zone.title') : t('board.zone.keepoutTitle');
  return (
    <Modal
      title={title}
      onClose={onClose}
      testId="zone-dialog"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={draft.layers.length === 0}
            data-testid="zone-save"
            onClick={save}
          >
            {isNew ? t('board.zone.create') : t('board.zone.save')}
          </button>
        </>
      }
    >
      <div className="board-form">
        <TextInput
          label={t('board.zone.name')}
          value={draft.name ?? ''}
          testId="zone-name"
          onSave={(name) => {
            setDraft({ ...draft, name });
          }}
        />
        {keepout === null && (
          <label className="field field--compact">
            <span className="field__label">{t('board.zone.net')}</span>
            <select
              className="field__control"
              value={draft.net ?? ''}
              data-testid="zone-net"
              onChange={(event) => {
                setDraft({ ...draft, net: event.target.value === '' ? null : event.target.value });
              }}
            >
              <option value="">{t('board.zone.noNet')}</option>
              {nets.map((net) => (
                <option key={net.id} value={net.id}>
                  {net.name}
                </option>
              ))}
            </select>
          </label>
        )}
        <fieldset className="board-form__group">
          <legend>{t('board.zone.layers')}</legend>
          {layers.map((layer) => (
            <CheckInput
              key={layer}
              label={layerName(t, layer)}
              checked={draft.layers.includes(layer)}
              testId={`zone-layer-${layer}`}
              onSave={(on) => {
                toggleLayer(layer, on);
              }}
            />
          ))}
        </fieldset>
        {keepout === null ? (
          <>
            <NumberInput
              label={t('board.zone.priority')}
              value={draft.priority ?? 0}
              integer
              min={0}
              testId="zone-priority"
              onSave={(priority) => {
                setDraft({ ...draft, priority });
              }}
            />
            <LengthInput
              label={t('board.zone.clearance')}
              value={fill.clearance}
              units={units}
              min={0}
              testId="zone-clearance"
              onSave={(clearance) => {
                setFill({ clearance });
              }}
            />
            <LengthInput
              label={t('board.zone.minWidth')}
              value={fill.minWidth}
              units={units}
              min={0}
              onSave={(minWidth) => {
                setFill({ minWidth });
              }}
            />
            <SelectInput
              label={t('board.zone.padConnection')}
              value={fill.padConnection}
              testId="zone-pad-connection"
              options={(['thermal', 'solid', 'none'] as const).map((value) => [
                value,
                t(`board.zone.connections.${value}`),
              ])}
              onSave={(padConnection) => {
                setFill({ padConnection });
              }}
            />
            {fill.padConnection === 'thermal' && (
              <>
                <LengthInput
                  label={t('board.zone.thermalGap')}
                  value={fill.thermalGap}
                  units={units}
                  min={0}
                  onSave={(thermalGap) => {
                    setFill({ thermalGap });
                  }}
                />
                <LengthInput
                  label={t('board.zone.spokeWidth')}
                  value={fill.thermalSpokeWidth}
                  units={units}
                  min={0}
                  onSave={(thermalSpokeWidth) => {
                    setFill({ thermalSpokeWidth });
                  }}
                />
              </>
            )}
            <CheckInput
              label={t('board.zone.removeIslands')}
              checked={fill.islandRemoval !== 'never'}
              onSave={(on) => {
                setFill({ islandRemoval: on ? 'always' : 'never' });
              }}
            />
            <CheckInput
              label={t('board.zone.hatched')}
              checked={fill.hatch !== null && fill.hatch !== undefined}
              testId="zone-hatched"
              onSave={(on) => {
                setFill({ hatch: on ? [250_000, 750_000] : null });
              }}
            />
          </>
        ) : (
          <fieldset className="board-form__group">
            <legend>{t('board.zone.forbid')}</legend>
            {(['tracks', 'vias', 'pads', 'pours', 'footprints'] as const).map((key) => (
              <CheckInput
                key={key}
                label={t(`board.zone.keepout.${key}`)}
                checked={keepout[key]}
                testId={`keepout-${key}`}
                onSave={(on) => {
                  setDraft({ ...draft, keepout: { ...keepout, [key]: on } });
                }}
              />
            ))}
          </fieldset>
        )}
      </div>
    </Modal>
  );
}

/** A new zone (or keep-out area) for an outline. */
export function newZone(outline: readonly Point[], keepout: boolean, layer: string): ZoneJson {
  return {
    id: crypto.randomUUID(),
    name: '',
    net: null,
    layers: [layer],
    outline: [...outline],
    priority: 0,
    keepout: keepout
      ? { tracks: true, vias: true, pads: false, pours: true, footprints: false }
      : null,
    fill: { ...DEFAULT_FILL },
    filled: [],
  };
}

// --- Footprint chooser ------------------------------------------------------

interface FootprintEntry {
  readonly name: string;
  readonly description?: string;
  readonly pads?: number;
  readonly library: string;
}

/** Picks a footprint of the project libraries to place on the board. */
export function FootprintChooser({
  onPick,
  onClose,
}: {
  readonly onPick: (name: string) => void;
  readonly onClose: () => void;
}): ReactNode {
  const { t } = useAppState();
  const { project } = useProject();
  const { call, fail } = useCall();
  const [entries, setEntries] = useState<readonly FootprintEntry[] | null>(null);
  const [query, setQuery] = useState('');
  useEffect(() => {
    let cancelled = false;
    const libraries = project?.documents.filter((d) => d.kind === 'library') ?? [];
    Promise.all(
      libraries.map((library) =>
        call<{ footprints: { name: string; description?: string; pads?: number }[] }>(
          'libedit.list',
          { doc: library.id },
        ).then((list) => list.footprints.map((f) => ({ ...f, library: library.name }))),
      ),
    )
      .then((lists) => {
        if (!cancelled) {
          const seen = new Set<string>();
          const flat = lists.flat().filter((entry) => {
            if (seen.has(entry.name)) {
              return false;
            }
            seen.add(entry.name);
            return true;
          });
          flat.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
          setEntries(flat);
        }
      })
      .catch(fail);
    return () => {
      cancelled = true;
    };
  }, [call, fail, project]);
  const words = query.toLowerCase().split(/\s+/u).filter(Boolean);
  const shown = (entries ?? []).filter((entry) =>
    words.every(
      (word) =>
        entry.name.toLowerCase().includes(word) ||
        (entry.description ?? '').toLowerCase().includes(word),
    ),
  );
  return (
    <Modal title={t('board.chooser.title')} onClose={onClose} testId="footprint-chooser">
      <input
        className="field__control"
        autoFocus
        placeholder={t('board.chooser.search')}
        aria-label={t('board.chooser.search')}
        value={query}
        data-testid="footprint-search"
        onChange={(event) => {
          setQuery(event.target.value);
        }}
      />
      {entries === null ? (
        <p className="dialog__text dialog__text--muted">{t('board.chooser.loading')}</p>
      ) : shown.length === 0 ? (
        <p className="dialog__text dialog__text--muted">{t('board.chooser.empty')}</p>
      ) : (
        <ul className="board-chooser" data-testid="footprint-list">
          {shown.slice(0, 200).map((entry) => (
            <li key={entry.name}>
              <button
                type="button"
                className="board-chooser__item"
                data-testid={`footprint-${entry.name}`}
                onClick={() => {
                  onPick(entry.name);
                }}
              >
                <span className="board-chooser__name">{entry.name}</span>
                <span className="board-chooser__meta">
                  {entry.pads === undefined
                    ? entry.library
                    : t('board.chooser.pads', { count: entry.pads })}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}

// --- Schematic ↔ board ------------------------------------------------------

interface SyncChange {
  readonly index: number;
  readonly kind: string;
  readonly description: string;
  readonly designator?: string;
  readonly name?: string;
  readonly from?: string | null;
  readonly to?: string | null;
  readonly footprint?: string;
  readonly pad?: string;
  readonly value?: string;
}

interface SyncProblem {
  readonly code: string;
  readonly message: string;
  readonly designator: string;
}

/** The text of a change in the UI language. */
function changeText(t: ReturnType<typeof useAppState>['t'], change: SyncChange): string {
  const key = `board.sync.changes.${change.kind}` as MessageKey;
  const vars: Record<string, string> = {
    designator: change.designator ?? '',
    name: change.name ?? '',
    from: change.from ?? t('board.sync.noNet'),
    to: change.to ?? t('board.sync.noNet'),
    footprint: change.footprint ?? '',
    pad: change.pad ?? '',
  };
  const text = t(key, vars);
  return text === key ? change.description : text;
}

/** Updates the board from the schematic (forward) or the schematic from the board (back). */
export function SyncDialog({
  doc,
  onClose,
  onApplied,
}: {
  readonly doc: string;
  readonly onClose: () => void;
  readonly onApplied: (select: readonly string[]) => void;
}): ReactNode {
  const { t } = useAppState();
  const { call, fail } = useCall();
  const [direction, setDirection] = useState<'forward' | 'back'>('forward');
  const [preview, setPreview] = useState<{
    changes: readonly SyncChange[];
    problems: readonly SyncProblem[];
  } | null>(null);
  const [off, setOff] = useState<ReadonlySet<number>>(() => new Set());
  useEffect(() => {
    let cancelled = false;
    const method = direction === 'forward' ? 'pcb.syncPreview' : 'pcb.backPreview';
    call<{ changes: SyncChange[]; problems?: SyncProblem[] }>(method, { doc })
      .then((reply) => {
        if (!cancelled) {
          setPreview({ changes: reply.changes, problems: reply.problems ?? [] });
          setOff(new Set());
        }
      })
      .catch(fail);
    return () => {
      cancelled = true;
    };
  }, [call, fail, doc, direction]);
  const apply = (): void => {
    if (preview === null) {
      return;
    }
    const changes = preview.changes.map((c) => c.index).filter((i) => !off.has(i));
    const method = direction === 'forward' ? 'pcb.syncApply' : 'pcb.backApply';
    call<{ select?: string[] }>(method, { doc, changes })
      .then((reply) => {
        onApplied(reply.select ?? []);
        onClose();
      })
      .catch(fail);
  };
  const count = (preview?.changes.length ?? 0) - off.size;
  return (
    <Modal
      title={t('board.sync.title')}
      onClose={onClose}
      testId="sync-dialog"
      className="dialog--wide"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={count <= 0}
            data-testid="sync-apply"
            onClick={apply}
          >
            {t('board.sync.apply', { count })}
          </button>
        </>
      }
    >
      <div className="segmented" role="tablist">
        {(['forward', 'back'] as const).map((value) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={direction === value}
            className={
              direction === value ? 'segmented__item segmented__item--active' : 'segmented__item'
            }
            data-testid={`sync-${value}`}
            onClick={() => {
              setPreview(null);
              setDirection(value);
            }}
          >
            {t(`board.sync.${value}`)}
          </button>
        ))}
      </div>
      <p className="dialog__text dialog__text--muted">{t(`board.sync.${direction}Hint`)}</p>
      {preview === null ? (
        <p className="dialog__text dialog__text--muted">{t('board.sync.loading')}</p>
      ) : preview.changes.length === 0 ? (
        <p className="dialog__text" data-testid="sync-none">
          {t('board.sync.none')}
        </p>
      ) : (
        <ul className="board-sync" data-testid="sync-list">
          {preview.changes.map((change) => (
            <li key={change.index}>
              <label className="checkbox">
                <input
                  type="checkbox"
                  checked={!off.has(change.index)}
                  data-testid={`sync-change-${change.kind}`}
                  onChange={(event) => {
                    const next = new Set(off);
                    if (event.target.checked) {
                      next.delete(change.index);
                    } else {
                      next.add(change.index);
                    }
                    setOff(next);
                  }}
                />
                {changeText(t, change)}
              </label>
            </li>
          ))}
        </ul>
      )}
      {preview !== null && preview.problems.length > 0 && (
        <section className="board-sync__problems" data-testid="sync-problems">
          <h3 className="dialog__section">{t('board.sync.problems')}</h3>
          <ul>
            {preview.problems.map((problem, index) => (
              <li key={`${problem.code}-${index.toString()}`}>
                {t(`board.sync.problemCodes.${problem.code}` as MessageKey, {
                  designator: problem.designator,
                }) === `board.sync.problemCodes.${problem.code}`
                  ? problem.message
                  : t(`board.sync.problemCodes.${problem.code}` as MessageKey, {
                      designator: problem.designator,
                    })}
              </li>
            ))}
          </ul>
        </section>
      )}
    </Modal>
  );
}

// --- DXF import -------------------------------------------------------------

interface DxfPreview {
  readonly layers: readonly string[];
  readonly units: string;
  readonly count: number;
  readonly bounds: readonly [number, number, number, number] | null;
  readonly skipped: readonly (readonly [string, number])[];
}

const SCALES: Readonly<Record<string, number | null>> = {
  file: null,
  mm: 1e6,
  inch: 25.4e6,
  mil: 25_400,
};

/** Imports a DXF drawing as board outline or mechanical layer. */
export function DxfDialog({
  doc,
  units,
  onClose,
}: {
  readonly doc: string;
  readonly units: Units;
  readonly onClose: () => void;
}): ReactNode {
  const { t, snapshot } = useAppState();
  const { call, fail } = useCall();
  const [path, setPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<DxfPreview | null>(null);
  const [scale, setScale] = useState<keyof typeof SCALES>('file');
  const [layer, setLayer] = useState('outline');
  const [chosen, setChosen] = useState<ReadonlySet<string>>(() => new Set());
  const [replace, setReplace] = useState(true);
  const [toOrigin, setToOrigin] = useState(true);

  useEffect(() => {
    if (path === null) {
      return undefined;
    }
    let cancelled = false;
    call<DxfPreview>('pcb.dxfPreview', { path, scale: SCALES[scale] })
      .then((reply) => {
        if (!cancelled) {
          setPreview(reply);
          setChosen(new Set(reply.layers));
        }
      })
      .catch(fail);
    return () => {
      cancelled = true;
    };
  }, [call, fail, path, scale]);

  const choose = (): void => {
    window.ew.dialogs
      .chooseImport('dxf')
      .then((picked) => {
        if (picked !== null) {
          setPath(picked);
        }
      })
      .catch(fail);
  };
  const run = (): void => {
    if (path === null) {
      return;
    }
    call('pcb.importDxf', {
      doc,
      path,
      layer,
      layers: [...chosen],
      replace,
      scale: SCALES[scale],
      origin: toOrigin ? [0, 0] : null,
    })
      .then(onClose)
      .catch(fail);
  };
  const size =
    preview?.bounds === null || preview === null
      ? null
      : [preview.bounds[2] - preview.bounds[0], preview.bounds[3] - preview.bounds[1]];
  const mm = (nm: number): string =>
    new Intl.NumberFormat(snapshot.language, { maximumFractionDigits: 2 }).format(nm / 1e6);
  return (
    <Modal
      title={t('board.dxf.title')}
      onClose={onClose}
      testId="dxf-dialog"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={preview === null || chosen.size === 0}
            data-testid="dxf-import"
            onClick={run}
          >
            {t('board.dxf.import')}
          </button>
        </>
      }
    >
      <div className="board-form">
        <div className="board-form__row">
          <button type="button" className="button" data-testid="dxf-choose" onClick={choose}>
            {t('board.dxf.choose')}
          </button>
          <span className="board-form__path" data-testid="dxf-path">
            {path ?? t('board.dxf.noFile')}
          </span>
        </div>
        {preview !== null && (
          <>
            <p className="dialog__text" data-testid="dxf-summary">
              {t('board.dxf.summary', {
                count: preview.count,
                units: preview.units,
                width: size === null ? '–' : mm(size[0] ?? 0),
                height: size === null ? '–' : mm(size[1] ?? 0),
              })}
            </p>
            {preview.skipped.length > 0 && (
              <p className="dialog__text dialog__text--muted">
                {t('board.dxf.skipped', {
                  list: preview.skipped.map(([kind, n]) => `${kind} (${n.toString()})`).join(', '),
                })}
              </p>
            )}
            <fieldset className="board-form__group">
              <legend>{t('board.dxf.layers')}</legend>
              {preview.layers.map((name) => (
                <CheckInput
                  key={name}
                  label={name}
                  checked={chosen.has(name)}
                  onSave={(on) => {
                    const next = new Set(chosen);
                    if (on) {
                      next.add(name);
                    } else {
                      next.delete(name);
                    }
                    setChosen(next);
                  }}
                />
              ))}
            </fieldset>
          </>
        )}
        <SelectInput
          label={t('board.dxf.units')}
          value={scale}
          options={Object.keys(SCALES).map((key) => [
            key,
            t(`board.dxf.scales.${key}` as MessageKey),
          ])}
          onSave={(value) => {
            setScale(value);
          }}
        />
        <SelectInput
          label={t('board.dxf.target')}
          value={layer}
          testId="dxf-target"
          options={[
            ['outline', t('board.dxf.targets.outline')],
            ['mech.1', t('board.dxf.targets.mech', { n: 1 })],
            ['mech.2', t('board.dxf.targets.mech', { n: 2 })],
            ['user.1', t('board.dxf.targets.user', { n: 1 })],
          ]}
          onSave={(value) => {
            setLayer(value);
          }}
        />
        <CheckInput label={t('board.dxf.replace')} checked={replace} onSave={setReplace} />
        <CheckInput label={t('board.dxf.toOrigin')} checked={toOrigin} onSave={setToOrigin} />
        <p className="field__hint">{t('board.dxf.hint', { units })}</p>
      </div>
    </Modal>
  );
}
