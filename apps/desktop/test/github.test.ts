import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterAll, beforeAll, beforeEach, describe, expect, it } from 'vitest';

import type { GitResult } from '@shared/github';

import { GithubService, attempt } from '../src/main/github/service';
import { TEST_SECRET_BOX, TokenStore, type SecretBox } from '../src/main/github/token-store';
import { FAKE_USER_CODE, startFakeGithub, type FakeGithub } from './github-server';

let github: FakeGithub;
let work: string;

beforeAll(async () => {
  github = await startFakeGithub();
});

afterAll(async () => {
  await github.close();
});

beforeEach(() => {
  work = mkdtempSync(join(tmpdir(), 'ew-git-test-'));
  return () => {
    rmSync(work, { recursive: true, force: true });
  };
});

function service(clientId = 'Iv1.test-client', file = join(work, 'token.bin')): GithubService {
  return new GithubService({
    fetch: (url, init) => fetch(url, init),
    api: github.api,
    web: github.url,
    userAgent: 'ElectronicWerk-test',
    clientId,
    scopes: 'repo read:user',
    tokens: new TokenStore(file, TEST_SECRET_BOX),
  });
}

async function signedIn(): Promise<GithubService> {
  const result = service();
  await result.signInWithToken(github.token);
  return result;
}

/** A small project folder like the engine writes it. */
function makeProject(name: string): string {
  const dir = join(work, name);
  mkdirSync(join(dir, 'schematics'), { recursive: true });
  mkdirSync(join(dir, '.ewerk'), { recursive: true });
  writeFileSync(join(dir, `${name}.ewproj`), JSON.stringify({ name, documents: [] }, null, 2));
  writeFileSync(join(dir, 'schematics', 'main.ewsch'), '{\n  "symbols": []\n}\n');
  writeFileSync(join(dir, '.gitignore'), '.ewerk/\noutputs/generated/\n');
  writeFileSync(join(dir, '.ewerk', 'journal'), 'local state');
  return join(dir, `${name}.ewproj`);
}

function value<T>(result: GitResult<T>): T {
  if (!result.ok) {
    throw new Error(`${result.error.code}: ${result.error.message}`);
  }
  return result.value;
}

describe('the GitHub account', () => {
  it('signs in with a token and keeps it encrypted', async () => {
    const gh = service();
    expect((await gh.status()).account).toBeNull();
    const wrong = await attempt(() => gh.signInWithToken('ghp_wrong'));
    expect(wrong.ok ? null : wrong.error.code).toBe('auth');

    const account = await gh.signInWithToken(` ${github.token} `);
    expect(account).toMatchObject({ login: 'octo-maker', name: 'Octo Maker', id: 4242 });
    const stored = readFileSync(join(work, 'token.bin'), 'utf8');
    expect(stored).toBe(`test:${github.token}`);

    // A new start reads the token and the account again.
    const status = await service().status();
    expect(status).toMatchObject({
      account: { login: 'octo-maker' },
      deviceFlow: true,
      storage: 'keychain',
    });

    gh.signOut();
    expect(existsSync(join(work, 'token.bin'))).toBe(false);
    expect((await service().status()).account).toBeNull();
  });

  it('signs in with the device flow', async () => {
    const gh = service();
    const start = await gh.deviceStart();
    expect(start.userCode).toBe(FAKE_USER_CODE);
    expect(start.verificationUri).toBe(`${github.url}/login/device`);
    const waiting = gh.deviceWait();
    await new Promise((resolve) => setTimeout(resolve, 150));
    github.approveDevice();
    const result = await waiting;
    expect(result).toMatchObject({ state: 'done', account: { login: 'octo-maker' } });
    expect((await gh.status()).account?.login).toBe('octo-maker');
  });

  it('reports a denied device flow and a missing OAuth app', async () => {
    const gh = service();
    await gh.deviceStart();
    const waiting = gh.deviceWait();
    github.denyDevice();
    expect(await waiting).toEqual({ state: 'denied' });

    const noApp = service('');
    expect((await noApp.status()).deviceFlow).toBe(false);
    expect((await attempt(() => noApp.deviceStart())).ok).toBe(false);
  });

  it('keeps the token in memory only without encryption', () => {
    const none: SecretBox = {
      isEncryptionAvailable: () => false,
      encryptString: () => {
        throw new Error('unavailable');
      },
      decryptString: () => {
        throw new Error('unavailable');
      },
    };
    const store = new TokenStore(join(work, 'plain.bin'), none);
    store.set('secret');
    expect(store.get()).toBe('secret');
    expect(store.storage()).toBe('unavailable');
    expect(existsSync(join(work, 'plain.bin'))).toBe(false);

    const basic = new TokenStore(
      join(work, 'basic.bin'),
      { ...TEST_SECRET_BOX, getSelectedStorageBackend: () => 'basic_text' },
      'linux',
    );
    expect(basic.storage()).toBe('plain');
    const kwallet = new TokenStore(
      join(work, 'kw.bin'),
      { ...TEST_SECRET_BOX, getSelectedStorageBackend: () => 'kwallet5' },
      'linux',
    );
    expect(kwallet.storage()).toBe('keychain');
  });

  it('lists the repositories', async () => {
    const gh = await signedIn();
    github.addRepo({ owner: 'octo-maker', name: 'listed', private: true, description: 'A list' });
    const repos = await gh.repos();
    const listed = repos.find((repo) => repo.fullName === 'octo-maker/listed');
    expect(listed).toMatchObject({
      name: 'listed',
      owner: 'octo-maker',
      private: true,
      description: 'A list',
      empty: true,
      cloneUrl: `${github.url}/octo-maker/listed.git`,
    });
    const signedOut = service('', join(work, 'other-token.bin'));
    const denied = await attempt(() => signedOut.repos());
    expect(denied.ok ? null : denied.error.code).toBe('signedOut');
  });
});

describe('projects on GitHub', () => {
  it('publishes a project to a new repository', async () => {
    const gh = await signedIn();
    const manifest = makeProject('Blinky');
    const status = value(
      await attempt(() =>
        gh.publish(manifest, {
          kind: 'new',
          repo: { name: 'blinky', private: true, description: 'A blinking LED' },
        }),
      ),
    );
    expect(status).toMatchObject({
      repo: true,
      branch: 'main',
      github: 'octo-maker/blinky',
      changes: [],
      ahead: 0,
      behind: 0,
    });
    const files = github.git('octo-maker/blinky', ['ls-tree', '-r', '--name-only', 'main']);
    expect(files.trim().split('\n').sort()).toEqual([
      '.gitignore',
      'Blinky.ewproj',
      'README.md',
      'schematics/main.ewsch',
    ]);
    const readme = github.git('octo-maker/blinky', ['show', 'main:README.md']);
    expect(readme).toContain('# Blinky');
    expect(readme).toContain('A blinking LED');
    const author = github.git('octo-maker/blinky', ['log', '-1', '--format=%an <%ae>|%s']);
    expect(author.trim()).toBe(
      'Octo Maker <4242+octo-maker@users.noreply.github.com>|Create Blinky',
    );

    // The name is taken now.
    const again = await attempt(() =>
      gh.publish(makeProject('Other'), {
        kind: 'new',
        repo: { name: 'blinky', private: false, description: '' },
      }),
    );
    expect(again.ok ? null : again.error.code).toBe('exists');
  });

  it('publishes into an existing empty repository only', async () => {
    const gh = await signedIn();
    github.addRepo({ owner: 'octo-maker', name: 'empty-one', private: false, description: '' });
    const manifest = makeProject('Shield');
    // An older project without .gitignore gets one: local state stays out.
    rmSync(join(work, 'Shield', '.gitignore'));
    value(
      await attempt(() =>
        gh.publish(manifest, { kind: 'existing', fullName: 'octo-maker/empty-one' }),
      ),
    );
    const files = github.git('octo-maker/empty-one', ['ls-tree', '-r', '--name-only', 'main']);
    expect(files.trim().split('\n').sort()).toEqual([
      '.gitignore',
      'README.md',
      'Shield.ewproj',
      'schematics/main.ewsch',
    ]);
    const second = await attempt(() =>
      gh.publish(makeProject('Late'), { kind: 'existing', fullName: 'octo-maker/empty-one' }),
    );
    expect(second.ok ? null : second.error.code).toBe('notEmpty');
  });

  it('commits, pushes, clones and pulls', async () => {
    const gh = await signedIn();
    const manifest = makeProject('Amp');
    value(
      await attempt(() =>
        gh.publish(manifest, {
          kind: 'new',
          repo: { name: 'amp', private: true, description: '' },
        }),
      ),
    );
    const dir = join(work, 'Amp');

    // A change here: one modified file, one new, one deleted.
    writeFileSync(join(dir, 'schematics', 'main.ewsch'), '{\n  "symbols": [1]\n}\n');
    writeFileSync(join(dir, 'board.ewpcb'), '{}\n');
    rmSync(join(dir, 'README.md'));
    let status = await gh.projectStatus(manifest);
    expect(status.changes).toEqual([
      { path: 'README.md', state: 'deleted' },
      { path: 'board.ewpcb', state: 'added' },
      { path: 'schematics/main.ewsch', state: 'modified' },
    ]);
    // Only some files.
    status = value(await attempt(() => gh.commit(manifest, 'Add the board', ['board.ewpcb'])));
    expect(status.changes.map((change) => change.path)).toEqual([
      'README.md',
      'schematics/main.ewsch',
    ]);
    expect(status.ahead).toBe(1);
    status = value(await attempt(() => gh.commit(manifest, 'Edit the schematic', undefined)));
    expect(status.changes).toEqual([]);
    expect(status.ahead).toBe(2);
    status = value(await attempt(() => gh.push(manifest)));
    expect(status.ahead).toBe(0);

    // Somebody else clones it, changes it and pushes.
    const otherParent = join(work, 'elsewhere');
    mkdirSync(otherParent);
    const cloned = value(await attempt(() => gh.clone('octo-maker/amp', otherParent)));
    expect(cloned).toBe(join(otherParent, 'amp', 'Amp.ewproj'));
    expect(existsSync(join(otherParent, 'amp', 'board.ewpcb'))).toBe(true);
    expect(existsSync(join(otherParent, 'amp', '.ewerk'))).toBe(false);
    writeFileSync(join(otherParent, 'amp', 'board.ewpcb'), '{"tracks": []}\n');
    value(await attempt(() => gh.commit(cloned, 'Route the board', undefined)));
    value(await attempt(() => gh.push(cloned)));

    // Cloning into a folder that is not empty fails.
    const twice = await attempt(() => gh.clone('octo-maker/amp', otherParent));
    expect(twice.ok ? null : twice.error.code).toBe('notEmpty');

    // Here: fetch shows it, pull takes it (fast-forward).
    status = value(await attempt(() => gh.fetch(manifest)));
    expect(status.behind).toBe(1);
    const pulled = value(await attempt(() => gh.pull(manifest)));
    expect(pulled).toEqual({ state: 'fastForward', changed: true });
    expect(readFileSync(join(dir, 'board.ewpcb'), 'utf8')).toBe('{"tracks": []}\n');
    expect((await gh.projectStatus(manifest)).behind).toBe(0);
    expect(value(await attempt(() => gh.pull(manifest)))).toEqual({
      state: 'upToDate',
      changed: false,
    });

    const log = await gh.log(manifest);
    expect(log.map((commit) => commit.message)).toEqual([
      'Route the board',
      'Edit the schematic',
      'Add the board',
      'Create Amp',
    ]);
    expect(log[0]?.author).toBe('Octo Maker');
  });

  it('merges changes of different files and stops at conflicts', async () => {
    const gh = await signedIn();
    const manifest = makeProject('Psu');
    value(
      await attempt(() =>
        gh.publish(manifest, {
          kind: 'new',
          repo: { name: 'psu', private: false, description: '' },
        }),
      ),
    );
    const dir = join(work, 'Psu');
    const parent = join(work, 'second');
    mkdirSync(parent);
    const other = value(await attempt(() => gh.clone('octo-maker/psu', parent)));
    const otherDir = join(parent, 'psu');

    // Different files: a merge commit.
    writeFileSync(join(otherDir, 'notes.txt'), 'theirs\n');
    value(await attempt(() => gh.commit(other, 'Notes', undefined)));
    value(await attempt(() => gh.push(other)));
    writeFileSync(join(dir, 'schematics', 'main.ewsch'), '{"mine": true}\n');
    value(await attempt(() => gh.commit(manifest, 'Mine', undefined)));
    const rejected = await attempt(() => gh.push(manifest));
    expect(rejected.ok ? null : rejected.error.code).toBe('rejected');

    // Uncommitted changes block a pull.
    writeFileSync(join(dir, 'scratch.txt'), 'x');
    const blocked = await attempt(() => gh.pull(manifest));
    expect(blocked.ok ? null : blocked.error).toMatchObject({
      code: 'uncommitted',
      files: ['scratch.txt'],
    });
    rmSync(join(dir, 'scratch.txt'));

    expect(value(await attempt(() => gh.pull(manifest)))).toEqual({
      state: 'merged',
      changed: true,
    });
    expect(readFileSync(join(dir, 'notes.txt'), 'utf8')).toBe('theirs\n');
    value(await attempt(() => gh.push(manifest)));

    // The same file on both sides: a conflict, nothing changes.
    value(await attempt(() => gh.pull(other)));
    writeFileSync(join(otherDir, 'schematics', 'main.ewsch'), '{"theirs": 2}\n');
    value(await attempt(() => gh.commit(other, 'Theirs', undefined)));
    value(await attempt(() => gh.push(other)));
    writeFileSync(join(dir, 'schematics', 'main.ewsch'), '{"mine": 2}\n');
    value(await attempt(() => gh.commit(manifest, 'Mine again', undefined)));
    const conflict = await attempt(() => gh.pull(manifest));
    expect(conflict.ok ? null : conflict.error).toMatchObject({
      code: 'conflict',
      files: ['schematics/main.ewsch'],
    });
    expect(readFileSync(join(dir, 'schematics', 'main.ewsch'), 'utf8')).toBe('{"mine": 2}\n');
    expect((await gh.projectStatus(manifest)).changes).toEqual([]);
  });

  it('creates and switches branches', async () => {
    const gh = await signedIn();
    const manifest = makeProject('Led');
    value(
      await attempt(() =>
        gh.publish(manifest, {
          kind: 'new',
          repo: { name: 'led', private: false, description: '' },
        }),
      ),
    );
    const dir = join(work, 'Led');
    value(await attempt(() => gh.checkout(manifest, 'rev-b', true)));
    writeFileSync(join(dir, 'rev-b.txt'), 'b\n');
    value(await attempt(() => gh.commit(manifest, 'Revision B', undefined)));
    const pushed = value(await attempt(() => gh.push(manifest)));
    expect(pushed).toMatchObject({ branch: 'rev-b', ahead: 0 });
    expect(
      github.git('octo-maker/led', ['branch', '--list']).replace(/[ *]/gu, '').trim().split('\n'),
    ).toEqual(['main', 'rev-b']);
    const exists = await attempt(() => gh.checkout(manifest, 'main', true));
    expect(exists.ok ? null : exists.error.code).toBe('exists');

    value(await attempt(() => gh.checkout(manifest, 'main', false)));
    expect(existsSync(join(dir, 'rev-b.txt'))).toBe(false);
    expect(await gh.branches(manifest)).toEqual({
      current: 'main',
      local: ['main', 'rev-b'],
      remote: [],
    });

    // A clone sees the branch on GitHub and takes it.
    const parent = join(work, 'third');
    mkdirSync(parent);
    const other = value(await attempt(() => gh.clone('octo-maker/led', parent)));
    expect(await gh.branches(other)).toEqual({
      current: 'main',
      local: ['main'],
      remote: ['rev-b'],
    });
    value(await attempt(() => gh.checkout(other, 'rev-b', false)));
    expect(readFileSync(join(parent, 'led', 'rev-b.txt'), 'utf8')).toBe('b\n');
    expect((await gh.projectStatus(other)).branch).toBe('rev-b');
  });

  it('sends the token to GitHub only', async () => {
    const gh = await signedIn();
    const manifest = makeProject('Leak');
    const dir = join(work, 'Leak');
    await gh.git.init(dir);
    value(await attempt(() => gh.commit(manifest, 'First', undefined)));
    github.addRepo({ owner: 'octo-maker', name: 'leak', private: true, description: '' });
    // The same server under another host name: not GitHub.
    await gh.git.setRemote(
      dir,
      `${github.url.replace('127.0.0.1', 'localhost')}/octo-maker/leak.git`,
    );
    const status = await gh.projectStatus(manifest);
    expect(status.github).toBeNull();
    const push = await attempt(() => gh.push(manifest));
    expect(push.ok ? null : push.error.code).toBe('auth');
    expect(github.git('octo-maker/leak', ['for-each-ref']).trim()).toBe('');
  });

  it('sees a file rewritten in the second of the commit', async () => {
    const gh = await signedIn();
    const manifest = makeProject('Racy');
    const dir = join(work, 'Racy');
    await gh.git.init(dir);
    writeFileSync(join(dir, 'notes.txt'), 'Order 10 boards\n');
    value(await attempt(() => gh.commit(manifest, 'First', undefined)));
    // Same size, same inode, right after the commit.
    writeFileSync(join(dir, 'notes.txt'), 'Order 12 boards\n');
    const status = await gh.projectStatus(manifest);
    expect(status.changes).toEqual([{ path: 'notes.txt', state: 'modified' }]);
    value(await attempt(() => gh.commit(manifest, 'Second', undefined)));
    expect((await gh.projectStatus(manifest)).changes).toEqual([]);
  });

  it('reports a folder without git', async () => {
    const gh = service();
    const manifest = makeProject('Plain');
    expect(await gh.projectStatus(manifest)).toEqual({
      repo: false,
      branch: null,
      remote: null,
      github: null,
      changes: [],
      ahead: null,
      behind: null,
    });
    const commit = await attempt(() => gh.commit(manifest, 'x', undefined));
    expect(commit.ok).toBe(false);
  });

  it('uses the author of the repository settings', async () => {
    const gh = service();
    const manifest = makeProject('Configured');
    const dir = join(work, 'Configured');
    await gh.git.init(dir);
    expect((await attempt(() => gh.commit(manifest, 'x', undefined))).ok).toBe(false);
    spawnSync('git', ['config', 'user.name', 'Bench User'], { cwd: dir });
    spawnSync('git', ['config', 'user.email', 'bench@example.com'], { cwd: dir });
    value(await attempt(() => gh.commit(manifest, 'Local only', undefined)));
    const log = await gh.log(manifest);
    expect(log[0]).toMatchObject({ message: 'Local only', author: 'Bench User' });
  });
});
