/**
 * Setting up the base library: download the KiCad library pack from GitHub,
 * or build the library from KiCad libraries on this computer.
 */
import { useEffect, useState, type ReactNode } from 'react';

import { formatBytes, type LibraryRelease } from '@shared/library';

import { Icon } from '../components/Icon';
import { useAppState } from '../state';
import type { BuildProgress } from './types';

interface Found {
  readonly path: string;
}

type Download =
  | { readonly kind: 'idle' }
  | { readonly kind: 'downloading'; readonly file: string; readonly percent: number }
  | { readonly kind: 'installing' }
  | { readonly kind: 'failed'; readonly message: string };

/** Downloads and installs the index pack of a release. */
export async function installRelease(
  engine: ReturnType<typeof useAppState>['engine'],
  release: LibraryRelease,
  onProgress: (file: string, percent: number) => void,
  onInstalling: () => void,
): Promise<void> {
  const pack = release.packs.find((p) => p.kind === 'index');
  if (pack === undefined) {
    throw new Error('the release has no index pack');
  }
  const off = window.ew.library.onProgress((progress) => {
    if (progress.file === pack.file) {
      onProgress(pack.file, Math.floor((progress.received / Math.max(1, progress.total)) * 100));
    }
  });
  try {
    onProgress(pack.file, 0);
    const file = await window.ew.library.download(release.tag, pack.file);
    onInstalling();
    await engine.call('library.install', { file, release: release.tag, remove: true }, 300_000);
  } finally {
    off();
  }
}

export function LibrarySetup({
  building,
  failure,
}: {
  readonly building: BuildProgress | null;
  readonly failure: string | null;
}): ReactNode {
  const { engine, t } = useAppState();
  const [release, setRelease] = useState<LibraryRelease | null | undefined>(undefined);
  const [releaseError, setReleaseError] = useState<string | null>(null);
  const [found, setFound] = useState<readonly Found[]>([]);
  const [download, setDownload] = useState<Download>({ kind: 'idle' });
  const [buildError, setBuildError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void window.ew.library.latest().then(
      (latest) => {
        if (!cancelled) {
          setRelease(latest);
        }
      },
      (error: unknown) => {
        if (!cancelled) {
          setRelease(null);
          setReleaseError(error instanceof Error ? error.message : String(error));
        }
      },
    );
    void engine.call<Found[]>('library.detect').then(
      (list) => {
        if (!cancelled) {
          setFound(list);
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine]);

  const build = (path: string): void => {
    setBuildError(null);
    void engine.call('library.build', { sources: [{ kicad: path }] }).catch((error: unknown) => {
      setBuildError(error instanceof Error ? error.message : String(error));
    });
  };

  const indexPack = release?.packs.find((p) => p.kind === 'index');
  const number = (value: number): string => new Intl.NumberFormat().format(value);
  const busy =
    building !== null || download.kind === 'downloading' || download.kind === 'installing';

  return (
    <div className="library-setup" data-testid="library-setup">
      <div className="library-setup__intro">
        <Icon name="library" size={40} />
        <h2>{t('library.setup.title')}</h2>
        <p className="dialog__text">{t('library.setup.intro')}</p>
      </div>
      {building !== null && (
        <div className="library-setup__progress" data-testid="library-building">
          <p>
            {t('library.setup.building', {
              stage: t(
                `library.setup.stages.${(['symbols', 'footprints', 'models', 'index'] as const).find((s) => s === building.stage) ?? 'start'}`,
              ),
              done: building.done,
              total: building.total,
            })}
          </p>
          <progress max={Math.max(1, building.total)} value={building.done} />
        </div>
      )}
      {failure !== null && (
        <p className="callout callout--error">
          {t('library.setup.buildFailed', { message: failure })}
        </p>
      )}
      <div className="library-setup__options">
        <section className="library-setup__card">
          <h3 className="library-setup__card-title">
            <Icon name="download" size={20} /> {t('library.setup.download')}
          </h3>
          {release === undefined && (
            <p className="dialog__text dialog__text--muted">{t('library.setup.checking')}</p>
          )}
          {release === null && (
            <p className="dialog__text dialog__text--muted">
              {releaseError === null
                ? t('library.setup.noRelease')
                : t('library.setup.checkFailed', { message: releaseError })}
            </p>
          )}
          {release !== undefined && release !== null && indexPack !== undefined && (
            <>
              <p className="dialog__text">
                {t('library.setup.downloadHint', {
                  name: release.name,
                  symbols: number(release.counts.symbol),
                  footprints: number(release.counts.footprint),
                  models: number(release.counts.model),
                  size: formatBytes(indexPack.size),
                })}
              </p>
              {download.kind === 'downloading' && (
                <div className="library-setup__progress">
                  <p>
                    {t('library.setup.downloading', {
                      file: download.file,
                      percent: download.percent,
                    })}
                  </p>
                  <progress max={100} value={download.percent} />
                </div>
              )}
              {download.kind === 'installing' && <p>{t('library.setup.installing')}</p>}
              {download.kind === 'failed' && (
                <p className="callout callout--error">
                  {t('library.setup.downloadFailed', { message: download.message })}
                </p>
              )}
              <button
                type="button"
                className="button button--primary"
                data-testid="library-download"
                disabled={busy}
                onClick={() => {
                  void installRelease(
                    engine,
                    release,
                    (file, percent) => {
                      setDownload({ kind: 'downloading', file, percent });
                    },
                    () => {
                      setDownload({ kind: 'installing' });
                    },
                  ).then(
                    () => {
                      setDownload({ kind: 'idle' });
                    },
                    (error: unknown) => {
                      setDownload({
                        kind: 'failed',
                        message: error instanceof Error ? error.message : String(error),
                      });
                    },
                  );
                }}
              >
                {t('library.setup.download')}
              </button>
            </>
          )}
        </section>
        <section className="library-setup__card">
          <h3 className="library-setup__card-title">
            <Icon name="computer" size={20} /> {t('library.setup.local')}
          </h3>
          <p className="dialog__text dialog__text--muted">{t('library.setup.localHint')}</p>
          {found.map((item) => (
            <div key={item.path} className="library-setup__found">
              <span>{t('library.setup.found', { path: item.path })}</span>
              <button
                type="button"
                className="button"
                disabled={busy}
                onClick={() => {
                  build(item.path);
                }}
              >
                {t('library.setup.useFound')}
              </button>
            </div>
          ))}
          {buildError !== null && (
            <p className="callout callout--error">
              {t('library.setup.buildFailed', { message: buildError })}
            </p>
          )}
          <button
            type="button"
            className="button"
            data-testid="library-local"
            disabled={busy}
            onClick={() => {
              void window.ew.dialogs.chooseFolder('', 'library').then((folder) => {
                if (folder !== null) {
                  build(folder);
                }
              });
            }}
          >
            {t('library.setup.chooseFolder')}
          </button>
        </section>
      </div>
      <p className="dialog__text dialog__text--muted library-setup__license">
        {t('library.setup.license')}
      </p>
    </div>
  );
}
