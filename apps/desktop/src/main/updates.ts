import { readFile } from 'node:fs/promises';

import { net } from 'electron';

import {
  evaluateRelease,
  parseRelease,
  RELEASES_API_URL,
  type ReleaseInfo,
  type UpdateCheckResult,
} from '@shared/update';

export interface UpdateServiceOptions {
  readonly currentVersion: string;
  /** Reads the version the user chose to skip (from the settings). */
  readonly getSkippedVersion: () => string | null;
  /** Test hook: read the release JSON from this file instead of GitHub. */
  readonly feedFile?: string | undefined;
  readonly timeoutMs?: number;
}

/**
 * Asks GitHub for the newest release. Nothing is downloaded or installed:
 * the result only feeds the update notice in the UI.
 */
export class UpdateService {
  private pending: UpdateCheckResult | null = null;
  private readonly options: UpdateServiceOptions;

  constructor(options: UpdateServiceOptions) {
    this.options = options;
  }

  /** Result of the automatic startup check, if it found an update. */
  getPending(): UpdateCheckResult | null {
    return this.pending;
  }

  async check(manual: boolean): Promise<UpdateCheckResult> {
    const { currentVersion } = this.options;
    let result: UpdateCheckResult;
    try {
      const release = await this.loadRelease();
      result = evaluateRelease({
        currentVersion,
        release,
        skippedVersion: this.options.getSkippedVersion(),
        manual,
      });
    } catch (error) {
      result = {
        status: 'error',
        currentVersion,
        message: error instanceof Error ? error.message : String(error),
      };
    }
    if (!manual && result.status === 'available') {
      this.pending = result;
    }
    return result;
  }

  private async loadRelease(): Promise<ReleaseInfo | null> {
    const { feedFile } = this.options;
    if (feedFile !== undefined) {
      const raw: unknown = JSON.parse(await readFile(feedFile, 'utf8'));
      return raw === null ? null : requireRelease(raw);
    }
    const response = await net.fetch(RELEASES_API_URL, {
      headers: {
        Accept: 'application/vnd.github+json',
        'User-Agent': `ElectronicWerk/${this.options.currentVersion}`,
        'X-GitHub-Api-Version': '2022-11-28',
      },
      signal: AbortSignal.timeout(this.options.timeoutMs ?? 15_000),
    });
    if (response.status === 404) {
      // No release has been published yet.
      return null;
    }
    if (!response.ok) {
      throw new Error(`GitHub answered with HTTP ${String(response.status)}`);
    }
    return requireRelease(await response.json());
  }
}

function requireRelease(raw: unknown): ReleaseInfo {
  const release = parseRelease(raw);
  if (release === null) {
    throw new Error('unexpected release data from GitHub');
  }
  return release;
}
