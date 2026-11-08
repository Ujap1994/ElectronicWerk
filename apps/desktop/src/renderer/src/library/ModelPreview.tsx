/**
 * 3D preview of a library model, alone or placed on its footprint.
 */
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';

import { formatBytes } from '@shared/library';

import { Icon } from '../components/Icon';
import { useAppState } from '../state';
import { useDarkScheme } from '../use-dark-scheme';
import { loadMesh, type MeshStatus } from './mesh-loader';
import type { BoardSize, ModelView, Placement } from './model-view';
import type { EntryDetails } from './types';

type State =
  MeshStatus | { readonly kind: 'ready' } | { readonly kind: 'failed'; readonly message: string };

export function ModelPreview({
  modelId,
  placement,
  board,
  release,
}: {
  readonly modelId: number;
  readonly placement: Placement | null;
  readonly board: BoardSize | null;
  readonly release: string | null;
}): ReactNode {
  const { engine, t } = useAppState();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<ModelView | null>(null);
  const [state, setState] = useState<State>({ kind: 'loading' });
  const dark = useDarkScheme();

  // three.js is loaded on first use.
  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    const area = areaRef.current;
    if (canvas === null || area === null) {
      return undefined;
    }
    let disposed = false;
    let created: ModelView | null = null;
    let observer: ResizeObserver | null = null;
    void import('./model-view').then(({ ModelView: View }) => {
      if (disposed) {
        return;
      }
      created = new View(canvas);
      observer = new ResizeObserver(() => {
        const rect = area.getBoundingClientRect();
        created?.resize(rect.width, rect.height);
      });
      observer.observe(area);
      setView(created);
    });
    return () => {
      disposed = true;
      observer?.disconnect();
      created?.dispose();
    };
  }, []);

  useEffect(() => {
    view?.setTheme(dark);
  }, [view, dark]);

  useEffect(() => {
    if (view === null) {
      return undefined;
    }
    let cancelled = false;
    const stale = (): boolean => cancelled;
    void (async () => {
      try {
        const model = await engine.call<EntryDetails>('library.get', { id: modelId });
        const glb = await loadMesh(engine, model, release, (status) => {
          if (!stale()) {
            setState(status);
          }
        });
        if (stale()) {
          return;
        }
        await view.show(glb, placement, board);
        if (!stale()) {
          setState({ kind: 'ready' });
        }
      } catch (error) {
        if (!stale()) {
          const message = error instanceof Error ? error.message : String(error);
          setState({ kind: 'failed', message });
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [view, engine, modelId, placement, board, release]);

  let message: string | null = null;
  switch (state.kind) {
    case 'loading':
      message = t('library.model.loading');
      break;
    case 'tessellating':
      message = t('library.model.tessellating');
      break;
    case 'downloading':
      message = `${t('library.model.downloading', {
        library: state.library,
        size: formatBytes(state.size),
      })} ${state.percent.toString()} %`;
      break;
    case 'failed':
      message =
        state.message === 'unavailable'
          ? t('library.model.unavailable')
          : t('library.model.failed', { message: state.message });
      break;
    case 'ready':
      break;
  }
  return (
    <div
      ref={areaRef}
      className="library-preview library-preview--3d"
      data-testid="library-model"
      data-state={state.kind}
    >
      <canvas
        ref={canvasRef}
        className="library-preview__canvas"
        aria-label={t('library.preview3d')}
      />
      {message !== null && (
        <p
          className={`library-preview__message${state.kind === 'failed' ? ' library-preview__message--error' : ''}`}
        >
          {message}
        </p>
      )}
      {state.kind === 'ready' && (
        <button
          type="button"
          className="viewer-tool library-preview__reset"
          title={t('library.model.reset')}
          aria-label={t('library.model.reset')}
          onClick={() => {
            view?.reset();
          }}
        >
          <Icon name="fit" size={18} />
        </button>
      )}
    </div>
  );
}
