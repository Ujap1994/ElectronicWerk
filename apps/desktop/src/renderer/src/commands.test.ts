import { describe, expect, it } from 'vitest';

import { filterCommands, normalizeSearchText } from './commands';

const items = [
  { id: 'settings', searchText: ['Einstellungen öffnen', 'Open settings'] },
  { id: 'updates', searchText: ['Nach Updates suchen', 'Check for updates'] },
  { id: 'dark', searchText: ['Darstellung: Dunkel', 'Appearance: dark'] },
  { id: 'lang-de', searchText: ['Sprache: Deutsch', 'Language: German'] },
];

const ids = (query: string): string[] => filterCommands(items, query).map((item) => item.id);

describe('normalizeSearchText', () => {
  it('ignores case and accents', () => {
    expect(normalizeSearchText('Öffnen ÜBER')).toBe('offnen uber');
  });
});

describe('filterCommands', () => {
  it('returns everything for an empty query', () => {
    expect(ids('   ')).toEqual(['settings', 'updates', 'dark', 'lang-de']);
  });

  it('requires every word to match', () => {
    expect(ids('dark appear')).toEqual(['dark']);
    expect(ids('dark german')).toEqual([]);
  });

  it('matches German and English labels and ignores accents', () => {
    expect(ids('offnen')).toEqual(['settings']);
    expect(ids('check')).toEqual(['updates']);
  });

  it('ranks labels with a word starting with the query first', () => {
    expect(ids('d')).toEqual(['dark', 'lang-de', 'updates']);
    expect(ids('upd')).toEqual(['updates']);
  });
});
