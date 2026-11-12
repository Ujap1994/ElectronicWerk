import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from 'react';

import type { MessageKey } from '@shared/i18n';
import { formatLength, isEngineEvent, type Units } from '@shared/project';

import { Icon, type IconName } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { useDarkScheme } from '../use-dark-scheme';
import { zoomPercent } from '../viewer/camera';
import { ViewerController } from '../viewer/controller';
import { loadCanvasFont } from '../viewer/glyph-atlas';
import { layerLabel } from '../viewer/labels';
import type { SceneReply } from '../viewer/types';
import { ItemEditor, type ItemTool } from './item-editor';
import {
  DIRECTIONS,
  PIN_KINDS,
  type Body,
  type Direction,
  type ItemRef,
  type Pad,
  type PinKind,
  type Point,
} from './types';

const CSS_PX_PER_MM = 96 / 25.4;

/** Grid sizes offered per editor (nm). */
const GRIDS = {
  symbol: [254_000, 635_000, 1_270_000, 2_540_000],
  footprint: [10_000, 25_000, 50_000, 100_000, 250_000, 500_000, 1_000_000, 1_270_000],
} as const;

/** Pin lengths offered for new pins (nm): 100, 150, 200, 300 mil. */
const PIN_LENGTHS = [2_540_000, 3_810_000, 5_080_000, 7_620_000];

/** Layers for new footprint graphics. */
export const DRAW_LAYERS = [
  'silk.top',
  'fab.top',
  'courtyard.top',
  'cu.top',
  'silk.bottom',
  'fab.bottom',
  'courtyard.bottom',
  'cu.bottom',
] as const;

interface ToolSpec {
  readonly tool: ItemTool;
  readonly icon: IconName;
  readonly key: string;
}

function tools(kind: 'symbol' | 'footprint'): readonly ToolSpec[] {
  return [
    { tool: 'select', icon: 'pointer', key: 'Esc' },
    kind === 'symbol'
      ? { tool: 'pin', icon: 'pin', key: 'P' }
      : { tool: 'pad', icon: 'pad', key: 'P' },
    { tool: 'line', icon: 'line', key: 'L' },
    { tool: 'rect', icon: 'rectangle', key: 'B' },
    { tool: 'circle', icon: 'circle', key: 'C' },
    { tool: 'polyline', icon: 'polyline', key: 'K' },
    { tool: 'text', icon: 'text', key: 'T' },
  ];
}

function Overlay({
  controller,
  editor,
}: {
  readonly controller: ViewerController;
  readonly editor: ItemEditor;
}): ReactNode {
  const { t } = useAppState();
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  useSyncExternalStore(controller.subscribeView, controller.view);
  const [text, setText] = useState('');
  const screen = (p: Point): Point => controller.screenOf(p);
  const elements: ReactNode[] = [];
  // The origin of the item.
  const [ox, oy] = screen([0, 0]);
  elements.push(
    <path key="origin" className="origin-mark" d={`M${ox - 10} ${oy}h20M${ox} ${oy - 10}v20`} />,
  );
  const cursor = state.cursor;
  const first = state.points[0];
  if (cursor !== null && first !== undefined) {
    const points = [...state.points, cursor];
    if (state.tool === 'rect') {
      const [a, b] = [screen(first), screen(cursor)];
      elements.push(
        <rect
          key="rect"
          className="preview"
          x={Math.min(a[0], b[0])}
          y={Math.min(a[1], b[1])}
          width={Math.abs(b[0] - a[0])}
          height={Math.abs(b[1] - a[1])}
        />,
      );
    } else if (state.tool === 'circle') {
      const c = screen(first);
      const e = screen(cursor);
      elements.push(
        <circle
          key="circle"
          className="preview"
          cx={c[0]}
          cy={c[1]}
          r={Math.hypot(e[0] - c[0], e[1] - c[1])}
        />,
      );
    } else {
      elements.push(
        <polyline
          key="line"
          className="preview"
          points={points.map((p) => screen(p).join(',')).join(' ')}
          data-testid="preview-shape"
        />,
      );
    }
  }
  if (cursor !== null && state.tool !== 'select') {
    const [x, y] = screen(cursor);
    elements.push(
      <path key="cursor" className="snap-cursor" d={`M${x - 6} ${y}h12M${x} ${y - 6}v12`} />,
    );
  }
  const prompt = state.prompt;
  const at = prompt === null ? null : screen(prompt);
  return (
    <>
      <svg className="viewer__overlay editor-overlay" aria-hidden="true">
        {elements}
      </svg>
      {at !== null && (
        <form
          className="prompt"
          style={{ left: at[0] + 12, top: at[1] + 12 }}
          onSubmit={(event) => {
            event.preventDefault();
            editor.submitText(text);
            setText('');
          }}
        >
          <label className="prompt__label" htmlFor="item-text">
            {t('libedit.prompts.text')}
          </label>
          <input
            id="item-text"
            className="prompt__input"
            value={text}
            autoFocus
            data-testid="item-text-input"
            onChange={(event) => {
              setText(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                editor.cancelText();
              }
            }}
          />
        </form>
      )}
    </>
  );
}

/**
 * The drawing area of the symbol or footprint editor: the editor scene of
 * one item (one body of a symbol), drawing tools, grid and status line.
 */
export function ItemCanvas({
  doc,
  kind,
  name,
  body,
  units,
  pads,
  onBody,
  onSelection,
  controllerRef,
  toolbar,
}: {
  readonly doc: string;
  readonly kind: 'symbol' | 'footprint';
  readonly name: string;
  readonly body: Body;
  readonly units: Units;
  /** The pads of the footprint (the pad tool copies the selected one). */
  readonly pads: readonly Pad[];
  /** An edit chose another body (a new unit, the alternate body). */
  readonly onBody: (body: Body) => void;
  readonly onSelection: (items: readonly ItemRef[]) => void;
  /** Gives the parent the controller and editor (tables select through them). */
  readonly controllerRef: (
    handles: { controller: ViewerController; editor: ItemEditor } | null,
  ) => void;
  /** Extra controls for the toolbar (units, layers). */
  readonly toolbar?: ReactNode;
}): ReactNode {
  const { t, engine, snapshot: settings } = useAppState();
  const { notify } = useProject();
  const dark = useDarkScheme();
  const controller = useMemo(() => new ViewerController(), []);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const [loaded, setLoaded] = useState(false);
  const viewer = useSyncExternalStore(controller.subscribe, controller.snapshot);
  const view = useSyncExternalStore(controller.subscribeView, controller.view);

  const editor = useMemo(
    () =>
      new ItemEditor(controller, {
        kind,
        edit: async (edit, shown) => {
          try {
            const method = kind === 'symbol' ? 'libedit.symbol' : 'libedit.footprint';
            const reply = await engine.call<{ select: ItemRef[]; body?: Body }>(method, {
              doc,
              name,
              body: shown,
              edit,
            });
            const next = reply.body;
            if (
              next !== undefined &&
              (next.unit !== shown.unit || next.alternate !== shown.alternate)
            ) {
              onBody(next);
            }
            return reply.select;
          } catch (error) {
            notify(errorMessage(t, error), 'error');
            return null;
          }
        },
      }),
    [controller, kind, doc, name, engine, notify, t, onBody],
  );

  useEffect(() => {
    editor.setContext(body, pads);
  });

  useEffect(() => {
    controller.setInteraction(editor);
    controllerRef({ controller, editor });
    return () => {
      controller.setInteraction(null);
      controllerRef(null);
    };
  }, [controller, editor, controllerRef]);

  // Selection changes go to the parent (tables, properties).
  useEffect(() => {
    onSelection(editor.selection());
  }, [viewer.selection, editor, onSelection]);

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

  // The scene of the item, reloaded after every change.
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = async (): Promise<void> => {
      try {
        await loadCanvasFont();
        const reply = await engine.call<SceneReply>('view.item', {
          doc,
          kind,
          name,
          unit: body.unit,
          alternate: body.alternate,
        });
        if (!cancelled) {
          controller.setScene(reply);
          setLoaded(true);
        }
      } catch {
        // The item was renamed or deleted; the parent shows another one.
      }
    };
    void load();
    const off = engine.onEvent((event) => {
      if (isEngineEvent(event)) {
        clearTimeout(timer);
        timer = setTimeout(() => {
          void load();
        }, 30);
      }
    });
    return () => {
      cancelled = true;
      clearTimeout(timer);
      off();
    };
  }, [engine, controller, doc, kind, name, body.unit, body.alternate]);

  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  const length = (nm: number): string => formatLength(nm, units, settings.language);
  const percent = new Intl.NumberFormat(settings.language, { maximumSignificantDigits: 3 }).format(
    zoomPercent(view.scale, CSS_PX_PER_MM),
  );
  const hasSelection = viewer.selection.length > 0;
  return (
    <div className="item-canvas" data-testid="item-canvas">
      <div className="viewer__toolbar" role="toolbar" aria-label={t('libedit.toolbar')}>
        <button
          type="button"
          className="viewer-tool"
          title={`${t('viewer.fitAll')} (Home)`}
          aria-label={t('viewer.fitAll')}
          onClick={() => {
            controller.fitAll();
          }}
        >
          <Icon name="fit" size={18} />
        </button>
        <span className="viewer-tool__separator" />
        {tools(kind).map(({ tool, icon, key }) => {
          const label = t(`libedit.tools.${tool}` as MessageKey);
          return (
            <button
              key={tool}
              type="button"
              className="viewer-tool"
              title={`${label} (${key})`}
              aria-label={label}
              aria-pressed={state.tool === tool}
              data-testid={`item-tool-${tool}`}
              onClick={() => {
                editor.setTool(tool);
              }}
            >
              <Icon name={icon} size={18} />
            </button>
          );
        })}
        <span className="viewer-tool__separator" />
        {(
          [
            [
              'rotate',
              'libedit.actions.rotate',
              'R',
              () => {
                editor.orientSelection('left');
              },
            ],
            [
              'mirrorH',
              'libedit.actions.mirrorHorizontal',
              'Y',
              () => {
                editor.orientSelection('horizontal');
              },
            ],
            [
              'mirrorV',
              'libedit.actions.mirrorVertical',
              'X',
              () => {
                editor.orientSelection('vertical');
              },
            ],
            [
              'duplicate',
              'libedit.actions.duplicate',
              'Ctrl+D',
              () => {
                editor.duplicateSelection();
              },
            ],
            [
              'trash',
              'libedit.actions.delete',
              'Del',
              () => {
                editor.deleteSelection();
              },
            ],
          ] as const
        ).map(([icon, label, key, run]) => (
          <button
            key={icon}
            type="button"
            className="viewer-tool"
            title={`${t(label)} (${key})`}
            aria-label={t(label)}
            disabled={!hasSelection}
            data-testid={`item-action-${icon}`}
            onClick={run}
          >
            <Icon name={icon} size={18} />
          </button>
        ))}
      </div>
      <div className="item-canvas__options" data-testid="item-options">
        <label className="item-canvas__field">
          <span>{t('libedit.grid')}</span>
          <select
            value={state.grid}
            data-testid="item-grid"
            onChange={(event) => {
              editor.setGrid(Number(event.target.value));
            }}
          >
            {GRIDS[kind].map((grid) => (
              <option key={grid} value={grid}>
                {length(grid)}
              </option>
            ))}
          </select>
        </label>
        {kind === 'footprint' && (
          <label className="item-canvas__field">
            <span>{t('libedit.layer')}</span>
            <select
              value={state.layer}
              data-testid="item-layer"
              onChange={(event) => {
                editor.setLayer(event.target.value);
              }}
            >
              {DRAW_LAYERS.map((layer) => (
                <option key={layer} value={layer}>
                  {layerLabel(t, layer)}
                </option>
              ))}
            </select>
          </label>
        )}
        {toolbar}
        {kind === 'symbol' && state.tool === 'pin' && (
          <span className="unit-bar">
            <label className="item-canvas__field">
              <span>{t('libedit.pinDefaults.direction')}</span>
              <select
                value={state.direction}
                data-testid="pin-default-direction"
                onChange={(event) => {
                  editor.setPinDefaults({ direction: event.target.value as Direction });
                }}
              >
                {DIRECTIONS.map((direction) => (
                  <option key={direction} value={direction}>
                    {t(`libedit.directions.${direction}` as MessageKey)}
                  </option>
                ))}
              </select>
            </label>
            <label className="item-canvas__field">
              <span>{t('libedit.pinDefaults.kind')}</span>
              <select
                value={state.pinKind}
                data-testid="pin-default-kind"
                onChange={(event) => {
                  editor.setPinDefaults({ pinKind: event.target.value as PinKind });
                }}
              >
                {PIN_KINDS.map((pinKind) => (
                  <option key={pinKind} value={pinKind}>
                    {t(`schematic.erc.pinKinds.${pinKind}` as MessageKey)}
                  </option>
                ))}
              </select>
            </label>
            <label className="item-canvas__field">
              <span>{t('libedit.pinDefaults.length')}</span>
              <select
                value={state.pinLength}
                data-testid="pin-default-length"
                onChange={(event) => {
                  editor.setPinDefaults({ pinLength: Number(event.target.value) });
                }}
              >
                {PIN_LENGTHS.map((pinLength) => (
                  <option key={pinLength} value={pinLength}>
                    {length(pinLength)}
                  </option>
                ))}
              </select>
            </label>
          </span>
        )}
      </div>
      <div className="viewer__area" ref={areaRef}>
        <canvas
          ref={canvasRef}
          className="viewer__canvas"
          tabIndex={0}
          aria-label={t('libedit.canvas', { name })}
          data-testid="item-canvas-area"
          data-ready={viewer.status === 'ready' && loaded}
          data-version={viewer.version}
        />
        <Overlay controller={controller} editor={editor} />
      </div>
      <footer className="viewer__footer">
        <span className="viewer__cursor">
          {view.cursor === null
            ? ' '
            : t('viewer.cursor', { x: length(view.cursor[0]), y: length(view.cursor[1]) })}
        </span>
        <span className="viewer__hint">{t(state.hint)}</span>
        <span className="viewer__spacer" />
        <span>{t('viewer.zoom', { percent })}</span>
      </footer>
    </div>
  );
}
