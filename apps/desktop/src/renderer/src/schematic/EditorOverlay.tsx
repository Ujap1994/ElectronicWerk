import { useEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react';

import { useAppState } from '../state';
import type { ViewerController } from '../viewer/controller';
import { route, stretch, type EditorSnapshot, type Point, type SchematicEditor } from './editor';
import { SymbolShapes } from './SymbolShapes';

function PromptInput({
  editor,
  state,
  at,
}: {
  readonly editor: SchematicEditor;
  readonly state: EditorSnapshot;
  readonly at: Point;
}): ReactNode {
  const { t } = useAppState();
  const prompt = state.prompt;
  const [value, setValue] = useState(prompt?.value ?? '');
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  if (prompt === null) {
    return null;
  }
  const suggestions = prompt.kind === 'power' ? ['GND', 'VCC', '+5V', '+3V3', '+12V', 'VBUS'] : [];
  return (
    <form
      className="prompt"
      style={{ left: at[0] + 12, top: at[1] + 12 }}
      data-testid="editor-prompt"
      onSubmit={(event) => {
        event.preventDefault();
        editor.submitPrompt(value);
      }}
    >
      <label className="prompt__label" htmlFor="editor-prompt-input">
        {t(`schematic.prompts.${prompt.kind}`)}
      </label>
      <input
        ref={ref}
        id="editor-prompt-input"
        className="prompt__input"
        value={value}
        list={suggestions.length > 0 ? 'editor-prompt-list' : undefined}
        spellCheck={false}
        data-testid="editor-prompt-input"
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
      {suggestions.length > 0 && (
        <datalist id="editor-prompt-list">
          {suggestions.map((name) => (
            <option key={name} value={name} />
          ))}
        </datalist>
      )}
    </form>
  );
}

/** Previews of the schematic editor drawn over the canvas. */
export function EditorOverlay({
  controller,
  editor,
}: {
  readonly controller: ViewerController;
  readonly editor: SchematicEditor;
}): ReactNode {
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  // Re-render on camera changes too.
  const view = useSyncExternalStore(controller.subscribeView, controller.view);
  const scale = view.scale;
  const screen = (p: Point): Point => controller.screenOf(p);
  const cursor = state.cursor;
  const elements: ReactNode[] = [];

  // The wire being drawn and the next segments to the cursor.
  if (state.wire.length > 0) {
    const last = state.wire.at(-1);
    const points = [
      ...state.wire,
      ...(cursor === null || last === undefined ? [] : route(last, cursor, state.bend).slice(1)),
    ];
    elements.push(
      <polyline
        key="wire"
        className={state.tool === 'bus' ? 'preview preview--bus' : 'preview'}
        points={points.map((p) => screen(p).join(',')).join(' ')}
        data-testid="preview-wire"
      />,
    );
  }

  // Stretched wires while dragging.
  const move = state.move;
  if (move !== null) {
    for (const wire of move.wires) {
      const shift = (p: Point): Point => [p[0] + move.delta[0], p[1] + move.delta[1]];
      const points =
        wire.startMoves && wire.endMoves
          ? [shift(wire.start), shift(wire.end)]
          : wire.startMoves
            ? stretch(wire.end, wire.start, move.delta)
            : stretch(wire.start, wire.end, move.delta);
      elements.push(
        <polyline
          key={`drag-${wire.id}`}
          className="preview"
          points={points.map((p) => screen(p).join(',')).join(' ')}
        />,
      );
    }
  }

  // The symbol being placed.
  if (state.tool === 'symbol' && state.ghost !== null && cursor !== null) {
    const [sx, sy] = screen(cursor);
    elements.push(
      <g
        key="ghost"
        className="ghost"
        data-testid="preview-symbol"
        transform={`translate(${sx} ${sy}) scale(${scale} ${-scale}) rotate(${state.rotation}) scale(${state.mirror ? -1 : 1} 1)`}
      >
        <SymbolShapes def={state.ghost} />
      </g>,
    );
  }

  // A new sheet.
  if (state.sheetCorner !== null && cursor !== null) {
    const [a, b] = [screen(state.sheetCorner), screen(cursor)];
    elements.push(
      <rect
        key="sheet"
        className="preview preview--sheet"
        x={Math.min(a[0], b[0])}
        y={Math.min(a[1], b[1])}
        width={Math.abs(b[0] - a[0])}
        height={Math.abs(b[1] - a[1])}
      />,
    );
  }

  // The cursor on the grid for the drawing tools.
  if (cursor !== null && state.tool !== 'select') {
    const [x, y] = screen(cursor);
    elements.push(
      <path
        key="cursor"
        className="snap-cursor"
        d={`M${x - 6} ${y}h12M${x} ${y - 6}v12`}
        data-testid="snap-cursor"
      />,
    );
  }

  const prompt = state.prompt;
  return (
    <>
      <svg className="viewer__overlay editor-overlay" aria-hidden="true">
        {elements}
      </svg>
      {prompt !== null && (
        <PromptInput
          key={`${prompt.kind}-${prompt.at.join(',')}`}
          editor={editor}
          state={state}
          at={screen(prompt.at)}
        />
      )}
    </>
  );
}
