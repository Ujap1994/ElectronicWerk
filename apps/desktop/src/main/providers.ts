/**
 * Provider windows: the website of an online part library (SnapEDA, Ultra
 * Librarian) in a window of its own, as in a browser. Each provider gets
 * its own persistent browser profile (`persist:provider-<id>`), so the
 * sign-in stays there and never reaches the app: the window has no preload
 * script, runs sandboxed and cannot talk to ElectronicWerk. Files the user
 * downloads are saved into the app's download folder and reported to the
 * main window, which offers to import them.
 */
import { mkdirSync } from 'node:fs';
import { basename, extname, join } from 'node:path';

import { app, BrowserWindow, session, type Session, type WebContents } from 'electron';

import {
  isImportableDownload,
  providerById,
  providerUrl,
  type ProviderDownload,
  type ProviderId,
  type ProviderPage,
  type ProviderStatus,
  PROVIDERS,
} from '@shared/providers';

import { paths } from './paths';

/** Web contents of provider windows (the app's navigation rules skip them). */
export const providerContents = new WeakSet<WebContents>();

export interface ProviderOptions {
  /** Folder for downloads (one subfolder per provider). */
  readonly downloadsDir: string;
  /** A download finished. */
  readonly onDownload: (download: ProviderDownload) => void;
  /** Tests: a local website instead of the providers' (only with test hooks). */
  readonly base?: string;
  readonly isPackaged: boolean;
}

/** A file name that does not overwrite an earlier download. */
function freeName(dir: string, name: string, taken: (path: string) => boolean): string {
  const safe = basename(name).replace(/[<>:"/\\|?*\p{Cc}]/gu, '_') || 'download';
  const extension = extname(safe);
  const stem = safe.slice(0, safe.length - extension.length);
  let candidate = join(dir, safe);
  for (let n = 2; taken(candidate); n += 1) {
    candidate = join(dir, `${stem} (${String(n)})${extension}`);
  }
  return candidate;
}

export class ProviderWindows {
  private readonly windows = new Map<ProviderId, BrowserWindow>();
  private readonly prepared = new Set<ProviderId>();
  private readonly saved = new Set<string>();

  constructor(private readonly options: ProviderOptions) {}

  private partition(id: ProviderId): string {
    return `persist:provider-${id}`;
  }

  private session(id: ProviderId): Session {
    return session.fromPartition(this.partition(id));
  }

  /** Permissions, downloads and the user agent of a provider's profile. */
  private prepare(id: ProviderId): void {
    if (this.prepared.has(id)) {
      return;
    }
    this.prepared.add(id);
    const ses = this.session(id);
    ses.setPermissionRequestHandler((_contents, _permission, callback) => {
      callback(false);
    });
    ses.setPermissionCheckHandler(() => false);
    // Websites see a regular Chromium browser.
    ses.setUserAgent(
      app.userAgentFallback.replace(/ Electron\/\S+/u, '').replace(/ ElectronicWerk\/\S+/u, ''),
    );
    ses.on('will-download', (_event, item) => {
      const dir = join(this.options.downloadsDir, id);
      mkdirSync(dir, { recursive: true });
      const target = freeName(dir, item.getFilename(), (path) => this.saved.has(path));
      this.saved.add(target);
      item.setSavePath(target);
      item.once('done', (_done, state) => {
        const name = basename(target);
        this.options.onDownload({
          provider: id,
          path: target,
          name,
          state: state === 'completed' ? 'completed' : 'failed',
          importable: state === 'completed' && isImportableDownload(name),
        });
      });
    });
  }

  private webPreferences(id: ProviderId): Electron.WebPreferences {
    return {
      partition: this.partition(id),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      nodeIntegrationInWorker: false,
      webSecurity: true,
      allowRunningInsecureContent: false,
      spellcheck: false,
      devTools: !this.options.isPackaged,
    };
  }

  /** Rules for a provider window and the pop-ups it opens (sign-in). */
  private guard(id: ProviderId, contents: WebContents): void {
    providerContents.add(contents);
    contents.on('will-navigate', (event, url) => {
      if (!this.allowed(url)) {
        event.preventDefault();
      }
    });
    contents.setWindowOpenHandler(({ url }) => {
      if (!this.allowed(url)) {
        return { action: 'deny' };
      }
      return {
        action: 'allow',
        overrideBrowserWindowOptions: {
          width: 600,
          height: 720,
          ...(process.platform === 'darwin' ? {} : { icon: paths.windowIcon }),
          webPreferences: this.webPreferences(id),
        },
      };
    });
    contents.on('did-create-window', (child) => {
      this.guard(id, child.webContents);
    });
  }

  /** Pages may only be https (or the local test website). */
  allowed(url: string): boolean {
    if (this.options.base !== undefined && url.startsWith(this.options.base)) {
      return true;
    }
    try {
      return new URL(url).protocol === 'https:';
    } catch {
      return false;
    }
  }

  /** Opens (or reuses) the window of a provider at a page. */
  open(id: ProviderId, page: ProviderPage, query = ''): void {
    this.prepare(id);
    const url = providerUrl(id, page, query, this.options.base);
    const existing = this.windows.get(id);
    if (existing !== undefined && !existing.isDestroyed()) {
      void existing.loadURL(url);
      if (existing.isMinimized()) {
        existing.restore();
      }
      existing.focus();
      return;
    }
    const window = new BrowserWindow({
      width: 1200,
      height: 860,
      minWidth: 640,
      minHeight: 480,
      title: `${providerById(id).name} – ElectronicWerk`,
      ...(process.platform === 'darwin' ? {} : { icon: paths.windowIcon }),
      webPreferences: this.webPreferences(id),
    });
    this.guard(id, window.webContents);
    this.windows.set(id, window);
    window.on('closed', () => {
      if (this.windows.get(id) === window) {
        this.windows.delete(id);
      }
    });
    void window.loadURL(url);
  }

  /** Signs out: deletes the provider's cookies and stored data. */
  async signOut(id: ProviderId): Promise<void> {
    const window = this.windows.get(id);
    if (window !== undefined && !window.isDestroyed()) {
      window.close();
    }
    await this.session(id).clearStorageData();
    await this.session(id).clearCache();
  }

  /** Whether each provider's profile holds a session. */
  async status(): Promise<ProviderStatus[]> {
    return Promise.all(
      PROVIDERS.map(async (provider) => {
        const cookies = await this.session(provider.id).cookies.get({});
        return { id: provider.id, session: cookies.length > 0 };
      }),
    );
  }

  closeAll(): void {
    for (const window of this.windows.values()) {
      if (!window.isDestroyed()) {
        window.close();
      }
    }
    this.windows.clear();
  }

  /** Test hook: the open provider windows. */
  openIds(): ProviderId[] {
    return [...this.windows.keys()];
  }
}
