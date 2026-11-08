/**
 * 2D preview of a library symbol or footprint, drawn by the same WebGL2
 * engine as the sheet and board view (`library.preview` returns a scene).
 */
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import { useAppState } from '../state';
import { useDarkScheme } from '../use-dark-scheme';
import { ViewerController } from '../viewer/controller';
import { loadCanvasFont } from '../viewer/glyph-atlas';
import type { SceneReply } from '../viewer/types';

export function ScenePreview({
  id,
  unit,
  label,
  testId = 'library-preview',
  compact = false,
}: {
  readonly id: number;
  readonly unit?: number;
  readonly label: string;
  readonly testId?: string;
  readonly compact?: boolean;
}): ReactNode {
  const { engine, t } = useAppState();
  const controller = useMemo(() => new ViewerController(), []);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const request = `${id.toString()}:${(unit ?? 1).toString()}`;
  const [result, setResult] = useState<{ request: string; state: 'ready' | 'failed' } | null>(null);
  const state = result?.request === request ? result.state : 'loading';
  const dark = useDarkScheme();

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
    const done = (state: 'ready' | 'failed'): void => {
      if (!cancelled) {
        setResult({ request: `${id.toString()}:${(unit ?? 1).toString()}`, state });
      }
    };
    void loadCanvasFont()
      .then(() => engine.call<SceneReply>('library.preview', { id, unit: unit ?? 1 }))
      .then(
        (reply) => {
          if (!cancelled) {
            controller.setScene(reply, null);
          }
          done('ready');
        },
        () => {
          done('failed');
        },
      );
    return () => {
      cancelled = true;
    };
  }, [controller, engine, id, unit]);

  return (
    <div
      ref={areaRef}
      className={`library-preview${compact ? ' library-preview--compact' : ''}`}
      data-testid={testId}
      data-state={state}
    >
      <canvas ref={canvasRef} className="library-preview__canvas" aria-label={label} />
      {state === 'failed' && (
        <p className="library-preview__message">{t('library.previewFailed')}</p>
      )}
    </div>
  );
}
