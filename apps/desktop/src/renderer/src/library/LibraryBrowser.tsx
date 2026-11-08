/**
 * Search and browse the base library: search field, kind tabs, filters,
 * a virtualized result list and the details of the selected entry.
 */
import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';

import type { LibraryKind } from '@shared/library';

import { Icon } from '../components/Icon';
import { useAppState } from '../state';
import { LibraryDetails } from './LibraryDetails';
import type { EntrySummary, Facet, SearchQuery, SearchResult } from './types';
import { VirtualList } from './VirtualList';

const ROW_HEIGHT = 44;
const KINDS: readonly (LibraryKind | 'all')[] = ['all', 'symbol', 'footprint', 'model'];

export interface BrowseFilters {
  readonly text: string;
  readonly kind: LibraryKind | 'all';
  readonly libraries: readonly string[];
  readonly mounting: string;
  readonly withModel: boolean;
  readonly duplicates: boolean;
  readonly includeMerged: boolean;
}

export const EMPTY_FILTERS: BrowseFilters = {
  text: '',
  kind: 'all',
  libraries: [],
  mounting: '',
  withModel: false,
  duplicates: false,
  includeMerged: false,
};

function useDebounced<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebounced(value);
    }, delayMs);
    return () => {
      clearTimeout(timer);
    };
  }, [value, delayMs]);
  return debounced;
}

export function LibraryBrowser({
  filters,
  onFilters,
  selected,
  onSelect,
  release,
  version,
}: {
  readonly filters: BrowseFilters;
  readonly onFilters: (filters: BrowseFilters) => void;
  readonly selected: number | null;
  readonly onSelect: (id: number | null) => void;
  readonly release: string | null;
  /** Changes when the index changes (rebuild, install, decision). */
  readonly version: number;
}): ReactNode {
  const { engine, t } = useAppState();
  const text = useDebounced(filters.text, 120);
  const [result, setResult] = useState<SearchResult | null>(null);
  const [facetState, setFacetState] = useState<{ kind: string; list: readonly Facet[] } | null>(
    null,
  );
  const facets = facetState?.kind === filters.kind ? facetState.list : [];
  const [libraryFilter, setLibraryFilter] = useState('');

  const query = useMemo<Omit<SearchQuery, 'offset' | 'limit'>>(
    () => ({
      text,
      ...(filters.kind === 'all' ? {} : { kind: filters.kind }),
      libraries: filters.libraries,
      ...(filters.mounting === '' ? {} : { mounting: filters.mounting }),
      withModel: filters.withModel,
      duplicates: filters.duplicates,
      includeMerged: filters.includeMerged,
    }),
    [
      text,
      filters.kind,
      filters.libraries,
      filters.mounting,
      filters.withModel,
      filters.duplicates,
      filters.includeMerged,
    ],
  );
  const queryKey = `${JSON.stringify(query)}#${version.toString()}`;

  // The first page tells the total; the list fetches the rest itself.
  useEffect(() => {
    let cancelled = false;
    void engine.call<SearchResult>('library.search', { ...query, offset: 0, limit: 1 }).then(
      (reply) => {
        if (!cancelled) {
          setResult(reply);
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, query, version]);

  useEffect(() => {
    const kind = filters.kind;
    if (kind === 'all') {
      return undefined;
    }
    let cancelled = false;
    void engine.call<Facet[]>('library.facets', { kind }).then(
      (reply) => {
        if (!cancelled) {
          setFacetState({ kind, list: reply });
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, filters.kind, version]);

  const fetchPage = useCallback(
    async (offset: number, limit: number) => {
      const reply = await engine.call<SearchResult>('library.search', { ...query, offset, limit });
      return reply.items;
    },
    [engine, query],
  );

  const set = (patch: Partial<BrowseFilters>): void => {
    onFilters({ ...filters, ...patch });
  };
  const shownFacets = facets.filter((facet) =>
    facet.library.toLowerCase().includes(libraryFilter.toLowerCase()),
  );
  const filtered =
    filters.libraries.length > 0 ||
    filters.mounting !== '' ||
    filters.withModel ||
    filters.duplicates ||
    filters.includeMerged;

  return (
    <div className="library-browser">
      <aside className="library-filters" aria-label={t('library.filters')}>
        <div className="library-kinds" role="tablist">
          {KINDS.map((kind) => (
            <button
              key={kind}
              type="button"
              role="tab"
              aria-selected={filters.kind === kind}
              className={`library-kinds__item${filters.kind === kind ? ' library-kinds__item--active' : ''}`}
              data-testid={`library-kind-${kind}`}
              onClick={() => {
                set({ kind, libraries: [], mounting: '', withModel: false });
              }}
            >
              {t(`library.kinds.${kind}`)}
            </button>
          ))}
        </div>
        {filters.kind === 'footprint' && (
          <label className="field">
            <span className="field__label">{t('library.mounting')}</span>
            <select
              className="field__control"
              value={filters.mounting}
              data-testid="library-mounting"
              onChange={(event) => {
                set({ mounting: event.target.value });
              }}
            >
              <option value="">{t('library.mountingAny')}</option>
              <option value="smd">{t('library.mountings.smd')}</option>
              <option value="tht">{t('library.mountings.tht')}</option>
              <option value="other">{t('library.mountings.other')}</option>
            </select>
          </label>
        )}
        {filters.kind === 'footprint' && (
          <label className="checkbox">
            <input
              type="checkbox"
              checked={filters.withModel}
              data-testid="library-with-model"
              onChange={(event) => {
                set({ withModel: event.target.checked });
              }}
            />
            {t('library.withModel')}
          </label>
        )}
        <label className="checkbox">
          <input
            type="checkbox"
            checked={filters.duplicates}
            data-testid="library-only-duplicates"
            onChange={(event) => {
              set({ duplicates: event.target.checked });
            }}
          />
          {t('library.onlyDuplicates')}
        </label>
        <label className="checkbox">
          <input
            type="checkbox"
            checked={filters.includeMerged}
            data-testid="library-show-merged"
            onChange={(event) => {
              set({ includeMerged: event.target.checked });
            }}
          />
          {t('library.showMerged')}
        </label>
        {filters.kind !== 'all' && facets.length > 0 && (
          <div className="library-facets">
            <span className="field__label">{t('library.libraries')}</span>
            <input
              className="field__control"
              placeholder={t('library.filterLibraries')}
              value={libraryFilter}
              data-testid="library-facet-filter"
              onChange={(event) => {
                setLibraryFilter(event.target.value);
              }}
            />
            <ul className="library-facets__list" data-testid="library-facets">
              {shownFacets.map((facet) => {
                const checked = filters.libraries.includes(facet.library);
                return (
                  <li key={facet.library}>
                    <label className="checkbox">
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => {
                          set({
                            libraries: checked
                              ? filters.libraries.filter((l) => l !== facet.library)
                              : [...filters.libraries, facet.library],
                          });
                        }}
                      />
                      <span className="library-facets__name">{facet.library}</span>
                      <span className="library-facets__count">{facet.count}</span>
                    </label>
                  </li>
                );
              })}
            </ul>
          </div>
        )}
        {filtered && (
          <button
            type="button"
            className="link-button"
            onClick={() => {
              set({
                libraries: [],
                mounting: '',
                withModel: false,
                duplicates: false,
                includeMerged: false,
              });
            }}
          >
            {t('library.clearFilters')}
          </button>
        )}
      </aside>
      <section className="library-results">
        <div className="library-search">
          <Icon name="search" size={18} />
          <input
            className="library-search__input"
            type="search"
            // The search field is the first thing to use in the library.
            autoFocus
            placeholder={t('library.searchPlaceholder')}
            aria-label={t('library.searchPlaceholder')}
            value={filters.text}
            data-testid="library-search"
            onChange={(event) => {
              set({ text: event.target.value });
            }}
          />
        </div>
        <p className="library-results__count" data-testid="library-count">
          {result === null
            ? t('library.loading')
            : t(result.total === 1 ? 'library.resultsOne' : 'library.results', {
                count: new Intl.NumberFormat().format(result.total),
                ms: result.elapsedMs.toFixed(1),
              })}
        </p>
        <VirtualList<EntrySummary>
          total={result?.total ?? 0}
          rowHeight={ROW_HEIGHT}
          key={queryKey}
          fetchPage={fetchPage}
          rowKey={(item) => item.id}
          selected={selected}
          label={t('library.title')}
          testId="library-list"
          empty={<p className="dialog__text dialog__text--muted">{t('library.noResults')}</p>}
          onSelect={(item) => {
            onSelect(item.id);
          }}
          renderRow={(item) => (
            <div className="library-row" data-name={`${item.library}:${item.name}`}>
              <span className={`library-kind library-kind--${item.kind}`}>
                {t(`library.kindOne.${item.kind}`)}
              </span>
              <span className="library-row__text">
                <span className="library-row__name">
                  {item.name}
                  <span className="library-row__library">{item.library}</span>
                </span>
                <span className="library-row__description">{item.description}</span>
              </span>
              {item.groups > 0 && (
                <span className="library-row__badge" title={t('library.fields.groups')}>
                  <Icon name="duplicate" size={14} />
                </span>
              )}
            </div>
          )}
        />
      </section>
      <section className="library-side">
        {selected === null ? (
          <p className="library-details library-details--empty">{t('library.selectHint')}</p>
        ) : (
          <LibraryDetails key={selected} id={selected} release={release} onSelect={onSelect} />
        )}
      </section>
    </div>
  );
}
