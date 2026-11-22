/**
 * Watches the user's downloads folder for a while after a provider website
 * was opened in the user's own browser. A part downloaded there (a ZIP file,
 * a KiCad library or a STEP model) is offered for import as if it came from
 * the provider window — the user's browser passes the websites' bot checks,
 * which an embedded browser often does not.
 *
 * Only files that appear after the website was opened count, and only
 * while the watch runs (30 minutes after the last opening).
 */
import { watch, type FSWatcher } from 'node:fs';
import { stat } from 'node:fs/promises';
import { join } from 'node:path';

import { isImportableDownload, type ProviderDownload, type ProviderId } from '@shared/providers';

const WATCH_MINUTES = 30;
/** A file is complete when its size stays the same this long. */
const SETTLE_MS = 400;
const SETTLE_TRIES = 50;

export class DownloadWatch {
  private watcher: FSWatcher | null = null;
  private provider: ProviderId = 'snapeda';
  private since = 0;
  private stopTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly reported = new Set<string>();
  private readonly checking = new Set<string>();

  constructor(
    private readonly folder: () => string,
    private readonly onFile: (download: ProviderDownload) => void,
  ) {}

  /** Starts (or extends) the watch for downloads from a provider. */
  start(provider: ProviderId): void {
    this.provider = provider;
    if (this.watcher === null) {
      // A little slack: the clock of the file system is coarse.
      this.since = Date.now() - 2000;
      try {
        this.watcher = watch(this.folder(), (_event, name) => {
          if (name !== null && name !== '') {
            this.check(name);
          }
        });
        this.watcher.on('error', () => {
          this.stop();
        });
      } catch {
        // No downloads folder: the user imports with "Import a download…".
        this.watcher = null;
        return;
      }
    }
    if (this.stopTimer !== null) {
      clearTimeout(this.stopTimer);
    }
    this.stopTimer = setTimeout(() => {
      this.stop();
    }, WATCH_MINUTES * 60_000);
    this.stopTimer.unref();
  }

  stop(): void {
    this.watcher?.close();
    this.watcher = null;
    if (this.stopTimer !== null) {
      clearTimeout(this.stopTimer);
      this.stopTimer = null;
    }
  }

  get watching(): boolean {
    return this.watcher !== null;
  }

  private check(name: string): void {
    if (!isImportableDownload(name) || this.reported.has(name) || this.checking.has(name)) {
      return;
    }
    this.checking.add(name);
    void this.settle(join(this.folder(), name), name).finally(() => {
      this.checking.delete(name);
    });
  }

  /** Waits until the browser has finished writing the file. */
  private async settle(path: string, name: string): Promise<void> {
    let last = -1;
    for (let attempt = 0; attempt < SETTLE_TRIES; attempt += 1) {
      await new Promise((done) => setTimeout(done, SETTLE_MS));
      let info;
      try {
        info = await stat(path);
      } catch {
        // Renamed again or removed (a browser's temporary name).
        return;
      }
      if (!info.isFile() || Math.max(info.mtimeMs, info.birthtimeMs) < this.since) {
        return;
      }
      // Firefox creates the file empty first and fills it at the end.
      if (info.size > 0 && info.size === last) {
        this.reported.add(name);
        this.onFile({ provider: this.provider, path, name, state: 'completed', importable: true });
        return;
      }
      last = info.size;
    }
  }
}
