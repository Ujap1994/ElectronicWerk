import { existsSync, mkdirSync, statSync } from 'node:fs';
import { isAbsolute, join } from 'node:path';

import { ipcMain, net, safeStorage, shell, type WebFrameMain } from 'electron';

import { IPC } from '@shared/api';
import {
  GITHUB_API,
  GITHUB_CLIENT_ID,
  GITHUB_SCOPES,
  GITHUB_WEB,
  isBranchName,
  isFullName,
  isPublishTarget,
  type GitResult,
} from '@shared/github';

import { GithubError } from './api';
import { attempt, GithubService } from './service';
import { TEST_SECRET_BOX, TokenStore } from './token-store';

export interface GithubIpcOptions {
  readonly userData: string;
  readonly appVersion: string;
  readonly testHooks: boolean;
  readonly isTrustedSender: (frame: WebFrameMain | null) => boolean;
  /** The manifest of the project open in the main window. */
  readonly openProject: () => string | null;
  readonly broadcast: (channel: string, payload: unknown) => void;
}

export interface GithubIpc {
  readonly service: GithubService;
  /** Pages opened in the browser (tests only; nothing is opened then). */
  readonly openedPages: string[];
}

function env(name: string, testHooks: boolean): string | undefined {
  const value = process.env[name];
  return testHooks && value !== undefined && value !== '' ? value : undefined;
}

/** Registers the `github:*` and `git:*` IPC handlers. */
export function registerGithubIpc(options: GithubIpcOptions): GithubIpc {
  const { testHooks, isTrustedSender } = options;
  const web = env('EW_GITHUB_WEB', testHooks) ?? GITHUB_WEB;
  const clientId = process.env['EW_GITHUB_CLIENT_ID'] ?? GITHUB_CLIENT_ID;
  const service = new GithubService({
    fetch: (url, init) => net.fetch(url, init),
    api: env('EW_GITHUB_API', testHooks) ?? GITHUB_API,
    web,
    userAgent: `ElectronicWerk/${options.appVersion}`,
    clientId,
    scopes: GITHUB_SCOPES,
    // Tests never touch the real keychain.
    tokens: new TokenStore(
      join(options.userData, 'github-token.bin'),
      testHooks ? TEST_SECRET_BOX : safeStorage,
    ),
  });
  const openedPages: string[] = [];

  // One git action at a time: they share the repository files.
  let queue: Promise<unknown> = Promise.resolve();
  const serial = <T>(action: () => Promise<T>): Promise<T> => {
    const next = queue.then(action, action);
    queue = next.catch(() => undefined);
    return next;
  };

  const changed = async (): Promise<void> => {
    options.broadcast(IPC.githubChanged, await service.status());
  };

  const notProject = <T>(): GitResult<T> => ({
    ok: false,
    error: { code: 'notProject', message: 'not the open project' },
  });

  /** A git action on the open project only. */
  const project = <T>(
    channel: string,
    action: (manifestPath: string, ...args: unknown[]) => Promise<T> | undefined,
  ): void => {
    ipcMain.handle(channel, async (event, manifestPath: unknown, ...args: unknown[]) => {
      if (!isTrustedSender(event.senderFrame)) {
        return notProject();
      }
      if (typeof manifestPath !== 'string' || manifestPath !== options.openProject()) {
        return notProject();
      }
      return serial(() =>
        attempt(async () => {
          const result = action(manifestPath, ...args);
          if (result === undefined) {
            throw new GithubError('failed', 'invalid request');
          }
          return result;
        }),
      );
    });
  };

  ipcMain.handle(IPC.githubStatus, (event) =>
    isTrustedSender(event.senderFrame) ? service.status() : null,
  );

  ipcMain.handle(IPC.githubSignInToken, async (event, token: unknown) => {
    if (!isTrustedSender(event.senderFrame) || typeof token !== 'string' || token.length > 400) {
      return notProject();
    }
    const result = await attempt(() => service.signInWithToken(token));
    if (result.ok) {
      await changed();
    }
    return result;
  });

  ipcMain.handle(IPC.githubDeviceStart, (event) =>
    isTrustedSender(event.senderFrame) ? attempt(() => service.deviceStart()) : notProject(),
  );

  ipcMain.handle(IPC.githubDeviceWait, async (event) => {
    if (!isTrustedSender(event.senderFrame)) {
      return { state: 'failed' };
    }
    const result = await service.deviceWait();
    if (result.state === 'done') {
      await changed();
    }
    return result;
  });

  ipcMain.on(IPC.githubDeviceCancel, (event) => {
    if (isTrustedSender(event.senderFrame)) {
      service.deviceCancel();
    }
  });

  ipcMain.handle(IPC.githubSignOut, async (event) => {
    if (isTrustedSender(event.senderFrame)) {
      service.signOut();
      await changed();
    }
  });

  ipcMain.handle(IPC.githubRepos, (event) =>
    isTrustedSender(event.senderFrame) ? attempt(() => service.repos()) : notProject(),
  );

  ipcMain.on(IPC.githubOpenPage, (event, page: unknown, fullName: unknown) => {
    if (!isTrustedSender(event.senderFrame)) {
      return;
    }
    let url: string | null = null;
    if (page === 'repo' && isFullName(fullName)) {
      url = `${web}/${fullName}`;
    } else if (page === 'device') {
      url = `${web}/login/device`;
    } else if (page === 'tokens') {
      url = `${web}/settings/tokens/new?scopes=${encodeURIComponent(GITHUB_SCOPES.replace(' ', ','))}&description=ElectronicWerk`;
    }
    if (url === null) {
      return;
    }
    if (testHooks) {
      openedPages.push(url);
    } else {
      void shell.openExternal(url);
    }
  });

  project(IPC.gitStatus, (manifestPath) => service.projectStatus(manifestPath));
  project(IPC.gitPublish, (manifestPath, target) =>
    isPublishTarget(target) ? service.publish(manifestPath, target) : undefined,
  );
  project(IPC.gitCommit, (manifestPath, message, paths) => {
    if (typeof message !== 'string' || message.trim() === '' || message.length > 10_000) {
      return undefined;
    }
    if (paths !== null && !isPathList(paths)) {
      return undefined;
    }
    return service.commit(manifestPath, message.trim(), paths ?? undefined);
  });
  project(IPC.gitPush, (manifestPath) => service.push(manifestPath));
  project(IPC.gitFetch, (manifestPath) => service.fetch(manifestPath));
  project(IPC.gitPull, (manifestPath) => service.pull(manifestPath));
  project(IPC.gitBranches, (manifestPath) => service.branches(manifestPath));
  project(IPC.gitCheckout, (manifestPath, branch, create) =>
    typeof branch === 'string' && isBranchName(branch) && typeof create === 'boolean'
      ? service.checkout(manifestPath, branch, create)
      : undefined,
  );
  project(IPC.gitLog, (manifestPath) => service.log(manifestPath));

  ipcMain.handle(IPC.gitClone, (event, fullName: unknown, parent: unknown) => {
    if (
      !isTrustedSender(event.senderFrame) ||
      !isFullName(fullName) ||
      typeof parent !== 'string' ||
      !isAbsolute(parent) ||
      (existsSync(parent) && !statSync(parent).isDirectory())
    ) {
      return notProject();
    }
    return serial(() =>
      attempt(() => {
        mkdirSync(parent, { recursive: true });
        return service.clone(fullName, parent);
      }),
    );
  });

  return { service, openedPages };
}

function isPathList(value: unknown): value is string[] {
  return (
    Array.isArray(value) &&
    value.length <= 100_000 &&
    value.every(
      (path) =>
        typeof path === 'string' &&
        path !== '' &&
        !path.startsWith('/') &&
        !path.split('/').includes('..'),
    )
  );
}
