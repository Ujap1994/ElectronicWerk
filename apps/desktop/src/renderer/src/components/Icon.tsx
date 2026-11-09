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
  zoomIn: 'M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4M8 11h6M11 8v6',
  zoomOut: 'M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4M8 11h6',
  fit: 'M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5',
  focus: 'M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5M9 9h6v6H9z',
  flip: 'M12 3v18M9 7l-5 5 5 5zM15 7l5 5-5 5z',
  boxSelect: 'M4 7V4h3M10 4h4M17 4h3v3M20 10v4M20 17v3h-3M14 20h-4M7 20H4v-3M4 14v-4',
  lasso:
    'M12 4c4.4 0 8 2.5 8 5.5S16.4 15 12 15s-8-2.5-8-5.5S7.6 4 12 4zM7.5 13.5c-1.5 2-1 4.5 1.5 5.5',
  contrast: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 3v18M12 8h4M12 12h5M12 16h4',
  eye: 'M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z',
  eyeOff:
    'M3 3l18 18M10.6 5.1A10.9 10.9 0 0 1 12 5c6.5 0 10 7 10 7a17.6 17.6 0 0 1-3.2 4.2M6.6 6.6A17.4 17.4 0 0 0 2 12s3.5 7 10 7a10.3 10.3 0 0 0 5.4-1.6M9.9 9.9a3 3 0 0 0 4.2 4.2',
  layers: 'M12 3 2 8l10 5 10-5zM2 13l10 5 10-5M2 17.5l10 5 10-5',
  search: 'M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4',
  download: 'M12 4v11m0 0-4-4m4 4 4-4M4 19h16',
  cube: 'M12 3 4 7.5v9L12 21l8-4.5v-9zM4 7.5l8 4.5 8-4.5M12 12v9',
  duplicate: 'M8 8h12v12H8zM4 16V4h12',
  merge: 'M6 3v5l6 6v7M18 3v5l-6 6',
  split: 'M12 3v7l-6 6v5M12 10l6 6v5',
  copy: 'M9 9h11v11H9zM5 15H4V4h11v1',
  pointer: 'M5 3l14 8-6 1.5L10 19z',
  wire: 'M3 18h6V6h12M3 18h.01M21 6h.01',
  bus: 'M3 17h7V7h11M3 19h9V9h9',
  label: 'M3 8h12l5 4-5 4H3zM7 12h.01',
  globalLabel: 'M8 7h9l4 5-4 5H8l-4-5zM8 12h6',
  hierLabel: 'M3 7h11l6 5-6 5H3zM3 12h7',
  power: 'M12 21V9M6 9h12M12 9V3',
  ground: 'M12 3v9M5 12h14M8 16h8M11 20h2',
  noConnect: 'M6 6l12 12M18 6 6 18',
  junction: 'M3 12h18M12 12v9M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z',
  text: 'M5 6V4h14v2M12 4v16M9 20h6',
  chip: 'M7 4h10v16H7zM3 8h4M3 12h4M3 16h4M17 8h4M17 12h4M17 16h4',
  sheet: 'M4 4h16v16H4zM4 9h16M20 13h-3M20 16h-3',
  highlight: 'M3 12h4l2-5 4 10 2-5h6M17 5l2-2M19 9h2',
  rotate: 'M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7',
  mirrorH: 'M12 3v18M9 7l-5 5 5 5zM15 7l5 5-5 5z',
  mirrorV: 'M3 12h18M7 9l5-5 5 5zM7 15l5 5 5-5z',
  annotate: 'M9 4 7 20M17 4l-2 16M4 9h16M3 15h16',
  erc: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zM8 12l3 3 5-6',
  print: 'M7 9V3h10v6M7 17H4V9h16v8h-3M7 14h10v7H7z',
  grid: 'M4 4h.01M12 4h.01M20 4h.01M4 12h.01M12 12h.01M20 12h.01M4 20h.01M12 20h.01M20 20h.01',
  guide: 'M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9V16h7v-2.1A6 6 0 0 0 12 3z',
  cut: 'M6 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM8.1 7.9 20 20M8.1 16.1 20 4',
  paste: 'M9 4h6v3H9zM7 5H5v16h14V5h-2M9 12h6M9 16h6',
  move: 'M12 3v18M3 12h18M12 3l-3 3M12 3l3 3M12 21l-3-3M12 21l3-3M3 12l3-3M3 12l3 3M21 12l-3-3M21 12l-3 3',
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
