import { useEffect, useId, useState, type ReactNode } from 'react';

import { repoName, type GithubRepo, type PublishTarget } from '@shared/github';

import { Icon } from '../components/Icon';
import { useAppState } from '../state';
import { describeGitError, signInOpen, useGithubStatus } from './stores';

/** The fields of a GitHub repository choice. */
export interface RepoChoice {
  readonly kind: 'new' | 'existing';
  /** Name of a new repository (`null`: derived from the project name). */
  readonly name: string | null;
  readonly private: boolean;
  readonly description: string;
  /** `owner/name` of the existing repository. */
  readonly existing: string | null;
}

export const DEFAULT_REPO_CHOICE: RepoChoice = {
  kind: 'new',
  name: null,
  private: true,
  description: '',
  existing: null,
};

/** The repository to publish to, or `null` while the choice is incomplete. */
export function publishTarget(choice: RepoChoice, projectName: string): PublishTarget | null {
  if (choice.kind === 'existing') {
    return choice.existing === null ? null : { kind: 'existing', fullName: choice.existing };
  }
  const name = choice.name ?? repoName(projectName);
  if (name === '' || repoName(name) !== name) {
    return null;
  }
  return {
    kind: 'new',
    repo: { name, private: choice.private, description: choice.description.trim() },
  };
}

/**
 * Chooses where a project goes on GitHub: a new repository of the account
 * or an existing empty one. Asks to sign in first.
 */
export function RepoTarget({
  choice,
  projectName,
  onChange,
}: {
  readonly choice: RepoChoice;
  readonly projectName: string;
  readonly onChange: (choice: RepoChoice) => void;
}): ReactNode {
  const { t } = useAppState();
  const id = useId();
  const status = useGithubStatus();
  const account = status?.account ?? null;
  const [repos, setRepos] = useState<readonly GithubRepo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const login = account?.login ?? null;

  useEffect(() => {
    if (login === null || choice.kind !== 'existing') {
      return undefined;
    }
    let cancelled = false;
    void window.ew.github.repos().then((result) => {
      if (cancelled) {
        return;
      }
      if (result.ok) {
        setRepos(result.value.filter((repo) => repo.empty));
        setError(null);
      } else {
        setError(describeGitError(t, result.error));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [login, choice.kind, t]);

  if (account === null) {
    return (
      <div className="github-target" data-testid="repo-target">
        <p className="field__hint">{t('github.target.signInFirst')}</p>
        <div>
          <button
            type="button"
            className="button"
            data-testid="repo-target-sign-in"
            onClick={() => {
              signInOpen.set(true);
            }}
          >
            <Icon name="github" size={16} />
            {t('github.account.signIn')}
          </button>
        </div>
      </div>
    );
  }

  const name = choice.name ?? repoName(projectName);
  const nameInvalid = name !== '' && repoName(name) !== name;
  return (
    <div className="github-target" data-testid="repo-target">
      <span className="field__hint" data-testid="repo-target-account">
        {t('github.target.account', { login: account.login })}
      </span>
      <div className="choice-grid choice-grid--two" role="radiogroup">
        <label
          className="choice"
          data-selected={choice.kind === 'new'}
          data-testid="repo-target-new"
        >
          <input
            type="radio"
            name={`${id}-kind`}
            checked={choice.kind === 'new'}
            onChange={() => {
              onChange({ ...choice, kind: 'new' });
            }}
          />
          <Icon name="plus" size={20} />
          <span className="choice__title">{t('github.target.newRepo')}</span>
          <span className="choice__hint">{t('github.target.newRepoHint')}</span>
        </label>
        <label
          className="choice"
          data-selected={choice.kind === 'existing'}
          data-testid="repo-target-existing"
        >
          <input
            type="radio"
            name={`${id}-kind`}
            checked={choice.kind === 'existing'}
            onChange={() => {
              onChange({ ...choice, kind: 'existing' });
            }}
          />
          <Icon name="github" size={20} />
          <span className="choice__title">{t('github.target.existing')}</span>
          <span className="choice__hint">{t('github.target.existingHint')}</span>
        </label>
      </div>
      {choice.kind === 'new' ? (
        <>
          <label className="field">
            <span className="field__label">{t('github.target.name')}</span>
            <div className="field__row">
              <span className="github-target__owner">{account.login} /</span>
              <input
                className="field__control field__control--grow"
                type="text"
                value={name}
                spellCheck={false}
                aria-invalid={nameInvalid}
                data-testid="repo-target-name"
                onChange={(event) => {
                  onChange({ ...choice, name: event.target.value });
                }}
              />
            </div>
            {nameInvalid && <span className="field__error">{t('github.target.invalidName')}</span>}
          </label>
          <label className="field">
            <span className="field__label">{t('github.target.description')}</span>
            <input
              className="field__control"
              type="text"
              maxLength={350}
              value={choice.description}
              data-testid="repo-target-description"
              onChange={(event) => {
                onChange({ ...choice, description: event.target.value });
              }}
            />
          </label>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={choice.private}
              data-testid="repo-target-private"
              onChange={(event) => {
                onChange({ ...choice, private: event.target.checked });
              }}
            />
            <span>{t('github.target.private')}</span>
          </label>
        </>
      ) : (
        <label className="field">
          <span className="field__label">{t('github.target.choose')}</span>
          {error !== null ? (
            <span className="field__error">{error}</span>
          ) : repos === null ? (
            <span className="field__hint">{t('github.target.loading')}</span>
          ) : repos.length === 0 ? (
            <span className="field__hint" data-testid="repo-target-none">
              {t('github.target.noEmpty')}
            </span>
          ) : (
            <select
              className="field__control"
              value={choice.existing ?? ''}
              data-testid="repo-target-select"
              onChange={(event) => {
                onChange({
                  ...choice,
                  existing: event.target.value === '' ? null : event.target.value,
                });
              }}
            >
              <option value="">—</option>
              {repos.map((repo) => (
                <option key={repo.fullName} value={repo.fullName}>
                  {repo.fullName}
                  {repo.private ? ` (${t('github.clone.private')})` : ''}
                </option>
              ))}
            </select>
          )}
        </label>
      )}
    </div>
  );
}
