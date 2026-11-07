import type { ReactNode } from 'react';

import { LANGUAGES, isLanguage, type LanguageSetting } from '@shared/i18n';
import type { ThemeSetting } from '@shared/settings';

import { useAppState } from '../state';
import { Modal } from './Modal';

interface SettingsDialogProps {
  readonly onClose: () => void;
  readonly onCheckUpdates: () => void;
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

export function SettingsDialog({ onClose, onCheckUpdates }: SettingsDialogProps): ReactNode {
  const { t, snapshot, updateSettings } = useAppState();
  const { settings } = snapshot;

  return (
    <Modal title={t('settings.title')} onClose={onClose} testId="settings-dialog">
      <label className="field">
        <span className="field__label">{t('settings.language')}</span>
        <select
          className="field__control"
          value={settings.language}
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
          value={settings.theme}
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

      <fieldset className="field field--group">
        <legend className="field__label">{t('settings.updates')}</legend>
        <label className="checkbox">
          <input
            type="checkbox"
            checked={settings.updates.checkOnStartup}
            data-testid="setting-check-updates"
            onChange={(event) => {
              updateSettings({ updates: { checkOnStartup: event.target.checked } });
            }}
          />
          <span>{t('settings.checkOnStartup')}</span>
        </label>
        {settings.updates.skippedVersion !== null && (
          <div className="field__row" data-testid="skipped-version">
            <span className="field__hint">
              {t('settings.skippedVersion', { version: settings.updates.skippedVersion })}
            </span>
            <button
              type="button"
              className="button button--small"
              onClick={() => {
                updateSettings({ updates: { skippedVersion: null } });
              }}
            >
              {t('settings.resetSkipped')}
            </button>
          </div>
        )}
        <div>
          <button
            type="button"
            className="button"
            data-testid="check-updates-now"
            onClick={onCheckUpdates}
          >
            {t('settings.checkNow')}
          </button>
        </div>
      </fieldset>
    </Modal>
  );
}
