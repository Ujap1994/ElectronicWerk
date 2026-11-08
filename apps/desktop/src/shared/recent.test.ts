import { describe, expect, it } from 'vitest';

import { addRecent, EMPTY_RECENT, MAX_RECENT, removeRecent, sanitizeRecent } from './recent';

const entry = (index: number) => ({
  manifestPath: `/p/${String(index)}/P.ewproj`,
  name: `P${String(index)}`,
  openedAt: '2026-11-08T10:00:00.000Z',
});

describe('recent projects', () => {
  it('sanitizes untrusted input', () => {
    expect(sanitizeRecent(null)).toEqual(EMPTY_RECENT);
    const state = sanitizeRecent({
      projects: [entry(1), { manifestPath: '' }, 42, { ...entry(1), name: 'dupe' }, entry(2)],
      lastLocation: '/p',
    });
    expect(state.projects.map((p) => p.name)).toEqual(['P1', 'P2']);
    expect(state.lastLocation).toBe('/p');
    expect(
      sanitizeRecent({ projects: [{ manifestPath: '/a', openedAt: 'never' }] }).projects[0],
    ).toMatchObject({ name: '', openedAt: '1970-01-01T00:00:00.000Z' });
  });

  it('keeps the newest entries on top without duplicates', () => {
    let state = EMPTY_RECENT;
    for (let index = 0; index < MAX_RECENT + 3; index += 1) {
      state = addRecent(state, entry(index));
    }
    expect(state.projects).toHaveLength(MAX_RECENT);
    expect(state.projects[0]?.name).toBe(`P${String(MAX_RECENT + 2)}`);
    state = addRecent(state, { ...entry(5), manifestPath: '/P/5/p.EWPROJ' });
    expect(state.projects[0]?.manifestPath).toBe('/P/5/p.EWPROJ');
    expect(state.projects.filter((p) => p.name === 'P5')).toHaveLength(1);
    state = removeRecent(state, '/p/5/P.ewproj');
    expect(state.projects.some((p) => p.name === 'P5')).toBe(false);
  });
});
