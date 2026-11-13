/**
 * Keeps state that several windows show in step: the selection of parts
 * (layout editor, schematic and the 3D view, which may have its own
 * window). Local changes go to the other windows through the main
 * process; changes from there are applied without sending them back.
 */
import { crossSelection, type CrossSelection } from './board/stores';

let installed = false;

function isSelection(value: unknown): value is CrossSelection | null {
  if (value === null) {
    return true;
  }
  if (typeof value !== 'object') {
    return false;
  }
  const record = value as Record<string, unknown>;
  return (
    (record['source'] === 'board' ||
      record['source'] === 'schematic' ||
      record['source'] === '3d') &&
    Array.isArray(record['footprints']) &&
    Array.isArray(record['symbols'])
  );
}

/** Starts the synchronization (once per window). */
export function installWindowSync(): void {
  if (installed || typeof window === 'undefined') {
    return;
  }
  installed = true;
  let remote: CrossSelection | null | undefined;
  crossSelection.subscribe(() => {
    const selection = crossSelection.get();
    if (selection === remote) {
      return;
    }
    window.ew.sync.publish({ topic: 'selection', payload: selection });
  });
  window.ew.sync.onMessage((message) => {
    if (message.topic === 'request') {
      // A new window wants to know what is selected.
      const selection = crossSelection.get();
      if (selection !== null) {
        window.ew.sync.publish({ topic: 'selection', payload: selection });
      }
      return;
    }
    if (isSelection(message.payload)) {
      remote = message.payload;
      crossSelection.set(message.payload);
    }
  });
  window.ew.sync.publish({ topic: 'request', payload: null });
}
