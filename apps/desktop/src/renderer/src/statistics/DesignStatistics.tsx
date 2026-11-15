/**
 * The design in numbers (engine `project.statistics`): schematic, bill of
 * materials and every board, refreshed after every change.
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import { formatLength, isEngineEvent, type Units } from '@shared/project';

import { useAppState } from '../state';

interface BoardStatistics {
  readonly board: string;
  readonly id: string;
  readonly size: readonly [number, number] | null;
  readonly area: number | null;
  readonly copperLayers: number;
  readonly thickness: number;
  readonly footprints: Readonly<Record<'total' | 'top' | 'bottom' | 'smd' | 'throughHole', number>>;
  readonly pads: { readonly total: number; readonly smd: number; readonly throughHole: number };
  readonly holes: {
    readonly plated: number;
    readonly nonPlated: number;
    readonly drills: readonly { diameter: number; plated: number; nonPlated: number }[];
  };
  readonly vias: number;
  readonly zones: number;
  readonly tracks: {
    readonly segments: number;
    readonly length: number;
    readonly layers: readonly { layer: string; length: number }[];
  };
  readonly smallest: {
    readonly track: number | null;
    readonly via: number | null;
    readonly hole: number | null;
  };
  readonly density: { readonly parts: number; readonly pads: number } | null;
  readonly routing: {
    readonly connections: number;
    readonly unrouted: number;
    readonly completion: number;
  };
}

export interface Statistics {
  readonly schematic: Readonly<
    Record<'sheets' | 'parts' | 'pins' | 'nets' | 'namedNets' | 'singlePinNets', number>
  >;
  readonly bom: Readonly<
    Record<'lines' | 'parts' | 'dnp' | 'withoutFootprint' | 'withoutPartNumber', number>
  >;
  readonly boards: readonly BoardStatistics[];
}

type Item = readonly [MessageKey, string | number | null];

function Figures({
  items,
  testId,
}: {
  readonly items: readonly Item[];
  readonly testId?: string;
}): ReactNode {
  const { t } = useAppState();
  return (
    <dl className="stats stats--compact" data-testid={testId}>
      {items.map(([label, value]) => (
        <div key={label} className="stats__item">
          <dt>{t(label)}</dt>
          <dd>{value ?? t('statistics.none')}</dd>
        </div>
      ))}
    </dl>
  );
}

/** Copper layer keys (`cu.top`, `cu.in1`) as names. */
function layerName(key: string): string {
  const inner = /^cu\.in(\d+)$/u.exec(key);
  if (inner !== null) {
    return `In${inner[1] ?? ''}.Cu`;
  }
  return key === 'cu.top' ? 'F.Cu' : key === 'cu.bottom' ? 'B.Cu' : key;
}

export function DesignStatistics({ units }: { readonly units: Units }): ReactNode {
  const { t, engine, snapshot } = useAppState();
  const [stats, setStats] = useState<Statistics | null>(null);
  const language = snapshot.language;
  const number = new Intl.NumberFormat(language, { maximumFractionDigits: 2 });
  const length = (mm: number | null): string | null =>
    mm === null ? null : formatLength(mm * 1e6, units, language);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = (): void => {
      engine.call<Statistics>('project.statistics').then(
        (result) => {
          if (!cancelled) {
            setStats(result);
          }
        },
        () => undefined,
      );
    };
    load();
    const off = engine.onEvent((event) => {
      if (isEngineEvent(event)) {
        clearTimeout(timer);
        timer = setTimeout(load, 300);
      }
    });
    return () => {
      cancelled = true;
      clearTimeout(timer);
      off();
    };
  }, [engine]);

  if (stats === null) {
    return (
      <section className="panel" data-testid="design-statistics">
        <h3 className="panel__title">{t('statistics.title')}</h3>
        <p className="field__hint">{t('statistics.loading')}</p>
      </section>
    );
  }
  const { schematic, bom } = stats;
  return (
    <section className="panel" data-testid="design-statistics">
      <h3 className="panel__title">{t('statistics.title')}</h3>
      <h4 className="statistics__heading">{t('statistics.schematic')}</h4>
      <Figures
        testId="statistics-schematic"
        items={[
          ['statistics.sheets', schematic.sheets],
          ['statistics.parts', schematic.parts],
          ['statistics.pins', schematic.pins],
          ['statistics.nets', schematic.nets],
          ['statistics.namedNets', schematic.namedNets],
          ['statistics.singlePinNets', schematic.singlePinNets],
        ]}
      />
      <h4 className="statistics__heading">{t('statistics.bom')}</h4>
      <Figures
        testId="statistics-bom"
        items={[
          ['statistics.lines', bom.lines],
          ['statistics.bomParts', bom.parts],
          ['statistics.dnp', bom.dnp],
          ['statistics.withoutFootprint', bom.withoutFootprint],
          ['statistics.withoutPartNumber', bom.withoutPartNumber],
        ]}
      />
      {stats.boards.map((board) => {
        const percent = Math.round(board.routing.completion * 1000) / 10;
        return (
          <div key={board.id} className="statistics__board" data-testid="statistics-board">
            <h4 className="statistics__heading">{t('statistics.board', { name: board.board })}</h4>
            <Figures
              items={[
                [
                  'statistics.size',
                  board.size === null
                    ? null
                    : `${length(board.size[0]) ?? ''} × ${length(board.size[1]) ?? ''}`,
                ],
                [
                  'statistics.area',
                  board.area === null
                    ? null
                    : t('statistics.areaValue', { value: number.format(board.area) }),
                ],
                ['statistics.copperLayers', board.copperLayers],
                ['statistics.thickness', length(board.thickness)],
                ['statistics.footprints', board.footprints.total],
                ['statistics.top', board.footprints.top],
                ['statistics.bottom', board.footprints.bottom],
                ['statistics.smd', board.footprints.smd],
                ['statistics.throughHole', board.footprints.throughHole],
                ['statistics.pads', board.pads.total],
                ['statistics.smdPads', board.pads.smd],
                ['statistics.thtPads', board.pads.throughHole],
                ['statistics.holes', board.holes.plated + board.holes.nonPlated],
                ['statistics.vias', board.vias],
                ['statistics.zones', board.zones],
                ['statistics.segments', board.tracks.segments],
                ['statistics.trackLength', length(board.tracks.length)],
                ['statistics.smallestTrack', length(board.smallest.track)],
                ['statistics.smallestVia', length(board.smallest.via)],
                ['statistics.smallestHole', length(board.smallest.hole)],
                [
                  'statistics.partDensity',
                  board.density === null ? null : number.format(board.density.parts),
                ],
                [
                  'statistics.padDensity',
                  board.density === null ? null : number.format(board.density.pads),
                ],
              ]}
            />
            <div className="statistics__routing" data-testid="statistics-routing">
              <span>{t('statistics.completion', { percent: number.format(percent) })}</span>
              <div
                className="assembly__progress"
                role="progressbar"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={percent}
              >
                <span className="assembly__bar" style={{ width: `${String(percent)}%` }} />
              </div>
              <span className="field__hint">
                {t('statistics.unrouted', {
                  count: board.routing.unrouted,
                  total: board.routing.connections,
                })}
              </span>
            </div>
            <div className="statistics__tables">
              <table className="table" data-testid="statistics-drills">
                <caption className="statistics__caption">{t('statistics.drillTable')}</caption>
                <thead>
                  <tr>
                    <th className="table__number">{t('statistics.diameter')}</th>
                    <th className="table__number">{t('statistics.plated')}</th>
                    <th className="table__number">{t('statistics.nonPlated')}</th>
                  </tr>
                </thead>
                <tbody>
                  {board.holes.drills.map((drill) => (
                    <tr key={drill.diameter}>
                      <td className="table__number">{length(drill.diameter)}</td>
                      <td className="table__number">{drill.plated}</td>
                      <td className="table__number">{drill.nonPlated}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <table className="table" data-testid="statistics-layers">
                <caption className="statistics__caption">{t('statistics.layerTable')}</caption>
                <thead>
                  <tr>
                    <th>{t('statistics.layer')}</th>
                    <th className="table__number">{t('statistics.length')}</th>
                  </tr>
                </thead>
                <tbody>
                  {board.tracks.layers.map((layer) => (
                    <tr key={layer.layer}>
                      <td>{layerName(layer.layer)}</td>
                      <td className="table__number">{length(layer.length)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        );
      })}
    </section>
  );
}
