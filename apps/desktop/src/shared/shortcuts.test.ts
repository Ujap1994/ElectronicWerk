import { describe, expect, it } from 'vitest';

import { sanitizeSettings } from './settings';
import {
  SHORTCUT_COMMANDS,
  assignable,
  conflictOf,
  displayKey,
  eventKey,
  keyMap,
  normalizeKey,
  sanitizeShortcuts,
} from './shortcuts';

const press = (
  key: string,
  mods: { ctrl?: boolean; meta?: boolean; shift?: boolean; alt?: boolean } = {},
) => ({
  key,
  ctrlKey: mods.ctrl ?? false,
  metaKey: mods.meta ?? false,
  shiftKey: mods.shift ?? false,
  altKey: mods.alt ?? false,
});

describe('keyboard shortcuts', () => {
  it('writes keys in one normal form', () => {
    expect(normalizeKey('Shift+Ctrl+G')).toBe('ctrl+shift+g');
    expect(normalizeKey('PageUp')).toBe('pageup');
    expect(normalizeKey('F2')).toBe('f2');
    expect(normalizeKey('shift+/')).toBe('/');
    expect(normalizeKey('ctrl+ctrl+g')).toBeNull();
    expect(normalizeKey('hyper+g')).toBeNull();
    expect(normalizeKey('arrowleft')).toBeNull();
    expect(normalizeKey('')).toBeNull();
  });

  it('reads key presses, with ⌘ as Ctrl and Shift only where it counts', () => {
    expect(eventKey(press('x'))).toBe('x');
    expect(eventKey(press('L', { shift: true }))).toBe('shift+l');
    expect(eventKey(press('g', { meta: true }))).toBe('ctrl+g');
    expect(eventKey(press('G', { ctrl: true, shift: true }))).toBe('ctrl+shift+g');
    // German keyboard: "/" is Shift+7.
    expect(eventKey(press('/', { shift: true }))).toBe('/');
    expect(eventKey(press('PageDown'))).toBe('pagedown');
    expect(eventKey(press('Shift', { shift: true }))).toBeNull();
  });

  it('every default key is valid and unique within its editor', () => {
    const seen = new Set<string>();
    for (const command of SHORTCUT_COMMANDS) {
      expect(assignable(command.key), command.id).toBe(true);
      const slot = `${command.context}|${command.key}`;
      expect(seen.has(slot), slot).toBe(false);
      seen.add(slot);
    }
  });

  it('refuses reserved keys and keys of other commands', () => {
    expect(assignable('ctrl+z')).toBe(false);
    expect(assignable('escape')).toBe(false);
    expect(assignable('ctrl+h')).toBe(true);
    expect(conflictOf({}, 'board.route', 'v')).toBe('board.via');
    // The schematic uses "v" for nothing: no conflict across editors.
    expect(conflictOf({}, 'schematic.wire', 'x')).toBe('schematic.mirrorX');
    expect(conflictOf({}, 'schematic.wire', 'v')).toBeNull();
    // A moved key frees its old place.
    expect(conflictOf({ 'board.via': 'shift+v' }, 'board.route', 'v')).toBeNull();
  });

  it('keeps only valid overrides from the settings file', () => {
    expect(
      sanitizeShortcuts({ 'board.route': 'w', 'board.nothing': 'q', 'board.via': 'ctrl+c', x: 1 }),
    ).toEqual({ 'board.route': 'w' });
    expect(sanitizeShortcuts([1, 2])).toEqual({});
    expect(sanitizeSettings({ shortcuts: { 'schematic.wire': 'e' } }).shortcuts).toEqual({
      'schematic.wire': 'e',
    });
    expect(keyMap({ 'schematic.wire': 'e' }).get('schematic.wire')).toBe('e');
  });

  it('shows keys for people', () => {
    expect(displayKey('shift+l', false)).toBe('Shift+L');
    expect(displayKey('ctrl+shift+g', false)).toBe('Ctrl+Shift+G');
    expect(displayKey('ctrl+shift+g', true)).toBe('⌘⇧G');
    expect(displayKey('pageup', false)).toBe('PageUp');
    expect(displayKey('`', false)).toBe('`');
  });
});
