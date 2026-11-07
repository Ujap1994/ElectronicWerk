import { describe, expect, it } from 'vitest';

import {
  compareVersions,
  evaluateRelease,
  isReleaseUrl,
  normalizeVersion,
  parseRelease,
  type ReleaseInfo,
} from './update';

describe('compareVersions', () => {
  it('orders by major, minor and patch', () => {
    expect(compareVersions('1.0.0', '0.9.9')).toBe(1);
    expect(compareVersions('0.2.0', '0.10.0')).toBe(-1);
    expect(compareVersions('0.1.1', '0.1.0')).toBe(1);
    expect(compareVersions('v1.2.3', '1.2.3')).toBe(0);
  });

  it('follows the SemVer prerelease precedence', () => {
    const ordered = [
      '1.0.0-alpha',
      '1.0.0-alpha.1',
      '1.0.0-alpha.beta',
      '1.0.0-beta',
      '1.0.0-beta.2',
      '1.0.0-beta.11',
      '1.0.0-rc.1',
      '1.0.0',
    ];
    for (let index = 1; index < ordered.length; index += 1) {
      expect(compareVersions(ordered[index - 1]!, ordered[index]!)).toBe(-1);
      expect(compareVersions(ordered[index]!, ordered[index - 1]!)).toBe(1);
    }
  });

  it('ignores build metadata', () => {
    expect(compareVersions('1.0.0+build.5', '1.0.0')).toBe(0);
  });

  it('sorts invalid versions first', () => {
    expect(compareVersions('garbage', '0.0.1')).toBe(-1);
    expect(compareVersions('0.0.1', 'garbage')).toBe(1);
    expect(compareVersions('x', 'y')).toBe(0);
  });
});

describe('normalizeVersion', () => {
  it('strips the v prefix and build metadata', () => {
    expect(normalizeVersion('v0.2.0')).toBe('0.2.0');
    expect(normalizeVersion('1.0.0-rc.1+abc')).toBe('1.0.0-rc.1');
  });

  it('rejects non-SemVer strings', () => {
    for (const text of ['1.0', '01.0.0', 'latest', '', '1.0.0.0']) {
      expect(normalizeVersion(text)).toBeNull();
    }
  });
});

const apiRelease = {
  tag_name: 'v0.2.0',
  name: 'ElectronicWerk 0.2.0',
  body: '## Added\n\n- Projects',
  html_url: 'https://github.com/Ujap1994/ElectronicWerk/releases/tag/v0.2.0',
  published_at: '2026-12-01T10:00:00Z',
  draft: false,
  prerelease: false,
};

describe('parseRelease', () => {
  it('reads a GitHub API release', () => {
    expect(parseRelease(apiRelease)).toEqual({
      version: '0.2.0',
      name: 'ElectronicWerk 0.2.0',
      notes: '## Added\n\n- Projects',
      url: 'https://github.com/Ujap1994/ElectronicWerk/releases/tag/v0.2.0',
      publishedAt: '2026-12-01T10:00:00Z',
    });
  });

  it('falls back to the tag as name and tolerates missing notes', () => {
    const release = parseRelease({ ...apiRelease, name: '', body: null, published_at: 'never' });
    expect(release).toMatchObject({ name: 'v0.2.0', notes: '', publishedAt: null });
  });

  it('rejects drafts, prereleases, foreign URLs and bad tags', () => {
    expect(parseRelease({ ...apiRelease, draft: true })).toBeNull();
    expect(parseRelease({ ...apiRelease, prerelease: true })).toBeNull();
    expect(parseRelease({ ...apiRelease, html_url: 'https://evil.example/releases' })).toBeNull();
    expect(
      parseRelease({
        ...apiRelease,
        html_url: 'https://github.com/Ujap1994/ElectronicWerkX/releases',
      }),
    ).toBeNull();
    expect(parseRelease({ ...apiRelease, tag_name: 'nightly' })).toBeNull();
    expect(parseRelease(null)).toBeNull();
    expect(parseRelease([])).toBeNull();
  });

  it('caps very long release notes', () => {
    const release = parseRelease({ ...apiRelease, body: 'x'.repeat(200_000) });
    expect(release?.notes.length).toBe(100_000);
  });
});

describe('isReleaseUrl', () => {
  it('accepts only release pages of this repository', () => {
    expect(isReleaseUrl('https://github.com/Ujap1994/ElectronicWerk/releases')).toBe(true);
    expect(isReleaseUrl('https://github.com/Ujap1994/ElectronicWerk/releases/tag/v1.0.0')).toBe(
      true,
    );
    expect(isReleaseUrl('https://github.com/Ujap1994/ElectronicWerk/issues')).toBe(false);
  });
});

describe('evaluateRelease', () => {
  const release: ReleaseInfo = {
    version: '0.2.0',
    name: '0.2.0',
    notes: '',
    url: 'https://github.com/Ujap1994/ElectronicWerk/releases/tag/v0.2.0',
    publishedAt: null,
  };

  it('reports a newer release', () => {
    expect(
      evaluateRelease({ currentVersion: '0.1.0', release, skippedVersion: null, manual: false }),
    ).toEqual({ status: 'available', currentVersion: '0.1.0', release });
  });

  it('reports up to date for the same or an older release', () => {
    for (const currentVersion of ['0.2.0', '0.3.0']) {
      expect(
        evaluateRelease({ currentVersion, release, skippedVersion: null, manual: false }),
      ).toMatchObject({ status: 'up-to-date', latestVersion: '0.2.0' });
    }
  });

  it('reports up to date when nothing is published yet', () => {
    expect(
      evaluateRelease({
        currentVersion: '0.1.0',
        release: null,
        skippedVersion: null,
        manual: true,
      }),
    ).toEqual({ status: 'up-to-date', currentVersion: '0.1.0', latestVersion: null });
  });

  it('hides a skipped version only from the automatic check', () => {
    const base = { currentVersion: '0.1.0', release, skippedVersion: '0.2.0' };
    expect(evaluateRelease({ ...base, manual: false }).status).toBe('skipped');
    expect(evaluateRelease({ ...base, manual: true }).status).toBe('available');
  });

  it('reports a release newer than the skipped one', () => {
    expect(
      evaluateRelease({
        currentVersion: '0.1.0',
        release,
        skippedVersion: '0.1.5',
        manual: false,
      }).status,
    ).toBe('available');
  });
});
