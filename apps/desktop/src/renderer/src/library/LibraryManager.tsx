/**
 * The library manager (ADR-0005): browse and search the base library,
 * review duplicates, and set up or update the library. It covers the
 * window and keeps the project underneath open.
 */
import { useCallback, useEffect, useState, type ReactNode } from 'react';

import { formatBytes, type LibraryRelease } from '@shared/library';

import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { DEFAULT_DUPLICATE_FILTERS, DuplicatesView, type DuplicateFilters } from './DuplicatesView';
import { EMPTY_FILTERS, LibraryBrowser, type BrowseFilters } from './LibraryBrowser';
import { installRelease, LibrarySetup } from './LibrarySetup';
import { isLibraryEvent, type BuildProgress, type LibraryStatus } from './types';

/** View state kept while the app runs (the manager can be closed and reopened). */
let remembered: {
  tab: 'browse' | 'duplicates';
  filters: BrowseFilters;
  duplicates: DuplicateFilters;
  selected: number | null;
} = {
  tab: 'browse',
  filters: EMPTY_FILTERS,
  duplicates: DEFAULT_DUPLICATE_FILTERS,
  selected: null,
};

function InfoPanel({
  status,
  onRebuild,
  onClose,
}: {
  readonly status: LibraryStatus;
  readonly onRebuild: () => void;
  readonly onClose: () => void;
}): ReactNode {
  const { engine, t } = useAppState();
  const [check, setCheck] = useState<
    | { kind: 'idle' }
    | { kind: 'checking' }
    | { kind: 'current'; name: string }
    | { kind: 'newer'; release: LibraryRelease }
    | { kind: 'installing'; percent: number }
    | { kind: 'failed'; message: string }
  >({ kind: 'idle' });
  const index = status.status;
  if (index === null) {
    return null;
  }
  const source = index.sources[0];
  const number = (value: number): string => new Intl.NumberFormat().format(value);
  return (
    <div
      className="library-info"
      role="dialog"
      aria-label={t('library.info.title')}
      data-testid="library-info"
    >
      <h3 className="library-info__title">{t('library.info.title')}</h3>
      <p>
        {t('library.info.stats', {
          symbols: number(index.counts.symbol),
          footprints: number(index.counts.footprint),
          models: number(index.counts.model),
        })}
      </p>
      {source !== undefined && (
        <p className="dialog__text dialog__text--muted">
          {t('library.info.source', {
            name: source.name,
            version: source.version,
            license: source.license,
          })}
        </p>
      )}
      <p className="dialog__text dialog__text--muted">
        {t('library.info.groups', {
          exact: number(index.groups.exact),
          near: number(index.groups.near),
          pending: number(index.pending),
          alias: number(index.groups.alias),
        })}
      </p>
      <p className="dialog__text dialog__text--muted">
        <code className="path library-info__path">{index.path}</code> · {formatBytes(index.bytes)}
      </p>
      {check.kind === 'current' && <p>{t('library.info.upToDate', { name: check.name })}</p>}
      {check.kind === 'newer' && (
        <p>{t('library.info.updateAvailable', { name: check.release.name })}</p>
      )}
      {check.kind === 'installing' && <progress max={100} value={check.percent} />}
      {check.kind === 'failed' && <p className="callout callout--error">{check.message}</p>}
      <div className="library-info__actions">
        {check.kind === 'newer' ? (
          <button
            type="button"
            className="button button--primary"
            onClick={() => {
              const release = check.release;
              setCheck({ kind: 'installing', percent: 0 });
              void installRelease(
                engine,
                release,
                (_, percent) => {
                  setCheck({ kind: 'installing', percent });
                },
                () => undefined,
              ).then(
                () => {
                  setCheck({ kind: 'current', name: release.name });
                },
                (error: unknown) => {
                  setCheck({
                    kind: 'failed',
                    message: error instanceof Error ? error.message : String(error),
                  });
                },
              );
            }}
          >
            {t('library.info.install')}
          </button>
        ) : (
          <button
            type="button"
            className="button"
            disabled={check.kind === 'checking' || check.kind === 'installing'}
            onClick={() => {
              setCheck({ kind: 'checking' });
              void window.ew.library.latest().then(
                (latest) => {
                  if (latest === null || latest.tag === index.release) {
                    setCheck({ kind: 'current', name: latest?.name ?? source?.name ?? '' });
                  } else {
                    setCheck({ kind: 'newer', release: latest });
                  }
                },
                (error: unknown) => {
                  setCheck({
                    kind: 'failed',
                    message: error instanceof Error ? error.message : String(error),
                  });
                },
              );
            }}
          >
            {t('library.info.update')}
          </button>
        )}
        <button type="button" className="button" onClick={onRebuild}>
          {t('library.info.rebuild')}
        </button>
        <button
          type="button"
          className="button"
          onClick={() => {
            window.ew.projects.showInFolder(`${index.path}/dedup-report.md`);
          }}
        >
          {t('library.info.report')}
        </button>
        <button type="button" className="button" onClick={onClose}>
          {t('validation.close')}
        </button>
      </div>
    </div>
  );
}

async function loadStatus(
  engine: ReturnType<typeof useAppState>['engine'],
): Promise<LibraryStatus> {
  try {
    await engine.whenConnected();
    return await engine.call<LibraryStatus>('library.status');
  } catch {
    return { dir: null, installed: false, building: null, status: null };
  }
}

export function LibraryManager({ onClose }: { readonly onClose: () => void }): ReactNode {
  const { engine, t } = useAppState();
  const { notify } = useProject();
  const [status, setStatus] = useState<LibraryStatus | null>(null);
  const [building, setBuilding] = useState<BuildProgress | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [version, setVersion] = useState(0);
  const [tab, setTab] = useState(remembered.tab);
  const [filters, setFilters] = useState(remembered.filters);
  const [duplicates, setDuplicates] = useState(remembered.duplicates);
  const [selected, setSelected] = useState<number | null>(remembered.selected);
  const [info, setInfo] = useState(false);
  const [setup, setSetup] = useState(false);

  useEffect(() => {
    remembered = { tab, filters, duplicates, selected };
  }, [tab, filters, duplicates, selected]);

  const refresh = useCallback(async () => {
    const next = await loadStatus(engine);
    setStatus(next);
    setBuilding(next.building);
  }, [engine]);

  useEffect(() => {
    let cancelled = false;
    void loadStatus(engine).then((next) => {
      if (!cancelled) {
        setStatus(next);
        setBuilding(next.building);
      }
    });
    const off = engine.onEvent((event) => {
      if (!isLibraryEvent(event)) {
        return;
      }
      switch (event.action) {
        case 'progress':
          setBuilding({ stage: event.stage, done: event.done, total: event.total });
          break;
        case 'built': {
          setBuilding(null);
          setFailure(null);
          setSetup(false);
          setSelected(null);
          setVersion((n) => n + 1);
          void refresh();
          const counts = event.status.status?.counts;
          if (counts !== undefined) {
            notify(
              t('library.setup.built', {
                symbols: counts.symbol,
                footprints: counts.footprint,
                models: counts.model,
              }),
            );
          }
          break;
        }
        case 'failed':
          setBuilding(null);
          setFailure(event.message);
          break;
        case 'changed':
          setVersion((n) => n + 1);
          void refresh();
          break;
      }
    });
    return () => {
      cancelled = true;
      off();
    };
  }, [engine, notify, refresh, t]);

  // Escape closes the manager (unless a field or panel wants it first).
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === 'Escape' && !event.defaultPrevented) {
        if (info) {
          setInfo(false);
        } else {
          onClose();
        }
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [info, onClose]);

  const installed = status?.installed === true && !setup;
  const release = status?.status?.release ?? null;
  const pending = status?.status?.pending ?? 0;

  return (
    <div className="library" data-testid="library-manager" data-installed={installed}>
      <header className="library__header">
        <Icon name="library" size={20} />
        <h1 className="library__title">{t('library.title')}</h1>
        {installed && (
          <div className="library__tabs" role="tablist">
            {(['browse', 'duplicates'] as const).map((key) => (
              <button
                key={key}
                type="button"
                role="tab"
                aria-selected={tab === key}
                className={`library__tab${tab === key ? ' library__tab--active' : ''}`}
                data-testid={`library-tab-${key}`}
                onClick={() => {
                  setTab(key);
                }}
              >
                {t(`library.tabs.${key}`)}
                {key === 'duplicates' && pending > 0 && (
                  <span className="library__badge">{pending}</span>
                )}
              </button>
            ))}
          </div>
        )}
        <span className="library__spacer" />
        {installed && (
          <button
            type="button"
            className="button button--quiet"
            data-testid="library-info-button"
            aria-expanded={info}
            onClick={() => {
              setInfo((open) => !open);
            }}
          >
            <Icon name="info" size={18} /> {t('library.info.title')}
          </button>
        )}
        <button
          type="button"
          className="button button--quiet"
          title={t('library.close')}
          aria-label={t('library.close')}
          data-testid="library-close"
          onClick={onClose}
        >
          <Icon name="close" size={18} />
        </button>
      </header>
      {info && status !== null && (
        <InfoPanel
          status={status}
          onRebuild={() => {
            setInfo(false);
            setSetup(true);
          }}
          onClose={() => {
            setInfo(false);
          }}
        />
      )}
      <div className="library__body">
        {status === null && <p className="library__loading">{t('library.loading')}</p>}
        {status !== null && !installed && <LibrarySetup building={building} failure={failure} />}
        {installed && tab === 'browse' && (
          <LibraryBrowser
            filters={filters}
            onFilters={setFilters}
            selected={selected}
            onSelect={setSelected}
            release={release}
            version={version}
          />
        )}
        {installed && tab === 'duplicates' && (
          <DuplicatesView
            filters={duplicates}
            onFilters={setDuplicates}
            release={release}
            version={version}
          />
        )}
      </div>
    </div>
  );
}
