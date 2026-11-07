import { useState, type ReactNode } from 'react';

import type { UpdateCheckResult } from '@shared/update';

import { useAppState } from '../state';
import { Markdown } from './Markdown';
import { Modal } from './Modal';

/** What the update dialog currently shows. */
export type UpdateDialogState =
  { readonly kind: 'checking' } | { readonly kind: 'result'; readonly result: UpdateCheckResult };

interface UpdateDialogProps {
  readonly state: UpdateDialogState;
  readonly onClose: () => void;
}

export function UpdateDialog({ state, onClose }: UpdateDialogProps): ReactNode {
  const { t } = useAppState();

  if (state.kind === 'checking') {
    return (
      <Modal title={t('updates.checking')} onClose={onClose} testId="update-dialog">
        <p className="dialog__text" role="status">
          {t('updates.checking')}
        </p>
      </Modal>
    );
  }

  const { result } = state;
  switch (result.status) {
    case 'available':
    case 'skipped':
      return <UpdateAvailable result={result} onClose={onClose} />;
    case 'up-to-date':
      return (
        <MessageDialog title={t('updates.upToDateTitle')} onClose={onClose}>
          {result.latestVersion === null
            ? t('updates.noReleases', { version: result.currentVersion })
            : t('updates.upToDate', { version: result.currentVersion })}
        </MessageDialog>
      );
    case 'error':
      return (
        <MessageDialog title={t('updates.errorTitle')} onClose={onClose}>
          {t('updates.error', { error: result.message })}
        </MessageDialog>
      );
  }
}

function MessageDialog({
  title,
  onClose,
  children,
}: {
  readonly title: string;
  readonly onClose: () => void;
  readonly children: ReactNode;
}): ReactNode {
  const { t } = useAppState();
  return (
    <Modal
      title={title}
      onClose={onClose}
      testId="update-dialog"
      footer={
        <button type="button" className="button button--primary" onClick={onClose}>
          {t('updates.ok')}
        </button>
      }
    >
      <p className="dialog__text">{children}</p>
    </Modal>
  );
}

function UpdateAvailable({
  result,
  onClose,
}: {
  readonly result: Extract<UpdateCheckResult, { status: 'available' | 'skipped' }>;
  readonly onClose: () => void;
}): ReactNode {
  const { t, snapshot, updateSettings } = useAppState();
  const { release } = result;
  const [skip, setSkip] = useState(snapshot.settings.updates.skippedVersion === release.version);

  const close = (): void => {
    const skippedVersion = skip ? release.version : null;
    if (skippedVersion !== snapshot.settings.updates.skippedVersion) {
      updateSettings({ updates: { skippedVersion } });
    }
    onClose();
  };

  const releasedOn =
    release.publishedAt === null
      ? null
      : new Intl.DateTimeFormat(snapshot.language, { dateStyle: 'long' }).format(
          new Date(release.publishedAt),
        );

  return (
    <Modal
      title={t('updates.availableTitle')}
      onClose={close}
      testId="update-dialog"
      className="dialog--wide"
      footer={
        <>
          <label className="checkbox dialog__footer-start">
            <input
              type="checkbox"
              checked={skip}
              data-testid="update-skip"
              onChange={(event) => {
                setSkip(event.target.checked);
              }}
            />
            <span>{t('updates.skipVersion')}</span>
          </label>
          <button type="button" className="button" onClick={close} data-testid="update-later">
            {t('updates.later')}
          </button>
          <button
            type="button"
            className="button button--primary"
            data-testid="update-download"
            onClick={() => {
              window.ew.app.openExternal(release.url);
              close();
            }}
          >
            {t('updates.download')}
          </button>
        </>
      }
    >
      <p className="update__heading">
        {t('updates.availableHeading', { version: release.version })}
      </p>
      <p className="dialog__text dialog__text--muted">
        {t('updates.currentVersion', { version: result.currentVersion })}
        {releasedOn !== null && (
          <>
            {' · '}
            {t('updates.releasedOn', { date: releasedOn })}
          </>
        )}
      </p>
      <h3 className="update__section-title">{t('updates.whatsNew')}</h3>
      <div className="update__notes" data-testid="update-notes">
        {release.notes.trim() === '' ? (
          <p className="dialog__text dialog__text--muted">{t('updates.noNotes')}</p>
        ) : (
          <Markdown source={release.notes} />
        )}
      </div>
      <p className="dialog__text dialog__text--muted update__hint">{t('updates.installHint')}</p>
    </Modal>
  );
}
