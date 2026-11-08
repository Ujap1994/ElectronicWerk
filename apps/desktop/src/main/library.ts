/**
 * Library releases and pack downloads (ADR-0005). Library packs are assets
 * of GitHub releases tagged `library-…` in this repository; the main
 * process finds the newest one, downloads packs into the library folder
 * and checks their SHA-256 before the engine installs them.
 */
import { createHash } from 'node:crypto';
import { createWriteStream, existsSync } from 'node:fs';
import { copyFile, mkdir, readFile, rename, rm, stat } from 'node:fs/promises';
import { join } from 'node:path';

import { net } from 'electron';

import {
  newestLibraryTag,
  PACK_FILE,
  parseManifest,
  releaseFileUrl,
  type LibraryDownloadProgress,
  type LibraryRelease,
} from '@shared/library';

/** Tag of the test feed. */
const LOCAL_TAG = 'library-local';

const RELEASES_API_URL =
  'https://api.github.com/repos/Ujap1994/ElectronicWerk/releases?per_page=50';

export interface LibraryDownloaderOptions {
  /** The library folder (`<userData>/library`). */
  readonly dir: string;
  readonly appVersion: string;
  /** Test hook: a folder with `manifest.json` and the packs. */
  readonly feedDir?: string;
}

export class LibraryDownloader {
  private readonly releases = new Map<string, LibraryRelease>();

  constructor(private readonly options: LibraryDownloaderOptions) {}

  private headers(): Record<string, string> {
    return {
      'User-Agent': `ElectronicWerk/${this.options.appVersion}`,
      'X-GitHub-Api-Version': '2022-11-28',
    };
  }

  /** The newest library release, or `null` if none is published. */
  async latest(): Promise<LibraryRelease | null> {
    const { feedDir } = this.options;
    if (feedDir !== undefined) {
      // Test feed: no manifest means no release.
      return existsSync(join(feedDir, 'manifest.json')) ? this.release(LOCAL_TAG) : null;
    }
    const response = await net.fetch(RELEASES_API_URL, {
      headers: { ...this.headers(), Accept: 'application/vnd.github+json' },
      signal: AbortSignal.timeout(15_000),
    });
    if (!response.ok) {
      throw new Error(`GitHub answered with HTTP ${String(response.status)}`);
    }
    const tag = newestLibraryTag(await response.json());
    return tag === null ? null : this.release(tag);
  }

  /** A library release by tag (its manifest). */
  async release(tag: string): Promise<LibraryRelease> {
    const known = this.releases.get(tag);
    if (known !== undefined) {
      return known;
    }
    const { feedDir } = this.options;
    let manifest: unknown;
    if (feedDir !== undefined) {
      manifest = JSON.parse(await readFile(join(feedDir, 'manifest.json'), 'utf8'));
    } else {
      const file = await net.fetch(releaseFileUrl(tag, 'manifest.json'), {
        headers: this.headers(),
        signal: AbortSignal.timeout(15_000),
      });
      if (!file.ok) {
        throw new Error(`the library manifest is missing (HTTP ${String(file.status)})`);
      }
      manifest = await file.json();
    }
    const release = parseManifest(tag, manifest);
    if (release === null) {
      throw new Error('the library manifest is not valid');
    }
    this.releases.set(tag, release);
    return release;
  }

  /**
   * Downloads a pack of a release and returns its local path. The file is
   * checked against the manifest.
   */
  async download(
    tag: string,
    file: string,
    onProgress: (progress: LibraryDownloadProgress) => void,
  ): Promise<string> {
    const release = await this.release(tag);
    const pack = release.packs.find((p) => p.file === file);
    if (pack === undefined || !PACK_FILE.test(file)) {
      throw new Error(`${file} is not part of library release ${tag}`);
    }
    const folder = join(this.options.dir, 'downloads');
    await mkdir(folder, { recursive: true });
    const target = join(folder, file);
    const partial = `${target}.part`;
    const hash = createHash('sha256');
    let received = 0;
    let last = 0;
    const report = (force: boolean): void => {
      const now = Date.now();
      if (force || now - last > 150) {
        last = now;
        onProgress({ file, received, total: pack.size });
      }
    };
    const { feedDir } = this.options;
    if (feedDir !== undefined) {
      await copyFile(join(feedDir, file), partial);
      const data = await readFile(partial);
      hash.update(data);
      received = data.byteLength;
    } else {
      const response = await net.fetch(releaseFileUrl(tag, file), { headers: this.headers() });
      if (!response.ok || response.body === null) {
        throw new Error(`download of ${file} failed (HTTP ${String(response.status)})`);
      }
      const out = createWriteStream(partial);
      try {
        const reader = response.body.getReader();
        for (;;) {
          const { done, value } = await reader.read();
          if (done) {
            break;
          }
          hash.update(value);
          received += value.byteLength;
          if (!out.write(value)) {
            await new Promise<void>((resolve) => {
              out.once('drain', () => {
                resolve();
              });
            });
          }
          report(false);
        }
      } finally {
        await new Promise<void>((resolve) => {
          out.end(() => {
            resolve();
          });
        });
      }
    }
    report(true);
    const size = (await stat(partial)).size;
    if (size !== pack.size || hash.digest('hex') !== pack.sha256) {
      await rm(partial, { force: true });
      throw new Error(`${file} is damaged (size or checksum does not match)`);
    }
    await rename(partial, target);
    return target;
  }
}
