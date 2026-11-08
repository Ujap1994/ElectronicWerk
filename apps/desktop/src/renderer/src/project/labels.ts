import type { EngineErrorCode } from '@shared/engine';
import type { MessageKey, Translate } from '@shared/i18n';

import { EngineError } from '../engine-client';

/** Translation keys of the engine's transaction labels. */
const HISTORY_LABELS: Readonly<Record<string, MessageKey>> = {
  'add-document': 'history.addDocument',
  'add-sheet': 'history.addSheet',
  'remove-document': 'history.removeDocument',
  'rename-document': 'history.renameDocument',
  'set-document-properties': 'history.setDocumentProperties',
  'set-project-properties': 'history.setProjectProperties',
  apply: 'history.apply',
  recover: 'history.recover',
};

/** A readable name for an undo/redo step (scripts may use their own labels). */
export function historyLabel(t: Translate, label: string): string {
  const key = HISTORY_LABELS[label];
  return key === undefined ? label : t(key);
}

/** A user-facing message for a failed engine call. */
export function errorMessage(t: Translate, error: unknown): string {
  if (!(error instanceof EngineError)) {
    return t('errors.generic', { message: error instanceof Error ? error.message : String(error) });
  }
  const message = error.message;
  const code: EngineErrorCode = error.code;
  switch (code) {
    case 'disconnected':
    case 'timeout':
    case 'hostError':
      return t('errors.engine');
    case 'noProject':
      return t('errors.noProject');
    case 'projectOpen':
      return t('errors.projectOpen');
    case 'unsavedChanges':
      return t('errors.unsavedChanges');
    case 'recoveryPending':
      return t('errors.recoveryPending');
    case 'conflict':
      return t('errors.conflict', { message });
    case 'notFound':
      return t('errors.notFound', { message });
    case 'alreadyExists':
      return t('errors.alreadyExists');
    case 'ioError':
      return t('errors.ioError', { message });
    case 'formatError':
      return t('errors.formatError', { message });
    case 'newerFormat':
      return t('errors.newerFormat');
    case 'invalidParams':
      return t('errors.invalidParams', { message });
    case 'internal':
    case 'invalidRequest':
    case 'methodNotFound':
      return t('errors.generic', { message });
  }
}
