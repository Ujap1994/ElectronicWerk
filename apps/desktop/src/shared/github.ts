/**
 * GitHub and git: types shared by the main process (which holds the token
 * and runs git) and the UI.
 */

/**
 * Client id of the ElectronicWerk OAuth app for the device flow (public, no
 * secret). Empty while the app is not registered: then only personal access
 * tokens can be used.
 */
export const GITHUB_CLIENT_ID = '';

/** Scopes asked for: repositories (public and private) and the user's name. */
export const GITHUB_SCOPES = 'repo read:user';

export interface GithubAccount {
  readonly id: number;
  readonly login: string;
  readonly name: string | null;
  readonly avatarUrl: string;
}

/** How the token is protected on this computer. */
export type TokenStorage = 'keychain' | 'plain' | 'unavailable';

export interface GithubStatus {
  /** The signed-in account, `null` without a token. */
  readonly account: GithubAccount | null;
  /** The device flow is available (an OAuth app is configured). */
  readonly deviceFlow: boolean;
  readonly storage: TokenStorage;
}

/** A started device flow: the code the user enters on GitHub. */
export interface DeviceStart {
  readonly userCode: string;
  readonly verificationUri: string;
  /** Seconds until the code expires. */
  readonly expiresIn: number;
}

/** How a device flow ended. */
export type DeviceResult =
  | { readonly state: 'done'; readonly account: GithubAccount }
  | { readonly state: 'denied' | 'expired' | 'failed'; readonly message?: string };

export interface GithubRepo {
  readonly fullName: string;
  readonly name: string;
  readonly owner: string;
  readonly private: boolean;
  readonly description: string;
  readonly defaultBranch: string;
  readonly cloneUrl: string;
  readonly htmlUrl: string;
  /** The repository has no commits yet. */
  readonly empty: boolean;
  /** ISO 8601. */
  readonly updatedAt: string;
}

/** Options of a new repository. */
export interface NewRepo {
  readonly name: string;
  readonly private: boolean;
  readonly description: string;
}

/** A changed file of the working folder. */
export interface GitChange {
  readonly path: string;
  readonly state: 'added' | 'modified' | 'deleted';
}

export interface GitStatus {
  /** The project folder is a git repository. */
  readonly repo: boolean;
  readonly branch: string | null;
  /** URL of `origin`, if any. */
  readonly remote: string | null;
  /** `owner/name` of a GitHub remote. */
  readonly github: string | null;
  readonly changes: readonly GitChange[];
  /** Local commits not on the remote branch (`null`: no remote branch). */
  readonly ahead: number | null;
  /** Remote commits not here (as of the last fetch). */
  readonly behind: number | null;
}

export interface GitCommit {
  readonly oid: string;
  readonly message: string;
  readonly author: string;
  /** Milliseconds since 1970. */
  readonly time: number;
}

export interface GitBranches {
  readonly current: string | null;
  readonly local: readonly string[];
  /** Branches of `origin` that have no local branch. */
  readonly remote: readonly string[];
}

/** What a pull did. */
export interface PullResult {
  readonly state: 'upToDate' | 'fastForward' | 'merged';
  /** Files of the project changed: the project must be read again. */
  readonly changed: boolean;
}

/** Where a project is published: a new repository or an existing empty one. */
export type PublishTarget =
  | { readonly kind: 'new'; readonly repo: NewRepo }
  | { readonly kind: 'existing'; readonly fullName: string };

/** Why a GitHub or git action failed (the UI explains each). */
export type GitErrorCode =
  /** Not signed in to GitHub. */
  | 'signedOut'
  /** GitHub rejected the token, or the account may not do this. */
  | 'auth'
  /** GitHub could not be reached. */
  | 'network'
  /** A pull found files changed here and on GitHub. */
  | 'conflict'
  /** A push was rejected: GitHub has newer commits (pull first). */
  | 'rejected'
  /** The project has changes that are not committed. */
  | 'uncommitted'
  /** The folder or repository is not empty. */
  | 'notEmpty'
  /** A repository or branch of that name exists. */
  | 'exists'
  /** The project has no GitHub remote. */
  | 'noRemote'
  /** No name for commits: sign in to GitHub. */
  | 'noAuthor'
  /** The project is not open (or not a project). */
  | 'notProject'
  /** The repository or branch was not found. */
  | 'notFound'
  | 'failed';

export interface GitError {
  readonly code: GitErrorCode;
  /** Technical details (English). */
  readonly message: string;
  /** The files concerned (conflicts, uncommitted changes). */
  readonly files?: readonly string[];
}

/** The answer of a GitHub or git action. */
export type GitResult<T> =
  { readonly ok: true; readonly value: T } | { readonly ok: false; readonly error: GitError };

export const GITHUB_WEB = 'https://github.com';
export const GITHUB_API = 'https://api.github.com';

/**
 * `owner/name` of a GitHub URL (https or ssh), else `null`. `web` is the
 * GitHub website (tests use a local server).
 */
export function githubName(url: string, web: string = GITHUB_WEB): string | null {
  const text = url.trim();
  const base = web.replace(/\/+$/u, '');
  let rest: string | null = null;
  if (text.startsWith(`${base}/`)) {
    rest = text.slice(base.length + 1);
  } else if (base === GITHUB_WEB && text.startsWith('git@github.com:')) {
    rest = text.slice('git@github.com:'.length);
  } else if (base === GITHUB_WEB) {
    // https://user@github.com/owner/name
    const match = /^https:\/\/[^@/]+@github\.com\/(.*)$/u.exec(text);
    rest = match?.[1] ?? null;
  }
  if (rest === null) {
    return null;
  }
  const match = /^([\w.-]+)\/([\w.-]+?)(?:\.git)?\/?$/u.exec(rest);
  return match === null ? null : `${match[1] ?? ''}/${match[2] ?? ''}`;
}

/** `true` for a valid `owner/name`. */
export function isFullName(value: unknown): value is string {
  return (
    typeof value === 'string' &&
    value.length <= 200 &&
    /^[\w.-]+\/[\w.-]+$/u.test(value) &&
    value.split('/').every((part) => !/^\.+$/u.test(part))
  );
}

/** A valid name for a new repository (GitHub's rules). */
export function repoName(text: string): string {
  return text
    .trim()
    .replace(/[^\w.-]+/gu, '-')
    .replace(/^-+|-+$/gu, '')
    .slice(0, 100);
}

/** `true` if a branch name is valid for git. */
export function isBranchName(name: string): boolean {
  return (
    name.length > 0 &&
    name.length <= 200 &&
    !/[\s~^:?*[\\]|\.\.|@\{|\/\/|^\/|\/$|\.lock$|^-|\.$/u.test(name)
  );
}

/** Checks an untrusted publish target (from the UI). */
export function isPublishTarget(value: unknown): value is PublishTarget {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const target = value as Record<string, unknown>;
  if (target['kind'] === 'existing') {
    return isFullName(target['fullName']);
  }
  if (target['kind'] !== 'new' || typeof target['repo'] !== 'object' || target['repo'] === null) {
    return false;
  }
  const repo = target['repo'] as Record<string, unknown>;
  return (
    typeof repo['name'] === 'string' &&
    repo['name'] !== '' &&
    repoName(repo['name']) === repo['name'] &&
    typeof repo['private'] === 'boolean' &&
    typeof repo['description'] === 'string' &&
    repo['description'].length <= 350
  );
}
