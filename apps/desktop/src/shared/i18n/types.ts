import type { en } from './en';

/** Turns every string literal leaf of a message tree into `string`. */
type Widen<T> = { readonly [K in keyof T]: T[K] extends string ? string : Widen<T[K]> };

/** Shape every language dictionary must have (derived from English). */
export type Messages = Widen<typeof en>;

/** Dot-separated path to a leaf string, for example `start.title`. */
type LeafPaths<T> = {
  [K in keyof T & string]: T[K] extends string ? K : `${K}.${LeafPaths<T[K]>}`;
}[keyof T & string];

/** Every valid translation key. */
export type MessageKey = LeafPaths<Messages>;

/** Values that can be inserted into `{placeholder}` slots. */
export type MessageVars = Readonly<Record<string, string | number>>;

/** Supported UI languages. */
export const LANGUAGES = ['de', 'en'] as const;
export type Language = (typeof LANGUAGES)[number];
