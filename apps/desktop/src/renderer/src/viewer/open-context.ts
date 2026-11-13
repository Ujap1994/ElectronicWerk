import { createContext, use } from 'react';

import type { DocumentSummary } from '@shared/project';

/** Opens a sheet, board or library in a tab (provided by the workspace). */
export const OpenDocumentContext = createContext<((document: DocumentSummary) => void) | null>(
  null,
);

export function useOpenDocument(): ((document: DocumentSummary) => void) | null {
  return use(OpenDocumentContext);
}

/** Documents that open in a tab: sheets and boards in the viewer, libraries in the library editor. */
export function isViewable(document: DocumentSummary): boolean {
  return (
    document.kind === 'schematic' ||
    document.kind === 'pcb' ||
    document.kind === 'library' ||
    document.kind === 'outputJob'
  );
}

/** Opens the 3D view of a board in a tab (provided by the workspace). */
export const Open3dContext = createContext<((document: DocumentSummary) => void) | null>(null);

export function useOpen3d(): ((document: DocumentSummary) => void) | null {
  return use(Open3dContext);
}
