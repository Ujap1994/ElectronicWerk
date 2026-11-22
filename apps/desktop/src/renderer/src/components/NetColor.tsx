/**
 * The colour of a net (project setting `netColors`): a row of swatches, a
 * free colour and "none". The colour shows on the copper of the board and
 * on the wires of the schematic.
 */
import { useEffect, useRef, useState, type ReactNode } from 'react';

import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';

/** Colours that stand out on dark copper and on white paper. */
export const NET_SWATCHES: readonly string[] = [
  '#e53935',
  '#fb8c00',
  '#fdd835',
  '#43a047',
  '#00acc1',
  '#1e88e5',
  '#8e24aa',
  '#d81b60',
];

/** Schematic objects that belong to a net. */
const SHEET_NET_TYPES: ReadonlySet<string> = new Set([
  'wire',
  'bus',
  'junction',
  'label',
  'powerPort',
]);

/** Sets the colour of a net in one undo step (`null`: none). */
export function useSetNetColor(): (net: string, color: string | null) => void {
  const { setProperties } = useProject();
  return (net, color) => {
    void setProperties({ settings: { netColors: { [net]: color } } });
  };
}

export function NetColorPicker({
  net,
  testId = 'net-color',
}: {
  readonly net: string;
  readonly testId?: string;
}): ReactNode {
  const { t } = useAppState();
  const { project } = useProject();
  const setColor = useSetNetColor();
  const current = project?.settings.netColors?.[net] ?? null;
  const inputRef = useRef<HTMLInputElement | null>(null);
  // The picker reports every move as `input`; only its `change` (closed)
  // becomes an undo step.
  useEffect(() => {
    const element = inputRef.current;
    if (element === null) {
      return undefined;
    }
    const changed = (): void => {
      setColor(net, element.value);
    };
    element.addEventListener('change', changed);
    return () => {
      element.removeEventListener('change', changed);
    };
  });
  return (
    <div className="net-color" data-testid={testId}>
      {NET_SWATCHES.map((color) => (
        <button
          key={color}
          type="button"
          className="net-color__swatch"
          style={{ background: color }}
          aria-pressed={current === color}
          title={color}
          aria-label={t('netColor.swatch', { color })}
          data-testid={`${testId}-${color.slice(1)}`}
          onClick={() => {
            setColor(net, color);
          }}
        />
      ))}
      <input
        ref={inputRef}
        className="net-color__custom"
        type="color"
        defaultValue={current ?? '#808080'}
        key={current ?? 'none'}
        title={t('netColor.custom')}
        aria-label={t('netColor.custom')}
      />
      <button
        type="button"
        className="button button--small"
        disabled={current === null}
        data-testid={`${testId}-none`}
        onClick={() => {
          setColor(net, null);
        }}
      >
        {t('netColor.none')}
      </button>
    </div>
  );
}

/** The net colour of a selected object, in the inspector. */
export function NetColor({
  doc,
  id,
  type,
  net: boardNet,
}: {
  readonly doc: string;
  readonly id: string;
  readonly type: string;
  /** The net from the board scene (`undefined`: a schematic object). */
  readonly net: string | null | undefined;
}): ReactNode {
  const { t, engine } = useAppState();
  const [sheetNet, setSheetNet] = useState<{ id: string; net: string | null } | null>(null);
  const onSheet = boardNet === undefined && SHEET_NET_TYPES.has(type);
  useEffect(() => {
    if (!onSheet) {
      return undefined;
    }
    let cancelled = false;
    engine.call<{ net: string | null }>('schematic.net', { doc, id }).then(
      (reply) => {
        if (!cancelled) {
          setSheetNet({ id, net: reply.net });
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, doc, id, onSheet]);
  const net = boardNet ?? (sheetNet?.id === id ? sheetNet.net : null);
  if (net === null || net === '') {
    return null;
  }
  return (
    <div className="inspector__net-color">
      <p className="inspector__subtitle">{t('netColor.title', { net })}</p>
      <NetColorPicker net={net} />
    </div>
  );
}
