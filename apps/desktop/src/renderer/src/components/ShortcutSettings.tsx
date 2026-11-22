/**
 * Keyboard shortcuts in the settings: every editor command with its key.
 * Click a key, press the new one; Esc cancels. A key that another command
 * of the same editor uses, or that is reserved (clipboard, undo, Esc …),
 * is refused with a message.
 */
import { useState, type KeyboardEvent, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import {
  SHORTCUT_COMMANDS,
  assignable,
  conflictOf,
  displayKey,
  eventKey,
  type ShortcutContext,
} from '@shared/shortcuts';

import { useAppState } from '../state';

const CONTEXTS: readonly ShortcutContext[] = ['board', 'schematic'];

/** The user's keys without the one of a command. */
function without(
  overrides: Readonly<Record<string, string>>,
  id: string,
): Readonly<Record<string, string>> {
  return Object.fromEntries(Object.entries(overrides).filter(([key]) => key !== id));
}

export function ShortcutSettings(): ReactNode {
  const { t, snapshot, updateSettings, appInfo } = useAppState();
  const mac = appInfo.platform === 'darwin';
  const overrides = snapshot.settings.shortcuts;
  const [recording, setRecording] = useState<string | null>(null);
  const [message, setMessage] = useState<{ id: string; text: string } | null>(null);
  const changed = Object.keys(overrides).length;

  const label = (id: string): string => t(`shortcuts.commands.${id}` as MessageKey);

  const store = (next: Readonly<Record<string, string>>): void => {
    updateSettings({ shortcuts: next });
  };

  const record = (id: string, event: KeyboardEvent<HTMLButtonElement>): void => {
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      setRecording(null);
      return;
    }
    const key = eventKey(event.nativeEvent);
    if (key === null) {
      // A modifier alone: wait for the key.
      return;
    }
    if (!assignable(key)) {
      setMessage({ id, text: t('shortcuts.reserved', { key: displayKey(key, mac) }) });
      setRecording(null);
      return;
    }
    const other = conflictOf(overrides, id, key);
    if (other !== null) {
      setMessage({
        id,
        text: t('shortcuts.usedBy', { key: displayKey(key, mac), command: label(other) }),
      });
      setRecording(null);
      return;
    }
    const fallback = SHORTCUT_COMMANDS.find((c) => c.id === id)?.key;
    store(key === fallback ? without(overrides, id) : { ...overrides, [id]: key });
    setMessage(null);
    setRecording(null);
  };

  return (
    <details className="field field--group shortcuts" data-testid="shortcut-settings">
      <summary className="field__label">
        {t('shortcuts.title')}
        {changed > 0 && (
          <span className="shortcuts__changed">{t('shortcuts.changed', { count: changed })}</span>
        )}
      </summary>
      <p className="field__hint">{t('shortcuts.hint')}</p>
      {CONTEXTS.map((context) => (
        <section key={context} className="shortcuts__context">
          <h3 className="shortcuts__heading">{t(`shortcuts.contexts.${context}`)}</h3>
          <ul className="shortcuts__list">
            {SHORTCUT_COMMANDS.filter((c) => c.context === context).map((command) => {
              const key = overrides[command.id] ?? command.key;
              const own = overrides[command.id] !== undefined;
              const active = recording === command.id;
              return (
                <li
                  key={command.id}
                  className="shortcuts__row"
                  data-testid={`shortcut-${command.id}`}
                >
                  <span className="shortcuts__label">{label(command.id)}</span>
                  <button
                    type="button"
                    className={`shortcuts__key${own ? ' shortcuts__key--own' : ''}`}
                    aria-pressed={active}
                    aria-label={t('shortcuts.change', { command: label(command.id) })}
                    data-testid={`shortcut-key-${command.id}`}
                    onClick={() => {
                      setMessage(null);
                      setRecording(active ? null : command.id);
                    }}
                    onKeyDown={(event) => {
                      if (active) {
                        record(command.id, event);
                      }
                    }}
                    onBlur={() => {
                      if (active) {
                        setRecording(null);
                      }
                    }}
                  >
                    {active ? t('shortcuts.press') : displayKey(key, mac)}
                  </button>
                  <button
                    type="button"
                    className="button button--small"
                    disabled={!own}
                    title={t('shortcuts.resetTip', { key: displayKey(command.key, mac) })}
                    data-testid={`shortcut-reset-${command.id}`}
                    onClick={() => {
                      store(without(overrides, command.id));
                    }}
                  >
                    {t('shortcuts.reset')}
                  </button>
                  {message?.id === command.id && (
                    <span
                      className="shortcuts__message"
                      role="alert"
                      data-testid="shortcut-message"
                    >
                      {message.text}
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      ))}
      <button
        type="button"
        className="button"
        disabled={changed === 0}
        data-testid="shortcuts-reset-all"
        onClick={() => {
          store({});
          setMessage(null);
        }}
      >
        {t('shortcuts.resetAll')}
      </button>
    </details>
  );
}
