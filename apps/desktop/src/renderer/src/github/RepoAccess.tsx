import type { ReactNode } from 'react';

import { Icon } from '../components/Icon';
import { useAppState } from '../state';

/**
 * Signed in with the GitHub App: the repositories it may use are chosen on
 * GitHub. Opens that page and reads the list again afterwards.
 */
export function RepoAccess({
  onRefresh,
  newRepo,
}: {
  readonly onRefresh: () => void;
  /** Offers to create a repository on GitHub with this name. */
  readonly newRepo?: string;
}): ReactNode {
  const { t } = useAppState();
  return (
    <div className="field__row github-access" data-testid="repo-access">
      {newRepo !== undefined && (
        <button
          type="button"
          className="button button--small"
          data-testid="repo-access-new"
          onClick={() => {
            window.ew.github.openPage('newRepo', newRepo);
          }}
        >
          <Icon name="plus" size={16} />
          {t('github.target.createOnGithub')}
        </button>
      )}
      <button
        type="button"
        className="button button--small"
        title={t('github.account.chooseReposTip')}
        data-testid="repo-access-choose"
        onClick={() => {
          window.ew.github.openPage('install');
        }}
      >
        <Icon name="github" size={16} />
        {t('github.account.chooseRepos')}
      </button>
      <button
        type="button"
        className="button button--small"
        data-testid="repo-access-refresh"
        onClick={onRefresh}
      >
        {t('github.target.refresh')}
      </button>
    </div>
  );
}
