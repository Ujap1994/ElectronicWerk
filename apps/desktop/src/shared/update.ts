/**
 * Update check logic (requirement T7). ElectronicWerk never installs updates
 * by itself: it only tells the user about a newer release, shows the release
 * notes and links to the download page.
 */

/** Public GitHub API endpoint for the newest (non-prerelease) release. */
export const RELEASES_API_URL =
  'https://api.github.com/repos/Ujap1994/ElectronicWerk/releases/latest';

/** Only release pages of this repository may be opened from the dialog. */
const RELEASE_URL_PATTERN = /^https:\/\/github\.com\/Ujap1994\/ElectronicWerk\/releases(?:\/|$)/u;

export interface ReleaseInfo {
  /** Normalized version without a leading `v`, for example `0.2.0`. */
  readonly version: string;
  readonly name: string;
  /** Release notes as Markdown. */
  readonly notes: string;
  /** Release page on GitHub. */
  readonly url: string;
  /** ISO 8601 timestamp, or `null` if unknown. */
  readonly publishedAt: string | null;
}

export type UpdateCheckResult =
  | { readonly status: 'available'; readonly currentVersion: string; readonly release: ReleaseInfo }
  | {
      readonly status: 'up-to-date';
      readonly currentVersion: string;
      /** `null` when no release has been published yet. */
      readonly latestVersion: string | null;
    }
  | { readonly status: 'skipped'; readonly currentVersion: string; readonly release: ReleaseInfo }
  | { readonly status: 'error'; readonly currentVersion: string; readonly message: string };

interface ParsedVersion {
  readonly core: readonly [number, number, number];
  readonly prerelease: readonly string[];
}

const VERSION_PATTERN =
  /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z.-]+)?$/u;

/** Parses a SemVer string (an optional leading `v` is accepted). */
export function parseVersion(text: string): ParsedVersion | null {
  const match = VERSION_PATTERN.exec(text.trim());
  if (match === null) {
    return null;
  }
  const [, major, minor, patch, pre] = match;
  return {
    core: [Number(major), Number(minor), Number(patch)],
    prerelease: pre === undefined ? [] : pre.split('.'),
  };
}

/** Returns the version without a leading `v`, or `null` if it is not SemVer. */
export function normalizeVersion(text: string): string | null {
  return parseVersion(text) === null ? null : text.trim().replace(/^v/u, '').replace(/\+.*$/u, '');
}

function comparePrereleaseIdentifiers(a: string, b: string): number {
  const aNumeric = /^\d+$/u.test(a);
  const bNumeric = /^\d+$/u.test(b);
  if (aNumeric && bNumeric) {
    return Math.sign(Number(a) - Number(b));
  }
  if (aNumeric !== bNumeric) {
    return aNumeric ? -1 : 1;
  }
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * Compares two SemVer strings following the SemVer 2.0 precedence rules.
 * Returns a negative number if `a < b`, 0 if equal, positive if `a > b`.
 * Invalid versions sort before every valid version.
 */
export function compareVersions(a: string, b: string): number {
  const left = parseVersion(a);
  const right = parseVersion(b);
  if (left === null || right === null) {
    return left === right ? 0 : left === null ? -1 : 1;
  }
  for (let index = 0; index < 3; index += 1) {
    const diff = (left.core[index] ?? 0) - (right.core[index] ?? 0);
    if (diff !== 0) {
      return Math.sign(diff);
    }
  }
  if (left.prerelease.length === 0 || right.prerelease.length === 0) {
    // A release has higher precedence than any of its prereleases.
    return Math.sign(right.prerelease.length - left.prerelease.length);
  }
  const length = Math.max(left.prerelease.length, right.prerelease.length);
  for (let index = 0; index < length; index += 1) {
    const l = left.prerelease[index];
    const r = right.prerelease[index];
    if (l === undefined || r === undefined) {
      return l === undefined ? -1 : 1;
    }
    const diff = comparePrereleaseIdentifiers(l, r);
    if (diff !== 0) {
      return diff;
    }
  }
  return 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Maximum length of release notes accepted from the network. */
const MAX_NOTES_LENGTH = 100_000;

/**
 * Validates a GitHub "release" API object. Returns `null` for anything that
 * is not a usable, published, non-prerelease release of this repository.
 */
export function parseRelease(raw: unknown): ReleaseInfo | null {
  if (!isRecord(raw) || raw['draft'] === true || raw['prerelease'] === true) {
    return null;
  }
  const tag = raw['tag_name'];
  const url = raw['html_url'];
  if (typeof tag !== 'string' || typeof url !== 'string' || !RELEASE_URL_PATTERN.test(url)) {
    return null;
  }
  const version = normalizeVersion(tag);
  if (version === null) {
    return null;
  }
  const name = typeof raw['name'] === 'string' && raw['name'].trim() !== '' ? raw['name'] : tag;
  const body = typeof raw['body'] === 'string' ? raw['body'] : '';
  const published = raw['published_at'];
  return {
    version,
    name: name.slice(0, 200),
    notes: body.slice(0, MAX_NOTES_LENGTH),
    url,
    publishedAt:
      typeof published === 'string' && !Number.isNaN(Date.parse(published)) ? published : null,
  };
}

/** `true` if `url` is a release page of this repository. */
export function isReleaseUrl(url: string): boolean {
  return RELEASE_URL_PATTERN.test(url);
}

/**
 * Decides what to tell the user about a release.
 *
 * A skipped version is only hidden from the automatic startup check; a manual
 * check always reports it, so users can change their mind.
 */
export function evaluateRelease(options: {
  readonly currentVersion: string;
  readonly release: ReleaseInfo | null;
  readonly skippedVersion: string | null;
  readonly manual: boolean;
}): UpdateCheckResult {
  const { currentVersion, release, skippedVersion, manual } = options;
  if (release === null || compareVersions(release.version, currentVersion) <= 0) {
    return { status: 'up-to-date', currentVersion, latestVersion: release?.version ?? null };
  }
  if (
    !manual &&
    skippedVersion !== null &&
    compareVersions(release.version, skippedVersion) === 0
  ) {
    return { status: 'skipped', currentVersion, release };
  }
  return { status: 'available', currentVersion, release };
}
