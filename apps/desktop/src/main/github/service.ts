import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';

import {
  githubName,
  type DeviceResult,
  type DeviceStart,
  type GitBranches,
  type GitCommit,
  type GitError,
  type GithubAccount,
  type GithubRepo,
  type GithubStatus,
  type GitResult,
  type GitStatus,
  type PublishTarget,
  type PullResult,
} from '@shared/github';

import { GithubApi, GithubError, type DeviceFlow, type GithubApiOptions } from './api';
import { GitService, type Author } from './git';
import { fetchHttpClient } from './http';
import type { TokenStore } from './token-store';

export interface GithubServiceOptions extends GithubApiOptions {
  readonly tokens: TokenStore;
}

/** Waits, unless aborted. */
function delay(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, ms);
    signal.addEventListener('abort', () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

/** Runs an action and turns errors into a result for the UI. */
export async function attempt<T>(action: () => Promise<T>): Promise<GitResult<T>> {
  try {
    return { ok: true, value: await action() };
  } catch (error) {
    const failure = error instanceof GithubError ? error : new GithubError('failed', String(error));
    const result: GitError = {
      code: failure.code,
      message: failure.message,
      ...(failure.files === undefined ? {} : { files: failure.files }),
    };
    return { ok: false, error: result };
  }
}

/**
 * GitHub for the app: the account (token in the keychain), repositories
 * and git for project folders.
 */
export class GithubService {
  readonly api: GithubApi;
  readonly git: GitService;
  private readonly tokens: TokenStore;
  private readonly web: string;
  private account: GithubAccount | null = null;
  private device: { flow: DeviceFlow; abort: AbortController } | null = null;

  constructor(options: GithubServiceOptions) {
    this.api = new GithubApi(options);
    this.tokens = options.tokens;
    this.web = options.web.replace(/\/+$/u, '');
    const webHost = new URL(this.web).host;
    this.git = new GitService({
      http: fetchHttpClient(options.fetch),
      web: this.web,
      // The token only goes to GitHub, never to another server.
      token: (url) => {
        try {
          return new URL(url).host === webHost ? this.tokens.get() : null;
        } catch {
          return null;
        }
      },
    });
  }

  async status(): Promise<GithubStatus> {
    const token = this.tokens.get();
    if (token !== null && this.account === null) {
      try {
        this.account = await this.api.user(token);
      } catch (error) {
        // A revoked token signs out; without network the account stays unknown.
        if (error instanceof GithubError && error.code === 'auth') {
          this.tokens.clear();
        }
      }
    }
    return {
      account: this.tokens.get() === null ? null : this.account,
      deviceFlow: this.api.deviceFlowAvailable,
      storage: this.tokens.storage(),
    };
  }

  /** Signs in with a personal access token (checked with GitHub first). */
  async signInWithToken(token: string): Promise<GithubAccount> {
    const account = await this.api.user(token.trim());
    this.tokens.set(token.trim());
    this.account = account;
    return account;
  }

  async deviceStart(): Promise<DeviceStart> {
    this.deviceCancel();
    const flow = await this.api.deviceStart();
    this.device = { flow, abort: new AbortController() };
    return {
      userCode: flow.userCode,
      verificationUri: flow.verificationUri,
      expiresIn: flow.expiresIn,
    };
  }

  /** Waits until the user entered the code on GitHub (or gave up). */
  async deviceWait(): Promise<DeviceResult> {
    const device = this.device;
    if (device === null) {
      return { state: 'failed', message: 'no sign-in was started' };
    }
    const { flow, abort } = device;
    const end = Date.now() + flow.expiresIn * 1000;
    let interval = flow.interval;
    try {
      while (Date.now() < end) {
        await delay(interval * 1000, abort.signal);
        if (abort.signal.aborted) {
          return { state: 'failed', message: 'cancelled' };
        }
        const poll = await this.api.devicePoll(flow, interval);
        if (poll.state === 'token') {
          const account = await this.signInWithToken(poll.token);
          return { state: 'done', account };
        }
        if (poll.state !== 'pending') {
          return { state: poll.state };
        }
        interval = poll.interval;
      }
      return { state: 'expired' };
    } catch (error) {
      return { state: 'failed', message: error instanceof Error ? error.message : String(error) };
    } finally {
      if (this.device === device) {
        this.device = null;
      }
    }
  }

  deviceCancel(): void {
    this.device?.abort.abort();
    this.device = null;
  }

  signOut(): void {
    this.deviceCancel();
    this.tokens.clear();
    this.account = null;
  }

  async repos(): Promise<GithubRepo[]> {
    return this.api.repos(this.requireToken());
  }

  // --- Projects -----------------------------------------------------------

  async projectStatus(manifestPath: string): Promise<GitStatus> {
    return this.git.status(dirname(manifestPath));
  }

  /**
   * Puts a project on GitHub: git repository, README, first commit, the
   * repository (new or an existing empty one) and the push.
   */
  async publish(manifestPath: string, target: PublishTarget): Promise<GitStatus> {
    const token = this.requireToken();
    const author = await this.author(dirname(manifestPath));
    const dir = dirname(manifestPath);
    const name = projectName(manifestPath);
    let repo: GithubRepo;
    if (target.kind === 'new') {
      repo = await this.api.createRepo(token, target.repo);
    } else {
      repo = await this.api.repo(token, target.fullName);
      if (!repo.empty) {
        throw new GithubError('notEmpty', `the repository ${repo.fullName} is not empty`);
      }
    }
    if (!this.git.isRepo(dir)) {
      await this.git.init(dir, repo.defaultBranch);
    }
    // Local state and generated files stay out (older projects may lack it).
    const ignore = join(dir, '.gitignore');
    if (!existsSync(ignore)) {
      writeFileSync(ignore, DEFAULT_GITIGNORE, 'utf8');
    }
    const readme = join(dir, 'README.md');
    if (!existsSync(readme)) {
      writeFileSync(readme, readmeText(name, repo.description, basename(manifestPath)), 'utf8');
    }
    await this.git.commit(dir, `Create ${name}`, author);
    await this.git.setRemote(dir, repo.cloneUrl);
    await this.git.push(dir);
    return this.git.status(dir);
  }

  async commit(
    manifestPath: string,
    message: string,
    paths: readonly string[] | undefined,
  ): Promise<GitStatus> {
    const dir = dirname(manifestPath);
    await this.git.commit(dir, message, await this.author(dir), paths);
    return this.git.status(dir);
  }

  async push(manifestPath: string): Promise<GitStatus> {
    const dir = dirname(manifestPath);
    this.requireTokenFor(await this.git.remoteUrl(dir));
    await this.git.push(dir);
    return this.git.status(dir);
  }

  async fetch(manifestPath: string): Promise<GitStatus> {
    const dir = dirname(manifestPath);
    await this.git.fetch(dir);
    return this.git.status(dir);
  }

  async pull(manifestPath: string): Promise<PullResult> {
    const dir = dirname(manifestPath);
    return this.git.pull(dir, await this.author(dir));
  }

  async branches(manifestPath: string): Promise<GitBranches> {
    return this.git.branches(dirname(manifestPath));
  }

  async checkout(manifestPath: string, branch: string, create: boolean): Promise<GitStatus> {
    const dir = dirname(manifestPath);
    await this.git.checkout(dir, branch, create);
    return this.git.status(dir);
  }

  async log(manifestPath: string): Promise<GitCommit[]> {
    return this.git.log(dirname(manifestPath));
  }

  /**
   * Clones a repository into `<parent>/<name>` and returns the project
   * manifest (`.ewproj`) in it.
   */
  async clone(fullName: string, parent: string): Promise<string> {
    const token = this.requireToken();
    const repo = await this.api.repo(token, fullName);
    if (repo.empty) {
      throw new GithubError('notFound', `the repository ${repo.fullName} is empty`);
    }
    const dir = join(parent, repo.name);
    if (existsSync(dir) && (!statSync(dir).isDirectory() || readdirSync(dir).length > 0)) {
      throw new GithubError('notEmpty', `the folder ${dir} exists and is not empty`);
    }
    await this.git.clone(repo.cloneUrl, dir);
    const manifests = readdirSync(dir).filter((file) => file.toLowerCase().endsWith('.ewproj'));
    const manifest = manifests[0];
    if (manifest === undefined) {
      throw new GithubError(
        'notProject',
        `the repository ${repo.fullName} has no ElectronicWerk project`,
      );
    }
    return join(dir, manifest);
  }

  /** `owner/name` of a remote URL. */
  repoName(url: string): string | null {
    return githubName(url, this.web);
  }

  private requireToken(): string {
    const token = this.tokens.get();
    if (token === null) {
      throw new GithubError('signedOut', 'not signed in to GitHub');
    }
    return token;
  }

  /** GitHub remotes need the sign-in; other remotes are the user's business. */
  private requireTokenFor(url: string | null): void {
    if (url !== null && this.repoName(url) !== null) {
      this.requireToken();
    }
  }

  /**
   * The author of commits: the repository's `user.name`/`user.email`, else
   * the GitHub account with its private no-reply address.
   */
  private async author(dir: string): Promise<Author> {
    const configured = this.git.isRepo(dir) ? await this.git.configuredAuthor(dir) : null;
    if (configured !== null) {
      return configured;
    }
    const status = await this.status();
    const account = status.account;
    if (account === null) {
      throw new GithubError('noAuthor', 'sign in to GitHub to commit');
    }
    return {
      name: account.name ?? account.login,
      email: `${String(account.id)}+${account.login}@users.noreply.github.com`,
    };
  }
}

/** The `.gitignore` of new projects (as the engine writes it). */
const DEFAULT_GITIGNORE =
  '# ElectronicWerk: local state and generated files\n.ewerk/\noutputs/generated/\n';

/** The project name of a manifest (its `name`, else the file name). */
function projectName(manifestPath: string): string {
  try {
    const manifest = JSON.parse(readFileSync(manifestPath, 'utf8')) as { name?: unknown };
    if (typeof manifest.name === 'string' && manifest.name.trim() !== '') {
      return manifest.name.trim();
    }
  } catch {
    // Fall back to the file name.
  }
  return basename(manifestPath).replace(/\.ewproj$/iu, '');
}

function readmeText(name: string, description: string, manifest: string): string {
  const lines = [`# ${name}`, ''];
  if (description.trim() !== '') {
    lines.push(description.trim(), '');
  }
  lines.push(
    `An electronics project made with [ElectronicWerk](https://github.com/Ujap1994/ElectronicWerk).`,
    `Open \`${manifest}\` in ElectronicWerk, or choose **Open from GitHub** on its start screen.`,
    '',
  );
  return lines.join('\n');
}
