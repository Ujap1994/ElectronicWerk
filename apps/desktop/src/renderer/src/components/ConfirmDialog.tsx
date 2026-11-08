import type { ReactNode } from 'react';

import { Modal } from './Modal';

interface ConfirmDialogProps {
  readonly title: string;
  readonly message: string;
  readonly confirmLabel: string;
  readonly cancelLabel: string;
  readonly danger?: boolean;
  readonly testId?: string;
  readonly onAnswer: (confirmed: boolean) => void;
}

/** A yes/no question. */
export function ConfirmDialog({
  title,
  message,
  confirmLabel,
  cancelLabel,
  danger = false,
  testId,
  onAnswer,
}: ConfirmDialogProps): ReactNode {
  return (
    <Modal
      title={title}
      onClose={() => {
        onAnswer(false);
      }}
      {...(testId === undefined ? {} : { testId })}
      footer={
        <>
          <button
            type="button"
            className="button"
            onClick={() => {
              onAnswer(false);
            }}
          >
            {cancelLabel}
          </button>
          <button
            type="button"
            className={danger ? 'button button--danger' : 'button button--primary'}
            data-testid="confirm-yes"
            onClick={() => {
              onAnswer(true);
            }}
          >
            {confirmLabel}
          </button>
        </>
      }
    >
      <p className="dialog__text">{message}</p>
    </Modal>
  );
}
