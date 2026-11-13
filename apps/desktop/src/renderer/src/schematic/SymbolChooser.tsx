import { useEffect, useMemo, useState, type ReactNode } from 'react';

import { Modal } from '../components/Modal';
import type { EntrySummary, LibraryStatus, SearchResult } from '../library/types';
import { Icon } from '../components/Icon';
import { openOnlineParts } from '../parts/stores';
import { useAppState } from '../state';
import type { SymbolDefJson } from './editor';
import { SymbolPreview } from './SymbolShapes';

/** A symbol of the project libraries (`schematic.symbols`). */
export interface ProjectSymbol {
  readonly name: string;
  readonly description: string;
  readonly keywords: string;
  readonly prefix: string;
  readonly value: string;
  readonly power: boolean;
  readonly units: number;
  readonly pins: number;
  readonly library: string;
}

/** A component of the project libraries (`schematic.components`). */
export interface ProjectComponent {
  readonly name: string;
  readonly description: string;
  readonly keywords: string;
  readonly symbol: string;
  readonly footprint: string | null;
  readonly value: string;
  readonly manufacturer: string;
  readonly mpn: string;
  readonly library: string;
}

type Choice =
  | { readonly source: 'component'; readonly component: ProjectComponent }
  | { readonly source: 'project'; readonly symbol: ProjectSymbol }
  | { readonly source: 'base'; readonly entry: EntrySummary };

function matches(symbol: ProjectSymbol, words: readonly string[]): boolean {
  const text =
    `${symbol.name} ${symbol.description} ${symbol.keywords} ${symbol.value}`.toLowerCase();
  return words.every((word) => text.includes(word));
}

function componentMatches(component: ProjectComponent, words: readonly string[]): boolean {
  const text =
    `${component.name} ${component.description} ${component.keywords} ${component.value} ${component.manufacturer} ${component.mpn}`.toLowerCase();
  return words.every((word) => text.includes(word));
}

/**
 * Picks a symbol to place: the project's own symbols first, then the base
 * library (copied into the project when chosen).
 */
export function SymbolChooser({
  onPick,
  onClose,
}: {
  /** The chosen symbol's name in the project library (and the component placed with it). */
  readonly onPick: (name: string, component?: string) => void;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const [query, setQuery] = useState('');
  const [project, setProject] = useState<readonly ProjectSymbol[]>([]);
  const [components, setComponents] = useState<readonly ProjectComponent[]>([]);
  const [base, setBase] = useState<{ query: string; items: readonly EntrySummary[] } | null>(null);
  const [baseReady, setBaseReady] = useState(false);
  const [choice, setChoice] = useState<Choice | null>(null);
  const [preview, setPreview] = useState<{ name: string; def: SymbolDefJson } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void engine.call<ProjectSymbol[]>('schematic.symbols').then(
      (list) => {
        if (!cancelled) {
          setProject(list);
        }
      },
      () => undefined,
    );
    void engine.call<ProjectComponent[]>('schematic.components').then(
      (list) => {
        if (!cancelled) {
          setComponents(list);
        }
      },
      () => undefined,
    );
    void engine.call<LibraryStatus>('library.status').then(
      (status) => {
        if (!cancelled) {
          setBaseReady(status.installed);
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine]);

  const words = useMemo(
    () =>
      query
        .toLowerCase()
        .split(/\s+/u)
        .filter((word) => word !== ''),
    [query],
  );
  const shown = useMemo(
    () => project.filter((symbol) => matches(symbol, words)).slice(0, 200),
    [project, words],
  );
  const shownComponents = useMemo(
    () => components.filter((component) => componentMatches(component, words)).slice(0, 200),
    [components, words],
  );

  // The base library answers searches of two characters or more.
  useEffect(() => {
    if (!baseReady || query.trim().length < 2) {
      return undefined;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      void engine
        .call<SearchResult>('library.search', { text: query, kind: 'symbol', offset: 0, limit: 60 })
        .then(
          (reply) => {
            if (!cancelled) {
              setBase({ query, items: reply.items });
            }
          },
          () => undefined,
        );
    }, 120);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [engine, baseReady, query]);
  const baseItems =
    base !== null && base.query === query && query.trim().length >= 2 ? base.items : [];

  // Preview of a project symbol.
  const previewName =
    choice?.source === 'project'
      ? choice.symbol.name
      : choice?.source === 'component'
        ? choice.component.symbol
        : null;
  useEffect(() => {
    if (previewName === null) {
      return undefined;
    }
    let cancelled = false;
    void engine.call<SymbolDefJson>('schematic.symbol', { name: previewName }).then(
      (def) => {
        if (!cancelled) {
          setPreview({ name: previewName, def });
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, previewName]);

  const pick = async (picked: Choice | null): Promise<void> => {
    if (picked === null) {
      return;
    }
    if (picked.source === 'project') {
      onPick(picked.symbol.name);
      return;
    }
    if (picked.source === 'component') {
      onPick(picked.component.symbol, picked.component.name);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await engine.call('library.add', { ids: [picked.entry.id] }, 120_000);
      onPick(picked.entry.name);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  };

  const first: Choice | null =
    shownComponents[0] !== undefined && words.length > 0
      ? { source: 'component', component: shownComponents[0] }
      : shown[0] !== undefined
        ? { source: 'project', symbol: shown[0] }
        : baseItems[0] !== undefined
          ? { source: 'base', entry: baseItems[0] }
          : null;
  const current = choice ?? first;

  return (
    <Modal
      title={t('schematic.chooser.title')}
      onClose={onClose}
      testId="symbol-chooser"
      className="dialog--wide"
      footer={
        <button
          type="button"
          className="button button--primary"
          disabled={current === null || busy}
          data-testid="chooser-place"
          onClick={() => {
            void pick(current);
          }}
        >
          {current?.source === 'base'
            ? t('schematic.chooser.addAndPlace')
            : t('schematic.chooser.place')}
        </button>
      }
    >
      <div className="chooser">
        <div className="chooser__list">
          <input
            className="chooser__search"
            type="search"
            value={query}
            placeholder={t('schematic.chooser.search')}
            aria-label={t('schematic.chooser.search')}
            data-testid="chooser-search"
            autoFocus
            onChange={(event) => {
              setQuery(event.target.value);
              setChoice(null);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault();
                void pick(current);
              }
            }}
          />
          {shownComponents.length > 0 && (
            <>
              <h3 className="chooser__heading">{t('schematic.chooser.components')}</h3>
              <ul className="chooser__items" data-testid="chooser-components">
                {shownComponents.map((component) => (
                  <li key={component.name}>
                    <button
                      type="button"
                      className="chooser__item"
                      aria-pressed={
                        current?.source === 'component' && current.component.name === component.name
                      }
                      data-testid={`chooser-component-${component.name}`}
                      onClick={() => {
                        setChoice({ source: 'component', component });
                      }}
                      onDoubleClick={() => {
                        void pick({ source: 'component', component });
                      }}
                    >
                      <span className="chooser__name">{component.name}</span>
                      <span className="chooser__meta">{component.mpn}</span>
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
          <h3 className="chooser__heading">{t('schematic.chooser.project')}</h3>
          {shown.length === 0 && <p className="chooser__empty">{t('schematic.chooser.empty')}</p>}
          <button
            type="button"
            className="button button--quiet chooser__online"
            title={t('parts.openTip')}
            data-testid="chooser-online"
            onClick={() => {
              openOnlineParts(query);
            }}
          >
            <Icon name="search" size={16} />
            <span>{t('parts.searchOnline')}</span>
          </button>
          <ul className="chooser__items" data-testid="chooser-project">
            {shown.map((symbol) => (
              <li key={symbol.name}>
                <button
                  type="button"
                  className="chooser__item"
                  aria-pressed={
                    current?.source === 'project' && current.symbol.name === symbol.name
                  }
                  data-testid={`chooser-item-${symbol.name}`}
                  onClick={() => {
                    setChoice({ source: 'project', symbol });
                  }}
                  onDoubleClick={() => {
                    void pick({ source: 'project', symbol });
                  }}
                >
                  <span className="chooser__name">{symbol.name}</span>
                  <span className="chooser__meta">
                    {symbol.power
                      ? t('schematic.chooser.power')
                      : symbol.units > 1
                        ? t('schematic.chooser.units', { count: symbol.units })
                        : t('schematic.chooser.pins', { count: symbol.pins })}
                  </span>
                </button>
              </li>
            ))}
          </ul>
          <h3 className="chooser__heading">{t('schematic.chooser.base')}</h3>
          {!baseReady && <p className="chooser__empty">{t('schematic.chooser.noBase')}</p>}
          {baseReady && query.trim().length >= 2 && baseItems.length === 0 && (
            <p className="chooser__empty">{t('schematic.chooser.empty')}</p>
          )}
          <ul className="chooser__items" data-testid="chooser-base">
            {baseItems.map((entry) => (
              <li key={entry.id}>
                <button
                  type="button"
                  className="chooser__item"
                  aria-pressed={current?.source === 'base' && current.entry.id === entry.id}
                  onClick={() => {
                    setChoice({ source: 'base', entry });
                  }}
                  onDoubleClick={() => {
                    void pick({ source: 'base', entry });
                  }}
                >
                  <span className="chooser__name">{entry.name}</span>
                  <span className="chooser__meta">{entry.library}</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
        <div className="chooser__detail">
          {current?.source === 'project' && (
            <>
              {preview?.name === current.symbol.name && (
                <SymbolPreview
                  def={preview.def}
                  label={t('schematic.chooser.preview', { name: current.symbol.name })}
                />
              )}
              <p className="chooser__title">{current.symbol.name}</p>
              <p className="chooser__description">{current.symbol.description}</p>
            </>
          )}
          {current?.source === 'component' && (
            <>
              {preview?.name === current.component.symbol && (
                <SymbolPreview
                  def={preview.def}
                  label={t('schematic.chooser.preview', { name: current.component.name })}
                />
              )}
              <p className="chooser__title">{current.component.name}</p>
              <p className="chooser__meta">
                {t('schematic.chooser.component', {
                  symbol: current.component.symbol,
                  footprint: current.component.footprint ?? '–',
                })}
              </p>
              <p className="chooser__description">
                {[
                  current.component.manufacturer,
                  current.component.mpn,
                  current.component.description,
                ]
                  .filter((part) => part !== '')
                  .join(' · ')}
              </p>
            </>
          )}
          {current?.source === 'base' && (
            <>
              <p className="chooser__title">{current.entry.name}</p>
              <p className="chooser__meta">{current.entry.library}</p>
              <p className="chooser__description">{current.entry.description}</p>
            </>
          )}
          {error !== null && (
            <p className="callout callout--error" role="alert">
              {error}
            </p>
          )}
        </div>
      </div>
    </Modal>
  );
}
