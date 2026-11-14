import { useEffect, useMemo, useState, type ReactNode } from 'react';

import {
  isBranchName,
  type GitBranches,
  type GitChange,
  type GitCommit,
  type GitErrorCode,
  type GitResult,
  type GitStatus,
} from '@shared/github';
import type { MessageKey, Translate } from '@shared/i18n';

import { Icon, type IconName } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { DEFAULT_REPO_CHOICE, publishTarget, RepoTarget, type RepoChoice } from './RepoTarget';
import {
  describeGitError,
  refreshProjectGit,
  setProjectGitStatus,
  signInOpen,
  useGithubStatus,
  useProjectGit,
} from './stores';

interface Problem {
  readonly text: string;
  readonly code: GitErrorCode;
  readonly files: readonly string[];
}

const CHANGE_LABEL: Readonly<Record<GitChange['state'], MessageKey>> = {
  added: 'github.panel.added',
  modified: 'github.panel.modified',
  deleted: 'github.panel.deleted',
};

function ActionButton({
  icon,
  label,
  title,
  disabled,
  primary = false,
  testId,
  onClick,
}: {
  readonly icon: IconName;
  readonly label: string;
  readonly title?: string;
  readonly disabled: boolean;
  readonly primary?: boolean;
  readonly testId: string;
  readonly onClick: () => void;
}): ReactNode {
  return (
    <button
      type="button"
      className={primary ? 'button button--primary' : 'button'}
      title={title ?? label}
      disabled={disabled}
      data-testid={testId}
      onClick={onClick}
    >
      <Icon name={icon} size={16} />
      {label}
    </button>
  );
}

/** The sync state with GitHub in words. */
function syncText(t: Translate, status: GitStatus): string {
  if (status.ahead === null) {
    return t('github.panel.noUpstream');
  }
  const parts: string[] = [];
  if (status.behind !== null && status.behind > 0) {
    parts.push(t('github.panel.behind', { count: status.behind }));
  }
  if (status.ahead > 0) {
    parts.push(t('github.panel.ahead', { count: status.ahead }));
  }
  return parts.length === 0 ? t('github.panel.upToDate') : parts.join(' · ');
}

/**
 * Source control of the open project: commit the saved changes, push and
 * pull, switch branches and look at the history. A project without git can
 * be put on GitHub here.
 */
export function SourceControl(): ReactNode {
  const { t, snapshot } = useAppState();
  const { project, save, notify, reloadProject } = useProject();
  const account = useGithubStatus()?.account ?? null;
  const git = useProjectGit();
  const manifestPath = project?.manifestPath ?? '';
  const status = git?.manifestPath === manifestPath ? git.status : null;
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<Problem | null>(null);
  const [message, setMessage] = useState('');
  const [excluded, setExcluded] = useState<ReadonlySet<string>>(() => new Set());
  const [log, setLog] = useState<readonly GitCommit[]>([]);
  const [branches, setBranches] = useState<GitBranches | null>(null);
  const [newBranch, setNewBranch] = useState<string | null>(null);
  const [choice, setChoice] = useState<RepoChoice>(DEFAULT_REPO_CHOICE);
  const [version, setVersion] = useState(0);

  const isRepo = status?.repo === true;
  const head = status?.branch ?? null;
  useEffect(() => {
    if (!isRepo || manifestPath === '') {
      return undefined;
    }
    let cancelled = false;
    void Promise.all([window.ew.git.log(manifestPath), window.ew.git.branches(manifestPath)]).then(
      ([history, list]) => {
        if (cancelled) {
          return;
        }
        if (history.ok) {
          setLog(history.value);
        }
        if (list.ok) {
          setBranches(list.value);
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [isRepo, manifestPath, head, version, status?.ahead, status?.behind]);

  const changes = useMemo(() => status?.changes ?? [], [status]);
  const chosen = changes.filter((change) => !excluded.has(change.path));
  const dateFormat = new Intl.DateTimeFormat(snapshot.language, {
    dateStyle: 'medium',
    timeStyle: 'short',
  });

  if (project === null) {
    return null;
  }
  const dirty = project.history.dirty;
  const onGithub = status?.github ?? null;

  /** Runs a git action; failures are shown in the panel. */
  const act = async <T,>(
    action: () => Promise<GitResult<T>>,
    done: (value: T) => Promise<void> | void,
  ): Promise<boolean> => {
    setBusy(true);
    setProblem(null);
    try {
      const result = await action();
      if (!result.ok) {
        setProblem({
          text: describeGitError(t, result.error),
          code: result.error.code,
          files: result.error.files ?? [],
        });
        await refreshProjectGit(manifestPath);
        return false;
      }
      await done(result.value);
      setVersion((current) => current + 1);
      return true;
    } finally {
      setBusy(false);
    }
  };

  const update = (next: GitStatus): void => {
    setProjectGitStatus(manifestPath, next);
  };

  const commit = async (andPush: boolean): Promise<void> => {
    const text = message.trim();
    if (text === '' || chosen.length === 0) {
      return;
    }
    const paths = excluded.size === 0 ? undefined : chosen.map((change) => change.path);
    const committed = await act(
      () => window.ew.git.commit(manifestPath, text, paths),
      (next) => {
        update(next);
        setMessage('');
        setExcluded(new Set());
        notify(t('github.panel.committed', { message: text.split('\n')[0] ?? text }));
      },
    );
    if (committed && andPush) {
      await push();
    }
  };

  const push = (): Promise<boolean> =>
    act(
      () => window.ew.git.push(manifestPath),
      (next) => {
        update(next);
        notify(t('github.panel.pushed'));
      },
    );

  const pull = (): Promise<boolean> =>
    act(
      () => window.ew.git.pull(manifestPath),
      async (result) => {
        if (result.changed) {
          await reloadProject();
        }
        await refreshProjectGit(manifestPath);
        notify(t(result.changed ? 'github.panel.pulled' : 'github.panel.alreadyUpToDate'));
      },
    );

  const checkout = (branch: string, create: boolean): Promise<boolean> =>
    act(
      () => window.ew.git.checkout(manifestPath, branch, create),
      async (next) => {
        if (!create) {
          await reloadProject();
        }
        update(next);
        notify(t(create ? 'github.panel.created' : 'github.panel.switched', { branch }));
      },
    );

  if (status === null) {
    const failure = git?.manifestPath === manifestPath ? git.error : null;
    return (
      <div className="details__body source-control" data-testid="source-control">
        <h2 className="source-control__title">{t('github.panel.title')}</h2>
        {failure === null ? (
          <p className="field__hint">{t('github.panel.working')}</p>
        ) : (
          <div className="callout callout--warning" role="alert" data-testid="git-problem">
            <Icon name="warning" />
            <div>
              <p>{describeGitError(t, failure)}</p>
              <button
                type="button"
                className="button button--small"
                onClick={() => {
                  void refreshProjectGit(manifestPath);
                }}
              >
                {t('github.panel.retry')}
              </button>
            </div>
          </div>
        )}
      </div>
    );
  }

  if (!status.repo) {
    const target = publishTarget(choice, project.name);
    return (
      <div className="details__body source-control" data-testid="source-control">
        <h2 className="source-control__title">
          <Icon name="branch" size={20} />
          {t('github.panel.title')}
        </h2>
        <p className="dialog__text">{t('github.panel.notRepo')}</p>
        <p className="field__hint">{t('github.panel.notRepoHint')}</p>
        <section className="panel" aria-label={t('github.publish.title')}>
          <h3 className="panel__title source-control__heading">{t('github.publish.title')}</h3>
          <RepoTarget choice={choice} projectName={project.name} onChange={setChoice} />
          {dirty && <SaveHint onSave={save} />}
          {account !== null && (
            <div>
              <ActionButton
                icon="upload"
                label={busy ? t('github.publish.working') : t('github.publish.action')}
                primary
                disabled={busy || target === null || dirty}
                testId="git-publish"
                onClick={() => {
                  if (target === null) {
                    return;
                  }
                  void act(
                    () => window.ew.git.publish(manifestPath, target),
                    (next) => {
                      update(next);
                      notify(t('github.publish.done', { repo: next.github ?? '' }));
                    },
                  );
                }}
              />
            </div>
          )}
        </section>
        <ProblemNote problem={problem} />
      </div>
    );
  }

  const canSync = status.remote !== null && account !== null && !busy;
  const branchNameValid = newBranch !== null && isBranchName(newBranch.trim());

  return (
    <div className="details__body source-control" data-testid="source-control">
      <h2 className="source-control__title">
        <Icon name="branch" size={20} />
        {t('github.panel.title')}
      </h2>

      <div className="source-control__bar">
        <span className="source-control__remote" data-testid="git-remote">
          <Icon name="github" size={16} />
          {onGithub ?? status.remote ?? t('github.panel.localRepo')}
        </span>
        {onGithub !== null && (
          <button
            type="button"
            className="button button--small"
            data-testid="git-open-github"
            onClick={() => {
              window.ew.github.openPage('repo', onGithub);
            }}
          >
            {t('github.panel.openOnGithub')}
          </button>
        )}
      </div>

      <div className="source-control__bar">
        <label className="field source-control__branch">
          <span className="field__label">{t('github.panel.branch')}</span>
          <select
            className="field__control"
            value={head ?? ''}
            disabled={busy || dirty}
            data-testid="git-branch"
            onChange={(event) => {
              const value = event.target.value;
              if (value === '::new') {
                setNewBranch('');
              } else if (value !== head) {
                void checkout(value, false);
              }
            }}
          >
            {(branches?.local ?? (head === null ? [] : [head])).map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
            {(branches?.remote ?? []).map((name) => (
              <option key={`remote:${name}`} value={name}>
                {t('github.panel.remoteBranch', { branch: name })}
              </option>
            ))}
            <option value="::new">{t('github.panel.newBranch')}</option>
          </select>
        </label>
        <span className="source-control__sync" data-testid="git-sync">
          {status.remote === null ? '' : syncText(t, status)}
        </span>
      </div>

      {newBranch !== null && (
        <div className="source-control__new-branch" data-testid="git-new-branch">
          <label className="field">
            <span className="field__label">{t('github.panel.newBranchName')}</span>
            <div className="field__row">
              <input
                className="field__control field__control--grow"
                type="text"
                value={newBranch}
                spellCheck={false}
                autoFocus
                aria-invalid={newBranch !== '' && !branchNameValid}
                data-testid="git-new-branch-name"
                onChange={(event) => {
                  setNewBranch(event.target.value);
                }}
              />
              <button
                type="button"
                className="button button--primary"
                disabled={!branchNameValid || busy}
                data-testid="git-new-branch-create"
                onClick={() => {
                  const name = newBranch.trim();
                  void checkout(name, true).then((ok) => {
                    if (ok) {
                      setNewBranch(null);
                    }
                  });
                }}
              >
                {t('github.panel.createBranch')}
              </button>
              <button
                type="button"
                className="button"
                onClick={() => {
                  setNewBranch(null);
                }}
              >
                {t('wizard.cancel')}
              </button>
            </div>
            {newBranch !== '' && !branchNameValid ? (
              <span className="field__error">{t('github.panel.invalidBranch')}</span>
            ) : (
              <span className="field__hint">{t('github.panel.newBranchHint')}</span>
            )}
          </label>
        </div>
      )}

      {status.remote !== null && (
        <div className="source-control__actions">
          <ActionButton
            icon="sync"
            label={t('github.panel.fetch')}
            title={t('github.panel.fetchTip')}
            disabled={!canSync}
            testId="git-fetch"
            onClick={() => {
              void act(() => window.ew.git.fetch(manifestPath), update);
            }}
          />
          <ActionButton
            icon="download"
            label={t('github.panel.pull')}
            title={t('github.panel.pullTip')}
            disabled={!canSync || dirty || changes.length > 0}
            testId="git-pull"
            onClick={() => {
              void pull();
            }}
          />
          <ActionButton
            icon="upload"
            label={t('github.panel.push')}
            title={t('github.panel.pushTip')}
            disabled={!canSync || status.ahead === 0}
            testId="git-push"
            onClick={() => {
              void push();
            }}
          />
        </div>
      )}
      {account === null && status.remote !== null && (
        <p className="field__hint">
          {t('github.panel.signInHint')}{' '}
          <button
            type="button"
            className="link-button"
            onClick={() => {
              signInOpen.set(true);
            }}
          >
            {t('github.account.signIn')}
          </button>
        </p>
      )}

      <ProblemNote problem={problem} />
      {dirty && <SaveHint onSave={save} />}

      <section className="panel" aria-label={t('github.panel.changes')}>
        <h3 className="panel__title source-control__heading">
          {t('github.panel.changes')}
          {changes.length > 0 && (
            <span className="pill" data-testid="git-change-count">
              {changes.length}
            </span>
          )}
        </h3>
        {changes.length === 0 ? (
          <p className="field__hint" data-testid="git-no-changes">
            {t('github.panel.noChanges')}
          </p>
        ) : (
          <>
            <ul className="source-control__changes" data-testid="git-changes">
              {changes.map((change) => (
                <li key={change.path}>
                  <label className="checkbox">
                    <input
                      type="checkbox"
                      checked={!excluded.has(change.path)}
                      data-testid="git-change"
                      data-path={change.path}
                      onChange={(event) => {
                        const next = new Set(excluded);
                        if (event.target.checked) {
                          next.delete(change.path);
                        } else {
                          next.add(change.path);
                        }
                        setExcluded(next);
                      }}
                    />
                    <span className="source-control__path">{change.path}</span>
                    <span
                      className={`source-control__state source-control__state--${change.state}`}
                    >
                      {t(CHANGE_LABEL[change.state])}
                    </span>
                  </label>
                </li>
              ))}
            </ul>
            <label className="field">
              <span className="field__label">{t('github.panel.message')}</span>
              <textarea
                className="field__control source-control__message"
                rows={3}
                value={message}
                placeholder={t('github.panel.messagePlaceholder')}
                data-testid="git-message"
                onChange={(event) => {
                  setMessage(event.target.value);
                }}
              />
            </label>
            <div className="source-control__actions">
              <ActionButton
                icon="commit"
                label={t('github.panel.commit')}
                primary={status.remote === null}
                disabled={busy || message.trim() === '' || chosen.length === 0}
                testId="git-commit"
                onClick={() => {
                  void commit(false);
                }}
              />
              {status.remote !== null && (
                <ActionButton
                  icon="upload"
                  label={t('github.panel.commitPush')}
                  primary
                  disabled={
                    busy || message.trim() === '' || chosen.length === 0 || account === null
                  }
                  testId="git-commit-push"
                  onClick={() => {
                    void commit(true);
                  }}
                />
              )}
            </div>
          </>
        )}
      </section>

      <section className="panel" aria-label={t('github.panel.history')}>
        <h3 className="panel__title source-control__heading">
          <Icon name="history" size={16} />
          {t('github.panel.history')}
        </h3>
        {log.length === 0 ? (
          <p className="field__hint">{t('github.panel.noHistory')}</p>
        ) : (
          <ol className="source-control__log" data-testid="git-log">
            {log.map((entry) => (
              <li key={entry.oid} className="source-control__commit">
                <span className="source-control__subject">{entry.message.split('\n')[0]}</span>
                <span className="source-control__meta">
                  {entry.author} · {dateFormat.format(new Date(entry.time))} ·{' '}
                  <code>{entry.oid.slice(0, 7)}</code>
                </span>
              </li>
            ))}
          </ol>
        )}
      </section>
    </div>
  );
}

function SaveHint({ onSave }: { readonly onSave: () => Promise<boolean> }): ReactNode {
  const { t } = useAppState();
  return (
    <div
      className="callout callout--info source-control__save"
      role="status"
      data-testid="git-save-first"
    >
      <Icon name="info" />
      <span>{t('github.panel.saveFirst')}</span>
      <button
        type="button"
        className="button button--small"
        onClick={() => {
          void onSave();
        }}
      >
        {t('github.panel.save')}
      </button>
    </div>
  );
}

function ProblemNote({ problem }: { readonly problem: Problem | null }): ReactNode {
  const { t } = useAppState();
  if (problem === null) {
    return null;
  }
  const filesLabel =
    problem.code === 'conflict'
      ? t('github.panel.conflictFiles')
      : problem.code === 'uncommitted'
        ? t('github.panel.uncommittedFiles')
        : null;
  return (
    <div
      className="callout callout--warning source-control__problem"
      role="alert"
      data-testid="git-problem"
    >
      <Icon name="warning" />
      <div>
        <p>{problem.text}</p>
        {filesLabel !== null && problem.files.length > 0 && (
          <>
            <p>{filesLabel}</p>
            <ul>
              {problem.files.map((file) => (
                <li key={file}>
                  <code>{file}</code>
                </li>
              ))}
            </ul>
          </>
        )}
        {problem.code === 'conflict' && <p>{t('github.panel.conflictHint')}</p>}
      </div>
    </div>
  );
}
