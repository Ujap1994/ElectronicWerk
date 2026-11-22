/**
 * The keys of the editor commands as the user set them: the editors ask
 * which command a key press starts, the tool palettes show the keys.
 */
import {
  SHORTCUT_COMMANDS,
  displayKey,
  eventKey,
  keyMap,
  type ShortcutContext,
} from '@shared/shortcuts';

let current: Readonly<Record<string, string>> = {};
let keys = keyMap({});
let commands = index(keys);

function index(map: ReadonlyMap<string, string>): ReadonlyMap<string, string> {
  const out = new Map<string, string>();
  for (const command of SHORTCUT_COMMANDS) {
    const key = map.get(command.id);
    if (key !== undefined) {
      out.set(`${command.context}|${key}`, command.id);
    }
  }
  return out;
}

/** Takes the user's keys (settings `shortcuts`). */
export function setShortcuts(overrides: Readonly<Record<string, string>>): void {
  if (overrides === current) {
    return;
  }
  current = overrides;
  keys = keyMap(overrides);
  commands = index(keys);
}

/** The command a key press starts in an editor, or `null`. */
export function commandFor(
  context: ShortcutContext,
  event: {
    readonly key: string;
    readonly ctrlKey: boolean;
    readonly metaKey: boolean;
    readonly shiftKey: boolean;
    readonly altKey: boolean;
  },
): string | null {
  const key = eventKey(event);
  return key === null ? null : (commands.get(`${context}|${key}`) ?? null);
}

/** The key of a command for people (`X`, `Shift+L`, `⌘G`). */
export function shortcutLabel(id: string, mac: boolean): string {
  const key = keys.get(id);
  return key === undefined ? '' : displayKey(key, mac);
}
