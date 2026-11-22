import { describe, expect, it } from 'vitest';

import { applySettingsPatch, DEFAULT_SETTINGS, sanitizeSettings } from './settings';

describe('sanitizeSettings', () => {
  it('returns defaults for garbage', () => {
    for (const input of [null, undefined, 42, 'x', [], {}]) {
      expect(sanitizeSettings(input)).toEqual(DEFAULT_SETTINGS);
    }
  });

  it('keeps valid values and drops invalid ones', () => {
    expect(
      sanitizeSettings({
        language: 'de',
        theme: 'neon',
        workMode: 'advanced',
        updates: { checkOnStartup: false, skippedVersion: '0.2.0' },
        unknown: true,
      }),
    ).toEqual({
      schemaVersion: 1,
      language: 'de',
      theme: 'system',
      workMode: 'advanced',
      wheelZoom: 'normal',
      shortcuts: {},
      updates: { checkOnStartup: false, skippedVersion: '0.2.0' },
    });
  });

  it('rejects unsupported languages and oversized versions', () => {
    const result = sanitizeSettings({
      language: 'fr',
      updates: { skippedVersion: 'x'.repeat(65) },
    });
    expect(result.language).toBe('system');
    expect(result.updates.skippedVersion).toBeNull();
  });

  it('falls back to the beginner work mode', () => {
    expect(sanitizeSettings({ workMode: 'expert' }).workMode).toBe('beginner');
    expect(sanitizeSettings({}).workMode).toBe('beginner');
  });

  it('keeps a known wheel zoom step', () => {
    expect(sanitizeSettings({ wheelZoom: 'fine' }).wheelZoom).toBe('fine');
    expect(sanitizeSettings({ wheelZoom: 'huge' }).wheelZoom).toBe('normal');
  });
});

describe('applySettingsPatch', () => {
  it('merges nested update settings', () => {
    const current = sanitizeSettings({ updates: { skippedVersion: '0.3.0' } });
    const next = applySettingsPatch(current, { theme: 'dark', updates: { checkOnStartup: false } });
    expect(next.theme).toBe('dark');
    expect(next.updates).toEqual({ checkOnStartup: false, skippedVersion: '0.3.0' });
  });

  it('ignores invalid patches', () => {
    expect(applySettingsPatch(DEFAULT_SETTINGS, 'nope')).toEqual(DEFAULT_SETTINGS);
    expect(applySettingsPatch(DEFAULT_SETTINGS, { language: 42 })).toEqual(DEFAULT_SETTINGS);
  });
});
