import { useState, type ReactNode } from 'react';

import { DOCS_URL, PROJECT_URL } from '@shared/links';

import { Icon } from '../components/Icon';
import { useAppState } from '../state';

/** The MCP configuration for AI agents (Claude, Cursor, VS Code, …). */
export const MCP_CONFIG = JSON.stringify(
  { mcpServers: { electronicwerk: { command: 'ewerk', args: ['mcp'] } } },
  null,
  2,
);

/** The settings section for AI agents and scripts: how to connect them. */
export function AutomationSettings(): ReactNode {
  const { t } = useAppState();
  const [copied, setCopied] = useState(false);
  return (
    <fieldset className="field field--group" data-testid="settings-automation">
      <legend className="field__label">{t('automation.title')}</legend>
      <span className="field__hint">{t('automation.intro')}</span>
      <pre className="automation__config" data-testid="automation-config">
        {MCP_CONFIG}
      </pre>
      <div className="field__row">
        <button
          type="button"
          className="button button--small"
          data-testid="automation-copy"
          onClick={() => {
            void navigator.clipboard.writeText(MCP_CONFIG).then(() => {
              setCopied(true);
            });
          }}
        >
          <Icon name="copy" size={14} />
          {copied ? t('automation.copied') : t('automation.copy')}
        </button>
        <button
          type="button"
          className="button button--small"
          onClick={() => {
            window.ew.app.openExternal(`${PROJECT_URL}/releases/latest`);
          }}
        >
          <Icon name="download" size={14} />
          {t('automation.download')}
        </button>
        <button
          type="button"
          className="button button--small"
          onClick={() => {
            window.ew.app.openExternal(`${DOCS_URL}guide/automation`);
          }}
        >
          <Icon name="info" size={14} />
          {t('automation.guide')}
        </button>
      </div>
      <span className="field__hint">{t('automation.safety')}</span>
    </fieldset>
  );
}
