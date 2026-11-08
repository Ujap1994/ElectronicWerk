/**
 * Preview and details of the selected library entry.
 */
import { useEffect, useMemo, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';

import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import type { BoardSize, Placement } from './model-view';
import { ModelPreview } from './ModelPreview';
import { ScenePreview } from './ScenePreview';
import type { EntryDetails, GroupInfo } from './types';

function formatSize(t: ReturnType<typeof useAppState>['t'], size: readonly number[]): string {
  const [w = 0, d = 0, h = 0] = size.map((v) => Number(v.toFixed(2)));
  return h > 0 ? t('library.sizeValue3', { w, d, h }) : t('library.sizeValue', { w, d });
}

export function reasonLabel(t: ReturnType<typeof useAppState>['t'], group: GroupInfo): string {
  const key = `library.dup.reasons.${group.reason}` as MessageKey;
  const text = t(key);
  return text === key ? group.reason : text;
}

function Row({
  label,
  children,
}: {
  readonly label: string;
  readonly children: ReactNode;
}): ReactNode {
  return (
    <div className="library-fields__row">
      <dt className="library-fields__label">{label}</dt>
      <dd className="library-fields__value">{children}</dd>
    </div>
  );
}

export function LibraryDetails({
  id,
  release,
  onSelect,
}: {
  readonly id: number;
  readonly release: string | null;
  readonly onSelect: (id: number) => void;
}): ReactNode {
  const { engine, t } = useAppState();
  const { project, addFromLibrary, notify } = useProject();
  const [details, setDetails] = useState<EntryDetails | null>(null);
  const [tab, setTab] = useState<'2d' | '3d'>('2d');
  const [unit, setUnit] = useState(1);

  useEffect(() => {
    let cancelled = false;
    void engine.call<EntryDetails>('library.get', { id }).then(
      (reply) => {
        if (!cancelled) {
          setDetails(reply);
          setTab(reply.kind === 'model' ? '3d' : '2d');
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, id]);

  const footprintModel = details?.definition?.models?.find((model) => model.hidden !== true);
  const placement = useMemo<Placement | null>(() => {
    if (footprintModel === undefined) {
      return null;
    }
    const offset = footprintModel.offset ?? { x: 0, y: 0, z: 0 };
    const [rx = 0, ry = 0, rz = 0] = footprintModel.rotation ?? [];
    const [sx = 1, sy = 1, sz = 1] = footprintModel.scale ?? [];
    return {
      offset: [offset.x / 1e6, offset.y / 1e6, offset.z / 1e6],
      rotation: [rx, ry, rz],
      scale: [sx, sy, sz],
    };
  }, [footprintModel]);
  const board = useMemo<BoardSize | null>(() => {
    if (details?.kind !== 'footprint' || details.size === null) {
      return null;
    }
    return { width: details.size[0] + 1, depth: details.size[1] + 1 };
  }, [details]);

  if (details === null) {
    return <div className="library-details library-details--empty">{t('library.loading')}</div>;
  }
  const fullName = `${details.library}:${details.name}`;
  const has3d = details.kind === 'model' || details.modelId !== null;
  const units = details.kind === 'symbol' ? details.units : 1;
  const params = details.params.split('\n').filter((line) => line !== '');

  return (
    <div className="library-details" data-testid="library-details" data-name={fullName}>
      <header className="library-details__head">
        <span className={`library-kind library-kind--${details.kind}`}>
          {t(`library.kindOne.${details.kind}`)}
        </span>
        <h2 className="library-details__name" title={fullName}>
          {details.name}
        </h2>
        <span className="library-details__library">{details.library}</span>
      </header>
      {details.mergedInto !== null && (
        <p className="callout callout--info library-details__merged">
          <button
            type="button"
            className="link-button"
            onClick={() => {
              if (details.mergedInto !== null) {
                onSelect(details.mergedInto);
              }
            }}
          >
            {t('library.mergedInto', {
              name:
                details.duplicateGroups
                  .flatMap((g) => g.members)
                  .find((m) => m.id === details.mergedInto)?.name ?? '…',
            })}
          </button>
        </p>
      )}
      {details.kind === 'footprint' && (
        <div className="segmented" role="tablist">
          {(['2d', '3d'] as const).map((key) => (
            <button
              key={key}
              type="button"
              role="tab"
              aria-selected={tab === key}
              className={`segmented__item${tab === key ? ' segmented__item--active' : ''}`}
              data-testid={`library-tab-${key}`}
              disabled={key === '3d' && !has3d}
              onClick={() => {
                setTab(key);
              }}
            >
              {key === '2d' ? t('library.preview2d') : t('library.preview3d')}
            </button>
          ))}
        </div>
      )}
      {details.kind === 'symbol' && units > 1 && (
        <label className="library-details__unit">
          <select
            className="field__control"
            value={unit}
            data-testid="library-unit"
            onChange={(event) => {
              setUnit(Number(event.target.value));
            }}
          >
            {Array.from({ length: units }, (_, i) => i + 1).map((n) => (
              <option key={n} value={n}>
                {t('library.unit', { unit: n })}
              </option>
            ))}
          </select>
        </label>
      )}
      {details.kind !== 'model' && tab === '2d' && (
        <ScenePreview id={details.id} unit={unit} label={fullName} />
      )}
      {details.kind === 'model' && (
        <ModelPreview modelId={details.id} placement={null} board={null} release={release} />
      )}
      {details.kind === 'footprint' && tab === '3d' && details.modelId !== null && (
        <ModelPreview
          modelId={details.modelId}
          placement={placement}
          board={board}
          release={release}
        />
      )}
      {details.kind === 'footprint' && tab === '3d' && details.modelId === null && (
        <p className="dialog__text dialog__text--muted">{t('library.noModel')}</p>
      )}
      <div className="library-details__actions">
        {project !== null && details.kind !== 'model' && (
          <button
            type="button"
            className="button button--primary"
            data-testid="library-add"
            onClick={() => {
              void addFromLibrary([details.id], fullName);
            }}
          >
            {t('library.addToProject')}
          </button>
        )}
        <button
          type="button"
          className="button"
          data-testid="library-copy"
          onClick={() => {
            void navigator.clipboard.writeText(fullName).then(() => {
              notify(t('library.copied'));
            });
          }}
        >
          {t('library.copyName')}
        </button>
      </div>
      <dl className="library-fields">
        {details.description !== '' && (
          <Row label={t('library.fields.description')}>{details.description}</Row>
        )}
        {details.keywords !== '' && (
          <Row label={t('library.fields.keywords')}>{details.keywords}</Row>
        )}
        {details.value !== '' && <Row label={t('library.fields.value')}>{details.value}</Row>}
        {details.kind === 'symbol' &&
          details.definition?.footprint !== undefined &&
          details.definition.footprint !== null && (
            <Row label={t('library.fields.footprint')}>{details.definition.footprint}</Row>
          )}
        {details.mpn !== '' && <Row label={t('library.fields.mpn')}>{details.mpn}</Row>}
        {details.manufacturer !== '' && (
          <Row label={t('library.fields.manufacturer')}>{details.manufacturer}</Row>
        )}
        {details.kind !== 'model' && (
          <Row
            label={details.kind === 'symbol' ? t('library.fields.pins') : t('library.fields.pads')}
          >
            {details.pins}
          </Row>
        )}
        {details.kind === 'symbol' && details.units > 1 && (
          <Row label={t('library.fields.units')}>{details.units}</Row>
        )}
        {details.size !== null && (
          <Row label={t('library.fields.size')}>{formatSize(t, details.size)}</Row>
        )}
        {details.model !== null && <Row label={t('library.fields.model')}>{details.model}</Row>}
        {details.file !== null && <Row label={t('library.fields.file')}>{details.file}</Row>}
        {details.datasheet !== '' && (
          <Row label={t('library.fields.datasheet')}>
            <span className="library-fields__link" title={details.datasheet}>
              {details.datasheet}
            </span>
          </Row>
        )}
        {params.length > 0 && (
          <Row label={t('library.fields.parameters')}>
            <ul className="library-fields__list">
              {params.map((line) => (
                <li key={line}>{line}</li>
              ))}
            </ul>
          </Row>
        )}
        {details.aliases.length > 0 && (
          <Row label={t('library.fields.aliases')}>
            <ul className="library-fields__list" data-testid="library-aliases">
              {details.aliases.map((alias) => (
                <li key={alias}>{alias}</li>
              ))}
            </ul>
          </Row>
        )}
        {details.duplicateGroups.length > 0 && (
          <Row label={t('library.fields.groups')}>
            <ul className="library-fields__list" data-testid="library-groups">
              {details.duplicateGroups.map((group) => (
                <li key={group.key}>
                  <strong>{t(`library.dup.classOne.${group.class}`)}</strong> ·{' '}
                  {reasonLabel(t, group)} ·{' '}
                  {t('library.dup.members', { count: group.members.length })}
                </li>
              ))}
            </ul>
          </Row>
        )}
        <Row label={t('library.fields.source')}>
          {details.source.name} {details.source.version} · {details.source.license}
        </Row>
      </dl>
    </div>
  );
}
