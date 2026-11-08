/**
 * Review of duplicate groups: filters, the list of groups and the members
 * of the selected group side by side, with the decision buttons.
 */
import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';

import type { LibraryKind } from '@shared/library';

import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { reasonLabel } from './LibraryDetails';
import { ModelPreview } from './ModelPreview';
import { ScenePreview } from './ScenePreview';
import type { DuplicateClass, DuplicateDecision, GroupInfo, GroupPage } from './types';
import { VirtualList } from './VirtualList';

const ROW_HEIGHT = 52;
/** Members shown side by side; the rest are listed. */
const COMPARED = 4;

export interface DuplicateFilters {
  readonly kind: LibraryKind | 'all';
  readonly class: DuplicateClass | 'all';
  readonly decision: DuplicateDecision | 'all';
}

export const DEFAULT_DUPLICATE_FILTERS: DuplicateFilters = {
  kind: 'all',
  class: 'near',
  decision: 'pending',
};

export function DuplicatesView({
  filters,
  onFilters,
  release,
  version,
}: {
  readonly filters: DuplicateFilters;
  readonly onFilters: (filters: DuplicateFilters) => void;
  readonly release: string | null;
  readonly version: number;
}): ReactNode {
  const { engine, t } = useAppState();
  const { notify } = useProject();
  const [total, setTotal] = useState<number | null>(null);
  const [selected, setSelected] = useState<GroupInfo | null>(null);
  const [changes, setChanges] = useState(0);

  const params = useMemo(
    () => ({
      ...(filters.kind === 'all' ? {} : { kind: filters.kind }),
      ...(filters.class === 'all' ? {} : { class: filters.class }),
      ...(filters.decision === 'all' ? {} : { decision: filters.decision }),
    }),
    [filters],
  );
  const listKey = `${JSON.stringify(params)}#${version.toString()}#${changes.toString()}`;

  useEffect(() => {
    let cancelled = false;
    void engine.call<GroupPage>('library.groups', { ...params, offset: 0, limit: 1 }).then(
      (page) => {
        if (!cancelled) {
          setTotal(page.total);
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, params, version, changes]);

  const fetchPage = useCallback(
    async (offset: number, limit: number) =>
      (await engine.call<GroupPage>('library.groups', { ...params, offset, limit })).groups,
    [engine, params],
  );

  const decide = async (decision: DuplicateDecision, canonical?: number): Promise<void> => {
    if (selected === null) {
      return;
    }
    const group = await engine.call<GroupInfo>('library.decide', {
      key: selected.key,
      decision,
      ...(canonical === undefined ? {} : { canonical }),
    });
    setSelected(group);
    setChanges((n) => n + 1);
    notify(t('library.dup.decided', { decision: t(`library.dup.decisions.${group.decision}`) }));
  };

  const set = (patch: Partial<DuplicateFilters>): void => {
    setSelected(null);
    onFilters({ ...filters, ...patch });
  };

  return (
    <div className="library-browser library-browser--duplicates">
      <aside className="library-filters">
        <p className="dialog__text dialog__text--muted">{t('library.dup.intro')}</p>
        <label className="field">
          <span className="field__label">{t('library.filters')}</span>
          <select
            className="field__control"
            value={filters.kind}
            data-testid="duplicates-kind"
            onChange={(event) => {
              set({ kind: event.target.value as DuplicateFilters['kind'] });
            }}
          >
            {(['all', 'symbol', 'footprint', 'model'] as const).map((kind) => (
              <option key={kind} value={kind}>
                {t(`library.kinds.${kind}`)}
              </option>
            ))}
          </select>
        </label>
        <select
          className="field__control"
          value={filters.class}
          data-testid="duplicates-class"
          onChange={(event) => {
            set({ class: event.target.value as DuplicateFilters['class'] });
          }}
        >
          <option value="all">{t('library.dup.allClasses')}</option>
          {(['exact', 'near', 'alias'] as const).map((value) => (
            <option key={value} value={value}>
              {t(`library.dup.classes.${value}`)}
            </option>
          ))}
        </select>
        <select
          className="field__control"
          value={filters.decision}
          data-testid="duplicates-decision"
          onChange={(event) => {
            set({ decision: event.target.value as DuplicateFilters['decision'] });
          }}
        >
          <option value="all">{t('library.dup.allDecisions')}</option>
          {(['pending', 'merge', 'separate'] as const).map((value) => (
            <option key={value} value={value}>
              {t(`library.dup.decisions.${value}`)}
            </option>
          ))}
        </select>
      </aside>
      <section className="library-results">
        <p className="library-results__count" data-testid="duplicates-count">
          {total === null
            ? t('library.loading')
            : t(total === 1 ? 'library.dup.groupsOne' : 'library.dup.groups', { count: total })}
        </p>
        <VirtualList<GroupInfo>
          total={total ?? 0}
          rowHeight={ROW_HEIGHT}
          key={listKey}
          fetchPage={fetchPage}
          rowKey={(group) => group.key}
          selected={selected?.key ?? null}
          label={t('library.tabs.duplicates')}
          testId="duplicates-list"
          empty={<p className="dialog__text dialog__text--muted">{t('library.dup.empty')}</p>}
          onSelect={(group) => {
            setSelected(group);
          }}
          renderRow={(group) => {
            const canonical =
              group.members.find((m) => m.id === group.canonical) ?? group.members[0];
            return (
              <div className="library-row" data-name={canonical?.name ?? ''}>
                <span className={`library-kind library-kind--${group.kind}`}>
                  {t(`library.kindOne.${group.kind}`)}
                </span>
                <span className="library-row__text">
                  <span className="library-row__name">{canonical?.name ?? ''}</span>
                  <span className="library-row__description">
                    {reasonLabel(t, group)} ·{' '}
                    {t('library.dup.members', { count: group.members.length })}
                  </span>
                </span>
                <span className={`decision decision--${group.decision}`}>
                  {t(`library.dup.decisions.${group.decision}`)}
                </span>
              </div>
            );
          }}
        />
      </section>
      <section className="library-side library-side--wide">
        {selected === null ? (
          <p className="library-details library-details--empty">{t('library.dup.select')}</p>
        ) : (
          <div
            className="library-details"
            data-testid="duplicate-group"
            data-decision={selected.decision}
          >
            <header className="library-details__head">
              <span className={`decision decision--${selected.decision}`}>
                {t(`library.dup.decisions.${selected.decision}`)}
              </span>
              <h2 className="library-details__name">
                {t(`library.dup.classes.${selected.class}`)}
              </h2>
              <span className="library-details__library">{reasonLabel(t, selected)}</span>
            </header>
            <div className="library-details__actions">
              <button
                type="button"
                className="button button--primary"
                data-testid="duplicate-merge"
                disabled={selected.decision === 'merge'}
                onClick={() => {
                  void decide('merge', selected.canonical);
                }}
              >
                {t('library.dup.merge')}
              </button>
              <button
                type="button"
                className="button"
                data-testid="duplicate-separate"
                disabled={selected.decision === 'separate'}
                onClick={() => {
                  void decide('separate');
                }}
              >
                {t('library.dup.separate')}
              </button>
            </div>
            <div className="compare">
              {selected.members.slice(0, COMPARED).map((member) => (
                <figure
                  key={member.id}
                  className={`compare__item${member.id === selected.canonical ? ' compare__item--shown' : ''}`}
                  data-testid="duplicate-member"
                >
                  {selected.kind === 'model' ? (
                    <ModelPreview
                      modelId={member.id}
                      placement={null}
                      board={null}
                      release={release}
                    />
                  ) : (
                    <ScenePreview
                      id={member.id}
                      label={member.name}
                      compact
                      testId="duplicate-preview"
                    />
                  )}
                  <figcaption>
                    <span className="compare__name" title={member.name}>
                      {member.name}
                    </span>
                    {member.id === selected.canonical ? (
                      <span className="decision decision--merge">{t('library.dup.shown')}</span>
                    ) : (
                      <button
                        type="button"
                        className="link-button"
                        onClick={() => {
                          void decide(
                            selected.decision === 'merge' ? 'merge' : selected.decision,
                            member.id,
                          );
                        }}
                      >
                        {t('library.dup.makeShown')}
                      </button>
                    )}
                  </figcaption>
                </figure>
              ))}
            </div>
            {selected.members.length > COMPARED && (
              <ul className="library-fields__list">
                {selected.members.slice(COMPARED).map((member) => (
                  <li key={member.id}>{member.name}</li>
                ))}
              </ul>
            )}
          </div>
        )}
      </section>
    </div>
  );
}
