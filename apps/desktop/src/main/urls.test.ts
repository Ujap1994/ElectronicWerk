import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { APP_ORIGIN, isAllowedExternalUrl, originOf, resolveAppPath } from './urls';

describe('originOf', () => {
  it('handles the custom app scheme', () => {
    expect(originOf('app://bundle/index.html')).toBe(APP_ORIGIN);
  });

  it('keeps ports of http origins', () => {
    expect(originOf('http://localhost:5173/src/main.tsx')).toBe('http://localhost:5173');
  });

  it('returns null for invalid or host-less URLs', () => {
    expect(originOf('not a url')).toBeNull();
    expect(originOf('about:blank')).toBeNull();
  });
});

describe('resolveAppPath', () => {
  const root = resolve('/srv/renderer');

  it('maps the root to index.html', () => {
    expect(resolveAppPath(root, '/')).toBe(join(root, 'index.html'));
  });

  it('maps nested assets', () => {
    expect(resolveAppPath(root, '/assets/index.js')).toBe(join(root, 'assets', 'index.js'));
  });

  it.each(['/../secret', '/assets/../../secret', '/%2e%2e/secret', '/%2e%2e%2fsecret', '/a%00b'])(
    'blocks %s',
    (path) => {
      expect(resolveAppPath(root, path)).toBeNull();
    },
  );

  it('rejects malformed escapes', () => {
    expect(resolveAppPath(root, '/%E0%A4%A')).toBeNull();
  });
});

describe('isAllowedExternalUrl', () => {
  it('allows the project repository and documentation', () => {
    expect(isAllowedExternalUrl('https://ujap1994.github.io/ElectronicWerk/')).toBe(true);
    expect(isAllowedExternalUrl('https://ujap1994.github.io/ElectronicWerk/guide/updates')).toBe(
      true,
    );
    expect(isAllowedExternalUrl('https://github.com/Ujap1994/ElectronicWerk')).toBe(true);
    expect(isAllowedExternalUrl('https://github.com/Ujap1994/ElectronicWerk/issues')).toBe(true);
  });

  it('blocks look-alikes and other sites', () => {
    expect(isAllowedExternalUrl('https://github.com/Ujap1994/ElectronicWerkEvil')).toBe(false);
    expect(isAllowedExternalUrl('http://github.com/Ujap1994/ElectronicWerk')).toBe(false);
    expect(isAllowedExternalUrl('https://example.com')).toBe(false);
    expect(isAllowedExternalUrl('https://ujap1994.github.io/Other')).toBe(false);
    expect(isAllowedExternalUrl('https://evil.github.io/ElectronicWerk/')).toBe(false);
    expect(isAllowedExternalUrl('file:///etc/passwd')).toBe(false);
  });
});
