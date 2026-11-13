/**
 * Import of a part download (SnapEDA, Ultra Librarian, or any KiCad ZIP):
 * what it contains, which parts the base library already has, and the
 * import into a project library with the 3D models.
 */
import { useEffect, useState, type ReactNode } from 'react';

import { Icon } from '../components/Icon';
import { Modal } from '../components/Modal';
import { errorMessage } from '../project/labels';
import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { pendingImport } from './stores';

interface Duplicate {
  readonly id: number;
  readonly name: string;
  readonly class: 'exact' | 'pads' | 'near' | 'pins';
}

interface Item {
  readonly name: string;
  readonly description: string;
  readonly footprint?: string;
  readonly models?: readonly string[];
  readonly pins: number;
  readonly exists: boolean;
  readonly duplicate?: Duplicate;
}

interface Preview {
  readonly file: string;
  readonly provider: string | null;
  readonly nickname: string;
  readonly modelFolder: string;
  readonly symbols: readonly Item[];
  readonly footprints: readonly Item[];
  readonly models: readonly { file: string; bytes: number; used: boolean }[];
  readonly problems: readonly string[];
}

interface Result {
  readonly symbols: number;
  readonly footprints: number;
  readonly models: number;
  readonly replaced: number;
  readonly skipped: number;
}

const PROVIDER_NAMES: Readonly<Record<string, string>> = {
  SnapEDA: 'SnapEDA',
  UltraLibrarian: 'Ultra Librarian',
};

function fileName(path: string): string {
  return path.split(/[\\/]/u).pop() ?? path;
}

export function DownloadImport({ path }: { readonly path: string }): ReactNode {
  const { engine, t } = useAppState();
  const { project, notify } = useProject();
  const libraries = (project?.documents ?? []).filter((d) => d.kind === 'library');
  const [doc, setDoc] = useState<string | null>(libraries[0]?.id ?? null);
  const [nickname, setNickname] = useState<string | null>(null);
  const [preview, setPreview] = useState<{ key: string; value: Preview } | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Footprints to replace by the base library's (exact duplicates: yes).
  const [useLibrary, setUseLibrary] = useState<Readonly<Record<string, boolean>>>({});
  const [busy, setBusy] = useState(false);
  const key = `${path}|${nickname ?? ''}|${doc ?? ''}`;

  useEffect(() => {
    let cancelled = false;
    const params: Record<string, unknown> = { path };
    if (nickname !== null && nickname.trim() !== '') {
      params['nickname'] = nickname.trim();
    }
    if (doc !== null) {
      params['doc'] = doc;
    }
    engine.call<Preview>('import.archivePreview', params, 60_000).then(
      (value) => {
        if (!cancelled) {
          setPreview({ key, value });
          setError(null);
        }
      },
      (reason: unknown) => {
        if (!cancelled) {
          setError(errorMessage(t, reason));
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [engine, path, nickname, doc, key, t]);

  const close = (): void => {
    pendingImport.set(null);
  };
  const shown = preview?.value ?? null;
  const replace = (item: Item): boolean =>
    item.duplicate !== undefined &&
    item.duplicate.class !== 'pins' &&
    (useLibrary[item.name] ?? item.duplicate.class === 'exact');
  const importable =
    shown !== null && shown.symbols.length + shown.footprints.length > 0 && doc !== null && !busy;

  const run = async (): Promise<void> => {
    if (shown === null || doc === null) {
      return;
    }
    setBusy(true);
    try {
      const params: Record<string, unknown> = {
        path,
        doc,
        useLibrary: shown.footprints.filter(replace).map((f) => f.name),
      };
      if (nickname !== null && nickname.trim() !== '') {
        params['nickname'] = nickname.trim();
      }
      const result = await engine.call<Result>('import.archive', params, 120_000);
      const library = libraries.find((l) => l.id === doc)?.name ?? '';
      notify(
        t('parts.dialog.imported', {
          symbols: result.symbols,
          footprints: result.footprints,
          models: result.models,
          library,
        }),
      );
      close();
    } catch (reason) {
      setError(errorMessage(t, reason));
      setBusy(false);
    }
  };

  const item = (entry: Item, kind: 'symbol' | 'footprint'): ReactNode => (
    <li key={entry.name} className="parts-import__item" data-testid={`import-${kind}`}>
      <div className="parts-import__name">
        <Icon name={kind === 'symbol' ? 'component' : 'pad'} size={16} />
        <strong>{entry.name}</strong>
        <span className="field__hint">
          {kind === 'symbol'
            ? t('parts.dialog.pins', { count: entry.pins })
            : t('parts.dialog.pads', { count: entry.pins })}
        </span>
        {kind === 'footprint' && (entry.models ?? []).length === 0 && (
          <span className="field__hint">· {t('parts.dialog.noModel')}</span>
        )}
      </div>
      {entry.description !== '' && <p className="field__hint">{entry.description}</p>}
      {entry.exists && <p className="field__hint">{t('parts.dialog.exists')}</p>}
      {entry.duplicate !== undefined && (
        <p className="parts-import__duplicate" data-testid={`duplicate-${entry.duplicate.class}`}>
          <Icon name="duplicate" size={14} />
          {t(`parts.dialog.duplicate.${entry.duplicate.class}`, { name: entry.duplicate.name })}
        </p>
      )}
      {kind === 'footprint' &&
        entry.duplicate !== undefined &&
        entry.duplicate.class !== 'pins' && (
          <label className="checkbox">
            <input
              type="checkbox"
              checked={replace(entry)}
              data-testid="import-use-library"
              onChange={(event) => {
                setUseLibrary({ ...useLibrary, [entry.name]: event.target.checked });
              }}
            />
            <span>{t('parts.dialog.useLibrary', { name: entry.duplicate.name })}</span>
          </label>
        )}
    </li>
  );

  return (
    <Modal
      title={t('parts.dialog.title')}
      onClose={close}
      testId="download-import"
      className="dialog--wide"
      footer={
        <>
          <button type="button" className="button" onClick={close}>
            {t('parts.dialog.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={!importable}
            data-testid="download-import-start"
            onClick={() => {
              void run();
            }}
          >
            {t('parts.dialog.import')}
          </button>
        </>
      }
    >
      <p className="dialog__text">
        <strong>{fileName(path)}</strong>
        {shown !== null &&
          ` · ${
            shown.provider === null
              ? t('parts.dialog.unknownSource')
              : t('parts.dialog.from', {
                  provider: PROVIDER_NAMES[shown.provider] ?? shown.provider,
                })
          }`}
      </p>
      {error !== null && (
        <p className="callout callout--error" role="alert">
          {error}
        </p>
      )}
      {shown === null && error === null && (
        <p className="field__hint">{t('parts.dialog.reading', { file: fileName(path) })}</p>
      )}
      {shown !== null && (
        <div
          className="parts-import"
          data-testid="download-preview"
          data-current={preview?.key === key}
        >
          <div className="parts-import__options">
            <label className="field">
              <span className="field__label">{t('parts.dialog.nickname')}</span>
              <input
                className="field__control"
                value={nickname ?? shown.nickname}
                data-testid="download-nickname"
                onChange={(event) => {
                  setNickname(event.target.value);
                }}
              />
              <span className="field__hint">
                {t('parts.dialog.nicknameHint', {
                  example: shown.symbols[0]?.name ?? `${shown.nickname}:…`,
                })}
              </span>
            </label>
            {libraries.length > 1 && (
              <label className="field">
                <span className="field__label">{t('parts.dialog.target')}</span>
                <select
                  className="field__control"
                  value={doc ?? ''}
                  onChange={(event) => {
                    setDoc(event.target.value);
                  }}
                >
                  {libraries.map((library) => (
                    <option key={library.id} value={library.id}>
                      {library.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>
          {shown.symbols.length > 0 && (
            <section>
              <h3 className="parts-import__heading">{t('parts.dialog.symbols')}</h3>
              <ul className="parts-import__list">
                {shown.symbols.map((entry) => item(entry, 'symbol'))}
              </ul>
            </section>
          )}
          {shown.footprints.length > 0 && (
            <section>
              <h3 className="parts-import__heading">{t('parts.dialog.footprints')}</h3>
              <ul className="parts-import__list">
                {shown.footprints.map((entry) => item(entry, 'footprint'))}
              </ul>
            </section>
          )}
          {shown.models.length > 0 && (
            <section>
              <h3 className="parts-import__heading">{t('parts.dialog.models')}</h3>
              <ul className="parts-import__list">
                {shown.models.map((model) => (
                  <li key={model.file} className="parts-import__item" data-testid="import-model">
                    <div className="parts-import__name">
                      <Icon name="cube" size={16} />
                      <span>{model.file}</span>
                      {!model.used && (
                        <span className="field__hint">· {t('parts.dialog.unused')}</span>
                      )}
                    </div>
                  </li>
                ))}
              </ul>
              <p className="field__hint">
                {t('parts.dialog.modelsHint', { folder: shown.modelFolder })}
              </p>
            </section>
          )}
          {shown.problems.length > 0 && (
            <section>
              <h3 className="parts-import__heading">{t('parts.dialog.problems')}</h3>
              <ul className="parts-import__list">
                {shown.problems.map((problem) => (
                  <li key={problem} className="callout callout--warning">
                    {problem}
                  </li>
                ))}
              </ul>
            </section>
          )}
          {shown.symbols.length + shown.footprints.length === 0 && (
            <p className="callout callout--info">{t('parts.dialog.nothing')}</p>
          )}
        </div>
      )}
    </Modal>
  );
}
