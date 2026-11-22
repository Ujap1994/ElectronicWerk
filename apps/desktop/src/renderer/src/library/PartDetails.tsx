/**
 * A part of the catalog: its symbol, the package to use (a part that names
 * its footprint has just that one; a basic part offers the common packages
 * first), the footprint in 2D and 3D, and the actions to add the part to the
 * project or place it on the open schematic.
 */
import { useEffect, useMemo, useState, useSyncExternalStore, type ReactNode } from 'react';

import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { activeSheet } from '../schematic/stores';
import { useAppState } from '../state';
import { modelPlacement, previewBoard } from './LibraryDetails';
import { ModelPreview } from './ModelPreview';
import { ScenePreview } from './ScenePreview';
import type { EntryDetails, PartPackages } from './types';

export function PartDetails({
  id,
  release,
  onPlaced,
}: {
  readonly id: number;
  readonly release: string | null;
  /** The part is being placed: the library makes way for the schematic. */
  readonly onPlaced: () => void;
}): ReactNode {
  const { engine, t } = useAppState();
  const { project, addPart } = useProject();
  const sheet = useSyncExternalStore(activeSheet.subscribe, activeSheet.get);
  const [symbol, setSymbol] = useState<EntryDetails | null>(null);
  const [packages, setPackages] = useState<PartPackages | null>(null);
  const [chosen, setChosen] = useState<number | null>(null);
  const [footprint, setFootprint] = useState<EntryDetails | null>(null);
  const [tab, setTab] = useState<'2d' | '3d'>('2d');

  useEffect(() => {
    let cancelled = false;
    void Promise.all([
      engine.call<EntryDetails>('library.get', { id }),
      engine.call<PartPackages>('library.packages', { id }),
    ]).then(
      ([details, list]) => {
        if (!cancelled) {
          setSymbol(details);
          setPackages(list);
          setChosen(list.packages[0]?.id ?? null);
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, id]);

  useEffect(() => {
    if (chosen === null || chosen === 0) {
      return undefined;
    }
    let cancelled = false;
    void engine.call<EntryDetails>('library.get', { id: chosen }).then(
      (details) => {
        if (!cancelled) {
          setFootprint(details);
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, chosen]);

  const shown = footprint !== null && footprint.id === chosen ? footprint : null;
  const placement = useMemo(() => modelPlacement(shown), [shown]);
  const board = useMemo(() => previewBoard(shown), [shown]);

  if (symbol === null || packages === null) {
    return <div className="library-details library-details--empty">{t('library.loading')}</div>;
  }
  const common = packages.packages.filter((p) => p.recommended);
  const more = packages.packages.filter((p) => !p.recommended);
  const label = (p: PartPackages['packages'][number]): string =>
    [
      p.name,
      p.mounting === 'smd'
        ? t('library.part.smd')
        : p.mounting === 'tht'
          ? t('library.part.tht')
          : '',
      p.model ? t('library.part.with3d') : '',
    ]
      .filter((part) => part !== '')
      .join(' · ');
  const footprintId = chosen !== null && chosen > 0 ? chosen : null;
  const add = async (): Promise<{ component: string; symbol: string } | null> =>
    addPart(id, packages.fixed ? null : footprintId);

  return (
    <div
      className="library-details part-details"
      data-testid="part-details"
      data-name={symbol.name}
    >
      <header className="library-details__head">
        <h2 className="library-details__name" title={`${symbol.library}:${symbol.name}`}>
          {symbol.name}
        </h2>
        <span className="library-details__library">{symbol.library}</span>
      </header>
      {symbol.description !== '' && (
        <p className="part-details__description">{symbol.description}</p>
      )}
      <div className="part-details__previews">
        <figure className="part-details__preview">
          <figcaption>{t('library.part.symbol')}</figcaption>
          <ScenePreview id={symbol.id} label={symbol.name} compact testId="part-symbol" />
        </figure>
      </div>
      <label className="field">
        <span className="field__label">{t('library.part.package')}</span>
        {packages.packages.length === 0 ? (
          <span className="field__hint" data-testid="part-no-package">
            {t('library.part.none')}
          </span>
        ) : packages.fixed && packages.packages[0] !== undefined ? (
          <span className="part-details__fixed" data-testid="part-package-fixed">
            {label(packages.packages[0])}
            <span className="field__hint">{t('library.part.fixed')}</span>
          </span>
        ) : (
          <select
            className="field__control"
            value={chosen ?? ''}
            data-testid="part-package"
            onChange={(event) => {
              setChosen(Number(event.target.value));
            }}
          >
            {common.length > 0 && (
              <optgroup label={t('library.part.common')}>
                {common.map((p) => (
                  <option key={p.id} value={p.id}>
                    {label(p)}
                  </option>
                ))}
              </optgroup>
            )}
            {more.length > 0 && (
              <optgroup label={t('library.part.more')}>
                {more.map((p) => (
                  <option key={p.id} value={p.id}>
                    {label(p)}
                  </option>
                ))}
              </optgroup>
            )}
          </select>
        )}
      </label>
      {footprintId !== null && (
        <>
          <div className="segmented" role="tablist">
            {(['2d', '3d'] as const).map((key) => (
              <button
                key={key}
                type="button"
                role="tab"
                aria-selected={tab === key}
                className={`segmented__item${tab === key ? ' segmented__item--active' : ''}`}
                data-testid={`part-tab-${key}`}
                disabled={key === '3d' && (shown?.modelId ?? null) === null}
                onClick={() => {
                  setTab(key);
                }}
              >
                {key === '2d' ? t('library.preview2d') : t('library.preview3d')}
              </button>
            ))}
          </div>
          {tab === '2d' && (
            <ScenePreview
              key={footprintId}
              id={footprintId}
              label={shown?.name ?? ''}
              testId="part-footprint"
            />
          )}
          {tab === '3d' && shown !== null && typeof shown.modelId === 'number' && (
            <ModelPreview
              key={footprintId}
              modelId={shown.modelId}
              placement={placement}
              board={board}
              release={release}
            />
          )}
        </>
      )}
      <div className="library-details__actions">
        {project !== null && (
          <button
            type="button"
            className="button button--primary"
            title={t('library.part.addTip')}
            data-testid="part-add"
            onClick={() => {
              void add();
            }}
          >
            <Icon name="plus" size={16} />
            {t('library.part.add')}
          </button>
        )}
        {project !== null && sheet !== null && (
          <button
            type="button"
            className="button"
            title={t('library.part.placeTip')}
            data-testid="part-place"
            onClick={() => {
              void add().then((part) => {
                if (part !== null) {
                  onPlaced();
                  sheet.place(part.symbol, part.component);
                }
              });
            }}
          >
            <Icon name="chip" size={16} />
            {t('library.part.place')}
          </button>
        )}
      </div>
    </div>
  );
}
