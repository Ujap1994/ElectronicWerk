import type { ReactNode } from 'react';

import { isProjectUrl } from '@shared/links';

import { parseMarkdown, type Block, type Inline } from '../markdown';

function renderInline(parts: readonly Inline[]): ReactNode[] {
  return parts.map((part, index) => {
    const key = `${part.type}-${String(index)}`;
    switch (part.type) {
      case 'text':
        return part.text;
      case 'strong':
        return <strong key={key}>{part.text}</strong>;
      case 'code':
        return <code key={key}>{part.text}</code>;
      case 'link':
        // Only links into this project are clickable; others stay plain text.
        return isProjectUrl(part.url) ? (
          <a
            key={key}
            href={part.url}
            onClick={(event) => {
              event.preventDefault();
              window.ew.app.openExternal(part.url);
            }}
          >
            {part.text}
          </a>
        ) : (
          part.text
        );
    }
  });
}

function renderBlock(block: Block): ReactNode {
  const key = block.id;
  switch (block.type) {
    case 'heading':
      // Release notes live inside a dialog with an <h2> title.
      return block.level <= 2 ? (
        <h3 key={key}>{renderInline(block.content)}</h3>
      ) : (
        <h4 key={key}>{renderInline(block.content)}</h4>
      );
    case 'paragraph':
      return <p key={key}>{renderInline(block.content)}</p>;
    case 'list': {
      const items = block.items.map((item) => <li key={item.id}>{renderInline(item.content)}</li>);
      return block.ordered ? <ol key={key}>{items}</ol> : <ul key={key}>{items}</ul>;
    }
    case 'table':
      return (
        <table key={key}>
          <thead>
            <tr>
              {block.header.cells.map((cell) => (
                <th key={cell.id}>{renderInline(cell.content)}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {block.rows.map((row) => (
              <tr key={row.id}>
                {row.cells.map((cell) => (
                  <td key={cell.id}>{renderInline(cell.content)}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      );
  }
}

/** Renders trusted-format Markdown (release notes) as plain React elements. */
export function Markdown({ source }: { readonly source: string }): ReactNode {
  return <div className="markdown">{parseMarkdown(source).map(renderBlock)}</div>;
}
