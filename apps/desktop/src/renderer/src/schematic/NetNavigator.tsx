/**
 * The net navigator: every place of a net on every sheet (labels, power
 * ports, sheet pins and component pins), grouped by sheet. A click jumps
 * to the place, also on another sheet; Previous and Next step through the
 * list. It opens with Ctrl/⌘ + double-click on a label, wire or pin.
 */
import { useEffect, useRef, useSyncExternalStore, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';

import { Icon, type IconName } from '../components/Icon';
import { useAppState } from '../state';
import { netNavigation, type NetPlace } from './stores';

const KIND_ICONS: Readonly<Record<NetPlace['kind'], IconName>> = {
  globalLabel: 'globalLabel',
  hierarchicalLabel: 'hierLabel',
  label: 'label',
  powerPort: 'power',
  powerSymbol: 'power',
  sheetPin: 'sheet',
  pin: 'chip',
};

/** The sheets in list order, each with its places (and their list index). */
function groups(
  places: readonly NetPlace[],
): { key: string; sheet: string; path: string; entries: { place: NetPlace; index: number }[] }[] {
  const out: {
    key: string;
    sheet: string;
    path: string;
    entries: { place: NetPlace; index: number }[];
  }[] = [];
  places.forEach((place, index) => {
    const key = `${place.doc}${place.path}`;
    let group = out.find((entry) => entry.key === key);
    if (group === undefined) {
      group = { key, sheet: place.sheet, path: place.path, entries: [] };
      out.push(group);
    }
    group.entries.push({ place, index });
  });
  return out;
}

export function NetNavigator({
  onShow,
}: {
  /** Shows a place (opens its sheet, centres it and selects it). */
  readonly onShow: (place: NetPlace) => void;
}): ReactNode {
  const { t } = useAppState();
  const navigation = useSyncExternalStore(netNavigation.subscribe, netNavigation.get);
  const listRef = useRef<HTMLUListElement>(null);
  const current = navigation?.current ?? 0;

  // The current place stays visible in the list.
  useEffect(() => {
    listRef.current?.querySelector('[aria-current="true"]')?.scrollIntoView({ block: 'nearest' });
  }, [current]);

  if (navigation === null) {
    return null;
  }
  const { net, places } = navigation;
  const sheets = new Set(places.map((place) => `${place.doc}${place.path}`)).size;
  const go = (index: number): void => {
    const place = places[index];
    if (place !== undefined) {
      netNavigation.set({ ...navigation, current: index });
      onShow(place);
    }
  };
  const step = (delta: number): void => {
    if (places.length > 0) {
      go((current + delta + places.length) % places.length);
    }
  };
  const close = (): void => {
    netNavigation.set(null);
  };
  return (
    <section
      className="find net-navigator"
      aria-label={t('schematic.navigator.title', { name: net })}
      data-testid="net-navigator"
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.stopPropagation();
          close();
        } else if (event.key === 'ArrowDown') {
          event.preventDefault();
          step(1);
        } else if (event.key === 'ArrowUp') {
          event.preventDefault();
          step(-1);
        }
      }}
    >
      <header className="find__header">
        <h2 className="find__title">{t('schematic.navigator.title', { name: net })}</h2>
        <button
          type="button"
          className="icon-button"
          aria-label={t('schematic.navigator.close')}
          title={t('schematic.navigator.close')}
          data-testid="net-navigator-close"
          onClick={close}
        >
          <Icon name="close" size={16} />
        </button>
      </header>
      <div className="net-navigator__bar">
        <span className="find__count" data-testid="net-navigator-count">
          {t('schematic.navigator.summary', { count: places.length, sheets })}
        </span>
        <span className="net-navigator__steps">
          <button
            type="button"
            className="button button--small"
            disabled={places.length < 2}
            title={t('schematic.navigator.previous')}
            aria-label={t('schematic.navigator.previous')}
            data-testid="net-navigator-previous"
            onClick={() => {
              step(-1);
            }}
          >
            ‹
          </button>
          <button
            type="button"
            className="button button--small"
            disabled={places.length < 2}
            title={t('schematic.navigator.next')}
            aria-label={t('schematic.navigator.next')}
            data-testid="net-navigator-next"
            onClick={() => {
              step(1);
            }}
          >
            ›
          </button>
        </span>
      </div>
      <ul className="find__hits net-navigator__list" ref={listRef} data-testid="net-navigator-list">
        {groups(places).map((group) => (
          <li key={group.key} className="net-navigator__sheet">
            <p className="net-navigator__sheet-name">
              <Icon name="schematic" size={14} />
              <span>{group.sheet}</span>
              {group.path !== '/' && <span className="find__where">{group.path}</span>}
            </p>
            <ul className="net-navigator__places">
              {group.entries.map(({ place, index }) => (
                <li key={`${place.kind}-${place.id}-${String(index)}`}>
                  <button
                    type="button"
                    className="find__hit net-navigator__place"
                    aria-current={index === current}
                    data-testid={`net-place-${String(index)}`}
                    onClick={() => {
                      go(index);
                    }}
                  >
                    <Icon name={KIND_ICONS[place.kind]} size={14} />
                    <span className="find__text">
                      {place.text}
                      {place.kind === 'pin' && place.name !== undefined && place.name !== '' && (
                        <span className="find__where"> {place.name}</span>
                      )}
                    </span>
                    <span className="find__where">
                      {t(`schematic.navigator.kinds.${place.kind}` as MessageKey)}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </li>
        ))}
      </ul>
      <p className="net-navigator__hint">{t('schematic.navigator.hint')}</p>
    </section>
  );
}
