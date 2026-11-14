import { useEffect, useId, useMemo, useState, type ReactNode } from 'react';

import type { GithubRepo } from '@shared/github';
import { joinPath } from '@shared/project';

import { normalizeSearchText } from '../commands';
import { Icon } from '../components/Icon';
import { Modal } from '../components/Modal';
import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { describeGitError, signInOpen, useGithubStatus } from './stores';

/** "Open from GitHub": clones a repository and opens its project. */
export function CloneDialog({ onClose }: { readonly onClose: () => void }): ReactNode {
  const { t, appInfo, snapshot } = useAppState();
  const { openProject } = useProject();
  const formId = useId();
  const status = useGithubStatus();
  const login = status?.account?.login ?? null;
  const [repos, setRepos] = useState<readonly GithubRepo[] | null>(null);
  const [filter, setFilter] = useState('');
  const [selected, setSelected] = useState<string | null>(null);
  const [location, setLocation] = useState('');
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([window.ew.projects.recent(), window.ew.projects.defaultLocation()]).then(
      ([recent, fallback]) => {
        if (!cancelled) {
          setLocation((current) => (current === '' ? (recent.lastLocation ?? fallback) : current));
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (login === null) {
      return undefined;
    }
    let cancelled = false;
    void window.ew.github.repos().then((result) => {
      if (cancelled) {
        return;
      }
      if (result.ok) {
        setRepos(result.value.filter((repo) => !repo.empty));
      } else {
        setError(describeGitError(t, result.error));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [login, t]);

  const shown = useMemo(() => {
    if (repos === null) {
      return [];
    }
    const words = normalizeSearchText(filter).split(/\s+/u).filter(Boolean);
    return repos.filter((repo) => {
      const text = normalizeSearchText(`${repo.fullName} ${repo.description}`);
      return words.every((word) => text.includes(word));
    });
  }, [repos, filter]);
  const repo = repos?.find((entry) => entry.fullName === selected) ?? null;
  const dateFormat = new Intl.DateTimeFormat(snapshot.language, { dateStyle: 'medium' });

  const clone = (): void => {
    if (repo === null || location.trim() === '') {
      return;
    }
    setWorking(true);
    setError(null);
    void window.ew.git.clone(repo.fullName, location.trim()).then(async (result) => {
      if (!result.ok) {
        setWorking(false);
        setError(
          result.error.code === 'notProject'
            ? t('github.clone.notProject')
            : describeGitError(t, result.error),
        );
        return;
      }
      void window.ew.projects.setLastLocation(location.trim());
      onClose();
      await openProject(result.value);
    });
  };

  return (
    <Modal
      title={t('github.clone.title')}
      onClose={onClose}
      testId="github-clone"
      className="dialog--wide"
      footer={
        <>
          <button type="button" className="button dialog__footer-start" onClick={onClose}>
            {t('wizard.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={repo === null || location.trim() === '' || working}
            data-testid="github-clone-open"
            onClick={clone}
          >
            {working
              ? t('github.clone.working', { repo: repo?.name ?? '' })
              : t('github.clone.open')}
          </button>
        </>
      }
    >
      <p className="dialog__text">{t('github.clone.intro')}</p>
      {login === null ? (
        <div className="github-target">
          <p className="field__hint">{t('github.errors.signedOut')}</p>
          <div>
            <button
              type="button"
              className="button button--primary"
              data-testid="github-clone-sign-in"
              onClick={() => {
                signInOpen.set(true);
              }}
            >
              <Icon name="github" size={16} />
              {t('github.account.signIn')}
            </button>
          </div>
        </div>
      ) : (
        <>
          <label className="field">
            <span className="field__label">{t('github.clone.filter')}</span>
            <input
              className="field__control"
              type="search"
              value={filter}
              placeholder={t('github.clone.filterPlaceholder')}
              data-testid="github-clone-filter"
              onChange={(event) => {
                setFilter(event.target.value);
              }}
            />
          </label>
          {repos === null && error === null && (
            <p className="field__hint">{t('github.clone.loading')}</p>
          )}
          {repos !== null && shown.length === 0 && (
            <p className="field__hint">{t('github.clone.noRepos')}</p>
          )}
          {shown.length > 0 && (
            <ul className="github-repos" role="listbox" data-testid="github-clone-list">
              {shown.map((entry) => (
                <li key={entry.fullName}>
                  <button
                    type="button"
                    role="option"
                    className="github-repos__item"
                    aria-selected={entry.fullName === selected}
                    data-testid="github-clone-repo"
                    data-repo={entry.fullName}
                    onClick={() => {
                      setSelected(entry.fullName);
                    }}
                  >
                    <Icon name="github" size={18} />
                    <span className="github-repos__text">
                      <span className="github-repos__name">{entry.fullName}</span>
                      {entry.description !== '' && (
                        <span className="github-repos__description">{entry.description}</span>
                      )}
                    </span>
                    {entry.private && (
                      <span className="pill pill--muted">{t('github.clone.private')}</span>
                    )}
                    {entry.updatedAt !== '' && (
                      <span className="github-repos__date">
                        {t('github.clone.updated', {
                          date: dateFormat.format(new Date(entry.updatedAt)),
                        })}
                      </span>
                    )}
                  </button>
                </li>
              ))}
            </ul>
          )}
          <div className="field">
            <label className="field__label" htmlFor={`${formId}-location`}>
              {t('github.clone.location')}
            </label>
            <div className="field__row">
              <input
                id={`${formId}-location`}
                className="field__control field__control--grow"
                type="text"
                value={location}
                data-testid="github-clone-location"
                onChange={(event) => {
                  setLocation(event.target.value);
                }}
              />
              <button
                type="button"
                className="button"
                onClick={() => {
                  void window.ew.dialogs.chooseFolder(location, 'location').then((chosen) => {
                    if (chosen !== null) {
                      setLocation(chosen);
                    }
                  });
                }}
              >
                {t('wizard.choose')}
              </button>
            </div>
            {repo !== null && (
              <span className="field__hint" data-testid="github-clone-folder">
                {t('github.clone.folderPreview', {
                  path: joinPath(location.trim(), repo.name, appInfo.platform),
                })}
              </span>
            )}
          </div>
        </>
      )}
      {error !== null && (
        <p className="field__error" role="alert" data-testid="github-clone-error">
          {error}
        </p>
      )}
    </Modal>
  );
}
