/**
 * A local stand-in for GitHub in tests: the REST API (`/api`), the device
 * flow of a GitHub App with expiring tokens and their refresh, the app's
 * installation (the repositories it may use) and git over HTTP (the system's
 * `git http-backend`). Tests never contact github.com.
 */
import { spawn, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export interface FakeAccount {
  readonly id: number;
  readonly login: string;
  readonly name: string;
}

export interface FakeRepo {
  readonly owner: string;
  readonly name: string;
  readonly private: boolean;
  readonly description: string;
}

export interface FakeGithub {
  /** Base URL, e.g. `http://127.0.0.1:1234` (the website and git). */
  readonly url: string;
  /** REST API base (`${url}/api`). */
  readonly api: string;
  /** Folder with the bare repositories (`<owner>/<name>.git`). */
  readonly root: string;
  /** A personal access token (all repositories, never expires). */
  readonly token: string;
  readonly account: FakeAccount;
  /**
   * Installs the GitHub App for the account: on all repositories or on the
   * named ones (`owner/name`); `null` uninstalls it.
   */
  installApp: (repos: 'all' | readonly string[] | null) => void;
  /** How often a token of the app was renewed. */
  refreshes: () => number;
  /** The app tokens issued so far (the newest last). */
  appTokens: () => readonly string[];
  /** Adds a repository (empty, or a bare copy of a local repository). */
  addRepo: (repo: FakeRepo, from?: string) => void;
  /** The repositories by `owner/name`. */
  repos: () => readonly FakeRepo[];
  /** Lets the device flow finish (as if the user entered the code). */
  approveDevice: () => void;
  /** The device flow ends with "access denied". */
  denyDevice: () => void;
  /** Runs git in a bare repository (for checks). */
  git: (fullName: string, args: readonly string[]) => string;
  close: () => Promise<void>;
}

export interface FakeGithubOptions {
  readonly token?: string;
  readonly account?: FakeAccount;
  /** Seconds an app token is valid (GitHub: 8 hours). */
  readonly appTokenLifetime?: number;
}

const DEVICE_CODE = 'device-code-1';
export const FAKE_USER_CODE = 'WDJB-MJHT';

function runGit(cwd: string, args: readonly string[]): string {
  const result = spawnSync('git', args, { cwd, encoding: 'utf8' });
  if (result.status !== 0) {
    throw new Error(`git ${args.join(' ')} failed: ${result.stderr}`);
  }
  return result.stdout;
}

export async function startFakeGithub(options: FakeGithubOptions = {}): Promise<FakeGithub> {
  const token = options.token ?? 'ghp_test_token_1234567890';
  const account = options.account ?? { id: 4242, login: 'octo-maker', name: 'Octo Maker' };
  const lifetime = options.appTokenLifetime ?? 28_800;
  const root = mkdtempSync(join(tmpdir(), 'ew-fake-github-'));
  const repos = new Map<string, FakeRepo & { created: string }>();
  let device: 'pending' | 'approved' | 'denied' | 'used' = 'pending';
  let url = '';
  // The GitHub App: valid tokens (with their expiry) and refresh tokens.
  const appTokens = new Map<string, number>();
  /** Refresh token → the app token it replaces. */
  const refreshTokens = new Map<string, string>();
  const issued: string[] = [];
  let refreshes = 0;
  let installation: 'all' | Set<string> | null = null;

  const issueAppToken = (): Record<string, unknown> => {
    const number = String(issued.length + 1).padStart(4, '0');
    const access = `ghu_app_token_${number}`;
    const refresh = `ghr_refresh_token_${number}`;
    appTokens.set(access, Date.now() + lifetime * 1000);
    refreshTokens.set(refresh, access);
    issued.push(access);
    return {
      access_token: access,
      expires_in: lifetime,
      refresh_token: refresh,
      refresh_token_expires_in: 15_897_600,
      token_type: 'bearer',
      scope: '',
    };
  };

  /** The app may use a repository (a personal token may use all). */
  const appMayUse = (fullName: string): boolean =>
    installation === 'all' || installation?.has(fullName) === true;

  const bareDir = (fullName: string): string => join(root, `${fullName}.git`);

  const addRepo = (repo: FakeRepo, from?: string): void => {
    const fullName = `${repo.owner}/${repo.name}`;
    const dir = bareDir(fullName);
    mkdirSync(join(root, repo.owner), { recursive: true });
    if (from === undefined) {
      runGit(root, ['init', '--bare', '--initial-branch=main', dir]);
    } else {
      runGit(root, ['clone', '--bare', from, dir]);
    }
    runGit(dir, ['config', 'http.receivepack', 'true']);
    repos.set(fullName, {
      ...repo,
      created: new Date(Date.UTC(2026, 10, repos.size + 1)).toISOString(),
    });
  };

  const isEmpty = (fullName: string): boolean =>
    runGit(bareDir(fullName), ['for-each-ref', '--count=1', 'refs/heads']).trim() === '';

  const repoJson = (fullName: string): Record<string, unknown> => {
    const repo = repos.get(fullName);
    if (repo === undefined) {
      throw new Error(`no repo ${fullName}`);
    }
    let branch = 'main';
    if (!isEmpty(fullName)) {
      branch = runGit(bareDir(fullName), ['symbolic-ref', '--short', 'HEAD']).trim();
    }
    return {
      id: [...repos.keys()].indexOf(fullName) + 1,
      name: repo.name,
      full_name: fullName,
      owner: { login: repo.owner },
      private: repo.private,
      description: repo.description === '' ? null : repo.description,
      default_branch: branch,
      clone_url: `${url}/${fullName}.git`,
      html_url: `${url}/${fullName}`,
      size: isEmpty(fullName) ? 0 : 12,
      pushed_at: repo.created,
      updated_at: repo.created,
      permissions: { admin: true, push: true, pull: true },
    };
  };

  /** Who sends the request: a personal token, the app, or nobody. */
  const authorized = (request: IncomingMessage): 'token' | 'app' | null => {
    const header = request.headers.authorization ?? '';
    let sent: string | null = null;
    if (header.startsWith('Bearer ') || header.startsWith('token ')) {
      sent = header.slice(header.indexOf(' ') + 1);
    } else if (header.startsWith('Basic ')) {
      const [, password] = Buffer.from(header.slice(6), 'base64').toString('utf8').split(':');
      sent = password ?? null;
    }
    if (sent === token) {
      return 'token';
    }
    const expires = sent === null ? undefined : appTokens.get(sent);
    return expires !== undefined && expires > Date.now() ? 'app' : null;
  };

  const mayUse = (who: 'token' | 'app', fullName: string): boolean =>
    who === 'token' || appMayUse(fullName);

  const json = (response: ServerResponse, status: number, body: unknown): void => {
    response.writeHead(status, { 'Content-Type': 'application/json' });
    response.end(JSON.stringify(body));
  };

  const readBody = async (request: IncomingMessage): Promise<Buffer> => {
    const parts: Buffer[] = [];
    for await (const part of request) {
      parts.push(part as Buffer);
    }
    return Buffer.concat(parts);
  };

  const api = async (request: IncomingMessage, response: ServerResponse, path: string) => {
    const who = authorized(request);
    if (who === null) {
      json(response, 401, { message: 'Bad credentials' });
      return;
    }
    const method = request.method ?? 'GET';
    const query = new URL(request.url ?? '/', 'http://localhost').searchParams;
    const route = path.split('?')[0] ?? path;
    if (route.startsWith('/user/installations')) {
      if (who !== 'app') {
        json(response, 403, { message: 'Resource not accessible by personal access token' });
        return;
      }
      if (method === 'GET' && route === '/user/installations') {
        const list =
          installation === null
            ? []
            : [
                {
                  id: 1,
                  account: { login: account.login },
                  repository_selection: installation === 'all' ? 'all' : 'selected',
                },
              ];
        json(response, 200, { total_count: list.length, installations: list });
        return;
      }
      if (method === 'GET' && route === '/user/installations/1/repositories' && installation) {
        const page = Number(query.get('page') ?? '1');
        const list = [...repos.keys()].filter(appMayUse).reverse();
        const start = (page - 1) * 100;
        json(response, 200, {
          total_count: list.length,
          repositories: list.slice(start, start + 100).map((fullName) => repoJson(fullName)),
        });
        return;
      }
      json(response, 404, { message: 'Not Found' });
      return;
    }
    if (method === 'GET' && path === '/user') {
      json(response, 200, {
        id: account.id,
        login: account.login,
        name: account.name,
        avatar_url: '',
      });
      return;
    }
    if (method === 'GET' && route === '/user/repos') {
      json(
        response,
        200,
        [...repos.keys()]
          .filter((fullName) => mayUse(who, fullName))
          .reverse()
          .map((fullName) => repoJson(fullName)),
      );
      return;
    }
    if (method === 'POST' && route === '/user/repos') {
      if (who === 'app') {
        // The app has no administration permission.
        json(response, 403, { message: 'Resource not accessible by integration' });
        return;
      }
      const body = JSON.parse((await readBody(request)).toString('utf8')) as Record<
        string,
        unknown
      >;
      const name = String(body['name']);
      if (repos.has(`${account.login}/${name}`)) {
        json(response, 422, {
          message: 'Repository creation failed.',
          errors: [
            {
              resource: 'Repository',
              code: 'custom',
              field: 'name',
              message: 'name already exists on this account',
            },
          ],
        });
        return;
      }
      addRepo({
        owner: account.login,
        name,
        private: body['private'] === true,
        description: typeof body['description'] === 'string' ? body['description'] : '',
      });
      json(response, 201, repoJson(`${account.login}/${name}`));
      return;
    }
    const match = /^\/repos\/([\w.-]+\/[\w.-]+)$/u.exec(route);
    if (
      method === 'GET' &&
      match !== null &&
      repos.has(match[1] ?? '') &&
      mayUse(who, match[1] ?? '')
    ) {
      json(response, 200, repoJson(match[1] ?? ''));
      return;
    }
    json(response, 404, { message: 'Not Found' });
  };

  const deviceFlow = async (request: IncomingMessage, response: ServerResponse, path: string) => {
    const fields = new URLSearchParams((await readBody(request)).toString('utf8'));
    if (fields.get('client_id') === null) {
      json(response, 200, { error: 'unauthorized_client' });
      return;
    }
    if (path === '/login/oauth/access_token' && fields.get('grant_type') === 'refresh_token') {
      const refresh = fields.get('refresh_token') ?? '';
      const replaced = refreshTokens.get(refresh);
      if (replaced === undefined) {
        json(response, 200, {
          error: 'bad_refresh_token',
          error_description: 'The refresh token passed is incorrect or expired.',
        });
        return;
      }
      // The old token and refresh token stop working.
      refreshTokens.delete(refresh);
      appTokens.delete(replaced);
      refreshes += 1;
      json(response, 200, issueAppToken());
      return;
    }
    if (path === '/login/device/code') {
      device = 'pending';
      json(response, 200, {
        device_code: DEVICE_CODE,
        user_code: FAKE_USER_CODE,
        verification_uri: `${url}/login/device`,
        expires_in: 900,
        interval: 0.05,
      });
      return;
    }
    if (fields.get('device_code') !== DEVICE_CODE) {
      json(response, 200, { error: 'incorrect_device_code' });
      return;
    }
    switch (device) {
      case 'approved':
        device = 'used';
        json(response, 200, issueAppToken());
        return;
      case 'denied':
        json(response, 200, { error: 'access_denied' });
        return;
      case 'used':
        json(response, 200, { error: 'expired_token' });
        return;
      case 'pending':
        json(response, 200, { error: 'authorization_pending' });
    }
  };

  /** git over HTTP: hands the request to `git http-backend` (CGI). */
  const gitHttp = (
    request: IncomingMessage,
    response: ServerResponse,
    path: string,
    query: string,
  ) => {
    const match = /^\/([\w.-]+\/[\w.-]+)\.git(\/.*)$/u.exec(path);
    const fullName = match?.[1] ?? '';
    const repo = repos.get(fullName);
    if (match === null || repo === undefined) {
      response.writeHead(404).end();
      return;
    }
    const pushing = query.includes('git-receive-pack') || path.endsWith('/git-receive-pack');
    const who = authorized(request);
    if ((repo.private || pushing) && who === null) {
      response.writeHead(401, { 'WWW-Authenticate': 'Basic realm="GitHub"' }).end();
      return;
    }
    if (who !== null && !mayUse(who, fullName)) {
      response.writeHead(403).end('the app may not use this repository');
      return;
    }
    const backend = spawn('git', ['http-backend'], {
      env: {
        ...process.env,
        GIT_PROJECT_ROOT: root,
        GIT_HTTP_EXPORT_ALL: '1',
        PATH_INFO: `/${fullName}.git${match[2] ?? ''}`,
        REQUEST_METHOD: request.method ?? 'GET',
        QUERY_STRING: query,
        CONTENT_TYPE: request.headers['content-type'] ?? '',
        REMOTE_USER: account.login,
        REMOTE_ADDR: '127.0.0.1',
        ...(request.headers['content-encoding'] === undefined
          ? {}
          : { HTTP_CONTENT_ENCODING: request.headers['content-encoding'] }),
        ...(request.headers['git-protocol'] === undefined
          ? {}
          : { GIT_PROTOCOL: String(request.headers['git-protocol']) }),
      },
    });
    request.pipe(backend.stdin);
    let head = Buffer.alloc(0);
    let headerDone = false;
    backend.stdout.on('data', (chunk: Buffer) => {
      if (headerDone) {
        response.write(chunk);
        return;
      }
      head = Buffer.concat([head, chunk]);
      let end = head.indexOf('\r\n\r\n');
      let separator = 4;
      if (end < 0) {
        end = head.indexOf('\n\n');
        separator = 2;
      }
      if (end < 0) {
        return;
      }
      headerDone = true;
      let status = 200;
      const headers: Record<string, string> = {};
      for (const line of head.subarray(0, end).toString('utf8').split(/\r?\n/u)) {
        const colon = line.indexOf(':');
        if (colon < 0) {
          continue;
        }
        const name = line.slice(0, colon).trim();
        const value = line.slice(colon + 1).trim();
        if (name.toLowerCase() === 'status') {
          status = Number.parseInt(value, 10);
        } else {
          headers[name] = value;
        }
      }
      response.writeHead(status, headers);
      response.write(head.subarray(end + separator));
    });
    backend.on('close', () => {
      if (!headerDone) {
        response.writeHead(500);
      }
      response.end();
    });
  };

  const server = createServer((request, response) => {
    const parsed = new URL(request.url ?? '/', 'http://localhost');
    const path = parsed.pathname;
    const handle = async (): Promise<void> => {
      if (path.startsWith('/api/')) {
        await api(request, response, path.slice(4));
      } else if (path === '/login/device/code' || path === '/login/oauth/access_token') {
        await deviceFlow(request, response, path);
      } else if (path === '/login/device') {
        response.writeHead(200, { 'Content-Type': 'text/html' });
        response.end('<!doctype html><title>Device activation</title><p>Enter the code</p>');
      } else {
        gitHttp(request, response, path, parsed.search.replace(/^\?/u, ''));
      }
    };
    handle().catch((error: unknown) => {
      response.writeHead(500).end(String(error));
    });
  });

  await new Promise<void>((resolve) => {
    server.listen(0, '127.0.0.1', resolve);
  });
  url = `http://127.0.0.1:${String((server.address() as AddressInfo).port)}`;

  return {
    url,
    api: `${url}/api`,
    root,
    token,
    account,
    installApp: (chosen) => {
      installation = chosen === null ? null : chosen === 'all' ? 'all' : new Set(chosen);
    },
    refreshes: () => refreshes,
    appTokens: () => [...issued],
    addRepo,
    repos: () => [...repos.values()],
    approveDevice: () => {
      device = 'approved';
    },
    denyDevice: () => {
      device = 'denied';
    },
    git: (fullName, args) => runGit(bareDir(fullName), args),
    close: () =>
      new Promise<void>((resolve) => {
        server.closeAllConnections();
        server.close(() => {
          rmSync(root, { recursive: true, force: true });
          resolve();
        });
      }),
  };
}
