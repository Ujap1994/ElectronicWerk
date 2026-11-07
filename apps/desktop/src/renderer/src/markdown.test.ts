import { describe, expect, it } from 'vitest';

import { parseInline, parseMarkdown } from './markdown';

describe('parseInline', () => {
  it('splits bold, code and links from text', () => {
    expect(parseInline('Use **bold**, `code` and [docs](https://example.com) here')).toEqual([
      { type: 'text', text: 'Use ' },
      { type: 'strong', text: 'bold' },
      { type: 'text', text: ', ' },
      { type: 'code', text: 'code' },
      { type: 'text', text: ' and ' },
      { type: 'link', text: 'docs', url: 'https://example.com' },
      { type: 'text', text: ' here' },
    ]);
  });

  it('keeps HTML as plain text', () => {
    expect(parseInline('<img src=x onerror=alert(1)>')).toEqual([
      { type: 'text', text: '<img src=x onerror=alert(1)>' },
    ]);
  });
});

describe('parseMarkdown', () => {
  it('parses a changelog section', () => {
    const blocks = parseMarkdown(
      [
        '## Added',
        '',
        '- Start screen',
        '- Settings with',
        '  language and theme',
        '',
        '### Fixed',
        '1. First',
        '2. Second',
        '',
        'Thanks to **everyone**.',
        'Second line.',
      ].join('\n'),
    );
    expect(blocks).toEqual([
      { id: 'b0', type: 'heading', level: 2, content: [{ type: 'text', text: 'Added' }] },
      {
        id: 'b1',
        type: 'list',
        ordered: false,
        items: [
          { id: 'b1-0', content: [{ type: 'text', text: 'Start screen' }] },
          { id: 'b1-1', content: [{ type: 'text', text: 'Settings with language and theme' }] },
        ],
      },
      { id: 'b2', type: 'heading', level: 3, content: [{ type: 'text', text: 'Fixed' }] },
      {
        id: 'b3',
        type: 'list',
        ordered: true,
        items: [
          { id: 'b3-0', content: [{ type: 'text', text: 'First' }] },
          { id: 'b3-1', content: [{ type: 'text', text: 'Second' }] },
        ],
      },
      {
        id: 'b4',
        type: 'paragraph',
        content: [
          { type: 'text', text: 'Thanks to ' },
          { type: 'strong', text: 'everyone' },
          { type: 'text', text: '. Second line.' },
        ],
      },
    ]);
  });

  it('treats an indented line without an open list as paragraph text', () => {
    expect(parseMarkdown('   indented')).toEqual([
      { id: 'b0', type: 'paragraph', content: [{ type: 'text', text: 'indented' }] },
    ]);
  });

  it('handles Windows line endings and empty input', () => {
    expect(parseMarkdown('')).toEqual([]);
    expect(parseMarkdown('# Title\r\n\r\ntext')).toHaveLength(2);
  });

  it('starts a new list when the list type changes', () => {
    const blocks = parseMarkdown('- a\n1. b');
    expect(blocks.map((block) => block.type)).toEqual(['list', 'list']);
  });
});
