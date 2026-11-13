/**
 * Settings of the 3D view, kept per user in this browser profile, and the
 * 3D views that can be opened (for the command palette).
 */
import { Store } from '../schematic/stores';
import type { Projection, Visibility } from './types';

export interface View3dSettings {
  readonly visibility: Visibility;
  readonly projection: Projection;
}

const KEY = 'ew.view3d';

export const DEFAULT_SETTINGS: View3dSettings = {
  visibility: {
    partsTop: true,
    partsBottom: true,
    dnp: false,
    silk: true,
    mask: true,
    copper: true,
    paste: false,
    body: true,
    inner: false,
  },
  projection: 'perspective',
};

function stored(): View3dSettings {
  try {
    const text = window.localStorage.getItem(KEY);
    if (text === null) {
      return DEFAULT_SETTINGS;
    }
    const value = JSON.parse(text) as Partial<View3dSettings>;
    return {
      visibility: { ...DEFAULT_SETTINGS.visibility, ...value.visibility },
      projection: value.projection === 'orthographic' ? 'orthographic' : 'perspective',
    };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export const view3dSettings = new Store<View3dSettings>(
  typeof window === 'undefined' ? DEFAULT_SETTINGS : stored(),
);

export function setView3dSettings(settings: View3dSettings): void {
  view3dSettings.set(settings);
  try {
    window.localStorage.setItem(KEY, JSON.stringify(settings));
  } catch {
    // Storage may be unavailable; the settings then last for the session.
  }
}

/** Opens the 3D view of the board shown right now (command palette). */
export const open3dCommand = new Store<(() => void) | null>(null);
