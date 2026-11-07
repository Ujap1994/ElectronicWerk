import { isAbsolute, relative, resolve } from 'node:path';

/** Custom scheme the production UI is served from (instead of `file://`). */
export const APP_SCHEME = 'app';
export const APP_HOST = 'bundle';
export const APP_ORIGIN = `${APP_SCHEME}://${APP_HOST}`;

/**
 * Returns `scheme://host[:port]` for a URL. Node's `URL#origin` is `"null"`
 * for custom schemes such as `app:`, so it cannot be used here.
 */
export function originOf(url: string): string | null {
  try {
    const parsed = new URL(url);
    return parsed.host === '' ? null : `${parsed.protocol}//${parsed.host}`;
  } catch {
    return null;
  }
}

/** Links that may be opened in the user's browser. */
const EXTERNAL_ALLOWLIST = [/^https:\/\/github\.com\/Ujap1994\/ElectronicWerk(?:[/?#]|$)/u];

export function isAllowedExternalUrl(url: string): boolean {
  return EXTERNAL_ALLOWLIST.some((pattern) => pattern.test(url));
}

/**
 * Maps a request path to a file inside `root`. Returns `null` for anything
 * that would leave the root directory.
 */
export function resolveAppPath(root: string, pathname: string): string | null {
  let decoded: string;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    return null;
  }
  if (decoded.includes('\0')) {
    return null;
  }
  const relativePath =
    decoded === '/' || decoded === '' ? 'index.html' : decoded.replace(/^\/+/u, '');
  const target = resolve(root, relativePath);
  const rel = relative(root, target);
  if (rel === '' || rel.startsWith('..') || isAbsolute(rel)) {
    return null;
  }
  return target;
}
