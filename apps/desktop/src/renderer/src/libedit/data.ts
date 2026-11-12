/**
 * Data of the library editors: the item list of a library and the shown
 * item, both reloaded after every change of the project (also undo/redo).
 */
import { useEffect, useRef, useState } from 'react';

import { isEngineEvent } from '@shared/project';

import type { EngineClient } from '../engine-client';
import { useAppState } from '../state';
import type { ItemReply, LibraryKind, LibraryList } from './types';

interface Loaded<T> {
  /** What was loaded (a new key hides the old value). */
  readonly key: string;
  readonly value: T | null;
  readonly failed: boolean;
}

/**
 * Loads a value for `key` now and after every project change (debounced).
 * A `null` key loads nothing.
 */
function useReload<T>(
  engine: EngineClient,
  key: string | null,
  load: () => Promise<T>,
): { value: T | null; failed: boolean } {
  const [state, setState] = useState<Loaded<T> | null>(null);
  const loadRef = useRef(load);
  useEffect(() => {
    loadRef.current = load;
  });
  useEffect(() => {
    if (key === null) {
      return undefined;
    }
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const run = (): void => {
      loadRef.current().then(
        (value) => {
          if (!cancelled) {
            setState({ key, value, failed: false });
          }
        },
        () => {
          if (!cancelled) {
            setState((current) => ({
              key,
              value: current?.key === key ? current.value : null,
              failed: true,
            }));
          }
        },
      );
    };
    run();
    const off = engine.onEvent((event) => {
      if (isEngineEvent(event)) {
        clearTimeout(timer);
        timer = setTimeout(run, 20);
      }
    });
    return () => {
      cancelled = true;
      clearTimeout(timer);
      off();
    };
  }, [engine, key]);
  return state?.key === key
    ? { value: state.value, failed: state.failed }
    : { value: null, failed: false };
}

/** The items of a library with their check counts. */
export function useLibraryList(doc: string): LibraryList | null {
  const { engine } = useAppState();
  return useReload(engine, doc, () => engine.call<LibraryList>('libedit.list', { doc })).value;
}

/** One item with its usage and findings (`null` while loading or gone). */
export function useLibraryItem<T>(
  doc: string,
  kind: LibraryKind,
  name: string | null,
): ItemReply<T> | null {
  const { engine } = useAppState();
  const { value, failed } = useReload(
    engine,
    name === null ? null : JSON.stringify([doc, kind, name]),
    () => engine.call<ItemReply<T>>('libedit.item', { doc, kind, name }),
  );
  return failed ? null : value;
}
