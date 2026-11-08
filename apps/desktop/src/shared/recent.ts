/** The list of recently opened projects (persisted by the main process). */

export interface RecentProject {
  /** Full path of the `.ewproj` file. */
  readonly manifestPath: string;
  readonly name: string;
  /** ISO 8601 timestamp of the last opening. */
  readonly openedAt: string;
}

export interface RecentState {
  readonly projects: readonly RecentProject[];
  /** Folder of the last created project, offered in the wizard. */
  readonly lastLocation: string | null;
}

export const MAX_RECENT = 10;

export const EMPTY_RECENT: RecentState = { projects: [], lastLocation: null };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isPathLike(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= 4096;
}

function sanitizeEntry(raw: unknown): RecentProject | null {
  if (!isRecord(raw) || !isPathLike(raw['manifestPath'])) {
    return null;
  }
  const name = typeof raw['name'] === 'string' ? raw['name'].slice(0, 200) : '';
  const openedAt =
    typeof raw['openedAt'] === 'string' && !Number.isNaN(Date.parse(raw['openedAt']))
      ? raw['openedAt']
      : new Date(0).toISOString();
  return { manifestPath: raw['manifestPath'], name, openedAt };
}

/** Validates untrusted input (the JSON file, IPC payloads). */
export function sanitizeRecent(raw: unknown): RecentState {
  const input = isRecord(raw) ? raw : {};
  const list = Array.isArray(input['projects']) ? input['projects'] : [];
  const projects: RecentProject[] = [];
  for (const entry of list) {
    const project = sanitizeEntry(entry);
    if (project !== null && !projects.some((p) => samePath(p.manifestPath, project.manifestPath))) {
      projects.push(project);
    }
  }
  const location = input['lastLocation'];
  return {
    projects: projects.slice(0, MAX_RECENT),
    lastLocation: isPathLike(location) ? location : null,
  };
}

/** Compares paths, ignoring case on case-insensitive systems. */
export function samePath(a: string, b: string, caseInsensitive = true): boolean {
  return caseInsensitive ? a.toLowerCase() === b.toLowerCase() : a === b;
}

/** Puts a project at the top of the list. */
export function addRecent(state: RecentState, entry: RecentProject): RecentState {
  const clean = sanitizeEntry(entry);
  if (clean === null) {
    return state;
  }
  const others = state.projects.filter((p) => !samePath(p.manifestPath, clean.manifestPath));
  return { ...state, projects: [clean, ...others].slice(0, MAX_RECENT) };
}

export function removeRecent(state: RecentState, manifestPath: string): RecentState {
  return {
    ...state,
    projects: state.projects.filter((p) => !samePath(p.manifestPath, manifestPath)),
  };
}
