/**
 * Minimal Markdown parser for release notes. It understands the subset our
 * changelog and release notes use (headings, paragraphs, bullet and numbered
 * lists, tables, bold, inline code, links) and produces a data structure that
 * React renders as plain elements, so no HTML from the network ever reaches
 * the DOM.
 */

export type Inline =
  | { readonly type: 'text'; readonly text: string }
  | { readonly type: 'strong'; readonly text: string }
  | { readonly type: 'code'; readonly text: string }
  | { readonly type: 'link'; readonly text: string; readonly url: string };

export interface ListItem {
  /** Stable id for React keys. */
  readonly id: string;
  readonly content: readonly Inline[];
}

export interface TableCell {
  readonly id: string;
  readonly content: readonly Inline[];
}

export interface TableRow {
  readonly id: string;
  readonly cells: readonly TableCell[];
}

export type Block =
  | {
      readonly id: string;
      readonly type: 'heading';
      readonly level: number;
      readonly content: readonly Inline[];
    }
  | { readonly id: string; readonly type: 'paragraph'; readonly content: readonly Inline[] }
  | {
      readonly id: string;
      readonly type: 'list';
      readonly ordered: boolean;
      readonly items: readonly ListItem[];
    }
  | {
      readonly id: string;
      readonly type: 'table';
      readonly header: TableRow;
      readonly rows: readonly TableRow[];
    };

const INLINE_PATTERN = /\*\*([^*]+)\*\*|__([^_]+)__|`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\)/gu;

export function parseInline(text: string): Inline[] {
  const result: Inline[] = [];
  let last = 0;
  for (const match of text.matchAll(INLINE_PATTERN)) {
    const index = match.index;
    if (index > last) {
      result.push({ type: 'text', text: text.slice(last, index) });
    }
    const [, bold, boldAlt, code, linkText, linkUrl] = match;
    if (bold !== undefined || boldAlt !== undefined) {
      result.push({ type: 'strong', text: bold ?? boldAlt ?? '' });
    } else if (code !== undefined) {
      result.push({ type: 'code', text: code });
    } else if (linkText !== undefined && linkUrl !== undefined) {
      result.push({ type: 'link', text: linkText, url: linkUrl });
    }
    last = index + match[0].length;
  }
  if (last < text.length) {
    result.push({ type: 'text', text: text.slice(last) });
  }
  return result;
}

const HEADING = /^(#{1,6})\s+(.*?)\s*#*\s*$/u;
const BULLET = /^\s*[-*+]\s+(.*)$/u;
const NUMBERED = /^\s*\d+[.)]\s+(.*)$/u;
const CONTINUATION = /^\s{2,}\S/u;
const TABLE_LINE = /^\s*\|/u;
/** A line that only contains HTML tags (for example an image on the release page). */
const HTML_ONLY_LINE = /^\s*(?:<\/?[a-zA-Z][^>]*>\s*)+$/u;
const TABLE_SEPARATOR = /^\s*\|?\s*:?-{3,}:?\s*(?:\|\s*:?-{3,}:?\s*)*\|?\s*$/u;

function splitCells(line: string): string[] {
  return line
    .trim()
    .replace(/^\|/u, '')
    .replace(/\|$/u, '')
    .split('|')
    .map((cell) => cell.trim());
}

interface PendingList {
  readonly ordered: boolean;
  readonly items: string[];
}

/** Collects lines into blocks; keeps the open paragraph or list between lines. */
class BlockBuilder {
  readonly blocks: Block[] = [];
  private paragraph: string[] = [];
  private list: PendingList | null = null;
  private table: string[] = [];

  private nextId(): string {
    return `b${String(this.blocks.length)}`;
  }

  flushParagraph(): void {
    if (this.paragraph.length > 0) {
      this.blocks.push({
        id: this.nextId(),
        type: 'paragraph',
        content: parseInline(this.paragraph.join(' ')),
      });
      this.paragraph = [];
    }
  }

  flushList(): void {
    if (this.list !== null) {
      const id = this.nextId();
      this.blocks.push({
        id,
        type: 'list',
        ordered: this.list.ordered,
        items: this.list.items.map((text, index) => ({
          id: `${id}-${String(index)}`,
          content: parseInline(text),
        })),
      });
      this.list = null;
    }
  }

  flushTable(): void {
    if (this.table.length === 0) {
      return;
    }
    const [head, separator, ...body] = this.table;
    this.table = [];
    if (head === undefined || separator === undefined || !TABLE_SEPARATOR.test(separator)) {
      // Not a real table: keep the text.
      for (const line of [head, separator, ...body]) {
        if (line !== undefined) {
          this.paragraph.push(line.trim());
        }
      }
      this.flushParagraph();
      return;
    }
    const id = this.nextId();
    const row = (line: string, rowId: string): TableRow => ({
      id: rowId,
      cells: splitCells(line).map((text, index) => ({
        id: `${rowId}-${String(index)}`,
        content: parseInline(text),
      })),
    });
    this.blocks.push({
      id,
      type: 'table',
      header: row(head, `${id}-h`),
      rows: body.map((line, index) => row(line, `${id}-${String(index)}`)),
    });
  }

  tableLine(line: string): void {
    this.flushParagraph();
    this.flushList();
    this.table.push(line);
  }

  flush(): void {
    this.flushParagraph();
    this.flushList();
    this.flushTable();
  }

  heading(level: number, text: string): void {
    this.flush();
    this.blocks.push({ id: this.nextId(), type: 'heading', level, content: parseInline(text) });
  }

  listItem(ordered: boolean, text: string): void {
    this.flushParagraph();
    if (this.list?.ordered !== ordered) {
      this.flushList();
      this.list = { ordered, items: [] };
    }
    this.list.items.push(text);
  }

  /** Appends an indented line to the last list item. Returns false if no list is open. */
  continueListItem(text: string): boolean {
    const items = this.list?.items;
    if (items === undefined || items.length === 0) {
      return false;
    }
    const lastIndex = items.length - 1;
    items[lastIndex] = `${items[lastIndex] ?? ''} ${text}`;
    return true;
  }

  paragraphLine(text: string): void {
    this.flushList();
    this.paragraph.push(text);
  }
}

export function parseMarkdown(source: string): Block[] {
  const builder = new BlockBuilder();
  for (const rawLine of source.replace(/\r\n?/gu, '\n').split('\n')) {
    const line = rawLine.trimEnd();
    const heading = HEADING.exec(line);
    const bullet = BULLET.exec(line);
    const numbered = NUMBERED.exec(line);

    if (HTML_ONLY_LINE.test(line)) {
      // HTML is never rendered; decorative lines are skipped entirely.
      continue;
    }
    if (!TABLE_LINE.test(line)) {
      builder.flushTable();
    }
    if (line.trim() === '') {
      builder.flush();
    } else if (TABLE_LINE.test(line)) {
      builder.tableLine(line);
    } else if (heading !== null) {
      builder.heading(heading[1]?.length ?? 1, heading[2] ?? '');
    } else if (numbered !== null) {
      builder.listItem(true, numbered[1] ?? '');
    } else if (bullet !== null) {
      builder.listItem(false, bullet[1] ?? '');
    } else if (!(CONTINUATION.test(rawLine) && builder.continueListItem(line.trim()))) {
      builder.paragraphLine(line.trim());
    }
  }
  builder.flush();
  return builder.blocks;
}
