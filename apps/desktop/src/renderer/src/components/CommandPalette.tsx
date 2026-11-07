import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from 'react';

import { createTranslator } from '@shared/i18n';

import { filterCommands, type Command } from '../commands';
import { useAppState } from '../state';

interface CommandPaletteProps {
  readonly commands: readonly Command[];
  readonly onClose: () => void;
}

const english = createTranslator('en');

/** Keyboard-first command launcher (Cmd/Ctrl+K). */
export function CommandPalette({ commands, onClose }: CommandPaletteProps): ReactNode {
  const { t } = useAppState();
  const dialogRef = useRef<HTMLDialogElement>(null);
  const listId = useId();
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);

  const items = useMemo(
    () =>
      commands.map((command) => ({
        command,
        label: t(command.label),
        searchText: [t(command.label), english(command.label)],
      })),
    [commands, t],
  );
  const matches = useMemo(() => filterCommands(items, query), [items, query]);
  const activeIndex = Math.min(active, Math.max(matches.length - 1, 0));

  useEffect(() => {
    const dialog = dialogRef.current;
    dialog?.showModal();
    return () => {
      dialog?.close();
    };
  }, []);

  const execute = (index: number): void => {
    const match = matches[index];
    if (match !== undefined) {
      onClose();
      match.command.run();
    }
  };

  return (
    <dialog
      ref={dialogRef}
      className="palette"
      aria-label={t('commands.title')}
      data-testid="command-palette"
      onClose={onClose}
      onClick={(event) => {
        // A click on the backdrop (the dialog element itself) closes it.
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <input
        className="palette__input"
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-controls={listId}
        aria-activedescendant={matches.length > 0 ? `${listId}-${String(activeIndex)}` : undefined}
        placeholder={t('commands.placeholder')}
        value={query}
        data-testid="command-input"
        onChange={(event) => {
          setQuery(event.target.value);
          setActive(0);
        }}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown') {
            event.preventDefault();
            setActive((activeIndex + 1) % Math.max(matches.length, 1));
          } else if (event.key === 'ArrowUp') {
            event.preventDefault();
            setActive((activeIndex - 1 + matches.length) % Math.max(matches.length, 1));
          } else if (event.key === 'Enter') {
            event.preventDefault();
            execute(activeIndex);
          }
        }}
      />
      <ul id={listId} className="palette__list" role="listbox">
        {matches.length === 0 && <li className="palette__empty">{t('commands.empty')}</li>}
        {matches.map((match, index) => (
          <li
            key={match.command.id}
            id={`${listId}-${String(index)}`}
            role="option"
            aria-selected={index === activeIndex}
            className="palette__item"
            data-testid={`command-${match.command.id}`}
            onMouseMove={() => {
              setActive(index);
            }}
            onClick={() => {
              execute(index);
            }}
          >
            {match.label}
          </li>
        ))}
      </ul>
    </dialog>
  );
}
