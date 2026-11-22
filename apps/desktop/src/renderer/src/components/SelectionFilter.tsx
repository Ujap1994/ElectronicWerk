/**
 * The selection filter below the layers: which kinds of objects a click, a
 * box or a lasso selects. Collapsed until opened; a dot on the heading says
 * that something is filtered out.
 */
import { useState, useSyncExternalStore, type ReactNode } from 'react';

import {
  BOARD_GROUPS,
  SHEET_GROUPS,
  selectionFilter,
  type BoardFilterGroup,
  type SheetFilterGroup,
} from '../viewer/selection-filter';
import { useAppState } from '../state';
import { Icon } from './Icon';

export function SelectionFilter({ kind }: { readonly kind: 'pcb' | 'schematic' }): ReactNode {
  const { t } = useAppState();
  const filter = useSyncExternalStore(selectionFilter.subscribe, selectionFilter.get);
  const [open, setOpen] = useState(false);
  const groups: readonly (BoardFilterGroup | SheetFilterGroup)[] =
    kind === 'pcb' ? BOARD_GROUPS : SHEET_GROUPS;
  const off: ReadonlySet<string> = filter[kind];
  const set = (next: ReadonlySet<string>): void => {
    selectionFilter.set(
      kind === 'pcb'
        ? { ...filter, pcb: next as ReadonlySet<BoardFilterGroup> }
        : { ...filter, schematic: next as ReadonlySet<SheetFilterGroup> },
    );
  };
  return (
    <section className="selection-filter" data-testid="selection-filter">
      <button
        type="button"
        className="selection-filter__head"
        aria-expanded={open}
        data-testid="selection-filter-toggle"
        onClick={() => {
          setOpen(!open);
        }}
      >
        <Icon name={open ? 'chevron' : 'pointer'} size={14} />
        <span>{t('viewer.selectionFilter.title')}</span>
        {off.size > 0 && (
          <span className="selection-filter__badge" data-testid="selection-filter-active">
            {t('viewer.selectionFilter.active', { count: off.size })}
          </span>
        )}
      </button>
      {open && (
        <div className="selection-filter__body">
          {groups.map((group) => (
            <label key={group} className="checkbox">
              <input
                type="checkbox"
                checked={!off.has(group)}
                data-testid={`selection-filter-${group}`}
                onChange={(event) => {
                  const next = new Set(off);
                  if (event.target.checked) {
                    next.delete(group);
                  } else {
                    next.add(group);
                  }
                  set(next);
                }}
              />
              <span>{t(`viewer.selectionFilter.groups.${group}`)}</span>
            </label>
          ))}
          <button
            type="button"
            className="link-button"
            disabled={off.size === 0}
            data-testid="selection-filter-all"
            onClick={() => {
              set(new Set());
            }}
          >
            {t('viewer.selectionFilter.all')}
          </button>
        </div>
      )}
    </section>
  );
}
