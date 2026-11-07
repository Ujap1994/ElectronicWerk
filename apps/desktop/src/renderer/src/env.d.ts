import type { EwApi } from '@shared/api';

declare global {
  interface Window {
    /** API exposed by the preload script. */
    readonly ew: EwApi;
  }
}

export {};
