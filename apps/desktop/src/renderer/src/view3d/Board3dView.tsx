/**
 * The 3D view of a board: a tab of the project or its own window. It
 * follows every change of the board live (parts move at once, the copper
 * and the mask follow), shares the selection with the layout editor and
 * exports the board as STEP, GLB or VRML.
 */
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useReducer,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from 'react';

import type { MessageKey } from '@shared/i18n';

import { crossSelection } from '../board/stores';
import { Icon, type IconName } from '../components/Icon';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { useDarkScheme } from '../use-dark-scheme';
import { loadModelGlb } from './models';
import type { Board3dScene } from './scene3d';
import { setView3dSettings, view3dSettings } from './stores';
import type {
  ExportReply,
  ModelSource,
  Part3d,
  PartsReply,
  SceneReply,
  Section,
  ViewPreset,
  Visibility,
} from './types';

const NONE: readonly string[] = [];

const PRESETS: readonly ViewPreset[] = ['top', 'bottom', 'front', 'back', 'left', 'right'];

const SHOW: readonly (keyof Visibility)[] = [
  'partsTop',
  'partsBottom',
  'dnp',
  'silk',
  'mask',
  'copper',
  'paste',
  'body',
  'inner',
];

type Popover = 'views' | 'show' | 'section' | 'export' | 'missing' | null;

function Tool({
  icon,
  label,
  pressed,
  disabled = false,
  testId,
  onClick,
}: {
  readonly icon: IconName;
  readonly label: string;
  readonly pressed?: boolean;
  readonly disabled?: boolean;
  readonly testId: string;
  readonly onClick: () => void;
}): ReactNode {
  return (
    <button
      type="button"
      className="viewer-tool"
      title={label}
      aria-label={label}
      aria-pressed={pressed}
      disabled={disabled}
      data-testid={testId}
      onClick={onClick}
    >
      <Icon name={icon} size={18} />
    </button>
  );
}

/** Length with three decimals. */
function mm(value: number): string {
  return `${value.toFixed(3)} mm`;
}

/** Changed objects of a change event that belong to `doc`. */
function changesOf(event: unknown, doc: string): { any: boolean; parts: boolean } {
  const record = event as {
    type?: string;
    changeSet?: {
      truncated?: boolean;
      changes?: readonly { doc?: string; type?: string; op?: string }[];
    };
  } | null;
  if (record?.type !== 'changed' || record.changeSet === undefined) {
    return { any: false, parts: false };
  }
  if (record.changeSet.truncated === true) {
    return { any: true, parts: true };
  }
  const mine = (record.changeSet.changes ?? []).filter((c) => c.doc === doc);
  return {
    any: mine.length > 0,
    parts: mine.some((c) => c.type === 'footprint' || c.op === 'props'),
  };
}

export function Board3dView({
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
  const { engine, t } = useAppState();
  const dark = useDarkScheme();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const [scene, setScene] = useState<Board3dScene | null>(null);
  const settings = useSyncExternalStore(view3dSettings.subscribe, view3dSettings.get);
  const [popover, setPopover] = useState<Popover>(null);
  const [section, setSection] = useState<Section>({ axis: null, position: 0.5, flip: false });
  const [measuring, setMeasuring] = useState(false);
  const [measure, setMeasure] = useState<readonly (readonly [number, number, number])[]>([]);
  const [, redraw] = useReducer((version: number) => version + 1, 0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [warnings, setWarnings] = useState<readonly string[]>([]);
  const [parts, setParts] = useState<readonly Part3d[]>([]);
  // Applied part updates (live updates; for tests).
  const [updates, setUpdates] = useState(0);
  const [sources, setSources] = useState<readonly ModelSource[]>([]);
  const [modelState, setModelState] = useState<{ loaded: number; failed: readonly string[] }>({
    loaded: 0,
    failed: [],
  });
  const sourcesRef = useRef<readonly ModelSource[]>([]);
  const releaseRef = useRef<string | null>(null);

  // three.js is loaded on first use.
  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    const area = areaRef.current;
    if (canvas === null || area === null) {
      return undefined;
    }
    let disposed = false;
    let created: Board3dScene | null = null;
    let observer: ResizeObserver | null = null;
    void import('./scene3d').then(({ Board3dScene: View }) => {
      if (disposed) {
        return;
      }
      created = new View(canvas);
      observer = new ResizeObserver(() => {
        const rect = area.getBoundingClientRect();
        created?.resize(rect.width, rect.height);
      });
      observer.observe(area);
      setScene(created);
    });
    return () => {
      disposed = true;
      observer?.disconnect();
      created?.dispose();
    };
  }, []);

  useEffect(() => {
    scene?.setTheme(dark);
  }, [scene, dark]);

  useEffect(() => {
    scene?.setVisibility(settings.visibility);
  }, [scene, settings.visibility]);

  useEffect(() => {
    scene?.setProjection(settings.projection);
  }, [scene, settings.projection]);

  useEffect(() => {
    scene?.setSection(section);
  }, [scene, section]);

  useEffect(() => {
    void engine
      .call<{ status: { release?: string } | null }>('library.status')
      .then((status) => {
        releaseRef.current = status.status?.release ?? null;
      })
      .catch(() => undefined);
  }, [engine]);

  // Models: loaded once per path, reported for the status line.
  useEffect(() => {
    if (scene === null) {
      return;
    }
    scene.setCallbacks({
      loadModel: (path) =>
        loadModelGlb(
          engine,
          sourcesRef.current.find((s) => s.path === path),
          releaseRef.current,
        ),
      onModelLoaded: (path, ok) => {
        setModelState((state) => ({
          loaded: state.loaded + (ok ? 1 : 0),
          failed: ok ? state.failed : [...state.failed, path],
        }));
      },
    });
  }, [scene, engine]);

  // The board, and live updates after every change.
  useEffect(() => {
    if (scene === null) {
      return undefined;
    }
    let cancelled = false;
    let reloadTimer: ReturnType<typeof setTimeout> | undefined;
    let busy = false;
    let again = false;
    const loadScene = async (): Promise<void> => {
      if (busy) {
        again = true;
        return;
      }
      busy = true;
      try {
        const inner = view3dSettings.get().visibility.inner;
        const reply = await engine.call<SceneReply>('board3d.scene', { doc, inner }, 120_000);
        if (cancelled) {
          return;
        }
        sourcesRef.current = reply.models;
        setSources(reply.models);
        scene.setBoard(reply);
        scene.setParts(reply.parts);
        setParts(reply.parts);
        setWarnings(reply.board.warnings);
        setError(null);
      } catch (reason) {
        if (!cancelled) {
          setError(errorMessage(t, reason));
        }
      } finally {
        busy = false;
        if (!cancelled) {
          setLoading(false);
        }
        if (again && !cancelled) {
          again = false;
          void loadScene();
        }
      }
    };
    const loadParts = async (): Promise<void> => {
      try {
        const reply = await engine.call<PartsReply>('board3d.parts', { doc });
        if (!cancelled) {
          sourcesRef.current = reply.models;
          setSources(reply.models);
          scene.setParts(reply.parts);
          setParts(reply.parts);
          setUpdates((count) => count + 1);
        }
      } catch {
        // The full reload follows.
      }
    };
    void loadScene();
    const off = engine.onEvent((event) => {
      const change = changesOf(event, doc);
      if (!change.any) {
        return;
      }
      // Parts move at once; the layers follow when the edits pause.
      if (change.parts) {
        void loadParts();
      }
      clearTimeout(reloadTimer);
      reloadTimer = setTimeout(() => {
        void loadScene();
      }, 200);
    });
    return () => {
      cancelled = true;
      clearTimeout(reloadTimer);
      off();
    };
  }, [scene, engine, doc, t, settings.visibility.inner]);

  // Selection: shared with the layout editor (and its window).
  const shared = useSyncExternalStore(crossSelection.subscribe, crossSelection.get);
  const selected = shared?.footprints ?? NONE;

  useEffect(() => {
    scene?.setSelection(selected);
  }, [scene, selected]);

  useEffect(() => {
    if (scene === null) {
      return;
    }
    const onPick = (id: string | null, additive: boolean): void => {
      const current = crossSelection.get()?.footprints ?? [];
      const footprints =
        id === null
          ? []
          : additive
            ? current.includes(id)
              ? current.filter((f) => f !== id)
              : [...current, id]
            : [id];
      crossSelection.set({ source: '3d', footprints, symbols: [] });
      if (footprints.length > 0) {
        void engine
          .call<{ symbols: { doc: string; id: string }[] }>('pcb.crossProbe', {
            doc,
            footprints,
          })
          .then((reply) => {
            const now = crossSelection.get();
            if (now?.source === '3d' && now.footprints.join() === footprints.join()) {
              crossSelection.set({ source: '3d', footprints, symbols: reply.symbols });
            }
          })
          .catch(() => undefined);
      }
    };
    scene.setCallbacks({
      onPick,
      onMeasure: (points) => {
        setMeasure(points);
      },
    });
  }, [scene, engine, doc]);

  // Overlays follow the camera.
  useEffect(() => {
    if (scene === null) {
      return undefined;
    }
    return scene.onView(redraw);
  }, [scene]);

  useEffect(() => {
    scene?.setMeasuring(measuring);
  }, [scene, measuring]);

  const stopMeasuring = useCallback(() => {
    setMeasuring(false);
    setMeasure([]);
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') {
        stopMeasuring();
        setPopover(null);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [stopMeasuring]);

  const flash = useCallback((text: string) => {
    setMessage(text);
    setTimeout(() => {
      setMessage((current) => (current === text ? null : current));
    }, 6000);
  }, []);

  const setVisibility = (key: keyof Visibility, on: boolean): void => {
    const current = view3dSettings.get();
    setView3dSettings({ ...current, visibility: { ...current.visibility, [key]: on } });
  };

  const screenshot = async (): Promise<void> => {
    if (scene === null) {
      return;
    }
    try {
      const png = await scene.screenshot();
      const path = await window.ew.dialogs.saveImage(`${name}-3d.png`, png);
      if (path !== null) {
        flash(t('view3d.screenshotSaved', { path }));
      }
    } catch (reason) {
      flash(errorMessage(t, reason));
    }
  };

  const exportModel = async (format: 'step' | 'glb' | 'vrml'): Promise<void> => {
    setPopover(null);
    const extension = format === 'vrml' ? 'wrl' : format;
    const path = await window.ew.dialogs.chooseModelFile(format, `${name}.${extension}`);
    if (path === null) {
      return;
    }
    setMessage(t('view3d.exporting'));
    try {
      const reply = await engine.call<ExportReply>(
        'board3d.export',
        { doc, path, format, origin: 'drill' },
        300_000,
      );
      const missing =
        reply.missing.length > 0
          ? ` · ${t('view3d.exportMissing', { count: reply.missing.length })}`
          : '';
      flash(`${t('view3d.exported', { path: reply.path, parts: reply.placed })}${missing}`);
    } catch (reason) {
      flash(errorMessage(t, reason));
    }
  };

  // Models: every distinct path of the parts.
  const paths = new Set(parts.flatMap((p) => p.models.map((m) => m.path)));
  const notFound = sources
    .filter((s) => paths.has(s.path) && s.file === undefined && s.entry === undefined)
    .map((s) => s.path);
  const missing = [...new Set([...notFound, ...modelState.failed])];
  const total = paths.size;
  const done = Math.min(total, modelState.loaded + missing.length);
  const selectedPart =
    selected.length === 1 ? (parts.find((p) => p.id === selected[0]) ?? null) : null;

  const measureLabel = (): ReactNode => {
    const [a, b] = measure;
    if (scene === null || a === undefined || b === undefined) {
      return null;
    }
    const mid: [number, number, number] = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
    const at = scene.project(mid);
    if (at === null) {
      return null;
    }
    const d = Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]);
    return (
      <div
        className="view3d__measure"
        style={{ left: at.x, top: at.y }}
        data-testid="view3d-measure-label"
      >
        <strong>{t('view3d.distance', { d: mm(d) })}</strong>
        <span>
          {t('view3d.delta', {
            x: mm(Math.abs(b[0] - a[0])),
            y: mm(Math.abs(b[1] - a[1])),
            z: mm(Math.abs(b[2] - a[2])),
          })}
        </span>
      </div>
    );
  };

  const sectionAxis = (axis: Section['axis'], label: MessageKey): ReactNode => (
    <label className="view3d__radio">
      <input
        type="radio"
        name="view3d-section"
        checked={section.axis === axis}
        data-testid={`view3d-section-${axis ?? 'off'}`}
        onChange={() => {
          setSection({ ...section, axis });
        }}
      />
      <span>{t(label)}</span>
    </label>
  );

  return (
    <div className="view3d" data-testid="view3d" data-detached={detached}>
      <div className="viewer__toolbar" role="toolbar" aria-label={t('view3d.toolbar')}>
        <Tool
          icon="fit"
          label={`${t('view3d.fit')} (Home)`}
          testId="view3d-fit"
          onClick={() => {
            scene?.fit();
          }}
        />
        <Tool
          icon="cube"
          label={t('view3d.viewTip', { view: t('view3d.views.iso') })}
          testId="view3d-view-iso"
          onClick={() => {
            scene?.view('iso');
          }}
        />
        <Tool
          icon="views"
          label={t('view3d.viewsMenu')}
          pressed={popover === 'views'}
          testId="view3d-views"
          onClick={() => {
            setPopover(popover === 'views' ? null : 'views');
          }}
        />
        <Tool
          icon="perspective"
          label={
            settings.projection === 'perspective'
              ? t('view3d.perspective')
              : t('view3d.orthographic')
          }
          pressed={settings.projection === 'orthographic'}
          testId="view3d-projection"
          onClick={() => {
            setView3dSettings({
              ...settings,
              projection: settings.projection === 'perspective' ? 'orthographic' : 'perspective',
            });
          }}
        />
        <span className="viewer-tool__separator" />
        <Tool
          icon="layers"
          label={t('view3d.layers')}
          pressed={popover === 'show'}
          testId="view3d-show"
          onClick={() => {
            setPopover(popover === 'show' ? null : 'show');
          }}
        />
        <Tool
          icon="section"
          label={t('view3d.sectionTip')}
          pressed={section.axis !== null || popover === 'section'}
          testId="view3d-section"
          onClick={() => {
            setPopover(popover === 'section' ? null : 'section');
          }}
        />
        <Tool
          icon="measure"
          label={t('view3d.measureTip')}
          pressed={measuring}
          testId="view3d-measure"
          onClick={() => {
            if (measuring) {
              stopMeasuring();
            } else {
              setMeasuring(true);
            }
          }}
        />
        <span className="viewer-tool__separator" />
        <Tool
          icon="camera"
          label={t('view3d.screenshotTip')}
          testId="view3d-screenshot"
          onClick={() => {
            void screenshot();
          }}
        />
        <Tool
          icon="download"
          label={t('view3d.exportTip')}
          pressed={popover === 'export'}
          testId="view3d-export"
          onClick={() => {
            setPopover(popover === 'export' ? null : 'export');
          }}
        />
        {!detached && onDetach !== undefined && (
          <Tool
            icon="window"
            label={t('view3d.detachTip')}
            testId="view3d-detach"
            onClick={onDetach}
          />
        )}
        {popover === 'views' && (
          <div className="popover view3d__popover" role="menu" data-testid="view3d-views-menu">
            {PRESETS.map((preset) => (
              <button
                key={preset}
                type="button"
                role="menuitem"
                className="popover__item"
                data-testid={`view3d-view-${preset}`}
                onClick={() => {
                  scene?.view(preset);
                  setPopover(null);
                }}
              >
                {t(`view3d.views.${preset}`)}
              </button>
            ))}
          </div>
        )}
        {popover === 'show' && (
          <div className="popover view3d__popover" data-testid="view3d-show-menu">
            {SHOW.map((key) => (
              <label key={key} className="view3d__check">
                <input
                  type="checkbox"
                  checked={settings.visibility[key]}
                  data-testid={`view3d-show-${key}`}
                  onChange={(event) => {
                    setVisibility(key, event.target.checked);
                  }}
                />
                <span>{t(`view3d.show.${key}`)}</span>
              </label>
            ))}
          </div>
        )}
        {popover === 'section' && (
          <div className="popover view3d__popover" data-testid="view3d-section-menu">
            {sectionAxis(null, 'view3d.sectionOff')}
            {sectionAxis('x', 'view3d.sectionX')}
            {sectionAxis('y', 'view3d.sectionY')}
            {sectionAxis('z', 'view3d.sectionZ')}
            <label className="view3d__slider">
              <span>{t('view3d.sectionPosition')}</span>
              <input
                type="range"
                min={0}
                max={1}
                step={0.005}
                value={section.position}
                disabled={section.axis === null}
                data-testid="view3d-section-position"
                onChange={(event) => {
                  setSection({ ...section, position: Number(event.target.value) });
                }}
              />
            </label>
            <label className="view3d__check">
              <input
                type="checkbox"
                checked={section.flip}
                disabled={section.axis === null}
                data-testid="view3d-section-flip"
                onChange={(event) => {
                  setSection({ ...section, flip: event.target.checked });
                }}
              />
              <span>{t('view3d.sectionFlip')}</span>
            </label>
          </div>
        )}
        {popover === 'export' && (
          <div className="popover view3d__popover" role="menu" data-testid="view3d-export-menu">
            {(['step', 'glb', 'vrml'] as const).map((format) => (
              <button
                key={format}
                type="button"
                role="menuitem"
                className="popover__item"
                data-testid={`view3d-export-${format}`}
                onClick={() => {
                  void exportModel(format);
                }}
              >
                {t(
                  format === 'step'
                    ? 'view3d.exportStep'
                    : format === 'glb'
                      ? 'view3d.exportGlb'
                      : 'view3d.exportVrml',
                )}
              </button>
            ))}
          </div>
        )}
        {popover === 'missing' && (
          <div className="popover view3d__popover view3d__missing" data-testid="view3d-missing">
            <strong>{t('view3d.missingTitle')}</strong>
            <ul>
              {missing.map((path) => (
                <li key={path}>
                  <code>{path}</code>
                </li>
              ))}
            </ul>
            <p>{t('view3d.missingHint')}</p>
          </div>
        )}
      </div>
      <div className="viewer__area view3d__area" ref={areaRef}>
        <canvas
          ref={canvasRef}
          className="viewer__canvas"
          tabIndex={0}
          aria-label={t('view3d.canvas', { name })}
          data-testid="view3d-canvas"
          data-ready={!loading && error === null}
          data-models={`${done}/${total}`}
          data-updates={updates}
          onKeyDown={(event) => {
            if (event.key === 'Home') {
              scene?.fit();
            }
          }}
        />
        {loading && (
          <div className="view3d__loading" data-testid="view3d-loading">
            {t('view3d.loading')}
          </div>
        )}
        {error !== null && (
          <p className="viewer__message callout callout--error" role="alert">
            {error}
          </p>
        )}
        {measuring && measure.length < 2 && (
          <div className="view3d__hint" data-testid="view3d-measure-hint">
            {measure.length === 0 ? t('view3d.measureHint') : t('view3d.measureSecond')}
          </div>
        )}
        {measureLabel()}
        <div className="view3d__status" data-testid="view3d-status">
          {selectedPart !== null && (
            <span className="view3d__selected" data-testid="view3d-selected">
              {t('view3d.selected', {
                designator: selectedPart.designator,
                value: selectedPart.value,
                footprint: selectedPart.footprint,
              })}
              {selectedPart.models.length === 0 && ` · ${t('view3d.noModel')}`}
            </span>
          )}
          {total > 0 && (
            <span data-testid="view3d-models">
              {done < total
                ? t('view3d.modelsLoading', { loaded: done, total })
                : t('view3d.models', { loaded: total - missing.length, total })}
            </span>
          )}
          {missing.length > 0 && (
            <button
              type="button"
              className="view3d__link"
              data-testid="view3d-missing-button"
              onClick={() => {
                setPopover(popover === 'missing' ? null : 'missing');
              }}
            >
              <Icon name="warning" size={14} />
              {t('view3d.modelsMissing', { count: missing.length })}
            </button>
          )}
          {warnings.length > 0 && (
            <span className="view3d__warning" title={warnings.join('\n')}>
              <Icon name="info" size={14} />
              {warnings[0]}
            </span>
          )}
          {message !== null && (
            <span className="view3d__message" role="status" data-testid="view3d-message">
              {message}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
