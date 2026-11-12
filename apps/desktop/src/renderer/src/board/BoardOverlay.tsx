import { useEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react';

import { useAppState } from '../state';
import type { ViewerController } from '../viewer/controller';
import type { BoardEditor, BoardSnapshot, Point } from './editor';
import { drcState } from './stores';

function layerClass(layer: string): string {
  if (layer === 'cu.top') {
    return 'board-preview--top';
  }
  if (layer === 'cu.bottom') {
    return 'board-preview--bottom';
  }
  return 'board-preview--inner';
}

function TextPrompt({
  editor,
  state,
  at,
}: {
  readonly editor: BoardEditor;
  readonly state: BoardSnapshot;
  readonly at: Point;
}): ReactNode {
  const { t } = useAppState();
  const [value, setValue] = useState(state.prompt?.value ?? '');
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    ref.current?.focus();
  }, []);
  return (
    <form
      className="prompt"
      style={{ left: at[0] + 12, top: at[1] + 12 }}
      data-testid="board-prompt"
      onSubmit={(event) => {
        event.preventDefault();
        editor.submitPrompt(value);
      }}
    >
      <label className="prompt__label" htmlFor="board-prompt-input">
        {t('board.prompts.text')}
      </label>
      <input
        ref={ref}
        id="board-prompt-input"
        className="prompt__input"
        value={value}
        spellCheck={false}
        data-testid="board-prompt-input"
        onChange={(event) => {
          setValue(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault();
            editor.cancelPrompt();
          }
        }}
      />
    </form>
  );
}

/** Previews of the layout editor, the ratsnest and DRC markers over the canvas. */
export function BoardOverlay({
  controller,
  editor,
  doc,
}: {
  readonly controller: ViewerController;
  readonly editor: BoardEditor;
  readonly doc: string;
}): ReactNode {
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  const view = useSyncExternalStore(controller.subscribeView, controller.view);
  const drc = useSyncExternalStore(drcState.subscribe, drcState.get);
  const scale = view.scale;
  const screen = (p: Point): Point => controller.screenOf(p);
  const line = (points: readonly Point[]): string =>
    points.map((p) => screen(p).join(',')).join(' ');
  const elements: ReactNode[] = [];
  const report = drc.doc === doc ? drc.report : null;

  // Ratsnest.
  if (drc.ratsnest && report !== null) {
    report.airwires.forEach((wire, index) => {
      const [x1, y1] = screen(wire.from);
      const [x2, y2] = screen(wire.to);
      elements.push(
        <line
          key={`air-${index.toString()}`}
          className="board-airwire"
          x1={x1}
          y1={y1}
          x2={x2}
          y2={y2}
        />,
      );
    });
  }

  // DRC markers.
  if (drc.open && report !== null) {
    for (const finding of report.findings) {
      if (finding.excluded || finding.code === 'unrouted') {
        continue;
      }
      const [x, y] = screen(finding.at);
      elements.push(
        <circle
          key={`drc-${finding.key}`}
          className="board-marker"
          data-severity={finding.severity}
          cx={x}
          cy={y}
          r={6}
        />,
      );
    }
  }

  // The route being drawn.
  const route = state.route;
  if (route !== null) {
    route.fixed.forEach((segment, index) => {
      elements.push(
        <polyline
          key={`fixed-${index.toString()}`}
          className={`board-preview ${layerClass(segment.layer)}`}
          strokeWidth={Math.max(1, route.width * scale)}
          points={line(segment.points)}
        />,
      );
    });
    if (route.preview.length >= 2) {
      elements.push(
        <polyline
          key="live"
          className={`board-preview board-preview--live ${layerClass(route.layer)}`}
          data-reached={route.reached}
          strokeWidth={Math.max(1, route.width * scale)}
          points={line(route.preview)}
          data-testid="route-preview"
        />,
      );
    }
    for (const [index, via] of route.vias.entries()) {
      const [x, y] = screen(via);
      elements.push(
        <circle
          key={`via-${index.toString()}`}
          className="board-preview-via"
          cx={x}
          cy={y}
          r={Math.max(2, (route.viaDiameter * scale) / 2)}
        />,
      );
    }
    const end = route.preview.at(-1);
    if (end !== undefined) {
      const [x, y] = screen(end);
      elements.push(
        <circle
          key="via-ghost"
          className="board-preview-via board-preview-via--ghost"
          data-fits={route.viaFits}
          cx={x}
          cy={y}
          r={Math.max(2, (route.viaDiameter * scale) / 2)}
        />,
      );
    }
  }

  // A polygon or outline being drawn.
  const cursor = state.cursor;
  if (state.polygon.length > 0) {
    const first = state.polygon[0];
    if (
      state.tool === 'outline' &&
      state.outlineShape === 'rect' &&
      first !== undefined &&
      cursor !== null
    ) {
      const [x0, y0] = screen(first);
      const [x1, y1] = screen(cursor);
      elements.push(
        <rect
          key="outline-rect"
          className="board-polygon board-polygon--outline"
          x={Math.min(x0, x1)}
          y={Math.min(y0, y1)}
          width={Math.abs(x1 - x0)}
          height={Math.abs(y1 - y0)}
        />,
      );
    } else {
      const points = cursor === null ? state.polygon : [...state.polygon, cursor];
      elements.push(
        <polygon
          key="polygon"
          className={`board-polygon board-polygon--${state.tool}`}
          points={line(points)}
          data-testid="polygon-preview"
        />,
      );
    }
  }

  // The place of a new footprint.
  if (state.tool === 'footprint' && cursor !== null) {
    const [x, y] = screen(cursor);
    elements.push(
      <g key="ghost" className="board-ghost">
        <line x1={x - 10} y1={y} x2={x + 10} y2={y} />
        <line x1={x} y1={y - 10} x2={x} y2={y + 10} />
        <circle cx={x} cy={y} r={14} />
      </g>,
    );
  }

  const prompt = state.prompt;
  return (
    <>
      {elements.length > 0 && (
        <svg
          className="viewer__overlay board-overlay"
          aria-hidden="true"
          data-testid="board-overlay"
        >
          {elements}
        </svg>
      )}
      {prompt !== null && <TextPrompt editor={editor} state={state} at={screen(prompt.at)} />}
    </>
  );
}
