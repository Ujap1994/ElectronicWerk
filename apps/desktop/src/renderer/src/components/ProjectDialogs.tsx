import type { ReactNode } from 'react';

import { issueMessage } from '../project/labels';
import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { ConfirmDialog } from './ConfirmDialog';
import { Icon } from './Icon';
import { Modal } from './Modal';
import { NewProjectWizard } from './NewProjectWizard';

function CloseDialog(): ReactNode {
  const { t } = useAppState();
  const { prompts } = useProject();
  const prompt = prompts.close;
  if (prompt === null) {
    return null;
  }
  return (
    <Modal
      title={t('closePrompt.title')}
      testId="close-dialog"
      onClose={() => {
        prompt.answer('cancel');
      }}
      footer={
        <>
          <button
            type="button"
            className="button dialog__footer-start"
            data-testid="close-dont-save"
            onClick={() => {
              prompt.answer('discard');
            }}
          >
            {t('closePrompt.dontSave')}
          </button>
          <button
            type="button"
            className="button"
            data-testid="close-cancel"
            onClick={() => {
              prompt.answer('cancel');
            }}
          >
            {t('closePrompt.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            data-testid="close-save"
            onClick={() => {
              prompt.answer('save');
            }}
          >
            {t('closePrompt.save')}
          </button>
        </>
      }
    >
      <p className="dialog__text">{t('closePrompt.message', { name: prompt.name })}</p>
      <p className="dialog__text dialog__text--muted">{t('closePrompt.hint')}</p>
    </Modal>
  );
}

function RecoveryDialog(): ReactNode {
  const { t, snapshot: settings } = useAppState();
  const { prompts, answerRecovery } = useProject();
  const project = prompts.recovery;
  const recovery = project?.recovery ?? null;
  if (project === null || recovery === null) {
    return null;
  }
  const time =
    recovery.modified === null
      ? t('recovery.lastSession')
      : new Date(recovery.modified).toLocaleString(settings.language);
  return (
    <Modal
      title={t('recovery.title')}
      testId="recovery-dialog"
      onClose={() => {
        void answerRecovery(false);
      }}
      footer={
        <>
          <button
            type="button"
            className="button"
            data-testid="recovery-discard"
            onClick={() => {
              void answerRecovery(false);
            }}
          >
            {t('recovery.discard')}
          </button>
          <button
            type="button"
            className="button button--primary"
            data-testid="recovery-restore"
            onClick={() => {
              void answerRecovery(true);
            }}
          >
            {t('recovery.restore')}
          </button>
        </>
      }
    >
      <p className="dialog__text">
        {t('recovery.message', { name: project.name, count: recovery.commits, time })}
      </p>
      {!recovery.matchesFiles && (
        <p className="callout callout--warning">
          <Icon name="warning" />
          <span>{t('recovery.mismatch')}</span>
        </p>
      )}
      <p className="dialog__text dialog__text--muted">{t('recovery.discardHint')}</p>
    </Modal>
  );
}

function ValidationDialog(): ReactNode {
  const { t } = useAppState();
  const { prompts, closeValidation } = useProject();
  const report = prompts.validation;
  if (report === null) {
    return null;
  }
  const severityLabel = {
    error: t('validation.error'),
    warning: t('validation.warning'),
    info: t('validation.info'),
  } as const;
  return (
    <Modal
      title={t('validation.title')}
      testId="validation-dialog"
      className="dialog--wide"
      onClose={closeValidation}
      footer={
        <button type="button" className="button button--primary" onClick={closeValidation}>
          {t('validation.close')}
        </button>
      }
    >
      {report === 'running' ? (
        <p className="dialog__text">{t('validation.running')}</p>
      ) : (
        <>
          <p className="dialog__text" data-testid="validation-summary">
            {report.issues.length === 0
              ? t('validation.ok', { documents: report.documents })
              : t('validation.summary', {
                  errors: report.errors,
                  warnings: report.warnings,
                  documents: report.documents,
                })}
          </p>
          {report.issues.length > 0 && (
            <ul className="issues">
              {report.issues.map((issue, index) => (
                <li
                  // Issues have no id; the list never changes while shown.
                  // eslint-disable-next-line @eslint-react/no-array-index-key -- static list
                  key={index}
                  className={`issues__item issues__item--${issue.severity}`}
                >
                  <Icon
                    name={
                      issue.severity === 'error'
                        ? 'error'
                        : issue.severity === 'warning'
                          ? 'warning'
                          : 'info'
                    }
                  />
                  <div>
                    <div className="issues__head">
                      <strong>{severityLabel[issue.severity]}</strong>
                      {issue.path !== undefined && <code>{issue.path}</code>}
                    </div>
                    <div className="issues__message">{issueMessage(t, issue)}</div>
                    <code className="issues__code">{issue.code}</code>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </Modal>
  );
}

function RemoveDialog(): ReactNode {
  const { t } = useAppState();
  const { prompts, answerRemove } = useProject();
  if (prompts.remove === null) {
    return null;
  }
  return (
    <ConfirmDialog
      title={t('tree.removeTitle')}
      message={t('tree.removeMessage', { name: prompts.remove.name })}
      confirmLabel={t('tree.removeConfirm')}
      cancelLabel={t('tree.cancel')}
      danger
      testId="remove-dialog"
      onAnswer={(confirmed) => {
        void answerRemove(confirmed);
      }}
    />
  );
}

function Notices(): ReactNode {
  const { t } = useAppState();
  const { notices, dismissNotice } = useProject();
  return (
    <div className="notices" role="status" aria-live="polite" data-testid="notices">
      {notices.map((notice) => (
        <div key={notice.id} className={`toast toast--${notice.kind}`} data-testid="notice">
          <Icon name={notice.kind === 'error' ? 'error' : 'check'} />
          <span className="toast__text">{notice.text}</span>
          <button
            type="button"
            className="toast__close"
            aria-label={t('errors.dismiss')}
            onClick={() => {
              dismissNotice(notice.id);
            }}
          >
            <Icon name="close" size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}

/** Every dialog and notice of the project flow. */
export function ProjectDialogs(): ReactNode {
  const { prompts, busy } = useProject();
  const { t } = useAppState();
  return (
    <>
      {prompts.wizard && <NewProjectWizard />}
      <RecoveryDialog />
      <CloseDialog />
      <RemoveDialog />
      <ValidationDialog />
      <Notices />
      {busy !== null && (
        <div className="busy" role="progressbar" aria-label={t(busy)} data-testid="busy">
          <span className="busy__spinner" />
          <span>{t(busy)}</span>
        </div>
      )}
    </>
  );
}
