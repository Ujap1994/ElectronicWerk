import * as fs from 'node:fs';
import { join } from 'node:path';

import * as git from 'isomorphic-git';
import type { GitAuth, HttpClient } from 'isomorphic-git';

import type { GitBranches, GitChange, GitCommit, GitStatus, PullResult } from '@shared/github';
import { githubName } from '@shared/github';

import { GithubError } from './api';

/** Name and e-mail of commits. */
export interface Author {
  readonly name: string;
  readonly email: string;
}

export interface GitOptions {
  readonly http: HttpClient;
  /** The token for a remote URL, or `null` (only GitHub gets the token). */
  readonly token: (url: string) => string | null;
  /** GitHub's website, for `owner/name` of remotes. */
  readonly web: string;
}

const REMOTE = 'origin';
/** Commits compared for "ahead" and "behind". */
const COMPARE_DEPTH = 1000;

/**
 * Git for a project folder (isomorphic-git: no git installation needed).
 * Errors are `GithubError`s with a code the UI explains.
 */
export class GitService {
  private readonly options: GitOptions;
  private readonly cache = {};
  /** Blob ids of files by path and stat (see `racilyChanged`). */
  private readonly hashes = new Map<string, string>();

  constructor(options: GitOptions) {
    this.options = options;
  }

  /** `true` if the folder itself is a repository (not a folder inside one). */
  isRepo(dir: string): boolean {
    return fs.existsSync(join(dir, '.git'));
  }

  async status(dir: string): Promise<GitStatus> {
    if (!this.isRepo(dir)) {
      return {
        repo: false,
        branch: null,
        remote: null,
        github: null,
        changes: [],
        ahead: null,
        behind: null,
      };
    }
    const branch = (await git.currentBranch({ fs, dir, test: true })) ?? null;
    const remote = await this.remoteUrl(dir);
    const changes = await this.changes(dir);
    let ahead: number | null = null;
    let behind: number | null = null;
    if (branch !== null) {
      const local = await resolve(dir, branch);
      const tracking = await resolve(dir, `refs/remotes/${REMOTE}/${branch}`);
      if (tracking !== null) {
        const counts = await this.compare(dir, local, tracking);
        ahead = counts.ahead;
        behind = counts.behind;
      }
    }
    return {
      repo: true,
      branch,
      remote,
      github: remote === null ? null : githubName(remote, this.options.web),
      changes,
      ahead,
      behind,
    };
  }

  /** Files changed since the last commit (respecting `.gitignore`). */
  async changes(dir: string): Promise<GitChange[]> {
    const matrix = await git.statusMatrix({ fs, dir, cache: this.cache });
    const changes: GitChange[] = [];
    const unchanged: string[] = [];
    for (const [path, head, workdir, stage] of matrix) {
      if (head === 0 && workdir === 2) {
        changes.push({ path, state: 'added' });
      } else if (head === 1 && workdir === 2) {
        changes.push({ path, state: 'modified' });
      } else if (head === 1 && workdir === 0) {
        changes.push({ path, state: 'deleted' });
      } else if (head === 1 && workdir === 1 && stage === 1) {
        unchanged.push(path);
      }
    }
    for (const path of await this.racilyChanged(dir, unchanged)) {
      changes.push({ path, state: 'modified' });
    }
    return changes.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  }

  /**
   * Files that look unchanged by their stat but may not be: isomorphic-git
   * compares whole seconds, so a file rewritten with the same size in the
   * second the index was written looks clean ("racy git"). Their contents
   * are compared with the last commit.
   */
  private async racilyChanged(dir: string, paths: readonly string[]): Promise<string[]> {
    const index = fs.statSync(join(dir, '.git', 'index'), { throwIfNoEntry: false });
    if (index === undefined || paths.length === 0) {
      return [];
    }
    const indexSecond = Math.floor(index.mtimeMs / 1000);
    const head = await resolve(dir, 'HEAD');
    if (head === null) {
      return [];
    }
    const changed: string[] = [];
    for (const path of paths) {
      const stat = fs.statSync(join(dir, path), { throwIfNoEntry: false });
      if (stat === undefined || Math.floor(stat.mtimeMs / 1000) < indexSecond) {
        continue;
      }
      try {
        const committed = await git.readBlob({
          fs,
          dir,
          oid: head,
          filepath: path,
          cache: this.cache,
        });
        // After a clone every file is "racy": hash each version only once.
        const key = `${join(dir, path)}|${String(stat.mtimeMs)}|${String(stat.size)}|${String(stat.ino)}`;
        let current = this.hashes.get(key);
        if (current === undefined) {
          current = (await git.hashBlob({ object: fs.readFileSync(join(dir, path)) })).oid;
          if (this.hashes.size > 50_000) {
            this.hashes.clear();
          }
          this.hashes.set(key, current);
        }
        if (current !== committed.oid) {
          changed.push(path);
        }
      } catch {
        changed.push(path);
      }
    }
    return changed;
  }

  /** Makes a folder a repository (branch `main`). */
  async init(dir: string, branch = 'main'): Promise<void> {
    await git.init({ fs, dir, defaultBranch: branch });
  }

  /**
   * Commits changed files (all without `paths`). Returns the commit, or
   * `null` if nothing changed.
   */
  async commit(
    dir: string,
    message: string,
    author: Author,
    paths?: readonly string[],
  ): Promise<string | null> {
    const wanted = paths === undefined ? null : new Set(paths);
    const changes = (await this.changes(dir)).filter(
      (change) => wanted === null || wanted.has(change.path),
    );
    if (changes.length === 0) {
      return null;
    }
    for (const change of changes) {
      if (change.state === 'deleted') {
        await git.remove({ fs, dir, filepath: change.path, cache: this.cache });
      } else {
        await git.add({ fs, dir, filepath: change.path, cache: this.cache });
      }
    }
    return git.commit({ fs, dir, message, author, cache: this.cache });
  }

  /** `user.name` and `user.email` of the repository, if both are set. */
  async configuredAuthor(dir: string): Promise<Author | null> {
    const name: unknown = await git.getConfig({ fs, dir, path: 'user.name' });
    const email: unknown = await git.getConfig({ fs, dir, path: 'user.email' });
    return typeof name === 'string' && name !== '' && typeof email === 'string' && email !== ''
      ? { name, email }
      : null;
  }

  async setRemote(dir: string, url: string): Promise<void> {
    await git.addRemote({ fs, dir, remote: REMOTE, url, force: true });
  }

  async remoteUrl(dir: string): Promise<string | null> {
    const url: unknown = await git.getConfig({ fs, dir, path: `remote.${REMOTE}.url` });
    return typeof url === 'string' && url !== '' ? url : null;
  }

  /** Pushes the current branch and makes it track the remote branch. */
  async push(dir: string): Promise<void> {
    const branch = await this.requireBranch(dir);
    const url = await this.requireRemote(dir);
    let result: git.PushResult;
    try {
      result = await git.push({
        fs,
        http: this.options.http,
        dir,
        remote: REMOTE,
        ref: branch,
        remoteRef: branch,
        ...this.auth(url),
        cache: this.cache,
      });
    } catch (error) {
      throw translate(error);
    }
    if (!result.ok) {
      const reason = result.error ?? Object.values(result.refs).find((ref) => !ref.ok)?.error;
      throw new GithubError('rejected', `the push was rejected: ${reason ?? 'unknown reason'}`);
    }
    await git.setConfig({ fs, dir, path: `branch.${branch}.remote`, value: REMOTE });
    await git.setConfig({ fs, dir, path: `branch.${branch}.merge`, value: `refs/heads/${branch}` });
  }

  /** Reads the remote's branches and commits (changes nothing here). */
  async fetch(dir: string): Promise<void> {
    const url = await this.requireRemote(dir);
    try {
      await git.fetch({
        fs,
        http: this.options.http,
        dir,
        remote: REMOTE,
        tags: false,
        prune: true,
        ...this.auth(url),
        cache: this.cache,
      });
    } catch (error) {
      throw translate(error);
    }
  }

  /**
   * Takes the remote branch's commits. Files are never merged line by line:
   * a file changed here and on the remote is a conflict and nothing changes.
   */
  async pull(dir: string, author: Author): Promise<PullResult> {
    const branch = await this.requireBranch(dir);
    await this.requireClean(dir);
    await this.fetch(dir);
    const local = await resolve(dir, branch);
    const remote = await resolve(dir, `refs/remotes/${REMOTE}/${branch}`);
    if (remote === null || local === remote) {
      return { state: 'upToDate', changed: false };
    }
    if (local !== null && (await descends(dir, local, remote))) {
      // Only local commits: nothing to take.
      return { state: 'upToDate', changed: false };
    }
    let merged: git.MergeResult;
    try {
      merged = await git.merge({
        fs,
        dir,
        ours: branch,
        theirs: `${REMOTE}/${branch}`,
        author,
        message: `Merge ${REMOTE}/${branch}`,
        abortOnConflict: true,
        mergeDriver: ({ contents }) => {
          const ours = contents[1] ?? '';
          const theirs = contents[2] ?? '';
          return { cleanMerge: ours === theirs, mergedText: ours };
        },
        cache: this.cache,
      });
    } catch (error) {
      throw translate(error);
    }
    await git.checkout({ fs, dir, ref: branch, cache: this.cache });
    return {
      state: merged.fastForward === true ? 'fastForward' : 'merged',
      changed: merged.alreadyMerged !== true,
    };
  }

  async branches(dir: string): Promise<GitBranches> {
    const current = (await git.currentBranch({ fs, dir, test: true })) ?? null;
    const local = await git.listBranches({ fs, dir });
    let remote: string[] = [];
    if ((await this.remoteUrl(dir)) !== null) {
      remote = (await git.listBranches({ fs, dir, remote: REMOTE })).filter(
        (name) => name !== 'HEAD' && !local.includes(name),
      );
    }
    return { current, local: local.sort(), remote: remote.sort() };
  }

  /**
   * Switches to a branch (a remote branch becomes a local one). With
   * `create`, a new branch starts at the current commit.
   */
  async checkout(dir: string, name: string, create: boolean): Promise<void> {
    await this.requireClean(dir);
    try {
      if (create) {
        const existing = await git.listBranches({ fs, dir });
        if (existing.includes(name)) {
          throw new GithubError('exists', `the branch ${name} exists`);
        }
        await git.branch({ fs, dir, ref: name, checkout: true });
        return;
      }
      await git.checkout({ fs, dir, ref: name, remote: REMOTE, cache: this.cache });
    } catch (error) {
      throw translate(error);
    }
  }

  async log(dir: string, depth = 200): Promise<GitCommit[]> {
    const branch = (await git.currentBranch({ fs, dir, test: true })) ?? null;
    if (branch === null || (await resolve(dir, branch)) === null) {
      return [];
    }
    const entries = await git.log({ fs, dir, ref: branch, depth, cache: this.cache });
    return entries.map((entry) => ({
      oid: entry.oid,
      message: entry.commit.message.trimEnd(),
      author: entry.commit.author.name,
      time: entry.commit.author.timestamp * 1000,
    }));
  }

  /** Clones a repository into an empty folder. */
  async clone(url: string, dir: string): Promise<void> {
    try {
      await git.clone({
        fs,
        http: this.options.http,
        dir,
        url,
        singleBranch: false,
        noTags: true,
        ...this.auth(url),
        cache: this.cache,
      });
    } catch (error) {
      throw translate(error);
    }
  }

  private auth(url: string): {
    onAuth: () => GitAuth;
    onAuthFailure: () => GitAuth;
  } {
    return {
      onAuth: () => {
        const token = this.options.token(url);
        return token === null ? { cancel: true } : { username: 'x-access-token', password: token };
      },
      // Never ask twice with the same token.
      onAuthFailure: () => ({ cancel: true }),
    };
  }

  private async requireBranch(dir: string): Promise<string> {
    const branch = await git.currentBranch({ fs, dir, test: true });
    if (branch === undefined) {
      throw new GithubError('failed', 'no branch is checked out');
    }
    return branch;
  }

  private async requireRemote(dir: string): Promise<string> {
    const url = await this.remoteUrl(dir);
    if (url === null) {
      throw new GithubError('noRemote', 'the project has no remote repository');
    }
    return url;
  }

  private async requireClean(dir: string): Promise<void> {
    const changes = await this.changes(dir);
    if (changes.length > 0) {
      throw new GithubError(
        'uncommitted',
        'the project has changes that are not committed',
        changes.map((change) => change.path),
      );
    }
  }

  private async compare(
    dir: string,
    local: string | null,
    remote: string,
  ): Promise<{ ahead: number; behind: number }> {
    const localOids = local === null ? [] : await history(dir, local);
    const remoteOids = await history(dir, remote);
    const localSet = new Set(localOids);
    const remoteSet = new Set(remoteOids);
    return {
      ahead: localOids.filter((oid) => !remoteSet.has(oid)).length,
      behind: remoteOids.filter((oid) => !localSet.has(oid)).length,
    };
  }
}

async function resolve(dir: string, ref: string): Promise<string | null> {
  try {
    return await git.resolveRef({ fs, dir, ref });
  } catch {
    return null;
  }
}

async function history(dir: string, oid: string): Promise<string[]> {
  const entries = await git.log({ fs, dir, ref: oid, depth: COMPARE_DEPTH });
  return entries.map((entry) => entry.oid);
}

/** `true` if `oid` has `ancestor` in its history. */
async function descends(dir: string, oid: string, ancestor: string): Promise<boolean> {
  try {
    return await git.isDescendent({ fs, dir, oid, ancestor, depth: COMPARE_DEPTH });
  } catch {
    return false;
  }
}

/** Turns isomorphic-git's errors into errors the UI explains. */
export function translate(error: unknown): GithubError {
  if (error instanceof GithubError) {
    return error;
  }
  const code = (error as { code?: unknown }).code;
  const message = error instanceof Error ? error.message : String(error);
  const data = (error as { data?: Record<string, unknown> }).data ?? {};
  switch (code) {
    case 'MergeConflictError': {
      const files = Array.isArray(data['filepaths'])
        ? data['filepaths'].filter((file): file is string => typeof file === 'string')
        : [];
      return new GithubError('conflict', message, files);
    }
    case 'CheckoutConflictError': {
      const files = Array.isArray(data['filepaths'])
        ? data['filepaths'].filter((file): file is string => typeof file === 'string')
        : [];
      return new GithubError('uncommitted', message, files);
    }
    case 'PushRejectedError':
      return new GithubError('rejected', message);
    case 'UserCanceledError':
      return new GithubError('auth', message);
    case 'HttpError': {
      const status = data['statusCode'];
      if (status === 401 || status === 403) {
        return new GithubError('auth', message);
      }
      if (status === 404) {
        return new GithubError('notFound', message);
      }
      return new GithubError('failed', message);
    }
    case 'NotFoundError':
      return new GithubError('notFound', message);
    case 'AlreadyExistsError':
      return new GithubError('exists', message);
    default:
      if (/ENOTFOUND|ECONNREFUSED|ECONNRESET|ETIMEDOUT|fetch failed|ERR_INTERNET/u.test(message)) {
        return new GithubError('network', message);
      }
      return new GithubError('failed', message);
  }
}
