import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from 'react';

import { formatLength, type Units } from '@shared/project';

import { Icon } from '../components/Icon';
import { LayerPanel } from '../components/LayerPanel';
import { useAppState } from '../state';
import { useDarkScheme } from '../use-dark-scheme';
import { zoomPercent } from '../viewer/camera';
import { ViewerController } from '../viewer/controller';
import { loadCanvasFont } from '../viewer/glyph-atlas';
import type { SceneReply } from '../viewer/types';

/** CSS pixels per millimetre at 100 % zoom (96 dpi). */
const CSS_PX_PER_MM = 96 / 25.4;

interface GerberMeta {
  readonly files?: readonly { readonly file: string; readonly function: string | null }[];
  readonly problems?: readonly { readonly file: string; readonly message: string }[];
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
    <footer className="viewer__footer" data-testid="gerber-footer">
      <span className="viewer__cursor">
        {view.cursor === null
          ? ' '
          : t('viewer.cursor', { x: length(view.cursor[0]), y: length(view.cursor[1]) })}
      </span>
      <span className="viewer__spacer" />
      <span>{t('viewer.zoom', { percent })}</span>
    </footer>
  );
}

/** A read-only view of Gerber and drill files. */
export function GerberViewer({
  name,
  paths,
  units,
}: {
  readonly name: string;
  readonly paths: readonly string[];
  readonly units: Units;
}): ReactNode {
  const { t, engine } = useAppState();
  const dark = useDarkScheme();
  const controller = useMemo(() => new ViewerController(), []);
  const snapshot = useSyncExternalStore(controller.subscribe, controller.snapshot);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [meta, setMeta] = useState<GerberMeta>({});
  const [reload, setReload] = useState(0);

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

  const key = paths.join('\n');
  useEffect(() => {
    let cancelled = false;
    const load = async (): Promise<void> => {
      try {
        await loadCanvasFont();
        await engine.whenConnected();
        const reply = await engine.call<SceneReply & GerberMeta>(
          'gerber.scene',
          { paths: key.split('\n'), name },
          120_000,
        );
        if (!cancelled) {
          controller.setScene(reply, null);
          setMeta({ files: reply.files ?? [], problems: reply.problems ?? [] });
          setError(null);
        }
      } catch (reason) {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, [engine, controller, key, name, reload]);

  const problems = meta.problems ?? [];
  return (
    <div className="viewer" data-testid="gerber-viewer" data-kind="gerber">
      <div className="viewer__main">
        <div className="viewer__toolbar" role="toolbar" aria-label={t('outputs.viewer.title')}>
          <button
            type="button"
            className="viewer-tool"
            title={t('viewer.zoomIn')}
            aria-label={t('viewer.zoomIn')}
            onClick={() => {
              controller.zoomBy(1.5);
            }}
          >
            <Icon name="zoomIn" />
          </button>
          <button
            type="button"
            className="viewer-tool"
            title={t('viewer.zoomOut')}
            aria-label={t('viewer.zoomOut')}
            onClick={() => {
              controller.zoomBy(1 / 1.5);
            }}
          >
            <Icon name="zoomOut" />
          </button>
          <button
            type="button"
            className="viewer-tool"
            title={`${t('viewer.fitAll')} (Home)`}
            aria-label={t('viewer.fitAll')}
            data-testid="gerber-fit"
            onClick={() => {
              controller.fitAll();
            }}
          >
            <Icon name="fit" />
          </button>
          <button
            type="button"
            className="viewer-tool"
            title={t('viewer.flip')}
            aria-label={t('viewer.flip')}
            aria-pressed={snapshot.flipped}
            data-testid="gerber-flip"
            onClick={() => {
              controller.toggleFlip();
            }}
          >
            <Icon name="flip" />
          </button>
          <span className="viewer-tool__separator" />
          <button
            type="button"
            className="viewer-tool"
            title={t('outputs.viewer.reload')}
            aria-label={t('outputs.viewer.reload')}
            data-testid="gerber-reload"
            onClick={() => {
              setLoading(true);
              setReload((n) => n + 1);
            }}
          >
            <Icon name="sync" />
          </button>
          <span className="gerber__hint">{t('outputs.viewer.hint')}</span>
        </div>
        <div className="viewer__area" ref={areaRef}>
          <canvas
            ref={canvasRef}
            className="viewer__canvas"
            tabIndex={0}
            aria-label={t('outputs.viewer.tab', { name })}
            data-testid="gerber-canvas"
            data-ready={snapshot.status === 'ready' && !loading}
          />
          {error !== null && (
            <p className="viewer__message callout callout--error" role="alert">
              {t('viewer.failed', { message: error })}
            </p>
          )}
          {loading && error === null && (
            <p className="viewer__message viewer__message--quiet">{t('outputs.viewer.loading')}</p>
          )}
        </div>
        <Footer controller={controller} units={units} />
      </div>
      <aside className="viewer__side">
        <LayerPanel controller={controller} />
        {problems.length > 0 && (
          <section className="viewer-panel" data-testid="gerber-problems">
            <header className="viewer-panel__header">
              <Icon name="warning" size={16} />
              <h2 className="viewer-panel__title">{t('outputs.viewer.problems')}</h2>
            </header>
            <ul className="gerber__problems">
              {problems.map((problem, index) => (
                <li key={`${problem.file}-${String(index)}`}>
                  <strong>{problem.file}</strong>: {problem.message}
                </li>
              ))}
            </ul>
          </section>
        )}
      </aside>
    </div>
  );
}
