import type { ReactNode } from 'react';

import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { githubStatus, signInOpen, useGithubStatus } from './stores';

/** The GitHub section of the settings: account, sign-in and sign-out. */
export function GithubSettings(): ReactNode {
  const { t } = useAppState();
  const { notify } = useProject();
  const status = useGithubStatus();
  const account = status?.account ?? null;

  return (
    <fieldset className="field field--group" data-testid="settings-github">
      <legend className="field__label">{t('github.title')}</legend>
      <div className="field__row github-account">
        <Icon name="github" size={22} />
        <span className="github-account__name" data-testid="github-account">
          {account === null ? (
            t('github.account.signedOut')
          ) : (
            <>
              <strong>{t('github.account.signedInAs', { login: account.login })}</strong>
              {account.name !== null && <span className="field__hint">{account.name}</span>}
            </>
          )}
        </span>
        {account === null ? (
          <button
            type="button"
            className="button button--small"
            data-testid="github-sign-in-open"
            onClick={() => {
              signInOpen.set(true);
            }}
          >
            {t('github.account.signIn')}
          </button>
        ) : (
          <button
            type="button"
            className="button button--small"
            title={t('github.account.signOutTip')}
            data-testid="github-sign-out"
            onClick={() => {
              void window.ew.github.signOut().then(() => {
                const current = githubStatus.get();
                if (current !== null) {
                  githubStatus.set({ ...current, account: null });
                }
                notify(t('github.account.signedOutNotice'));
              });
            }}
          >
            {t('github.account.signOut')}
          </button>
        )}
      </div>
      {status !== null && (
        <span
          className={status.storage === 'keychain' ? 'field__hint' : 'field__error'}
          data-testid="github-storage"
        >
          {status.storage === 'keychain'
            ? t('github.account.storageKeychain')
            : status.storage === 'plain'
              ? t('github.account.storagePlain')
              : t('github.account.storageUnavailable')}
        </span>
      )}
    </fieldset>
  );
}
