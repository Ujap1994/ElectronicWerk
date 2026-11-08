import type { ReactNode } from 'react';

/** Stroke icons (24 × 24 grid, drawn with the current text colour). */
const PATHS = {
  plus: 'M12 5v14M5 12h14',
  folder: 'M3 7h6l2 2h10v10H3zM3 7V5h6',
  save: 'M5 3h11l3 3v15H5zM8 3v6h7V3M8 21v-7h8v7',
  undo: 'M9 14 4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3',
  redo: 'M15 14l5-5-5-5M20 9H10a6 6 0 0 0 0 12h3',
  archive: 'M3 4h18v4H3zM5 8v12h14V8M10 12h4',
  check: 'M4 12l5 5L20 6',
  close: 'M6 6l12 12M18 6 6 18',
  more: 'M5 12h.01M12 12h.01M19 12h.01',
  chevron: 'M9 6l6 6-6 6',
  schematic: 'M3 12h4l2-5 4 10 2-5h6',
  board: 'M4 4h16v16H4zM8 8h3v3H8zM14 13h2v3h-2zM11 9.5h5V13',
  library: 'M4 4h4v16H4zM10 4h4v16h-4zM16 5l3.5-1 2.5 15.5-3.5 1z',
  rules: 'M9 5h11M9 12h11M9 19h11M4 5h.01M4 12h.01M4 19h.01',
  output: 'M12 3v12m0 0-4-4m4 4 4-4M4 17v3h16v-3',
  file: 'M6 3h8l4 4v14H6zM14 3v4h4',
  project: 'M3 6l9-3 9 3v12l-9 3-9-3zM12 3v18M3 6l9 3 9-3',
  warning: 'M12 3 2 20h20zM12 10v4M12 17h.01',
  info: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zM12 11v6M12 7h.01',
  error: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zM15 9l-6 6M9 9l6 6',
  edit: 'M4 20h4L19 9l-4-4L4 16zM14 6l4 4',
  trash: 'M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3',
  reveal: 'M14 3h7v7M21 3l-9 9M19 14v6H4V5h6',
  github:
    'M9 19c-4 1.5-4-2-6-2.5M15 21v-3.5a3 3 0 0 0-.9-2.4c3-.3 6-1.5 6-6.6a5.2 5.2 0 0 0-1.4-3.6 4.8 4.8 0 0 0-.1-3.6s-1.1-.3-3.6 1.4a12.4 12.4 0 0 0-6.6 0C5.9 1 4.8 1.3 4.8 1.3a4.8 4.8 0 0 0-.1 3.6A5.2 5.2 0 0 0 3.3 8.5c0 5.1 3 6.3 6 6.6a3 3 0 0 0-.9 2.4V21',
  computer: 'M3 4h18v12H3zM8 20h8M12 16v4',
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({
  name,
  size = 18,
  className,
}: {
  readonly name: IconName;
  readonly size?: number;
  readonly className?: string;
}): ReactNode {
  return (
    <svg
      className={className === undefined ? 'icon' : `icon ${className}`}
      viewBox="0 0 24 24"
      width={size}
      height={size}
      aria-hidden="true"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
