import { describe, expect, it } from 'vitest';

import { githubName, isBranchName, isFullName, isPublishTarget, repoName } from './github';

describe('GitHub names', () => {
  it('reads owner/name of remote URLs', () => {
    expect(githubName('https://github.com/octo/blinky.git')).toBe('octo/blinky');
    expect(githubName('https://github.com/octo/blinky')).toBe('octo/blinky');
    expect(githubName('https://x-access-token@github.com/octo/my.board.git')).toBe('octo/my.board');
    expect(githubName('git@github.com:octo/blinky.git')).toBe('octo/blinky');
    expect(githubName('https://gitlab.com/octo/blinky.git')).toBeNull();
    expect(githubName('https://github.com.evil.example/octo/blinky.git')).toBeNull();
    expect(githubName('https://github.com/octo/blinky/tree/main')).toBeNull();
    // Tests use a local GitHub.
    expect(githubName('http://127.0.0.1:9/octo/x.git', 'http://127.0.0.1:9')).toBe('octo/x');
    expect(githubName('git@github.com:octo/x.git', 'http://127.0.0.1:9')).toBeNull();
  });

  it('makes valid repository and branch names', () => {
    expect(repoName('  My Shield (Rev B)  ')).toBe('My-Shield-Rev-B');
    expect(repoName('Lötstation')).toBe('L-tstation');
    expect(repoName('---')).toBe('');
    expect(isBranchName('rev-b')).toBe(true);
    expect(isBranchName('feature/new-psu')).toBe(true);
    for (const bad of ['', 'a b', 'a..b', '-x', 'x/', '/x', 'x.lock', 'a~b', 'a:b', 'x.', 'a@{b']) {
      expect(isBranchName(bad), bad).toBe(false);
    }
  });

  it('checks values from the UI', () => {
    expect(isFullName('octo/blinky')).toBe(true);
    expect(isFullName('octo/../x')).toBe(false);
    expect(isFullName('octo/..')).toBe(false);
    expect(isFullName('octo')).toBe(false);
    expect(isPublishTarget({ kind: 'existing', fullName: 'octo/blinky' })).toBe(true);
    expect(
      isPublishTarget({ kind: 'new', repo: { name: 'blinky', private: true, description: '' } }),
    ).toBe(true);
    expect(
      isPublishTarget({ kind: 'new', repo: { name: 'a b', private: true, description: '' } }),
    ).toBe(false);
    expect(
      isPublishTarget({ kind: 'new', repo: { name: 'x', private: 'yes', description: '' } }),
    ).toBe(false);
    expect(isPublishTarget({ kind: 'existing', fullName: '../../etc' })).toBe(false);
    expect(isPublishTarget(null)).toBe(false);
  });
});
