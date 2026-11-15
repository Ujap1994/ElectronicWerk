/**
 * The assembly assistant: the board with the parts to place next, and the
 * steps (groups of equal parts) to tick off. Progress is saved by the
 * engine at once; every window showing the board follows it.
 */
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';

import { isEngineEvent } from '@shared/project';

import { Icon } from '../components/Icon';
import { useAppState } from '../state';
import { useDarkScheme } from '../use-dark-scheme';
import { ViewerController } from '../viewer/controller';
import { loadCanvasFont } from '../viewer/glyph-atlas';
import type { SceneReply } from '../viewer/types';

export type AssemblyOrder = 'size' | 'value' | 'count' | 'designator';
export type BoardSide = 'top' | 'bottom';

export interface AssemblyGroup {
  readonly index: number;
  readonly side: BoardSide;
  readonly mounting: 'smd' | 'throughHole' | 'other';
  readonly value: string;
  readonly footprint: string;
  readonly description: string;
  readonly mpn: string;
  readonly manufacturer: string;
  readonly designators: readonly string[];
  readonly ids: readonly string[];
  readonly count: number;
  readonly placed: number;
}

export interface AssemblyPlan {
  readonly board: string;
  readonly boardId: string;
  readonly groups: readonly AssemblyGroup[];
  readonly dnp: readonly string[];
  readonly missing: readonly string[];
  readonly placed: readonly string[];
  readonly total: number;
  readonly done: number;
  readonly completed: number;
}

const ORDERS: readonly AssemblyOrder[] = ['size', 'value', 'count', 'designator'];

/** Layers that only clutter the assembly view. */
const HIDDEN_GROUPS = new Set(['solderMask', 'paste', 'adhesive', 'user', 'mechanical']);

function isAssemblyEvent(value: unknown, board: string): boolean {
  const event = value as { type?: unknown; board?: unknown } | null;
  return event?.type === 'assembly' && event.board === board;
}

/** The first group with parts still to place, from `start` on (wrapping). */
function nextOpen(groups: readonly AssemblyGroup[], start: number): number {
  for (let step = 0; step < groups.length; step += 1) {
    const index = (start + step) % groups.length;
    const group = groups[index];
    if (group !== undefined && group.placed < group.count) {
      return index;
    }
  }
  return -1;
}

export function AssemblyView({
  doc,
  name,
  detached = false,
  onDetach,
}: {
  readonly doc: string;
  readonly name: string;
  readonly detached?: boolean;
  readonly onDetach?: () => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const dark = useDarkScheme();
  const controller = useMemo(() => new ViewerController(), []);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const [order, setOrder] = useState<AssemblyOrder>('size');
  const [firstSide, setFirstSide] = useState<BoardSide>('top');
  const [plan, setPlan] = useState<AssemblyPlan | null>(null);
  // The group the user went to; `pinned` keeps it shown once it is complete.
  const [chosen, setChosen] = useState({ index: 0, pinned: false });
  const [large, setLarge] = useState(detached);
  const [hideDone, setHideDone] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);
  const [sceneReady, setSceneReady] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // --- The plan ----------------------------------------------------------

  const fetchPlan = useCallback(
    () =>
      engine
        .whenConnected()
        .then(() => engine.call<AssemblyPlan>('assembly.plan', { board: doc, order, firstSide })),
    [engine, doc, order, firstSide],
  );

  /** Reads the plan again and shows it (or the error). */
  const loadPlan = useCallback(
    () =>
      fetchPlan().then(
        (next) => {
          setPlan(next);
          setError(null);
        },
        (reason: unknown) => {
          setError(reason instanceof Error ? reason.message : String(reason));
        },
      ),
    [fetchPlan],
  );

  useEffect(() => {
    let cancelled = false;
    fetchPlan().then(
      (next) => {
        if (!cancelled) {
          setPlan(next);
          setError(null);
        }
      },
      (reason: unknown) => {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [fetchPlan]);

  // Another window ticked parts off, or the design changed.
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const off = engine.onEvent((event) => {
      if (!isAssemblyEvent(event, doc) && !isEngineEvent(event)) {
        return;
      }
      clearTimeout(timer);
      timer = setTimeout(() => {
        void loadPlan();
      }, 30);
    });
    return () => {
      clearTimeout(timer);
      off();
    };
  }, [engine, doc, loadPlan]);

  const groups = useMemo(() => plan?.groups ?? [], [plan]);
  // A finished group moves on by itself (also when ticked off in another
  // window), unless the user went there on purpose.
  const chosenGroup = groups[chosen.index];
  const current =
    chosenGroup !== undefined && (chosen.pinned || chosenGroup.placed < chosenGroup.count)
      ? chosen.index
      : nextOpen(groups, chosenGroup === undefined ? 0 : chosen.index + 1);
  const group = groups[current] ?? null;

  // --- The board -----------------------------------------------------------

  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    const area = areaRef.current;
    if (canvas === null || area === null) {
      return undefined;
    }
    controller.attach(canvas);
    const observer = new ResizeObserver(() => {
      const rect = area.getBoundingClientRect();
      controller.resize(rect.width, rect.height, window.devicePixelRatio);
    });
    observer.observe(area);
    return () => {
      observer.disconnect();
      controller.detach();
    };
  }, [controller]);

  useEffect(() => {
    controller.setTheme(dark ? 'dark' : 'light');
  }, [controller, dark]);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = async (): Promise<void> => {
      try {
        await loadCanvasFont();
        await engine.whenConnected();
        const reply = await engine.call<SceneReply>('view.scene', { doc, labels: {} }, 120_000);
        if (!cancelled) {
          controller.setScene(reply, null);
          setSceneReady(true);
        }
      } catch (reason) {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      }
    };
    void load();
    const off = engine.onEvent((event) => {
      if (!isEngineEvent(event)) {
        return;
      }
      clearTimeout(timer);
      timer = setTimeout(() => {
        void load();
      }, 30);
    });
    return () => {
      cancelled = true;
      clearTimeout(timer);
      off();
    };
  }, [engine, controller, doc]);

  // Which side, which layers, which parts stand out.
  const side: BoardSide = group?.side ?? firstSide;
  const placed = useMemo(() => new Set(plan?.placed ?? []), [plan]);
  useEffect(() => {
    const scene = controller.currentScene;
    if (!sceneReady || scene === null) {
      return;
    }
    scene.layers.forEach((layer, index) => {
      const meta = layer.meta;
      const otherSide = meta.side !== null && meta.side !== side && meta.group !== 'holes';
      controller.setLayerVisible(index, !HIDDEN_GROUPS.has(meta.group) && !otherSide);
      controller.setLayerOpacity(index, meta.group === 'copper' ? 0.45 : 1);
    });
    if (controller.snapshot().flipped !== (side === 'bottom')) {
      controller.toggleFlip();
    }
    const upcoming: string[] = [];
    for (const entry of groups) {
      if (entry.index === group?.index) {
        continue;
      }
      entry.designators.forEach((designator, at) => {
        const id = entry.ids[at];
        if (id !== undefined && !placed.has(designator)) {
          upcoming.push(id);
        }
      });
    }
    controller.setDimmed(upcoming);
    // The parts of the current group still to place stand out.
    controller.setHighlight(
      (group?.ids ?? []).filter((_, at) => !placed.has(group?.designators[at] ?? '')),
    );
  }, [controller, sceneReady, side, groups, group, placed]);

  useEffect(() => {
    if (sceneReady) {
      controller.fitAll(false);
    }
  }, [controller, sceneReady, side]);

  // --- Actions -------------------------------------------------------------

  const mark = useCallback(
    async (designators: readonly string[], value: boolean): Promise<void> => {
      try {
        await engine.call('assembly.mark', { board: doc, designators, placed: value });
        await loadPlan();
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
      }
    },
    [engine, doc, loadPlan],
  );

  const placeCurrent = useCallback(() => {
    if (group !== null) {
      setChosen({ index: group.index, pinned: false });
      void mark(group.designators, true);
    }
  }, [group, mark]);

  const step = useCallback(
    (delta: number) => {
      if (groups.length === 0) {
        return;
      }
      const index = (Math.max(current, 0) + delta + groups.length) % groups.length;
      setChosen({ index, pinned: true });
    },
    [groups.length, current],
  );

  const restart = useCallback(
    async (completed: boolean) => {
      setConfirmReset(false);
      try {
        await engine.call('assembly.reset', { board: doc, completed });
        setChosen({ index: 0, pinned: false });
        await loadPlan();
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
      }
    },
    [engine, doc, loadPlan],
  );

  // Keys: Space/Enter = placed and next, arrows = back and skip.
  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      const target = event.target as HTMLElement | null;
      if (
        target !== null &&
        (target.closest('input, select, textarea, [contenteditable]') !== null ||
          (!(rootRef.current?.contains(target) ?? false) && target !== document.body))
      ) {
        return;
      }
      if (event.key === ' ' || event.key === 'Enter') {
        // Before the board view, which pans with the space key.
        event.preventDefault();
        event.stopPropagation();
        placeCurrent();
      } else if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
        event.preventDefault();
        step(1);
      } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
        event.preventDefault();
        step(-1);
      } else if (event.key === 'F11' && detached) {
        event.preventDefault();
        window.ew.window.toggleFullScreen();
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
    };
  }, [placeCurrent, step, detached]);

  // --- Layout --------------------------------------------------------------

  const total = plan?.total ?? 0;
  const done = plan?.done ?? 0;
  const shown = hideDone ? groups.filter((entry) => entry.placed < entry.count) : groups;
  const finished = plan !== null && total > 0 && done >= total;

  return (
    <div
      ref={rootRef}
      className={`assembly${large ? ' assembly--large' : ''}`}
      data-testid="assembly"
      data-detached={detached}
    >
      <div className="assembly__board">
        <div className="assembly__area" ref={areaRef}>
          <canvas
            ref={canvasRef}
            className="viewer__canvas"
            tabIndex={0}
            aria-label={t('assembly.canvas', { name })}
            data-testid="assembly-canvas"
            data-ready={sceneReady}
            data-side={side}
          />
          {side === 'bottom' && (
            <span className="assembly__badge" data-testid="assembly-from-bottom">
              {t('assembly.fromBottom')}
            </span>
          )}
          {!sceneReady && error === null && (
            <span className="assembly__loading">{t('assembly.loading')}</span>
          )}
        </div>
      </div>

      <aside className="assembly__panel" aria-label={t('assembly.title')}>
        <header className="assembly__header">
          <div className="assembly__title">
            <strong>{name}</strong>
            <span className="assembly__build" data-testid="assembly-build">
              {t('assembly.build', { number: (plan?.completed ?? 0) + 1 })}
            </span>
          </div>
          <div
            className="assembly__progress"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={total}
            aria-valuenow={done}
          >
            <span
              className="assembly__bar"
              style={{ width: `${total === 0 ? 0 : (100 * done) / total}%` }}
            />
          </div>
          <span className="assembly__count" data-testid="assembly-progress">
            {t('assembly.progress', { done, total })}
          </span>
        </header>

        {error !== null && (
          <p className="field__error" role="alert">
            {error}
          </p>
        )}

        {plan !== null && groups.length === 0 && (
          <p className="field__hint" data-testid="assembly-empty">
            {t('assembly.noParts')}
          </p>
        )}

        {finished ? (
          <section
            className="assembly__current assembly__current--done"
            data-testid="assembly-done"
          >
            <Icon name="check" size={large ? 40 : 28} />
            <h2>{t('assembly.allDone')}</h2>
            <p>{t('assembly.allDoneHint')}</p>
            <button
              type="button"
              className="button button--primary"
              title={t('assembly.finishTip')}
              data-testid="assembly-finish"
              onClick={() => {
                void restart(true);
              }}
            >
              {t('assembly.finish')}
            </button>
          </section>
        ) : (
          group !== null && (
            <section className="assembly__current" data-testid="assembly-current">
              <span className="assembly__step">
                {t('assembly.step', { number: current + 1, total: groups.length })} ·{' '}
                {t(`assembly.sides.${group.side}`)} · {t(`assembly.mountings.${group.mounting}`)}
              </span>
              <h2 className="assembly__value" data-testid="assembly-value">
                {group.value}
              </h2>
              <dl className="assembly__facts">
                <dt className="assembly__term">{t('assembly.footprint')}</dt>
                <dd className="assembly__fact">{group.footprint}</dd>
                <dt className="assembly__term">{t('assembly.quantity')}</dt>
                <dd className="assembly__fact" data-testid="assembly-quantity">
                  {group.count - group.placed} / {group.count}
                </dd>
                {group.mpn !== '' && (
                  <>
                    <dt className="assembly__term">{t('assembly.mpn')}</dt>
                    <dd className="assembly__fact">{group.mpn}</dd>
                  </>
                )}
                {group.manufacturer !== '' && (
                  <>
                    <dt className="assembly__term">{t('assembly.manufacturer')}</dt>
                    <dd className="assembly__fact">{group.manufacturer}</dd>
                  </>
                )}
              </dl>
              <div className="assembly__parts" aria-label={t('assembly.designators')}>
                {group.designators.map((designator) => {
                  const isPlaced = placed.has(designator);
                  return (
                    <button
                      key={designator}
                      type="button"
                      className="assembly__part"
                      aria-pressed={isPlaced}
                      title={t('assembly.partTip', { designator })}
                      data-testid="assembly-part"
                      onClick={() => {
                        void mark([designator], !isPlaced);
                      }}
                    >
                      {isPlaced && <Icon name="check" size={large ? 18 : 12} />}
                      {designator}
                    </button>
                  );
                })}
              </div>
              <div className="assembly__actions">
                <button
                  type="button"
                  className="button"
                  title={t('assembly.backTip')}
                  data-testid="assembly-back"
                  onClick={() => {
                    step(-1);
                  }}
                >
                  {t('assembly.back')}
                </button>
                <button
                  type="button"
                  className="button button--primary assembly__place"
                  title={t('assembly.placeAllTip')}
                  data-testid="assembly-place"
                  onClick={placeCurrent}
                >
                  <Icon name="check" size={large ? 22 : 16} />
                  {t('assembly.placeAll')}
                </button>
                <button
                  type="button"
                  className="button"
                  title={t('assembly.skipTip')}
                  data-testid="assembly-skip"
                  onClick={() => {
                    step(1);
                  }}
                >
                  {t('assembly.skip')}
                </button>
              </div>
              <span className="field__hint assembly__keys">{t('assembly.keys')}</span>
            </section>
          )
        )}

        <section className="assembly__steps" aria-label={t('assembly.steps')}>
          <div className="assembly__steps-head">
            <h3 className="assembly__steps-title">{t('assembly.steps')}</h3>
            <label className="checkbox">
              <input
                type="checkbox"
                checked={hideDone}
                data-testid="assembly-hide-done"
                onChange={(event) => {
                  setHideDone(event.target.checked);
                }}
              />
              <span>{t('assembly.hideDone')}</span>
            </label>
          </div>
          <ol className="assembly__list" data-testid="assembly-steps">
            {shown.map((entry) => {
              const complete = entry.placed >= entry.count;
              return (
                <li key={`${entry.side}-${String(entry.index)}`}>
                  <button
                    type="button"
                    className="assembly__row"
                    aria-current={entry.index === current ? 'step' : undefined}
                    data-done={complete}
                    data-testid="assembly-step"
                    onClick={() => {
                      setChosen({ index: entry.index, pinned: true });
                    }}
                  >
                    <span className="assembly__tick" aria-hidden="true">
                      {complete && <Icon name="check" size={12} />}
                    </span>
                    <span className="assembly__row-value">{entry.value}</span>
                    <span className="assembly__row-footprint">{entry.footprint}</span>
                    <span className="assembly__row-count">
                      {entry.placed}/{entry.count}
                    </span>
                    <span className={`assembly__side assembly__side--${entry.side}`}>
                      {t(`assembly.sides.${entry.side}`)}
                    </span>
                  </button>
                </li>
              );
            })}
          </ol>
        </section>

        {plan !== null && plan.dnp.length > 0 && (
          <p className="field__hint" data-testid="assembly-dnp">
            {t('assembly.dnp', { list: plan.dnp.join(', ') })}
          </p>
        )}
        {plan !== null && plan.missing.length > 0 && (
          <p className="field__hint">{t('assembly.missing', { list: plan.missing.join(', ') })}</p>
        )}

        <footer className="assembly__footer">
          <label className="field assembly__option">
            <span className="field__label">{t('assembly.order')}</span>
            <select
              className="field__control"
              value={order}
              data-testid="assembly-order"
              onChange={(event) => {
                setOrder(ORDERS.find((entry) => entry === event.target.value) ?? 'size');
                setChosen({ index: 0, pinned: false });
              }}
            >
              {ORDERS.map((entry) => (
                <option key={entry} value={entry}>
                  {t(`assembly.orders.${entry}`)}
                </option>
              ))}
            </select>
          </label>
          <label className="field assembly__option">
            <span className="field__label">{t('assembly.firstSide')}</span>
            <select
              className="field__control"
              value={firstSide}
              data-testid="assembly-first-side"
              onChange={(event) => {
                setFirstSide(event.target.value === 'bottom' ? 'bottom' : 'top');
                setChosen({ index: 0, pinned: false });
              }}
            >
              <option value="top">{t('assembly.sides.top')}</option>
              <option value="bottom">{t('assembly.sides.bottom')}</option>
            </select>
          </label>
          <div className="assembly__tools">
            <label className="checkbox" title={t('assembly.largeTip')}>
              <input
                type="checkbox"
                checked={large}
                data-testid="assembly-large"
                onChange={(event) => {
                  setLarge(event.target.checked);
                }}
              />
              <span>{t('assembly.large')}</span>
            </label>
            {detached ? (
              <button
                type="button"
                className="button button--small"
                title={t('assembly.fullscreenTip')}
                data-testid="assembly-fullscreen"
                onClick={() => {
                  window.ew.window.toggleFullScreen();
                }}
              >
                <Icon name="window" size={14} />
                {t('assembly.fullscreen')}
              </button>
            ) : (
              onDetach !== undefined && (
                <button
                  type="button"
                  className="button button--small"
                  title={t('assembly.detachTip')}
                  data-testid="assembly-detach"
                  onClick={onDetach}
                >
                  <Icon name="window" size={14} />
                  {t('assembly.detach')}
                </button>
              )
            )}
            {!finished && (
              <button
                type="button"
                className="button button--small"
                title={t('assembly.finishTip')}
                data-testid="assembly-next-board"
                disabled={done === 0}
                onClick={() => {
                  void restart(true);
                }}
              >
                {t('assembly.finish')}
              </button>
            )}
            <button
              type="button"
              className="button button--small"
              title={t('assembly.resetTip')}
              data-testid="assembly-reset"
              disabled={done === 0}
              onClick={() => {
                if (confirmReset) {
                  void restart(false);
                } else {
                  setConfirmReset(true);
                }
              }}
            >
              {confirmReset ? t('assembly.resetConfirm') : t('assembly.reset')}
            </button>
          </div>
          {plan !== null && plan.completed > 0 && (
            <span className="field__hint">
              {t('assembly.buildsDone', { count: plan.completed })}
            </span>
          )}
        </footer>
      </aside>
    </div>
  );
}
