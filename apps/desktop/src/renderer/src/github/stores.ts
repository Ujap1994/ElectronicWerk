/**
 * GitHub state of the UI: the account, the git state of the open project
 * and which GitHub dialog is open.
 */
import { useEffect, useSyncExternalStore } from 'react';

import type { GitError, GitErrorCode, GithubStatus, GitStatus } from '@shared/github';
import type { MessageKey, Translate } from '@shared/i18n';

import { Store } from '../schematic/stores';

/** The GitHub account (`null` until known). */
export const githubStatus = new Store<GithubStatus | null>(null);

/** The sign-in dialog is open. */
export const signInOpen = new Store<boolean>(false);

/** The "Open from GitHub" dialog is open. */
export const cloneOpen = new Store<boolean>(false);

/** Shows the source control view of the open project (set by the workspace). */
export const sourceControlCommand = new Store<(() => void) | null>(null);

/** Git state of the open project. */
export interface ProjectGit {
  readonly manifestPath: string;
  readonly status: GitStatus | null;
  /** Why the state could not be read (`null`: read, or still reading). */
  readonly error: GitError | null;
}

export const projectGit = new Store<ProjectGit | null>(null);

let loading: Promise<void> | null = null;

/** Reads the account from the main process (once; again with `force`). */
export function loadGithubStatus(force = false): Promise<void> {
  if (loading !== null && !force) {
    return loading;
  }
  loading = window.ew.github.status().then((status) => {
    githubStatus.set(status);
  });
  return loading;
}

export function useGithubStatus(): GithubStatus | null {
  const status = useSyncExternalStore(githubStatus.subscribe, githubStatus.get);
  useEffect(() => {
    void loadGithubStatus();
  }, []);
  return status;
}

export function useProjectGit(): ProjectGit | null {
  return useSyncExternalStore(projectGit.subscribe, projectGit.get);
}

let gitRequest = 0;

/** Reads the git state of a project (newest request wins). */
export async function refreshProjectGit(manifestPath: string): Promise<void> {
  const request = ++gitRequest;
  const result = await window.ew.git.status(manifestPath);
  if (request !== gitRequest) {
    return;
  }
  if (result.ok) {
    projectGit.set({ manifestPath, status: result.value, error: null });
  } else {
    const current = projectGit.get();
    // A known state stays; the error is shown with it.
    projectGit.set({
      manifestPath,
      status: current?.manifestPath === manifestPath ? current.status : null,
      error: result.error,
    });
  }
}

export function setProjectGitStatus(manifestPath: string, status: GitStatus): void {
  gitRequest += 1;
  projectGit.set({ manifestPath, status, error: null });
}

const ERROR_TEXT: Readonly<Record<GitErrorCode, MessageKey>> = {
  signedOut: 'github.errors.signedOut',
  auth: 'github.errors.auth',
  network: 'github.errors.network',
  conflict: 'github.errors.conflict',
  rejected: 'github.errors.rejected',
  uncommitted: 'github.errors.uncommitted',
  notEmpty: 'github.errors.notEmpty',
  exists: 'github.errors.exists',
  noRemote: 'github.errors.noRemote',
  noAuthor: 'github.errors.noAuthor',
  notProject: 'github.errors.notProject',
  notFound: 'github.errors.notFound',
  failed: 'github.errors.failed',
};

/** Explains a failed GitHub action (with details for unexpected failures). */
export function describeGitError(t: Translate, error: GitError): string {
  const text = t(ERROR_TEXT[error.code]);
  return error.code === 'failed' || error.code === 'network'
    ? `${text} ${t('github.errors.details', { message: error.message })}`
    : text;
}
