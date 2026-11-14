import type { DeviceStart, GitError, GithubAccount, GithubRepo, NewRepo } from '@shared/github';

export type Fetch = (url: string, init?: RequestInit) => Promise<Response>;

export interface GithubApiOptions {
  readonly fetch: Fetch;
  /** REST API, `https://api.github.com`. */
  readonly api: string;
  /** Website (device flow), `https://github.com`. */
  readonly web: string;
  readonly userAgent: string;
  /** OAuth app for the device flow (empty: not available). */
  readonly clientId: string;
  readonly scopes: string;
  readonly timeoutMs?: number;
}

/** An error with a code the UI explains. */
export class GithubError extends Error {
  readonly code: GitError['code'];
  readonly files: readonly string[] | undefined;

  constructor(code: GitError['code'], message: string, files?: readonly string[]) {
    super(message);
    this.name = 'GithubError';
    this.code = code;
    this.files = files;
  }
}

/** A device flow waiting for the user (the device code stays here). */
export interface DeviceFlow extends DeviceStart {
  readonly deviceCode: string;
  /** Seconds between polls. */
  readonly interval: number;
}

/** How a device flow poll ended. */
export type DevicePoll =
  | { readonly state: 'token'; readonly token: string }
  | { readonly state: 'pending'; readonly interval: number }
  | { readonly state: 'denied' | 'expired' };

/** Repositories read at most (5 pages of 100). */
const MAX_REPO_PAGES = 5;

/** The GitHub REST API and the OAuth device flow. */
export class GithubApi {
  private readonly options: GithubApiOptions;

  constructor(options: GithubApiOptions) {
    this.options = options;
  }

  get deviceFlowAvailable(): boolean {
    return this.options.clientId !== '';
  }

  /** The account of a token (also checks the token). */
  async user(token: string): Promise<GithubAccount> {
    const raw = await this.request(token, 'GET', '/user');
    return parseAccount(raw);
  }

  /** The repositories the account may push to, newest first. */
  async repos(token: string): Promise<GithubRepo[]> {
    const repos: GithubRepo[] = [];
    for (let page = 1; page <= MAX_REPO_PAGES; page += 1) {
      const raw = await this.request(
        token,
        'GET',
        `/user/repos?per_page=100&sort=updated&affiliation=owner,collaborator,organization_member&page=${String(page)}`,
      );
      if (!Array.isArray(raw)) {
        throw new GithubError('failed', 'unexpected repository list from GitHub');
      }
      for (const item of raw) {
        const repo = parseRepo(item);
        if (repo !== null && canPush(item)) {
          repos.push(repo);
        }
      }
      if (raw.length < 100) {
        break;
      }
    }
    return repos;
  }

  async repo(token: string, fullName: string): Promise<GithubRepo> {
    const repo = parseRepo(await this.request(token, 'GET', `/repos/${fullName}`));
    if (repo === null) {
      throw new GithubError('failed', 'unexpected repository data from GitHub');
    }
    return repo;
  }

  /** Creates a repository of the signed-in account (without any commit). */
  async createRepo(token: string, options: NewRepo): Promise<GithubRepo> {
    const raw = await this.request(token, 'POST', '/user/repos', {
      name: options.name,
      private: options.private,
      description: options.description,
      auto_init: false,
      has_wiki: false,
    });
    const repo = parseRepo(raw);
    if (repo === null) {
      throw new GithubError('failed', 'unexpected repository data from GitHub');
    }
    return repo;
  }

  /** Starts the device flow: the user enters the code on GitHub. */
  async deviceStart(): Promise<DeviceFlow> {
    if (!this.deviceFlowAvailable) {
      throw new GithubError('failed', 'no OAuth app is configured');
    }
    const raw = await this.form('/login/device/code', {
      client_id: this.options.clientId,
      scope: this.options.scopes,
    });
    const deviceCode = raw['device_code'];
    const userCode = raw['user_code'];
    const verificationUri = raw['verification_uri'];
    const expiresIn = raw['expires_in'];
    const interval = raw['interval'];
    if (
      typeof deviceCode !== 'string' ||
      typeof userCode !== 'string' ||
      typeof verificationUri !== 'string' ||
      typeof expiresIn !== 'number'
    ) {
      throw new GithubError('failed', describeOAuthError(raw, 'unexpected device code answer'));
    }
    return {
      deviceCode,
      userCode,
      verificationUri,
      expiresIn,
      interval: typeof interval === 'number' && interval > 0 ? interval : 5,
    };
  }

  /** Asks once whether the user has entered the code. */
  async devicePoll(flow: DeviceFlow, interval: number): Promise<DevicePoll> {
    const raw = await this.form('/login/oauth/access_token', {
      client_id: this.options.clientId,
      device_code: flow.deviceCode,
      grant_type: 'urn:ietf:params:oauth:grant-type:device_code',
    });
    const token = raw['access_token'];
    if (typeof token === 'string' && token !== '') {
      return { state: 'token', token };
    }
    switch (raw['error']) {
      case 'authorization_pending':
        return { state: 'pending', interval };
      case 'slow_down': {
        const next = raw['interval'];
        return { state: 'pending', interval: typeof next === 'number' ? next : interval + 5 };
      }
      case 'access_denied':
        return { state: 'denied' };
      case 'expired_token':
        return { state: 'expired' };
      default:
        throw new GithubError('failed', describeOAuthError(raw, 'device flow failed'));
    }
  }

  private async request(
    token: string,
    method: string,
    path: string,
    body?: unknown,
  ): Promise<unknown> {
    const headers: Record<string, string> = {
      Accept: 'application/vnd.github+json',
      Authorization: `Bearer ${token}`,
      'User-Agent': this.options.userAgent,
      'X-GitHub-Api-Version': '2022-11-28',
    };
    if (body !== undefined) {
      headers['Content-Type'] = 'application/json';
    }
    const response = await this.send(`${this.options.api}${path}`, {
      method,
      headers,
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });
    const data: unknown = await response.json().catch(() => null);
    if (response.ok) {
      return data;
    }
    const message = errorMessage(data) ?? `HTTP ${String(response.status)}`;
    if (response.status === 401) {
      throw new GithubError('auth', `GitHub rejected the token: ${message}`);
    }
    if (response.status === 403) {
      throw new GithubError('auth', `GitHub denied access: ${message}`);
    }
    if (response.status === 404) {
      throw new GithubError('notFound', message);
    }
    if (response.status === 422 && /already exists/iu.test(JSON.stringify(data))) {
      throw new GithubError('exists', message);
    }
    throw new GithubError(
      'failed',
      `GitHub answered with HTTP ${String(response.status)}: ${message}`,
    );
  }

  private async form(
    path: string,
    fields: Record<string, string>,
  ): Promise<Record<string, unknown>> {
    const response = await this.send(`${this.options.web}${path}`, {
      method: 'POST',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/x-www-form-urlencoded',
        'User-Agent': this.options.userAgent,
      },
      body: new URLSearchParams(fields).toString(),
    });
    const data: unknown = await response.json().catch(() => null);
    if (typeof data !== 'object' || data === null) {
      throw new GithubError('failed', `GitHub answered with HTTP ${String(response.status)}`);
    }
    return data as Record<string, unknown>;
  }

  private async send(url: string, init: RequestInit): Promise<Response> {
    try {
      return await this.options.fetch(url, {
        ...init,
        signal: AbortSignal.timeout(this.options.timeoutMs ?? 20_000),
      });
    } catch (error) {
      throw new GithubError(
        'network',
        `GitHub could not be reached: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }
}

function parseAccount(raw: unknown): GithubAccount {
  if (typeof raw !== 'object' || raw === null) {
    throw new GithubError('failed', 'unexpected account data from GitHub');
  }
  const data = raw as Record<string, unknown>;
  const id = data['id'];
  const login = data['login'];
  if (typeof id !== 'number' || typeof login !== 'string') {
    throw new GithubError('failed', 'unexpected account data from GitHub');
  }
  const name = data['name'];
  const avatar = data['avatar_url'];
  return {
    id,
    login,
    name: typeof name === 'string' && name !== '' ? name : null,
    avatarUrl: typeof avatar === 'string' ? avatar : '',
  };
}

function parseRepo(raw: unknown): GithubRepo | null {
  if (typeof raw !== 'object' || raw === null) {
    return null;
  }
  const data = raw as Record<string, unknown>;
  const owner = data['owner'];
  const ownerLogin =
    typeof owner === 'object' && owner !== null
      ? (owner as Record<string, unknown>)['login']
      : null;
  const fullName = data['full_name'];
  const name = data['name'];
  const cloneUrl = data['clone_url'];
  const htmlUrl = data['html_url'];
  if (
    typeof fullName !== 'string' ||
    typeof name !== 'string' ||
    typeof ownerLogin !== 'string' ||
    typeof cloneUrl !== 'string' ||
    typeof htmlUrl !== 'string'
  ) {
    return null;
  }
  const description = data['description'];
  const branch = data['default_branch'];
  const updated = data['pushed_at'] ?? data['updated_at'];
  return {
    fullName,
    name,
    owner: ownerLogin,
    private: data['private'] === true,
    description: typeof description === 'string' ? description : '',
    defaultBranch: typeof branch === 'string' && branch !== '' ? branch : 'main',
    cloneUrl,
    htmlUrl,
    // GitHub reports no commits as size 0 (in kilobytes).
    empty: data['size'] === 0,
    updatedAt: typeof updated === 'string' ? updated : '',
  };
}

/** The account may push to a listed repository. */
function canPush(raw: unknown): boolean {
  const permissions = (raw as Record<string, unknown>)['permissions'];
  if (typeof permissions !== 'object' || permissions === null) {
    return true;
  }
  return (permissions as Record<string, unknown>)['push'] !== false;
}

function errorMessage(data: unknown): string | null {
  if (typeof data !== 'object' || data === null) {
    return null;
  }
  const message = (data as Record<string, unknown>)['message'];
  return typeof message === 'string' ? message : null;
}

function describeOAuthError(raw: Record<string, unknown>, fallback: string): string {
  const description = raw['error_description'];
  const error = raw['error'];
  if (typeof description === 'string') {
    return description;
  }
  return typeof error === 'string' ? error : fallback;
}
