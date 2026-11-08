import { createContext, use } from 'react';

import type { DocumentSummary } from '@shared/project';

/** Opens a sheet or board in a viewer tab (provided by the workspace). */
export const OpenDocumentContext = createContext<((document: DocumentSummary) => void) | null>(
  null,
);

export function useOpenDocument(): ((document: DocumentSummary) => void) | null {
  return use(OpenDocumentContext);
}

/** Documents that the 2D viewer can show. */
export function isViewable(document: DocumentSummary): boolean {
  return document.kind === 'schematic' || document.kind === 'pcb';
}
