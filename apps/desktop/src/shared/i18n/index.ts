import { de } from './de';
import { en } from './en';
import {
  LANGUAGES,
  type Language,
  type MessageKey,
  type MessageVars,
  type Messages,
} from './types';

export { LANGUAGES, type Language, type MessageKey, type MessageVars, type Messages };

/** All dictionaries, keyed by language. */
export const DICTIONARIES: Readonly<Record<Language, Messages>> = { de, en };

/** Language used when the system language is not supported. */
export const FALLBACK_LANGUAGE: Language = 'en';

/** A UI language setting: an explicit language or "follow the system". */
export type LanguageSetting = Language | 'system';

export function isLanguage(value: unknown): value is Language {
  return typeof value === 'string' && (LANGUAGES as readonly string[]).includes(value);
}

/**
 * Picks the UI language. An explicit setting wins; otherwise the first
 * supported entry of the system's preferred languages (for example `de-AT`)
 * is used, falling back to English.
 */
export function resolveLanguage(
  setting: LanguageSetting,
  preferredSystemLanguages: readonly string[],
): Language {
  if (setting !== 'system') {
    return setting;
  }
  for (const tag of preferredSystemLanguages) {
    const base = tag.toLowerCase().split(/[-_]/u)[0];
    if (isLanguage(base)) {
      return base;
    }
  }
  return FALLBACK_LANGUAGE;
}

function lookup(messages: Messages, key: MessageKey): string | undefined {
  let node: unknown = messages;
  for (const part of key.split('.')) {
    if (typeof node !== 'object' || node === null) {
      return undefined;
    }
    node = (node as Record<string, unknown>)[part];
  }
  return typeof node === 'string' ? node : undefined;
}

/** Replaces `{name}` placeholders. Unknown placeholders stay visible. */
export function interpolate(template: string, vars?: MessageVars): string {
  if (vars === undefined) {
    return template;
  }
  return template.replace(/\{(\w+)\}/gu, (match, name: string) => {
    const value = vars[name];
    return value === undefined ? match : String(value);
  });
}

/** A translate function bound to one language. */
export type Translate = (key: MessageKey, vars?: MessageVars) => string;

/** Creates a translate function. Missing keys fall back to English, then to the key. */
export function createTranslator(language: Language): Translate {
  const messages = DICTIONARIES[language];
  return (key, vars) => {
    const template = lookup(messages, key) ?? lookup(en, key) ?? key;
    return interpolate(template, vars);
  };
}

/** Lists every key of a dictionary (used by tests and tooling). */
export function listKeys(tree: object, prefix = ''): string[] {
  return Object.entries(tree).flatMap(([name, value]) => {
    const path = prefix === '' ? name : `${prefix}.${name}`;
    return typeof value === 'string' ? [path] : listKeys(value as object, path);
  });
}
