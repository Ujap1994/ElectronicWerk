import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { handleDatasheetRequest, isDatasheetMethod } from './datasheets';

describe('datasheet files of the engine host', () => {
  it('handles only its own methods', () => {
    expect(isDatasheetMethod('datasheet.file')).toBe(true);
    expect(isDatasheetMethod('datasheet.previewModel')).toBe(true);
    expect(isDatasheetMethod('datasheet.read')).toBe(false);
  });

  it('reads PDF files by absolute path only', () => {
    const dir = mkdtempSync(join(tmpdir(), 'ew-ds-'));
    const pdf = join(dir, 'part.pdf');
    writeFileSync(pdf, '%PDF-1.7\n');
    const reply = handleDatasheetRequest(1, 'datasheet.file', { path: pdf });
    expect('result' in reply && reply.result).toEqual({ data: new Uint8Array(readFileSync(pdf)) });
    expect('error' in reply).toBe(false);
    expect(handleDatasheetRequest(2, 'datasheet.file', { path: 'part.pdf' })).toMatchObject({
      error: { code: 'invalidParams' },
    });
    expect(handleDatasheetRequest(3, 'datasheet.file', { path: join(dir, 'x.txt') })).toMatchObject(
      {
        error: { code: 'invalidParams' },
      },
    );
    expect(
      handleDatasheetRequest(4, 'datasheet.file', { path: join(dir, 'gone.pdf') }),
    ).toMatchObject({
      error: { code: 'notFound' },
    });
  });

  it('keeps preview models in a temporary file named by content', () => {
    const step = 'ISO-10303-21;\nHEADER;\nENDSEC;\n';
    const a = handleDatasheetRequest(5, 'datasheet.previewModel', { step });
    const b = handleDatasheetRequest(6, 'datasheet.previewModel', { step });
    const file = 'result' in a ? (a.result as { file: string }).file : '';
    expect(file).toMatch(/\.step$/u);
    expect('result' in b ? (b.result as { file: string }).file : '').toBe(file);
    expect(readFileSync(file, 'utf8')).toBe(step);
    expect(handleDatasheetRequest(7, 'datasheet.previewModel', { step: 'nope' })).toMatchObject({
      error: { code: 'invalidParams' },
    });
  });
});
