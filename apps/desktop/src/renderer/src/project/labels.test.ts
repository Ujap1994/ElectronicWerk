import { describe, expect, it } from 'vitest';

import { createTranslator } from '@shared/i18n';

import { EngineError } from '../engine-client';
import { errorMessage, historyLabel, issueMessage } from './labels';

const t = createTranslator('en');

describe('historyLabel', () => {
  it('translates known labels and keeps custom ones', () => {
    expect(historyLabel(t, 'rename-document')).toBe('Rename document');
    expect(historyLabel(createTranslator('de'), 'add-sheet')).toBe('Blatt hinzufügen');
    expect(historyLabel(t, 'route bus')).toBe('route bus');
  });
});

describe('errorMessage', () => {
  it('maps engine error codes to messages', () => {
    expect(errorMessage(t, new EngineError({ code: 'alreadyExists', message: 'x' }))).toContain(
      'already exists',
    );
    expect(errorMessage(t, new EngineError({ code: 'ioError', message: 'disk full' }))).toContain(
      'disk full',
    );
    expect(errorMessage(t, new EngineError({ code: 'timeout', message: 'x' }))).toBe(
      t('errors.engine'),
    );
    expect(errorMessage(t, new EngineError({ code: 'internal', message: 'boom' }))).toBe(
      'Unexpected error: boom',
    );
    expect(errorMessage(t, new Error('plain'))).toBe('Unexpected error: plain');
    expect(errorMessage(t, 'text')).toBe('Unexpected error: text');
  });
});

describe('issueMessage', () => {
  it('translates known codes with their parameters', () => {
    const issue = {
      code: 'builtin-variable-shadowed',
      message: 'variable `REVISION` is built in',
      params: { name: 'REVISION' },
    };
    expect(issueMessage(createTranslator('de'), issue)).toBe(
      'Die Variable „REVISION“ ist eingebaut; der Projektwert wird ignoriert.',
    );
    expect(issueMessage(t, { code: 'new-check', message: 'raw text' })).toBe('raw text');
  });

  it('has a translation for every code the engine reports', () => {
    const de = createTranslator('de');
    for (const code of ['sheet-cycle', 'unknown-net', 'unlisted-file', 'unreadable-manifest']) {
      expect(issueMessage(de, { code, message: 'x' })).not.toContain('validation.codes');
    }
  });
});
