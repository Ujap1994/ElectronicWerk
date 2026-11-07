import { useEffect, useId, useRef, type ReactNode } from 'react';

import { useT } from '../state';

interface ModalProps {
  readonly title: string;
  readonly onClose: () => void;
  readonly children: ReactNode;
  readonly footer?: ReactNode;
  readonly testId?: string;
  readonly className?: string;
}

/**
 * Accessible modal dialog based on the native `<dialog>` element: focus is
 * trapped, Escape closes it and the page behind is inert.
 */
export function Modal({
  title,
  onClose,
  children,
  footer,
  testId,
  className,
}: ModalProps): ReactNode {
  const t = useT();
  const titleId = useId();
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    dialog?.showModal();
    // Focus the dialog itself instead of its first button (the close "×"),
    // so no focus ring flashes on open; Tab moves into the controls.
    dialog?.focus();
    return () => {
      dialog?.close();
    };
  }, []);

  return (
    <dialog
      ref={dialogRef}
      className={className === undefined ? 'dialog' : `dialog ${className}`}
      aria-labelledby={titleId}
      tabIndex={-1}
      data-testid={testId}
      onClose={onClose}
    >
      <header className="dialog__header">
        <h2 id={titleId} className="dialog__title">
          {title}
        </h2>
        <button
          type="button"
          className="dialog__close"
          aria-label={t('settings.close')}
          onClick={onClose}
        >
          ×
        </button>
      </header>
      <div className="dialog__body">{children}</div>
      {footer !== undefined && <footer className="dialog__footer">{footer}</footer>}
    </dialog>
  );
}
