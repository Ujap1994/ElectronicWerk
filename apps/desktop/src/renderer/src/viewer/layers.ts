/**
 * Layer appearance: colours, default opacity and draw order.
 */
import type { LayerGroup, SceneLayer } from './types';

export type CanvasTheme = 'light' | 'dark';

/** Colours of one canvas theme. */
export interface Palette {
  readonly background: string;
  readonly grid: string;
  /** Highlight colour and mix amounts for selection and hover. */
  readonly select: string;
  readonly selectMix: number;
  readonly hoverMix: number;
  /** Colour and mix amount of a highlighted net. */
  readonly highlight: string;
  readonly highlightMix: number;
}

const BOARD_PALETTE: Palette = {
  background: '#0e131a',
  grid: 'rgba(190, 205, 230, 0.32)',
  select: '#ffffff',
  selectMix: 0.6,
  hoverMix: 0.35,
  highlight: '#ffe14d',
  highlightMix: 0.75,
};

const SHEET_PALETTES: Readonly<Record<CanvasTheme, Palette>> = {
  light: {
    background: '#d4d1c9',
    grid: 'rgba(40, 40, 40, 0.28)',
    select: '#ff7a00',
    selectMix: 0.85,
    hoverMix: 0.45,
    highlight: '#0057ff',
    highlightMix: 0.9,
  },
  dark: {
    background: '#121418',
    grid: 'rgba(230, 230, 230, 0.2)',
    select: '#ffcf33',
    selectMix: 0.8,
    hoverMix: 0.4,
    highlight: '#5cc8ff',
    highlightMix: 0.9,
  },
};

export function palette(kind: 'pcb' | 'schematic', theme: CanvasTheme): Palette {
  return kind === 'pcb' ? BOARD_PALETTE : SHEET_PALETTES[theme];
}

const INNER_COLORS = [
  '#c2c200',
  '#c200c2',
  '#00c2c2',
  '#c26400',
  '#7f7fff',
  '#00b050',
  '#ff8080',
  '#80c0ff',
];

const BOARD_COLORS: Readonly<Record<string, string>> = {
  'cu.top': '#c83434',
  'cu.bottom': '#4d7fc4',
  'silk.top': '#f2eda1',
  'silk.bottom': '#e8b2a7',
  'mask.top': '#d864ff',
  'mask.bottom': '#02ffee',
  'paste.top': '#b4a0a0',
  'paste.bottom': '#a0b4b4',
  'adhesive.top': '#ff6060',
  'adhesive.bottom': '#6060ff',
  'fab.top': '#afafaf',
  'fab.bottom': '#6a74a0',
  'courtyard.top': '#ff26e2',
  'courtyard.bottom': '#26e9ff',
  outline: '#e8e04a',
  holes: '#05070a',
};

const SHEET_COLORS: Readonly<Record<CanvasTheme, Readonly<Record<string, string>>>> = {
  light: {
    'sch.page': '#fffef8',
    'sch.frame': '#8c3c3c',
    'sch.wires': '#008400',
    'sch.buses': '#0000c8',
    'sch.junctions': '#008400',
    'sch.labels': '#8c1515',
    'sch.power': '#8c1515',
    'sch.symbols': '#8c0000',
    'sch.symbolFill': '#ffffc2',
    'sch.pins': '#8c0000',
    'sch.pinText': '#007878',
    'sch.fields': '#006464',
    'sch.sheets': '#840084',
    'sch.noConnects': '#0000c8',
    'sch.texts': '#1d1d6e',
    'sch.shapes': '#1d1d6e',
  },
  dark: {
    'sch.page': '#1c1f26',
    'sch.frame': '#b06060',
    'sch.wires': '#3fbf3f',
    'sch.buses': '#5f8fff',
    'sch.junctions': '#3fbf3f',
    'sch.labels': '#e8c070',
    'sch.power': '#ef7f7f',
    'sch.symbols': '#d86a6a',
    'sch.symbolFill': '#2c2b22',
    'sch.pins': '#d86a6a',
    'sch.pinText': '#62c4c4',
    'sch.fields': '#86c8c8',
    'sch.sheets': '#c88ac8',
    'sch.noConnects': '#7090ff',
    'sch.texts': '#c4c4e6',
    'sch.shapes': '#c4c4e6',
  },
};

/** Default colour of a layer. */
export function layerColor(layer: SceneLayer, theme: CanvasTheme): string {
  if (layer.group === 'schematic') {
    return SHEET_COLORS[theme][layer.key] ?? '#888888';
  }
  const fixed = BOARD_COLORS[layer.key];
  if (fixed !== undefined) {
    return fixed;
  }
  const inner = /^cu\.in(\d+)$/u.exec(layer.key);
  if (inner !== null) {
    const n = Number(inner[1]);
    return INNER_COLORS[(n - 1) % INNER_COLORS.length] ?? '#c2c200';
  }
  if (layer.group === 'mechanical') {
    return '#00c896';
  }
  return '#c8c8c8';
}

const GROUP_OPACITY: Readonly<Record<LayerGroup, number>> = {
  copper: 0.85,
  silkscreen: 1,
  solderMask: 0.4,
  paste: 0.7,
  adhesive: 0.8,
  fabrication: 0.9,
  courtyard: 0.9,
  outline: 1,
  mechanical: 0.9,
  user: 0.9,
  holes: 1,
  schematic: 1,
};

export function defaultOpacity(layer: SceneLayer): number {
  return GROUP_OPACITY[layer.group];
}

/** `true` for layers that belong to a board side (mirrored when flipped). */
function sided(layer: SceneLayer): boolean {
  return layer.group === 'copper' || layer.side !== null;
}

/**
 * Draw order (back to front) as layer indices. When flipped, the board is
 * seen from the bottom. The active layer is drawn above the other sided
 * layers; the outline, mechanical and user layers and the drill holes stay
 * on top.
 */
export function drawOrder(
  layers: readonly SceneLayer[],
  active: number | null,
  flipped: boolean,
): number[] {
  const rank = (layer: SceneLayer, index: number): [number, number] => {
    if (layer.group === 'holes') {
      return [3, 0];
    }
    if (!sided(layer) && layer.group !== 'schematic') {
      return [2, layer.z];
    }
    if (index === active) {
      return [1, 0];
    }
    return [0, flipped && layer.group !== 'schematic' ? -layer.z : layer.z];
  };
  return layers
    .map((layer, index) => ({ index, rank: rank(layer, index) }))
    .sort((a, b) => a.rank[0] - b.rank[0] || a.rank[1] - b.rank[1])
    .map((entry) => entry.index);
}

/** Parses `#rrggbb` into 0–1 components. */
export function rgb(hex: string): [number, number, number] {
  const value = Number.parseInt(hex.slice(1, 7), 16);
  return [((value >> 16) & 255) / 255, ((value >> 8) & 255) / 255, (value & 255) / 255];
}

/** Parses `rgba(r, g, b, a)` or `#rrggbb` into 0–1 components. */
export function rgba(color: string): [number, number, number, number] {
  if (color.startsWith('#')) {
    return [...rgb(color), 1];
  }
  const parts = /rgba?\(([^)]+)\)/u.exec(color)?.[1]?.split(',') ?? [];
  const n = parts.map((part) => Number.parseFloat(part));
  return [(n[0] ?? 0) / 255, (n[1] ?? 0) / 255, (n[2] ?? 0) / 255, n[3] ?? 1];
}
