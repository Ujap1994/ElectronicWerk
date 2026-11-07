import type { MessageKey } from '@shared/i18n';

/** A command shown in the command palette. */
export interface Command {
  readonly id: string;
  readonly label: MessageKey;
  readonly run: () => void;
}

/** Lower-cases and strips accents so "uberspringen" finds "überspringen". */
export function normalizeSearchText(text: string): string {
  return text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase();
}

/**
 * Filters and ranks commands for a query. Every word of the query must occur
 * in the label; labels with a word that starts with the query rank first. Commands also
 * match by their English label, so both languages always work.
 */
export function filterCommands<T extends { readonly searchText: readonly string[] }>(
  items: readonly T[],
  query: string,
): T[] {
  const words = normalizeSearchText(query).split(/\s+/u).filter(Boolean);
  if (words.length === 0) {
    return [...items];
  }
  const scored: { item: T; score: number; index: number }[] = [];
  items.forEach((item, index) => {
    const texts = item.searchText.map(normalizeSearchText);
    const matches = texts.some((text) => words.every((word) => text.includes(word)));
    if (matches) {
      // Rank labels with a word starting with the query first ("d" → "Deutsch").
      const first = words[0] ?? '';
      const prefix = texts.some((text) =>
        text.split(/[\s:]+/u).some((part) => part.startsWith(first)),
      );
      scored.push({ item, score: prefix ? 0 : 1, index });
    }
  });
  return scored.sort((a, b) => a.score - b.score || a.index - b.index).map((entry) => entry.item);
}
