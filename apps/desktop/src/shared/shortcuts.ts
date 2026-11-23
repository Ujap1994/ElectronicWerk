/**
 * Keyboard shortcuts of the editors: every command that a key starts, with
 * its default key. The user can give a command another key (settings
 * `shortcuts`: command id → key); clipboard, undo and the fixed keys
 * (Esc, Enter, Delete …) stay as they are.
 *
 * A key is written in lower case as `ctrl+shift+alt+<key>` with the
 * modifiers in this order; `ctrl` means ⌘ on macOS. `<key>` is the
 * character (`x`, `6`, `/`, `` ` ``) or the name of the key (`pageup`,
 * `f2`).
 */

export type ShortcutContext = 'board' | 'schematic';

export interface ShortcutCommand {
  readonly id: string;
  readonly context: ShortcutContext;
  /** The default key. */
  readonly key: string;
}

const board = (id: string, key: string): ShortcutCommand => ({
  id: `board.${id}`,
  context: 'board',
  key,
});
const sheet = (id: string, key: string): ShortcutCommand => ({
  id: `schematic.${id}`,
  context: 'schematic',
  key,
});

export const SHORTCUT_COMMANDS: readonly ShortcutCommand[] = [
  board('select', 's'),
  board('footprint', 'a'),
  board('route', 'x'),
  board('pair', '6'),
  board('via', 'v'),
  board('zone', 'z'),
  board('keepout', 'k'),
  board('outline', 'o'),
  board('text', 't'),
  board('dimension', 'shift+d'),
  board('highlight', 'n'),
  board('rotate', 'r'),
  board('rotateBack', 'shift+r'),
  board('flip', 'shift+f'),
  board('move', 'm'),
  board('drag', 'd'),
  board('lock', 'l'),
  board('unlock', 'shift+l'),
  board('group', 'ctrl+g'),
  board('ungroup', 'ctrl+shift+g'),
  board('fillZones', 'shift+b'),
  board('highlightAt', '`'),
  board('bend', '/'),
  board('layerUp', 'pageup'),
  board('layerDown', 'pagedown'),
  sheet('symbol', 'a'),
  sheet('wire', 'w'),
  sheet('bus', 'b'),
  sheet('label', 'l'),
  sheet('globalLabel', 'shift+l'),
  sheet('hierarchicalLabel', 'h'),
  sheet('power', 'p'),
  sheet('noConnect', 'q'),
  sheet('junction', 'j'),
  sheet('text', 't'),
  sheet('sheet', 's'),
  sheet('highlight', 'n'),
  sheet('rotate', 'r'),
  sheet('rotateBack', 'shift+r'),
  sheet('mirrorX', 'x'),
  sheet('mirrorY', 'y'),
  sheet('move', 'm'),
  sheet('drag', 'g'),
  sheet('highlightAt', '`'),
  sheet('bend', '/'),
];

const COMMAND_IDS: ReadonlySet<string> = new Set(SHORTCUT_COMMANDS.map((c) => c.id));

const NAMED_KEYS: ReadonlySet<string> = new Set([
  'pageup',
  'pagedown',
  'home',
  'end',
  'insert',
  ...Array.from({ length: 12 }, (_, i) => `f${String(i + 1)}`),
]);

/** Keys that keep their meaning (editing, clipboard, history, the app). */
const RESERVED: ReadonlySet<string> = new Set([
  'escape',
  'enter',
  'backspace',
  'delete',
  'tab',
  'space',
  ...['a', 'c', 'd', 'e', 'f', 'k', 'n', 'o', 'p', 'q', 's', 'v', 'w', 'x', 'y', 'z'].map(
    (k) => `ctrl+${k}`,
  ),
  'ctrl+shift+z',
  'ctrl+shift+l',
  'ctrl+shift+s',
]);

/**
 * Shift is part of a key for letters and named keys only: a sign such as
 * `/` needs Shift on some keyboards (German: Shift+7) and not on others.
 */
function shiftCounts(base: string): boolean {
  return base.length > 1 || base.toLowerCase() !== base.toUpperCase();
}

/** A key in its normal form, or `null` if it is not a valid key. */
export function normalizeKey(text: string): string | null {
  const parts = text.trim().toLowerCase().split('+');
  // `ctrl++` would be the plus key.
  const base = parts.pop() ?? '';
  if (base === '' && text.endsWith('+')) {
    return null;
  }
  const mods = new Set(parts);
  if (parts.length !== mods.size || [...mods].some((m) => !['ctrl', 'shift', 'alt'].includes(m))) {
    return null;
  }
  const single = Array.from(base).length === 1 && base !== ' ' && base !== '+';
  if (!single && !NAMED_KEYS.has(base)) {
    return null;
  }
  return [
    mods.has('ctrl') ? 'ctrl' : null,
    mods.has('shift') && shiftCounts(base) ? 'shift' : null,
    mods.has('alt') ? 'alt' : null,
    base,
  ]
    .filter((p): p is string => p !== null)
    .join('+');
}

/** The key of a keyboard event in normal form (`null`: a modifier alone). */
export function eventKey(event: {
  readonly key: string;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
}): string | null {
  const name = event.key;
  if (['Control', 'Meta', 'Shift', 'Alt', 'AltGraph', 'CapsLock', 'Dead'].includes(name)) {
    return null;
  }
  const base = name === ' ' ? 'space' : name.toLowerCase();
  return [
    event.ctrlKey || event.metaKey ? 'ctrl' : null,
    event.shiftKey && shiftCounts(base) ? 'shift' : null,
    event.altKey ? 'alt' : null,
    base,
  ]
    .filter((p): p is string => p !== null)
    .join('+');
}

/** Whether a key may be given to a command. */
export function assignable(key: string): boolean {
  return normalizeKey(key) === key && !RESERVED.has(key);
}

/** A key for people: `Shift+L`, `Ctrl+G` (`⌘G`, `⇧L` on macOS), `PageUp`. */
export function displayKey(key: string, mac: boolean): string {
  const parts = key.split('+');
  const base = parts.pop() ?? '';
  const named: Record<string, string> = {
    pageup: 'PageUp',
    pagedown: 'PageDown',
    home: 'Home',
    end: 'End',
    insert: 'Insert',
  };
  const shown = named[base] ?? (base.length > 1 ? base.toUpperCase() : base.toUpperCase());
  if (mac) {
    const symbols: Record<string, string> = { ctrl: '⌘', shift: '⇧', alt: '⌥' };
    return parts.map((p) => symbols[p] ?? p).join('') + shown;
  }
  const words: Record<string, string> = { ctrl: 'Ctrl', shift: 'Shift', alt: 'Alt' };
  return [...parts.map((p) => words[p] ?? p), shown].join('+');
}

/** Keeps the valid entries of untrusted shortcut settings. */
export function sanitizeShortcuts(raw: unknown): Readonly<Record<string, string>> {
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    return {};
  }
  const out: Record<string, string> = {};
  for (const [id, key] of Object.entries(raw)) {
    if (COMMAND_IDS.has(id) && typeof key === 'string' && assignable(key)) {
      out[id] = key;
    }
  }
  return out;
}

/** The key of every command: the user's or the default. */
export function keyMap(overrides: Readonly<Record<string, string>>): ReadonlyMap<string, string> {
  return new Map(SHORTCUT_COMMANDS.map((c) => [c.id, overrides[c.id] ?? c.key]));
}

/** The other command of the same context that uses this key, if any. */
export function conflictOf(
  overrides: Readonly<Record<string, string>>,
  id: string,
  key: string,
): string | null {
  const command = SHORTCUT_COMMANDS.find((c) => c.id === id);
  if (command === undefined) {
    return null;
  }
  const keys = keyMap(overrides);
  const other = SHORTCUT_COMMANDS.find(
    (c) => c.id !== id && c.context === command.context && keys.get(c.id) === key,
  );
  return other?.id ?? null;
}
