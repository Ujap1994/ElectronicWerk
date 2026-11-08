import { describe, expect, it } from 'vitest';

import { createTranslator } from '@shared/i18n';

import { EngineError } from '../engine-client';
import { errorMessage, historyLabel } from './labels';

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
