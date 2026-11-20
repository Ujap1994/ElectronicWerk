/**
 * The datasheet next to the part generator: one page at a time, drawn by
 * pdf.js (loaded on first use), with a frame around the place a value was
 * read from. Engine boxes are points from the top left of the visible
 * page, which is pdf.js's viewport at scale 1.
 */
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';

import type { Box, DatasheetInfo } from '@shared/datasheet';

import { Icon } from '../components/Icon';
import { useAppState } from '../state';

interface PdfPage {
  getViewport: (options: { scale: number }) => { width: number; height: number };
  render: (options: {
    canvas: HTMLCanvasElement;
    canvasContext: CanvasRenderingContext2D;
    viewport: unknown;
    transform?: number[];
  }) => { promise: Promise<void>; cancel: () => void };
  cleanup: () => void;
}

interface PdfDocument {
  numPages: number;
  getPage: (n: number) => Promise<PdfPage>;
  /** Ends the loading task and its worker. */
  close: () => Promise<void>;
}

/** Loads pdf.js and opens a document (a copy of `data`: pdf.js keeps it). */
async function open(data: Uint8Array): Promise<PdfDocument> {
  const [lib, worker] = await Promise.all([
    import('pdfjs-dist'),
    import('pdfjs-dist/build/pdf.worker.min.mjs?url'),
  ]);
  lib.GlobalWorkerOptions.workerSrc = worker.default;
  const task = lib.getDocument({
    data: data.slice(),
    useSystemFonts: true,
  });
  const doc = (await task.promise) as unknown as Omit<PdfDocument, 'close'>;
  return {
    numPages: doc.numPages,
    getPage: (n) => doc.getPage(n),
    close: () => task.destroy(),
  };
}

export function DatasheetView({
  data,
  info,
  page,
  onPage,
  highlight,
}: {
  readonly data: Uint8Array | null;
  readonly info: DatasheetInfo | null;
  readonly page: number;
  readonly onPage: (page: number) => void;
  /** A box on the current page to frame. */
  readonly highlight: Box | null;
}): ReactNode {
  const { t } = useAppState();
  const areaRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [doc, setDoc] = useState<PdfDocument | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [width, setWidth] = useState(600);
  const [zoom, setZoom] = useState(1);
  const [rendered, setRendered] = useState<{ page: number; scale: number } | null>(null);
  const count = info?.pages.length ?? doc?.numPages ?? 0;

  useEffect(() => {
    if (data === null) {
      return undefined;
    }
    let cancelled = false;
    let opened: PdfDocument | null = null;
    open(data)
      .then((d) => {
        if (cancelled) {
          void d.close();
          return;
        }
        opened = d;
        setDoc(d);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setFailed(error instanceof Error ? error.message : String(error));
        }
      });
    return () => {
      cancelled = true;
      setDoc(null);
      void opened?.close();
    };
  }, [data]);

  useLayoutEffect(() => {
    const area = areaRef.current;
    if (area === null) {
      return undefined;
    }
    const observer = new ResizeObserver(() => {
      setWidth(Math.max(200, area.clientWidth - 24));
    });
    observer.observe(area);
    return () => {
      observer.disconnect();
    };
  }, []);

  const pageInfo = info?.pages[page - 1];
  const scale = ((pageInfo?.width ?? 612) > 0 ? width / (pageInfo?.width ?? 612) : 1) * zoom;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (doc === null || canvas === null) {
      return undefined;
    }
    let task: { promise: Promise<void>; cancel: () => void } | null = null;
    let cancelled = false;
    void doc.getPage(page).then((p) => {
      if (cancelled) {
        return;
      }
      const viewport = p.getViewport({ scale });
      const ratio = window.devicePixelRatio || 1;
      canvas.width = Math.floor(viewport.width * ratio);
      canvas.height = Math.floor(viewport.height * ratio);
      canvas.style.width = `${String(Math.floor(viewport.width))}px`;
      canvas.style.height = `${String(Math.floor(viewport.height))}px`;
      const context = canvas.getContext('2d');
      if (context === null) {
        return;
      }
      task = p.render({
        canvas,
        canvasContext: context,
        viewport,
        ...(ratio === 1 ? {} : { transform: [ratio, 0, 0, ratio, 0, 0] }),
      });
      task.promise
        .then(() => {
          if (!cancelled) {
            setRendered({ page, scale });
          }
        })
        .catch(() => {
          // Cancelled by a newer page or zoom.
        });
    });
    return () => {
      cancelled = true;
      task?.cancel();
    };
  }, [doc, page, scale]);

  // Keep the framed place in view.
  useEffect(() => {
    if (highlight === null || rendered?.page !== page) {
      return;
    }
    const area = areaRef.current;
    if (area === null) {
      return;
    }
    const top = highlight[1] * scale - area.clientHeight / 3;
    area.scrollTo({ top: Math.max(0, top), behavior: 'smooth' });
  }, [highlight, page, rendered, scale]);

  const ready = rendered?.page === page && rendered.scale === scale;
  return (
    <section
      className="partgen-viewer"
      aria-label={t('partgen.viewer.label')}
      data-testid="partgen-viewer"
    >
      <div className="partgen-viewer__bar">
        <button
          type="button"
          className="icon-button"
          aria-label={t('partgen.viewer.previous')}
          title={t('partgen.viewer.previous')}
          disabled={page <= 1}
          data-testid="partgen-page-previous"
          onClick={() => {
            onPage(page - 1);
          }}
        >
          <Icon name="previous" />
        </button>
        <span data-testid="partgen-page">{t('partgen.viewer.page', { page, count })}</span>
        <button
          type="button"
          className="icon-button"
          aria-label={t('partgen.viewer.next')}
          title={t('partgen.viewer.next')}
          disabled={page >= count}
          data-testid="partgen-page-next"
          onClick={() => {
            onPage(page + 1);
          }}
        >
          <Icon name="chevron" />
        </button>
        <span className="partgen-viewer__spacer" />
        <button
          type="button"
          className="icon-button"
          aria-label={t('partgen.viewer.zoomOut')}
          title={t('partgen.viewer.zoomOut')}
          onClick={() => {
            setZoom((z) => Math.max(0.5, z / 1.25));
          }}
        >
          <Icon name="zoomOut" />
        </button>
        <button
          type="button"
          className="button button--small button--quiet"
          onClick={() => {
            setZoom(1);
          }}
        >
          {t('partgen.viewer.fit')}
        </button>
        <button
          type="button"
          className="icon-button"
          aria-label={t('partgen.viewer.zoomIn')}
          title={t('partgen.viewer.zoomIn')}
          onClick={() => {
            setZoom((z) => Math.min(4, z * 1.25));
          }}
        >
          <Icon name="zoomIn" />
        </button>
      </div>
      <div className="partgen-viewer__area" ref={areaRef}>
        {failed !== null && <p className="field__hint">{failed}</p>}
        {pageInfo?.scanned === true && <p className="field__hint">{t('partgen.viewer.scanned')}</p>}
        <div className="partgen-viewer__page" data-ready={ready ? 'true' : 'false'}>
          <canvas ref={canvasRef} data-testid="partgen-canvas" />
          {highlight !== null && ready && (
            <div
              className="partgen-viewer__highlight"
              data-testid="partgen-highlight"
              style={{
                left: `${String(highlight[0] * scale - 3)}px`,
                top: `${String(highlight[1] * scale - 3)}px`,
                width: `${String((highlight[2] - highlight[0]) * scale + 6)}px`,
                height: `${String((highlight[3] - highlight[1]) * scale + 6)}px`,
              }}
            />
          )}
          {!ready && <p className="partgen-viewer__loading">{t('partgen.viewer.loading')}</p>}
        </div>
      </div>
    </section>
  );
}
