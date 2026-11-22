/**
 * The net inspector below the board: every net with its class, routed
 * length, tracks, vias, pads and unrouted connections. Sort by a column,
 * filter by name or class, click a net to highlight it, tick several to
 * give them a net class in one step.
 */
import { useEffect, useMemo, useState, type ReactNode } from 'react';

import { formatLength, type Units } from '@shared/project';

import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { NET_SWATCHES } from '../components/NetColor';
import { netInspector } from './stores';

interface NetRow {
  readonly id: string;
  readonly name: string;
  readonly class: string;
  readonly length: number;
  readonly tracks: number;
  readonly vias: number;
  readonly pads: number;
  readonly unrouted: number;
}

type Column = 'name' | 'class' | 'length' | 'tracks' | 'vias' | 'pads' | 'unrouted';
const COLUMNS: readonly Column[] = [
  'name',
  'class',
  'length',
  'tracks',
  'vias',
  'pads',
  'unrouted',
];

export function NetInspector({
  doc,
  version,
  units,
  onShow,
}: {
  readonly doc: string;
  /** Changes with every edit of the board: the table reloads. */
  readonly version: number;
  readonly units: Units;
  /** Highlight a net (`null`: none). */
  readonly onShow: (net: string | null) => void;
}): ReactNode {
  const { t, engine, snapshot } = useAppState();
  const { notify, project, setProperties } = useProject();
  const netColors = project?.settings.netColors ?? {};
  const [rows, setRows] = useState<readonly NetRow[] | null>(null);
  const [classes, setClasses] = useState<readonly string[]>([]);
  const [filter, setFilter] = useState('');
  const [sort, setSort] = useState<{ column: Column; down: boolean }>({
    column: 'name',
    down: false,
  });
  const [ticked, setTicked] = useState<ReadonlySet<string>>(() => new Set());
  const [shown, setShown] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([
      engine.call<{ nets: NetRow[] }>('pcb.nets', { doc }),
      engine.call<{ rules: { netClasses: { name: string }[] } }>('pcb.rules', { doc }),
    ]).then(
      ([nets, rules]) => {
        if (!cancelled) {
          setRows(nets.nets.filter((net) => net.name !== ''));
          setClasses(rules.rules.netClasses.map((c) => c.name));
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, doc, version]);

  const shownRows = useMemo(() => {
    const words = filter
      .toLowerCase()
      .split(/\s+/u)
      .filter((w) => w !== '');
    const kept = (rows ?? []).filter((row) =>
      words.every(
        (word) => row.name.toLowerCase().includes(word) || row.class.toLowerCase().includes(word),
      ),
    );
    const sign = sort.down ? -1 : 1;
    return [...kept].sort((a, b) => {
      const x = a[sort.column];
      const y = b[sort.column];
      const order =
        typeof x === 'number' && typeof y === 'number'
          ? x - y
          : String(x).localeCompare(String(y), undefined, { numeric: true });
      return sign * order || a.name.localeCompare(b.name, undefined, { numeric: true });
    });
  }, [rows, filter, sort]);

  const setClass = (name: string): void => {
    const ids = [...ticked];
    if (ids.length === 0) {
      return;
    }
    engine
      .call('edit.apply', {
        label: 'edit-properties',
        changes: ids.map((id) => ({ op: 'patch', doc, id, fields: { class: name } })),
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
  };
  const allTicked = shownRows.length > 0 && shownRows.every((row) => ticked.has(row.id));
  /** One colour (or none) for the ticked nets, in one undo step. */
  const colorTicked = (color: string | null): void => {
    const names = (rows ?? []).filter((row) => ticked.has(row.id)).map((row) => row.name);
    void setProperties({
      settings: { netColors: Object.fromEntries(names.map((name) => [name, color])) },
    });
  };

  return (
    <section
      className="erc net-inspector"
      aria-label={t('board.nets.title')}
      data-testid="net-inspector"
    >
      <header className="erc__header">
        <h2 className="erc__title">{t('board.nets.title')}</h2>
        <span className="erc__summary" data-testid="net-count">
          {rows === null
            ? t('library.loading')
            : t('board.nets.count', { shown: shownRows.length, total: rows.length })}
        </span>
        <input
          className="field__control net-inspector__filter"
          type="search"
          placeholder={t('board.nets.filter')}
          aria-label={t('board.nets.filter')}
          value={filter}
          data-testid="net-filter"
          onChange={(event) => {
            setFilter(event.target.value);
          }}
        />
        <span className="erc__spacer" />
        <button
          type="button"
          className="icon-button"
          title={t('board.nets.close')}
          aria-label={t('board.nets.close')}
          data-testid="net-inspector-close"
          onClick={() => {
            netInspector.set(false);
          }}
        >
          <Icon name="close" size={16} />
        </button>
      </header>
      {ticked.size > 0 && (
        <div className="net-inspector__bar" data-testid="net-ticked-bar">
          <span className="net-inspector__ticked">
            {t('board.nets.ticked', { count: ticked.size })}
          </span>
          <label className="net-inspector__class">
            <span>{t('board.nets.setClass')}</span>
            <select
              className="field__control"
              value=""
              data-testid="net-set-class"
              onChange={(event) => {
                setClass(event.target.value);
              }}
            >
              <option value="" disabled>
                {t('board.nets.chooseClass')}
              </option>
              {classes.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
          </label>
          <span className="net-inspector__colors" role="group" aria-label={t('netColor.forTicked')}>
            <span>{t('board.nets.color')}</span>
            {NET_SWATCHES.map((color) => (
              <button
                key={color}
                type="button"
                className="net-color__swatch"
                style={{ background: color }}
                title={color}
                aria-label={t('netColor.swatch', { color })}
                data-testid={`net-tick-color-${color.slice(1)}`}
                onClick={() => {
                  colorTicked(color);
                }}
              />
            ))}
            <button
              type="button"
              className="button button--small"
              data-testid="net-tick-color-none"
              onClick={() => {
                colorTicked(null);
              }}
            >
              {t('netColor.none')}
            </button>
          </span>
        </div>
      )}
      <div className="net-inspector__scroll">
        <table className="table net-inspector__table" data-testid="net-table">
          <thead>
            <tr>
              <th>
                <input
                  type="checkbox"
                  aria-label={t('board.nets.tickAll')}
                  checked={allTicked}
                  onChange={() => {
                    setTicked(allTicked ? new Set() : new Set(shownRows.map((row) => row.id)));
                  }}
                />
              </th>
              {COLUMNS.map((column) => (
                <th
                  key={column}
                  aria-sort={
                    sort.column === column ? (sort.down ? 'descending' : 'ascending') : 'none'
                  }
                >
                  <button
                    type="button"
                    className="net-inspector__sort"
                    data-testid={`net-sort-${column}`}
                    onClick={() => {
                      setSort({
                        column,
                        down:
                          sort.column === column
                            ? !sort.down
                            : column !== 'name' && column !== 'class',
                      });
                    }}
                  >
                    {t(`board.nets.columns.${column}`)}
                    {sort.column === column && (sort.down ? ' ↓' : ' ↑')}
                  </button>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {shownRows.map((row) => (
              <tr
                key={row.id}
                data-selected={shown === row.name}
                data-testid="net-row"
                data-name={row.name}
                onClick={() => {
                  const next = shown === row.name ? null : row.name;
                  setShown(next);
                  onShow(next);
                }}
              >
                <td>
                  <input
                    type="checkbox"
                    aria-label={t('board.nets.tick', { name: row.name })}
                    checked={ticked.has(row.id)}
                    onClick={(event) => {
                      event.stopPropagation();
                    }}
                    onChange={() => {
                      const next = new Set(ticked);
                      if (next.has(row.id)) {
                        next.delete(row.id);
                      } else {
                        next.add(row.id);
                      }
                      setTicked(next);
                    }}
                  />
                </td>
                <td className="net-inspector__name">
                  <span
                    className="net-inspector__dot"
                    style={{ background: netColors[row.name] ?? 'transparent' }}
                    data-color={netColors[row.name] ?? ''}
                  />
                  {row.name}
                </td>
                <td>{row.class}</td>
                <td className="net-inspector__number">
                  {row.length > 0
                    ? formatLength(Math.round(row.length), units, snapshot.language)
                    : '–'}
                </td>
                <td className="net-inspector__number">{row.tracks}</td>
                <td className="net-inspector__number">{row.vias}</td>
                <td className="net-inspector__number">{row.pads}</td>
                <td
                  className={`net-inspector__number${row.unrouted > 0 ? ' net-inspector__open' : ''}`}
                >
                  {row.unrouted}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
