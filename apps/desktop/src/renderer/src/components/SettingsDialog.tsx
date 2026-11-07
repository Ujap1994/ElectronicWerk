import { useEffect, useRef, type ReactNode } from 'react';

import { LANGUAGES, isLanguage, type LanguageSetting } from '@shared/i18n';
import type { ThemeSetting } from '@shared/settings';

import { useAppState } from '../state';

interface SettingsDialogProps {
  readonly onClose: () => void;
}

const THEMES = [
  { value: 'system', label: 'settings.themeSystem' },
  { value: 'light', label: 'settings.themeLight' },
  { value: 'dark', label: 'settings.themeDark' },
] as const satisfies readonly { value: ThemeSetting; label: string }[];

function toLanguageSetting(value: string): LanguageSetting {
  return isLanguage(value) ? value : 'system';
}

function toThemeSetting(value: string): ThemeSetting {
  return THEMES.find((theme) => theme.value === value)?.value ?? 'system';
}

export function SettingsDialog({ onClose }: SettingsDialogProps): ReactNode {
  const { t, snapshot, updateSettings } = useAppState();
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    dialog?.showModal();
    return () => {
      dialog?.close();
    };
  }, []);

  return (
    <dialog
      ref={dialogRef}
      className="dialog"
      aria-labelledby="settings-title"
      data-testid="settings-dialog"
      onClose={onClose}
    >
      <header className="dialog__header">
        <h2 id="settings-title" className="dialog__title">
          {t('settings.title')}
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

      <div className="dialog__body">
        <label className="field">
          <span className="field__label">{t('settings.language')}</span>
          <select
            className="field__control"
            value={snapshot.settings.language}
            data-testid="setting-language"
            onChange={(event) => {
              updateSettings({ language: toLanguageSetting(event.target.value) });
            }}
          >
            <option value="system">{t('settings.languageSystem')}</option>
            {LANGUAGES.map((language) => (
              <option key={language} value={language}>
                {t(`languages.${language}`)}
              </option>
            ))}
          </select>
        </label>

        <label className="field">
          <span className="field__label">{t('settings.theme')}</span>
          <select
            className="field__control"
            value={snapshot.settings.theme}
            data-testid="setting-theme"
            onChange={(event) => {
              updateSettings({ theme: toThemeSetting(event.target.value) });
            }}
          >
            {THEMES.map((theme) => (
              <option key={theme.value} value={theme.value}>
                {t(theme.label)}
              </option>
            ))}
          </select>
        </label>
      </div>
    </dialog>
  );
}
