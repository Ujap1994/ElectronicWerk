/**
 * The enclosure panel of the 3D view (M17): the mechanical bodies of the
 * board's 3D assembly (enclosure parts, panels, screws, carrier boards),
 * their placement, visibility and transparency, and the results of the
 * enclosure check.
 */
import { useEffect, useRef, useState, type ReactNode } from 'react';

import { findingMessage } from '../board/DrcPanel';
import type { DrcFinding } from '../board/stores';
import { Icon } from '../components/Icon';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import {
  MECHANICAL_KINDS,
  type EnclosureCheck,
  type EnclosureFinding,
  type MechanicalBody,
} from './types';

/** Millimetres with at most three decimals. */
function fromNm(nm: number): number {
  return Math.round(nm / 1000) / 1000;
}

/** A number field that applies its value on Enter or when it loses focus. */
function NumberField({
  value,
  label,
  step,
  testId,
  onCommit,
}: {
  readonly value: number;
  readonly label: string;
  readonly step: number;
  readonly testId: string;
  readonly onCommit: (value: number) => void;
}): ReactNode {
  const [draft, setDraft] = useState<string | null>(null);
  const commit = (): void => {
    if (draft === null) {
      return;
    }
    const parsed = Number(draft.replace(',', '.'));
    setDraft(null);
    if (Number.isFinite(parsed) && parsed !== value) {
      onCommit(parsed);
    }
  };
  return (
    <input
      type="number"
      className="input mech__number"
      step={step}
      value={draft ?? String(value)}
      aria-label={label}
      title={label}
      data-testid={testId}
      onChange={(event) => {
        setDraft(event.target.value);
      }}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          commit();
        } else if (event.key === 'Escape') {
          setDraft(null);
        }
      }}
    />
  );
}

export function MechanicalPanel({
  doc,
  bodies,
  clearance,
  selected,
  onSelect,
  check,
  checking,
  live,
  onLive,
  onRun,
  onFocus,
  centerOf,
  flash,
  onClose,
}: {
  readonly doc: string;
  readonly bodies: readonly MechanicalBody[];
  readonly clearance: number;
  readonly selected: string | null;
  readonly onSelect: (id: string | null) => void;
  readonly check: EnclosureCheck | null;
  readonly checking: boolean;
  readonly live: boolean;
  readonly onLive: (on: boolean) => void;
  readonly onRun: () => void;
  readonly onFocus: (finding: EnclosureFinding) => void;
  /** How far a body must move (x, y in mm) for its middle to lie over the board's. */
  readonly centerOf: (id: string) => readonly [number, number] | null;
  readonly flash: (text: string) => void;
  readonly onClose: () => void;
}): ReactNode {
  const { engine, t } = useAppState();
  const [busy, setBusy] = useState(false);
  const opacityTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const [opacityDraft, setOpacityDraft] = useState<{ id: string; value: number } | null>(null);
  const body = bodies.find((b) => b.id === selected) ?? null;

  useEffect(
    () => () => {
      clearTimeout(opacityTimerRef.current);
    },
    [],
  );

  const update = (id: string, fields: Record<string, unknown>): void => {
    engine.call('mechanical.update', { doc, id, ...fields }).catch((error: unknown) => {
      flash(errorMessage(t, error));
    });
  };

  const add = async (): Promise<void> => {
    const file = await window.ew.dialogs.chooseStep();
    if (file === null) {
      return;
    }
    setBusy(true);
    try {
      const reply = await engine.call<{ id: string; path: string }>('mechanical.add', {
        doc,
        file,
      });
      onSelect(reply.id);
      flash(t('view3d.mechanical.added', { path: reply.path }));
    } catch (error) {
      flash(errorMessage(t, error));
    } finally {
      setBusy(false);
    }
  };

  const makeBox = async (): Promise<void> => {
    setBusy(true);
    try {
      const reply = await engine.call<{
        ids: string[];
        spec: { width: number; depth: number; height: number };
      }>('mechanical.enclosure', { doc }, 120_000);
      onSelect(reply.ids[0] ?? null);
      const round = (v: number): string => String(Math.round(v * 10) / 10);
      flash(
        t('view3d.mechanical.made', {
          width: round(reply.spec.width),
          depth: round(reply.spec.depth),
          height: round(reply.spec.height),
        }),
      );
    } catch (error) {
      flash(errorMessage(t, error));
    } finally {
      setBusy(false);
    }
  };

  const outline = async (): Promise<void> => {
    setBusy(true);
    try {
      const reply = await engine.call<{ outline?: [number, number][] }>(
        'mechanical.outline',
        { doc },
        120_000,
      );
      const points = reply.outline ?? [];
      const xs = points.map((p) => p[0]);
      const ys = points.map((p) => p[1]);
      const round = (v: number): string => String(Math.round(v * 100) / 100);
      flash(
        t('view3d.mechanical.outlineDone', {
          width: round(Math.max(...xs) - Math.min(...xs)),
          depth: round(Math.max(...ys) - Math.min(...ys)),
        }),
      );
    } catch (error) {
      flash(errorMessage(t, error));
    } finally {
      setBusy(false);
    }
  };

  const areas = async (): Promise<void> => {
    setBusy(true);
    try {
      const reply = await engine.call<{ keepouts: number; heights: unknown[] }>(
        'mechanical.areas',
        { doc },
        300_000,
      );
      flash(
        t('view3d.mechanical.areasDone', {
          keepouts: reply.keepouts,
          heights: reply.heights.length,
        }),
      );
    } catch (error) {
      flash(errorMessage(t, error));
    } finally {
      setBusy(false);
    }
  };

  const setPosition = (axis: number, mm: number): void => {
    if (body === null) {
      return;
    }
    const position = [...body.position];
    position[axis] = Math.round(mm * 1e6);
    update(body.id, { position });
  };

  const setRotation = (axis: number, degrees: number): void => {
    if (body === null) {
      return;
    }
    const rotation = [...body.rotation];
    rotation[axis] = degrees;
    update(body.id, { rotation });
  };

  const center = (): void => {
    if (body === null) {
      return;
    }
    const shift = centerOf(body.id);
    if (shift === null) {
      return;
    }
    const [x, y, z] = body.position;
    update(body.id, {
      position: [Math.round(x + shift[0] * 1e6), Math.round(y + shift[1] * 1e6), z],
    });
  };

  const findings = check?.findings.filter((f) => !f.excluded) ?? [];
  const opacity =
    body !== null && opacityDraft?.id === body.id ? opacityDraft.value : (body?.opacity ?? 100);
  const axes: readonly ['x', 'y', 'z'] = ['x', 'y', 'z'];

  return (
    <aside
      className="view3d__panel mech"
      data-testid="mech-panel"
      aria-label={t('view3d.mechanical.title')}
    >
      <header className="mech__header">
        <h2 className="mech__title">{t('view3d.mechanical.title')}</h2>
        <button
          type="button"
          className="icon-button"
          title={t('view3d.mechanical.close')}
          aria-label={t('view3d.mechanical.close')}
          data-testid="mech-close"
          onClick={onClose}
        >
          <Icon name="close" size={16} />
        </button>
      </header>
      <div className="mech__actions">
        <button
          type="button"
          className="button"
          title={t('view3d.mechanical.addTip')}
          disabled={busy}
          data-testid="mech-add"
          onClick={() => {
            void add();
          }}
        >
          <Icon name="plus" size={16} />
          {t('view3d.mechanical.add')}
        </button>
        <button
          type="button"
          className="button"
          title={t('view3d.mechanical.boxTip')}
          disabled={busy}
          data-testid="mech-box"
          onClick={() => {
            void makeBox();
          }}
        >
          <Icon name="enclosure" size={16} />
          {t('view3d.mechanical.box')}
        </button>
      </div>
      {bodies.length === 0 ? (
        <p className="mech__empty" data-testid="mech-empty">
          {t('view3d.mechanical.empty')}
        </p>
      ) : (
        <ul className="mech__list" role="listbox" aria-label={t('view3d.mechanical.title')}>
          {bodies.map((b) => (
            <li
              key={b.id}
              role="option"
              aria-selected={b.id === selected}
              className="mech__row"
              data-testid="mech-body"
              data-name={b.name}
              onClick={() => {
                onSelect(b.id === selected ? null : b.id);
              }}
            >
              <button
                type="button"
                className="icon-button"
                title={t(b.hidden ? 'view3d.mechanical.show' : 'view3d.mechanical.hide', {
                  name: b.name,
                })}
                aria-label={t(b.hidden ? 'view3d.mechanical.show' : 'view3d.mechanical.hide', {
                  name: b.name,
                })}
                aria-pressed={!b.hidden}
                data-testid="mech-visible"
                onClick={(event) => {
                  event.stopPropagation();
                  update(b.id, { hidden: !b.hidden });
                }}
              >
                <Icon name={b.hidden ? 'eyeOff' : 'eye'} size={16} />
              </button>
              <span className="mech__name">{b.name}</span>
              <span className="mech__kind">{t(`view3d.mechanical.kinds.${b.kind}`)}</span>
              {!b.exists && (
                <span title={t('view3d.mechanical.fileMissing', { path: b.path })}>
                  <Icon name="warning" size={14} />
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
      {body !== null && (
        <div className="mech__editor" data-testid="mech-editor">
          <label className="mech__field">
            <span>{t('view3d.mechanical.name')}</span>
            <input
              key={`${body.id}:${body.name}`}
              className="input"
              defaultValue={body.name}
              data-testid="mech-name"
              onBlur={(event) => {
                const name = event.target.value.trim();
                if (name !== '' && name !== body.name) {
                  update(body.id, { name });
                }
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.currentTarget.blur();
                }
              }}
            />
          </label>
          <label className="mech__field">
            <span>{t('view3d.mechanical.kind')}</span>
            <select
              className="input"
              value={body.kind}
              data-testid="mech-kind"
              onChange={(event) => {
                update(body.id, { kind: event.target.value });
              }}
            >
              {MECHANICAL_KINDS.map((kind) => (
                <option key={kind} value={kind}>
                  {t(`view3d.mechanical.kinds.${kind}`)}
                </option>
              ))}
            </select>
          </label>
          <div className="mech__field">
            <span>{t('view3d.mechanical.position')}</span>
            <div className="mech__triple">
              {axes.map((axis, index) => (
                <NumberField
                  key={`${body.id}:${axis}:${String(body.position[index])}`}
                  value={fromNm(body.position[index] ?? 0)}
                  label={`${t('view3d.mechanical.position')} ${axis}`}
                  step={0.1}
                  testId={`mech-${axis}`}
                  onCommit={(value) => {
                    setPosition(index, value);
                  }}
                />
              ))}
            </div>
          </div>
          <div className="mech__field">
            <span>{t('view3d.mechanical.rotation')}</span>
            <div className="mech__triple">
              {axes.map((axis, index) => (
                <NumberField
                  key={`${body.id}:r${axis}:${String(body.rotation[index])}`}
                  value={body.rotation[index] ?? 0}
                  label={`${t('view3d.mechanical.rotation')} ${axis}`}
                  step={90}
                  testId={`mech-r${axis}`}
                  onCommit={(value) => {
                    setRotation(index, value);
                  }}
                />
              ))}
            </div>
          </div>
          <label className="mech__field">
            <span>
              {t('view3d.mechanical.opacity')} {opacity} %
            </span>
            <input
              type="range"
              min={5}
              max={100}
              step={5}
              value={opacity}
              data-testid="mech-opacity"
              onChange={(event) => {
                const value = Number(event.target.value);
                setOpacityDraft({ id: body.id, value });
                clearTimeout(opacityTimerRef.current);
                const id = body.id;
                opacityTimerRef.current = setTimeout(() => {
                  update(id, { opacity: value });
                  setOpacityDraft(null);
                }, 250);
              }}
            />
          </label>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={body.check}
              data-testid="mech-check"
              onChange={(event) => {
                update(body.id, { check: event.target.checked });
              }}
            />
            {t('view3d.mechanical.check')}
          </label>
          <p className="mech__file" title={body.file}>
            {t('view3d.mechanical.file')}: <code>{body.path}</code>
          </p>
          <div className="mech__actions">
            <button
              type="button"
              className="button"
              title={t('view3d.mechanical.centerTip')}
              data-testid="mech-center"
              onClick={center}
            >
              <Icon name="focus" size={16} />
              {t('view3d.mechanical.center')}
            </button>
            <button
              type="button"
              className="button button--danger"
              title={t('view3d.mechanical.removeTip')}
              data-testid="mech-remove"
              onClick={() => {
                onSelect(null);
                engine
                  .call('mechanical.remove', { doc, ids: [body.id] })
                  .catch((error: unknown) => {
                    flash(errorMessage(t, error));
                  });
              }}
            >
              <Icon name="trash" size={16} />
              {t('view3d.mechanical.remove')}
            </button>
          </div>
        </div>
      )}
      {bodies.length > 0 && (
        <section className="mech__helpers" aria-label={t('view3d.mechanical.helpers')}>
          <h3 className="mech__subtitle">{t('view3d.mechanical.helpers')}</h3>
          <div className="mech__actions">
            <button
              type="button"
              className="button"
              title={t('view3d.mechanical.outlineTip')}
              disabled={busy}
              data-testid="mech-outline"
              onClick={() => {
                void outline();
              }}
            >
              <Icon name="outline" size={16} />
              {t('view3d.mechanical.outline')}
            </button>
            <button
              type="button"
              className="button"
              title={t('view3d.mechanical.areasTip')}
              disabled={busy}
              data-testid="mech-areas"
              onClick={() => {
                void areas();
              }}
            >
              <Icon name="keepout" size={16} />
              {t('view3d.mechanical.areas')}
            </button>
          </div>
        </section>
      )}
      <section className="mech__results" aria-label={t('view3d.mechanical.results')}>
        <header className="mech__results-header">
          <h3 className="mech__subtitle">{t('view3d.mechanical.results')}</h3>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={live}
              data-testid="mech-live"
              onChange={(event) => {
                onLive(event.target.checked);
              }}
            />
            {t('view3d.mechanical.live')}
          </label>
          <button
            type="button"
            className="button"
            disabled={checking || bodies.length === 0}
            data-testid="mech-run"
            onClick={onRun}
          >
            {t('view3d.mechanical.run')}
          </button>
        </header>
        <p className="mech__summary" data-testid="mech-summary">
          {checking
            ? t('view3d.mechanical.checking')
            : check === null
              ? ''
              : findings.length === 0
                ? t('view3d.mechanical.clean', { clearance })
                : t('view3d.mechanical.summary', {
                    errors: check.errors,
                    warnings: check.warnings,
                    clearance,
                  })}
        </p>
        {findings.length > 0 && (
          <ul className="mech__findings">
            {findings.map((finding) => (
              <li key={finding.key}>
                <button
                  type="button"
                  className="mech__finding"
                  data-testid="mech-finding"
                  data-code={finding.code}
                  onClick={() => {
                    onFocus(finding);
                  }}
                >
                  <Icon name={finding.severity === 'error' ? 'error' : 'warning'} size={14} />
                  <span>{findingMessage(t, finding as unknown as DrcFinding)}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
        {check !== null && check.standIns.length > 0 && (
          <p className="mech__note" data-testid="mech-stand-ins">
            {t('view3d.mechanical.standIns', { count: check.standIns.length })}
          </p>
        )}
        {check !== null && check.unchecked.length > 0 && (
          <p className="mech__note">
            {t('view3d.mechanical.unchecked', { count: check.unchecked.length })}
          </p>
        )}
      </section>
    </aside>
  );
}
