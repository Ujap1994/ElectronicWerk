/**
 * A footprint drawn as SVG (wizard and component previews): pads in their
 * shapes, then the graphics of the silkscreen, fabrication and courtyard
 * layers. Model coordinates are y up, so the drawing is flipped.
 */
import type { ReactNode } from 'react';

import { arcPath } from '../schematic/SymbolShapes';
import type { BoardGraphic, FootprintDef, Geometry, Pad, PadKind, Point } from './types';

function layerClass(layer: string): string {
  const [family = 'other', side = 'top'] = layer.split('.');
  return `fp-${family} fp--${side}`;
}

/** The outline of a pad around its centre (before rotation). */
export function padPath(pad: Pad): string {
  const [w, h] = pad.size;
  const hw = w / 2;
  const hh = h / 2;
  const shape = pad.shape;
  const rounded = (r: number): string =>
    `M${-hw + r} ${-hh}H${hw - r}A${r} ${r} 0 0 1 ${hw} ${-hh + r}V${hh - r}` +
    `A${r} ${r} 0 0 1 ${hw - r} ${hh}H${-hw + r}A${r} ${r} 0 0 1 ${-hw} ${hh - r}` +
    `V${-hh + r}A${r} ${r} 0 0 1 ${-hw + r} ${-hh}Z`;
  if (shape === 'circle') {
    return `M${-hw} 0A${hw} ${hw} 0 1 0 ${hw} 0A${hw} ${hw} 0 1 0 ${-hw} 0Z`;
  }
  if (shape === 'rect') {
    return `M${-hw} ${-hh}H${hw}V${hh}H${-hw}Z`;
  }
  if (shape === 'oval') {
    return rounded(Math.min(hw, hh));
  }
  if ('roundRect' in shape) {
    return rounded((Math.min(w, h) * shape.roundRect.ratio) / 1000);
  }
  if ('trapezoid' in shape) {
    const [dx, dy] = shape.trapezoid.delta;
    return (
      `M${-hw - dy / 2} ${-hh + dx / 2}L${hw + dy / 2} ${-hh - dx / 2}` +
      `L${hw - dy / 2} ${hh + dx / 2}L${-hw + dy / 2} ${hh - dx / 2}Z`
    );
  }
  const points = shape.custom.points;
  return points.length === 0
    ? ''
    : `M${points.map((p) => `${String(p[0])} ${String(p[1])}`).join('L')}Z`;
}

function geometryElement(
  geometry: Geometry,
  key: string,
  className: string,
  width: number,
  filled: boolean,
): ReactNode {
  const props = {
    key,
    className: `${className}${filled ? ' fp--filled' : ''}`,
    strokeWidth: width,
  };
  const path = (points: readonly Point[], close: boolean): string =>
    `M${points.map((p) => `${String(p[0])} ${String(p[1])}`).join('L')}${close ? 'Z' : ''}`;
  if ('line' in geometry) {
    return <path {...props} d={path([geometry.line.start, geometry.line.end], false)} />;
  }
  if ('rect' in geometry) {
    const [a, b] = [geometry.rect.start, geometry.rect.end];
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
  if ('circle' in geometry) {
    const { center, radius } = geometry.circle;
    return <circle {...props} cx={center[0]} cy={center[1]} r={radius} />;
  }
  if ('arc' in geometry) {
    const { start, mid, end } = geometry.arc;
    return <path {...props} d={arcPath([...start], [...mid], [...end])} />;
  }
  if ('polyline' in geometry) {
    return <path {...props} d={path(geometry.polyline.points, false)} />;
  }
  return <path {...props} d={path(geometry.polygon.points, true)} />;
}

/** The points a footprint covers (for the view box). */
function bounds(def: FootprintDef): [number, number, number, number] {
  const xs: number[] = [];
  const ys: number[] = [];
  const add = (p: Point, r = 0): void => {
    xs.push(p[0] - r, p[0] + r);
    ys.push(p[1] - r, p[1] + r);
  };
  for (const pad of def.pads ?? []) {
    add(pad.at, Math.hypot(pad.size[0], pad.size[1]) / 2);
  }
  for (const graphic of def.graphics ?? []) {
    const g = graphic.geometry;
    if ('line' in g) {
      add(g.line.start);
      add(g.line.end);
    } else if ('rect' in g) {
      add(g.rect.start);
      add(g.rect.end);
    } else if ('circle' in g) {
      add(g.circle.center, g.circle.radius);
    } else if ('arc' in g) {
      add(g.arc.start);
      add(g.arc.mid);
      add(g.arc.end);
    } else if ('polyline' in g) {
      g.polyline.points.forEach((p) => {
        add(p);
      });
    } else {
      g.polygon.points.forEach((p) => {
        add(p);
      });
    }
  }
  if (xs.length === 0) {
    return [-1e6, -1e6, 1e6, 1e6];
  }
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

const LAYER_ORDER = ['courtyard', 'fab', 'silk', 'cu'];

const PAD_CLASS: Readonly<Record<PadKind, string>> = {
  smd: 'smd',
  throughHole: 'through-hole',
  npThroughHole: 'np-through-hole',
  connector: 'connector',
};

function order(graphic: BoardGraphic): number {
  return LAYER_ORDER.indexOf(graphic.layer.split('.')[0] ?? '');
}

/** A footprint scaled to fit its box. */
export function FootprintSvg({
  def,
  label,
  numbers = true,
  testId,
}: {
  readonly def: FootprintDef;
  readonly label: string;
  /** Pad numbers on the pads. */
  readonly numbers?: boolean;
  readonly testId?: string;
}): ReactNode {
  const [x0, y0, x1, y1] = bounds(def);
  const margin = Math.max(x1 - x0, y1 - y0) * 0.06 + 1e5;
  const graphics = [...(def.graphics ?? [])]
    .map((graphic, index) => ({ graphic, index }))
    .filter(({ graphic }) => graphic.layer.endsWith('.top'))
    .sort((a, b) => order(a.graphic) - order(b.graphic));
  return (
    <svg
      className="footprint-svg"
      viewBox={`${x0 - margin} ${-(y1 + margin)} ${x1 - x0 + 2 * margin} ${y1 - y0 + 2 * margin}`}
      role="img"
      aria-label={label}
      data-testid={testId}
    >
      <g transform="scale(1 -1)">
        {graphics
          .filter(({ graphic }) => !graphic.layer.startsWith('silk'))
          .map(({ graphic, index }) =>
            geometryElement(
              graphic.geometry,
              `g${String(index)}`,
              layerClass(graphic.layer),
              graphic.width,
              graphic.filled === true,
            ),
          )}
        {(def.pads ?? []).map((pad, index) => (
          <g
            // Pads have ids, but wizard previews may repeat them.
            // eslint-disable-next-line @eslint-react/no-array-index-key -- see above
            key={index}
            transform={`translate(${String(pad.at[0])} ${String(pad.at[1])}) rotate(${String(pad.rotation ?? 0)})`}
          >
            <path className={`fp-pad fp-pad--${PAD_CLASS[pad.kind ?? 'smd']}`} d={padPath(pad)} />
            {pad.drill !== undefined && <circle className="fp-hole" r={pad.drill.diameter / 2} />}
          </g>
        ))}
        {graphics
          .filter(({ graphic }) => graphic.layer.startsWith('silk'))
          .map(({ graphic, index }) =>
            geometryElement(
              graphic.geometry,
              `s${String(index)}`,
              layerClass(graphic.layer),
              graphic.width,
              graphic.filled === true,
            ),
          )}
      </g>
      {numbers && (
        // Chromium caps font sizes, so the numbers are laid out in µm.
        <g transform="scale(1000)">
          {(def.pads ?? []).map((pad, index) => {
            const size = (Math.min(pad.size[0], pad.size[1]) * 0.55) / 1000;
            return (
              <text
                // eslint-disable-next-line @eslint-react/no-array-index-key -- see above
                key={index}
                className="fp-number"
                x={pad.at[0] / 1000}
                y={-pad.at[1] / 1000}
                fontSize={size}
                textAnchor="middle"
                dominantBaseline="central"
              >
                {pad.number}
              </text>
            );
          })}
        </g>
      )}
    </svg>
  );
}
