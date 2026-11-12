/** Dialogs of the library editor: a name for a new, copied or renamed item; deleting. */
import { useState, type ReactNode } from 'react';

import { Modal } from '../components/Modal';
import { useAppState } from '../state';
import type { Usage } from './types';

export function NameDialog({
  title,
  initial,
  hint,
  taken,
  confirm,
  onSubmit,
  onClose,
}: {
  readonly title: string;
  readonly initial: string;
  readonly hint?: string;
  /** Names already in use (the item's own name is allowed). */
  readonly taken: readonly string[];
  readonly confirm: string;
  readonly onSubmit: (name: string) => void;
  readonly onClose: () => void;
}): ReactNode {
  const { t } = useAppState();
  const [name, setName] = useState(initial);
  const trimmed = name.trim();
  const problem =
    trimmed === ''
      ? t('libedit.dialogs.nameEmpty')
      : trimmed !== initial && taken.includes(trimmed)
        ? t('libedit.dialogs.nameTaken')
        : null;
  const submit = (): void => {
    if (problem === null) {
      onSubmit(trimmed);
    }
  };
  return (
    <Modal
      title={title}
      onClose={onClose}
      testId="libedit-name-dialog"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('tree.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={problem !== null}
            data-testid="libedit-name-confirm"
            onClick={submit}
          >
            {confirm}
          </button>
        </>
      }
    >
      {hint !== undefined && <p className="dialog__text dialog__text--muted">{hint}</p>}
      <label className="field">
        <span className="field__label">{t('libedit.dialogs.nameLabel')}</span>
        <input
          className="field__control"
          value={name}
          autoFocus
          spellCheck={false}
          aria-invalid={problem !== null}
          data-testid="libedit-name-input"
          onChange={(event) => {
            setName(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              submit();
            }
          }}
        />
        {problem !== null && trimmed !== '' && <span className="field__error">{problem}</span>}
      </label>
    </Modal>
  );
}

export function DeleteDialog({
  name,
  usage,
  onConfirm,
  onClose,
}: {
  readonly name: string;
  readonly usage: Usage | null;
  readonly onConfirm: () => void;
  readonly onClose: () => void;
}): ReactNode {
  const { t } = useAppState();
  const used = usage !== null && usage.placed + usage.board + usage.symbols + usage.components > 0;
  return (
    <Modal
      title={t('libedit.dialogs.deleteTitle', { name })}
      onClose={onClose}
      testId="libedit-delete-dialog"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('tree.cancel')}
          </button>
          <button
            type="button"
            className="button button--danger"
            data-testid="libedit-delete-confirm"
            onClick={onConfirm}
          >
            {t('libedit.dialogs.delete')}
          </button>
        </>
      }
    >
      <p className="dialog__text">{t('libedit.dialogs.deleteText')}</p>
      {used && (
        <p className="callout callout--warning">{t('libedit.dialogs.deleteUsed', { ...usage })}</p>
      )}
    </Modal>
  );
}
