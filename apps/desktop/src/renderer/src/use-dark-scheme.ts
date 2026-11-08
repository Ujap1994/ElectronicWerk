import { useSyncExternalStore } from 'react';

/** `true` while the app shows its dark appearance. */
export function useDarkScheme(): boolean {
  return useSyncExternalStore(
    (listener) => {
      const query = window.matchMedia('(prefers-color-scheme: dark)');
      query.addEventListener('change', listener);
      return () => {
        query.removeEventListener('change', listener);
      };
    },
    () => window.matchMedia('(prefers-color-scheme: dark)').matches,
  );
}
