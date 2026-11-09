import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { BrowserWindow } from 'electron';

import type { ExportKind } from '@shared/api';

/**
 * Prints an HTML document (one sheet per page, page sizes from CSS) to PDF
 * in a hidden window without scripts.
 */
export async function printPdf(html: string): Promise<Buffer> {
  const dir = await mkdtemp(join(tmpdir(), 'ew-print-'));
  const file = join(dir, 'print.html');
  await writeFile(file, html, 'utf8');
  const window = new BrowserWindow({
    show: false,
    webPreferences: {
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      javascript: false,
    },
  });
  try {
    await window.loadFile(file);
    return await window.webContents.printToPDF({ printBackground: true, preferCSSPageSize: true });
  } finally {
    window.destroy();
    await rm(dir, { recursive: true, force: true });
  }
}

/** Writes an export: text as it is, a print document as PDF. */
export async function writeExport(kind: ExportKind, path: string, content: string): Promise<void> {
  if (kind === 'pdf') {
    await writeFile(path, await printPdf(content));
  } else {
    await writeFile(path, content, 'utf8');
  }
}
