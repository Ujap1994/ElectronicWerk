import type { ReactNode } from 'react';

import type { Point, SymbolDefJson } from './editor';

type Geometry = Readonly<Record<string, Readonly<Record<string, unknown>>>>;

function point(value: unknown): Point {
  return Array.isArray(value) && value.length === 2 ? [Number(value[0]), Number(value[1])] : [0, 0];
}

/** An SVG arc through three points. */
export function arcPath(a: Point, m: Point, b: Point): string {
  const d = 2 * (a[0] * (m[1] - b[1]) + m[0] * (b[1] - a[1]) + b[0] * (a[1] - m[1]));
  if (Math.abs(d) < 1e-9) {
    return `M${a[0]} ${a[1]}L${b[0]} ${b[1]}`;
  }
  const sq = (p: Point): number => p[0] * p[0] + p[1] * p[1];
  const cx = (sq(a) * (m[1] - b[1]) + sq(m) * (b[1] - a[1]) + sq(b) * (a[1] - m[1])) / d;
  const cy = (sq(a) * (b[0] - m[0]) + sq(m) * (a[0] - b[0]) + sq(b) * (m[0] - a[0])) / d;
  const r = Math.hypot(a[0] - cx, a[1] - cy);
  const angle = (p: Point): number => Math.atan2(p[1] - cy, p[0] - cx);
  const turn = (x: number): number => ((x % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  // Counterclockwise from a to b, if m lies on that way.
  const ccw = turn(angle(m) - angle(a)) < turn(angle(b) - angle(a));
  const sweep = ccw ? turn(angle(b) - angle(a)) : turn(angle(a) - angle(b));
  const large = sweep > Math.PI ? 1 : 0;
  return `M${a[0]} ${a[1]}A${r} ${r} 0 ${large} ${ccw ? 1 : 0} ${b[0]} ${b[1]}`;
}

/** SVG elements of a symbol unit in its own coordinates (y up). */
export function SymbolShapes({ def }: { readonly def: SymbolDefJson }): ReactNode {
  const unit = def.units[0];
  if (unit === undefined) {
    return null;
  }
  const shapes = (unit.graphics ?? []).map((graphic, index) => {
    const geometry: Geometry = graphic.geometry;
    const [kind] = Object.keys(geometry);
    const g = kind === undefined ? undefined : geometry[kind];
    if (kind === undefined || g === undefined) {
      return null;
    }
    const filled = graphic.fill === 'solid';
    const props = { key: index, className: filled ? 'ghost__fill' : undefined };
    switch (kind) {
      case 'line': {
        const [a, b] = [point(g['start']), point(g['end'])];
        return <path {...props} d={`M${a[0]} ${a[1]}L${b[0]} ${b[1]}`} />;
      }
      case 'rect': {
        const [a, b] = [point(g['start']), point(g['end'])];
        return (
          <rect
            {...props}
            x={Math.min(a[0], b[0])}
            y={Math.min(a[1], b[1])}
            width={Math.abs(b[0] - a[0])}
            height={Math.abs(b[1] - a[1])}
          />
        );
      }
      case 'circle': {
        const c = point(g['center']);
        return <circle {...props} cx={c[0]} cy={c[1]} r={Number(g['radius'] ?? 0)} />;
      }
      case 'arc':
        return <path {...props} d={arcPath(point(g['start']), point(g['mid']), point(g['end']))} />;
      case 'polyline':
      case 'polygon': {
        const points = Array.isArray(g['points']) ? (g['points'] as unknown[]).map(point) : [];
        const d = points.map((p, i) => `${i === 0 ? 'M' : 'L'}${p[0]} ${p[1]}`).join('');
        return <path {...props} d={kind === 'polygon' ? `${d}Z` : d} />;
      }
      case 'bezier': {
        const [s, c1, c2, e] = ['start', 'control1', 'control2', 'end'].map((k) => point(g[k]));
        if (s === undefined || c1 === undefined || c2 === undefined || e === undefined) {
          return null;
        }
        return (
          <path
            {...props}
            d={`M${s[0]} ${s[1]}C${c1[0]} ${c1[1]} ${c2[0]} ${c2[1]} ${e[0]} ${e[1]}`}
          />
        );
      }
      default:
        return null;
    }
  });
  const pins = (unit.pins ?? [])
    .filter((pin) => pin.hidden !== true)
    .map((pin) => {
      const length = pin.length ?? 0;
      const [dx, dy] =
        pin.direction === 'left'
          ? [-1, 0]
          : pin.direction === 'up'
            ? [0, 1]
            : pin.direction === 'down'
              ? [0, -1]
              : [1, 0];
      const [x, y] = pin.at;
      return (
        <path
          key={`pin-${pin.number ?? ''}-${x.toString()}-${y.toString()}`}
          className="ghost__pin"
          d={`M${x} ${y}L${x + dx * length} ${y + dy * length}`}
        />
      );
    });
  return (
    <>
      {shapes}
      {pins}
    </>
  );
}

/** The bounding box of a symbol unit's drawing `[minX, minY, maxX, maxY]`. */
export function symbolBounds(def: SymbolDefJson): [number, number, number, number] {
  const unit = def.units[0];
  const xs: number[] = [];
  const ys: number[] = [];
  const add = (p: Point): void => {
    xs.push(p[0]);
    ys.push(p[1]);
  };
  for (const graphic of unit?.graphics ?? []) {
    for (const value of Object.values(graphic.geometry)) {
      for (const [key, v] of Object.entries(value)) {
        if (key === 'points' && Array.isArray(v)) {
          (v as unknown[]).forEach((p) => {
            add(point(p));
          });
        } else if (key === 'radius' && typeof v === 'number') {
          const c = point(value['center']);
          add([c[0] - v, c[1] - v]);
          add([c[0] + v, c[1] + v]);
        } else if (Array.isArray(v)) {
          add(point(v));
        }
      }
    }
  }
  for (const pin of unit?.pins ?? []) {
    const length = pin.length ?? 0;
    add(pin.at);
    const d =
      pin.direction === 'left'
        ? [-1, 0]
        : pin.direction === 'up'
          ? [0, 1]
          : pin.direction === 'down'
            ? [0, -1]
            : [1, 0];
    add([pin.at[0] + (d[0] ?? 0) * length, pin.at[1] + (d[1] ?? 0) * length]);
  }
  if (xs.length === 0) {
    return [-1e6, -1e6, 1e6, 1e6];
  }
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

/** A symbol drawn to fit a box (the chooser preview). */
export function SymbolPreview({
  def,
  label,
}: {
  readonly def: SymbolDefJson;
  readonly label: string;
}): ReactNode {
  const [x0, y0, x1, y1] = symbolBounds(def);
  const pad = Math.max(x1 - x0, y1 - y0) * 0.08 + 2e5;
  const w = x1 - x0 + 2 * pad;
  const h = y1 - y0 + 2 * pad;
  return (
    <svg
      className="symbol-preview"
      viewBox={`${x0 - pad} ${-(y1 + pad)} ${w} ${h}`}
      role="img"
      aria-label={label}
      data-testid="symbol-preview"
    >
      <g className="ghost ghost--preview" transform="scale(1 -1)">
        <SymbolShapes def={def} />
      </g>
    </svg>
  );
}
