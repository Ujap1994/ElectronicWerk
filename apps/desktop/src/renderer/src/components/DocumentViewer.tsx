import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from 'react';

import { formatLength, isEngineEvent, type DocumentSummary, type Units } from '@shared/project';

import { useAppState } from '../state';
import { useDarkScheme } from '../use-dark-scheme';
import { zoomPercent } from '../viewer/camera';
import { ViewerController, type ViewMemory } from '../viewer/controller';
import { loadCanvasFont } from '../viewer/glyph-atlas';
import { typeLabel } from '../viewer/labels';
import type { SceneReply } from '../viewer/types';
import { Icon, type IconName } from './Icon';
import { Inspector } from './Inspector';
import { LayerPanel } from './LayerPanel';

/** View settings per document, kept while the project is open. */
const memories = new Map<string, ViewMemory>();

/** Forgets the view settings (when a project closes). */
export function forgetViewMemories(): void {
  memories.clear();
}

/** CSS pixels per millimetre at 100 % zoom (96 dpi). */
const CSS_PX_PER_MM = 96 / 25.4;

function ToolButton({
  icon,
  label,
  pressed,
  testId,
  onClick,
}: {
  readonly icon: IconName;
  readonly label: string;
  readonly pressed?: boolean;
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
      data-testid={testId}
      onClick={onClick}
    >
      <Icon name={icon} size={18} />
    </button>
  );
}

function Overlay({ controller }: { readonly controller: ViewerController }): ReactNode {
  const view = useSyncExternalStore(controller.subscribeView, controller.view);
  const overlay = view.overlay;
  if (overlay === null) {
    return null;
  }
  return (
    <svg className="viewer__overlay" aria-hidden="true">
      {overlay.kind === 'box' ? (
        <rect
          className="viewer__box"
          data-crossing={overlay.crossing}
          x={Math.min(overlay.x0, overlay.x1)}
          y={Math.min(overlay.y0, overlay.y1)}
          width={Math.abs(overlay.x1 - overlay.x0)}
          height={Math.abs(overlay.y1 - overlay.y0)}
        />
      ) : (
        <polygon className="viewer__lasso" points={overlay.points.join(' ')} />
      )}
    </svg>
  );
}

function Footer({
  controller,
  units,
}: {
  readonly controller: ViewerController;
  readonly units: Units;
}): ReactNode {
  const { t, snapshot } = useAppState();
  const view = useSyncExternalStore(controller.subscribeView, controller.view);
  const locale = snapshot.language;
  const length = (nm: number): string => formatLength(nm, units, locale);
  const percent = new Intl.NumberFormat(locale, { maximumSignificantDigits: 3 }).format(
    zoomPercent(view.scale, CSS_PX_PER_MM),
  );
  return (
    <footer className="viewer__footer" data-testid="viewer-footer">
      <span className="viewer__cursor" data-testid="viewer-cursor">
        {view.cursor === null
          ? ' '
          : t('viewer.cursor', { x: length(view.cursor[0]), y: length(view.cursor[1]) })}
      </span>
      <span className="viewer__hover" data-testid="viewer-hover">
        {view.hover === null
          ? ''
          : view.hover.net === null
            ? typeLabel(t, view.hover.type)
            : `${typeLabel(t, view.hover.type)} · ${view.hover.net}`}
      </span>
      <span className="viewer__spacer" />
      {view.grid > 0 && <span>{t('viewer.grid', { size: length(view.grid) })}</span>}
      <span data-testid="viewer-zoom">{t('viewer.zoom', { percent })}</span>
    </footer>
  );
}

/** The 2D view of a sheet or board, with layer panel and inspector. */
export function DocumentViewer({
  document,
  units,
}: {
  readonly document: DocumentSummary;
  readonly units: Units;
}): ReactNode {
  const { t, engine } = useAppState();
  const controller = useMemo(() => new ViewerController(), []);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const snapshot = useSyncExternalStore(controller.subscribe, controller.snapshot);
  const dark = useDarkScheme();
  const doc = document.id;

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
      const memory = controller.exportMemory();
      if (memory !== null) {
        memories.set(doc, memory);
      }
      controller.detach();
    };
  }, [controller, doc]);

  useEffect(() => {
    controller.setTheme(dark ? 'dark' : 'light');
  }, [controller, dark]);

  const labels = useMemo(
    () => ({
      file: t('viewer.titleFile'),
      size: t('viewer.titleSize'),
      date: t('viewer.titleDate'),
      revision: t('viewer.titleRevision'),
    }),
    [t],
  );

  // Load the scene, and reload it after every change of the project.
  useEffect(() => {
    let cancelled = false;
    let request = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = async (): Promise<void> => {
      request += 1;
      const mine = request;
      try {
        await loadCanvasFont();
        await engine.whenConnected();
        const reply = await engine.call<SceneReply>('view.scene', { doc, labels }, 120_000);
        if (!cancelled && mine === request) {
          controller.setScene(reply, memories.get(doc) ?? null);
          setError(null);
        }
      } catch (reason) {
        if (!cancelled && mine === request) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      } finally {
        if (!cancelled && mine === request) {
          setLoading(false);
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
      }, 80);
    });
    return () => {
      cancelled = true;
      clearTimeout(timer);
      off();
    };
  }, [engine, controller, doc, labels]);

  const board = snapshot.kind === 'pcb';
  return (
    <div className="viewer" data-testid="viewer" data-kind={snapshot.kind ?? ''}>
      <div className="viewer__main">
        <div className="viewer__toolbar" role="toolbar" aria-label={t('viewer.toolbar')}>
          <ToolButton
            icon="zoomIn"
            label={t('viewer.zoomIn')}
            testId="viewer-zoom-in"
            onClick={() => {
              controller.zoomBy(1.5);
            }}
          />
          <ToolButton
            icon="zoomOut"
            label={t('viewer.zoomOut')}
            testId="viewer-zoom-out"
            onClick={() => {
              controller.zoomBy(1 / 1.5);
            }}
          />
          <ToolButton
            icon="fit"
            label={`${t('viewer.fitAll')} (Home)`}
            testId="viewer-fit"
            onClick={() => {
              controller.fitAll();
            }}
          />
          <ToolButton
            icon="focus"
            label={`${t('viewer.fitSelection')} (F)`}
            testId="viewer-fit-selection"
            onClick={() => {
              controller.fitSelection();
            }}
          />
          <span className="viewer-tool__separator" />
          <ToolButton
            icon="boxSelect"
            label={t('viewer.boxSelect')}
            pressed={snapshot.tool === 'box'}
            testId="viewer-tool-box"
            onClick={() => {
              controller.setTool('box');
            }}
          />
          <ToolButton
            icon="lasso"
            label={`${t('viewer.lassoSelect')} (Alt)`}
            pressed={snapshot.tool === 'lasso'}
            testId="viewer-tool-lasso"
            onClick={() => {
              controller.setTool('lasso');
            }}
          />
          {board && (
            <>
              <span className="viewer-tool__separator" />
              <ToolButton
                icon="contrast"
                label={t('viewer.highContrast')}
                pressed={snapshot.highContrast}
                testId="viewer-high-contrast"
                onClick={() => {
                  controller.setHighContrast(!snapshot.highContrast);
                }}
              />
              <ToolButton
                icon="flip"
                label={`${t('viewer.flip')} (B)`}
                pressed={snapshot.flipped}
                testId="viewer-flip"
                onClick={() => {
                  controller.toggleFlip();
                }}
              />
            </>
          )}
        </div>
        <div className="viewer__area" ref={areaRef}>
          <canvas
            ref={canvasRef}
            className="viewer__canvas"
            tabIndex={0}
            aria-label={t('viewer.canvas', { name: document.name })}
            data-testid="viewer-canvas"
            data-ready={snapshot.status === 'ready' && !loading}
            data-version={snapshot.version}
          />
          <Overlay controller={controller} />
          {snapshot.status === 'unavailable' && (
            <p className="viewer__message callout callout--error" role="alert">
              {t('viewer.unavailable')}
            </p>
          )}
          {error !== null && (
            <p
              className="viewer__message callout callout--error"
              role="alert"
              data-testid="viewer-error"
            >
              {t('viewer.failed', { message: error })}
            </p>
          )}
          {loading && error === null && (
            <p className="viewer__message viewer__message--quiet" data-testid="viewer-loading">
              {t('viewer.loading', { name: document.name })}
            </p>
          )}
        </div>
        <Footer controller={controller} units={units} />
      </div>
      <aside className="viewer__side">
        <LayerPanel controller={controller} />
        <Inspector controller={controller} doc={doc} units={units} />
      </aside>
    </div>
  );
}
