// @ts-check
import { describe, it } from 'node:test';

import { RuleTester } from 'eslint';

import plugin from './index.js';

RuleTester.describe = describe;
RuleTester.it = it;

const tester = new RuleTester({
  languageOptions: {
    ecmaVersion: 'latest',
    sourceType: 'module',
    parserOptions: { ecmaFeatures: { jsx: true } },
  },
});

tester.run('no-literal-ui-text', plugin.rules['no-literal-ui-text'], {
  valid: [
    { code: '<p>{t("start.title")}</p>' },
    { code: '<button aria-label={t("settings.close")}>×</button>' },
    { code: '<span>{count} · {total}</span>' },
    { code: '<div className="start" data-testid="new-project" />' },
    { code: '<svg viewBox="0 0 24 24"><path d="M0 0" /></svg>' },
  ],
  invalid: [
    { code: '<p>Hello world</p>', errors: [{ messageId: 'literal' }] },
    {
      code: '<button title="Close">x</button>',
      errors: [{ messageId: 'literal' }, { messageId: 'literal' }],
    },
    { code: '<input placeholder="Suchen" />', errors: [{ messageId: 'literal' }] },
    { code: '<img alt="Logo" />', errors: [{ messageId: 'literal' }] },
    { code: '<span aria-label="Einstellungen" />', errors: [{ messageId: 'literal' }] },
  ],
});
