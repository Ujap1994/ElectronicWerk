/**
 * A generated symbol unit as SVG, with pin names and numbers, for the
 * part generator's result. Symbol coordinates are nanometres, y up.
 */
import type { ReactNode } from 'react';

type Point = readonly [number, number];

interface PinJson {
  readonly number: string;
  readonly name?: string;
  readonly at: Point;
  readonly direction?: 'right' | 'left' | 'up' | 'down';
  readonly length?: number;
}

interface UnitJson {
  readonly pins?: readonly PinJson[];
  readonly graphics?: readonly {
    readonly geometry: { readonly rect?: { readonly start: Point; readonly end: Point } };
  }[];
}

export interface SymbolJson {
  readonly name: string;
  readonly units: readonly UnitJson[];
}

/** Drawing units: millimetres (browsers limit very large font sizes). */
const MM = 1_000_000;
const TEXT = 1.27;

function step(direction: PinJson['direction']): Point {
  switch (direction) {
    case 'left':
      return [-1, 0];
    case 'up':
      return [0, 1];
    case 'down':
      return [0, -1];
    case 'right':
    case undefined:
      return [1, 0];
  }
}

export function SymbolView({
  def,
  unit,
}: {
  readonly def: SymbolJson;
  readonly unit: number;
}): ReactNode {
  const u = def.units[unit] ?? def.units[0];
  if (u === undefined) {
    return null;
  }
  const pins = (u.pins ?? []).map((p) => ({
    ...p,
    at: [p.at[0] / MM, p.at[1] / MM] as const,
    length: (p.length ?? 2_540_000) / MM,
  }));
  const rects = (u.graphics ?? []).flatMap((g) => {
    const r = g.geometry.rect;
    return r === undefined
      ? []
      : [
          {
            start: [r.start[0] / MM, r.start[1] / MM] as const,
            end: [r.end[0] / MM, r.end[1] / MM] as const,
          },
        ];
  });
  const xs: number[] = [];
  const ys: number[] = [];
  for (const r of rects) {
    xs.push(r.start[0], r.end[0]);
    ys.push(r.start[1], r.end[1]);
  }
  for (const p of pins) {
    xs.push(p.at[0]);
    ys.push(p.at[1]);
  }
  if (xs.length === 0) {
    return null;
  }
  const margin = 4 * TEXT;
  const x0 = Math.min(...xs) - margin;
  const x1 = Math.max(...xs) + margin;
  const y0 = Math.min(...ys) - margin;
  const y1 = Math.max(...ys) + margin;
  // Drawn y down: every y is negated.
  return (
    <svg
      className="partgen-symbol"
      viewBox={`${String(x0)} ${String(-y1)} ${String(x1 - x0)} ${String(y1 - y0)}`}
      role="img"
      aria-label={def.name}
    >
      {rects.map((r, i) => {
        return (
          <rect
            // eslint-disable-next-line @eslint-react/no-array-index-key -- graphics have no id
            key={i}
            className="partgen-symbol__body"
            x={Math.min(r.start[0], r.end[0])}
            y={-Math.max(r.start[1], r.end[1])}
            width={Math.abs(r.end[0] - r.start[0])}
            height={Math.abs(r.end[1] - r.start[1])}
          />
        );
      })}
      {pins.map((p) => {
        const [dx, dy] = step(p.direction);
        const length = p.length;
        const end: Point = [p.at[0] + dx * length, p.at[1] + dy * length];
        const horizontal = dy === 0;
        const nameAt: Point = [end[0] + dx * TEXT * 0.6, end[1] + dy * TEXT * 0.6];
        const numberAt: Point = horizontal
          ? [(p.at[0] + end[0]) / 2, p.at[1] + TEXT * 0.35]
          : [p.at[0] - TEXT * 0.35, (p.at[1] + end[1]) / 2];
        const anchor = horizontal ? (dx > 0 ? 'start' : 'end') : 'start';
        return (
          <g key={`${p.number}-${String(p.at[0])}-${String(p.at[1])}`}>
            <line
              className="partgen-symbol__pin"
              x1={p.at[0]}
              y1={-p.at[1]}
              x2={end[0]}
              y2={-end[1]}
            />
            <text
              className="partgen-symbol__name"
              x={nameAt[0]}
              y={-nameAt[1]}
              fontSize={TEXT}
              textAnchor={anchor}
              dominantBaseline="middle"
              transform={
                horizontal
                  ? undefined
                  : `rotate(${dy < 0 ? 90 : -90} ${String(nameAt[0])} ${String(-nameAt[1])})`
              }
            >
              {p.name ?? ''}
            </text>
            <text
              className="partgen-symbol__number"
              x={numberAt[0]}
              y={-numberAt[1]}
              fontSize={TEXT * 0.8}
              textAnchor="middle"
              transform={
                horizontal
                  ? undefined
                  : `rotate(-90 ${String(numberAt[0])} ${String(-numberAt[1])})`
              }
            >
              {p.number}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
