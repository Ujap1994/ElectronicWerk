/**
 * Translated names and formatted values for the viewer, layer panel and
 * inspector.
 */
import type { MessageKey, Translate } from '@shared/i18n';
import { formatLength, formatSize, type Units } from '@shared/project';

import type { InspectorRow } from './types';

const FIXED_LAYERS: Readonly<Record<string, MessageKey>> = {
  'cu.top': 'layerNames.copperTop',
  'cu.bottom': 'layerNames.copperBottom',
  'silk.top': 'layerNames.silkTop',
  'silk.bottom': 'layerNames.silkBottom',
  'mask.top': 'layerNames.maskTop',
  'mask.bottom': 'layerNames.maskBottom',
  'paste.top': 'layerNames.pasteTop',
  'paste.bottom': 'layerNames.pasteBottom',
  'adhesive.top': 'layerNames.adhesiveTop',
  'adhesive.bottom': 'layerNames.adhesiveBottom',
  'fab.top': 'layerNames.fabTop',
  'fab.bottom': 'layerNames.fabBottom',
  'courtyard.top': 'layerNames.courtyardTop',
  'courtyard.bottom': 'layerNames.courtyardBottom',
  outline: 'layerNames.outline',
  holes: 'layerNames.holes',
  'sch.page': 'layerNames.schPage',
  'sch.frame': 'layerNames.schFrame',
  'sch.wires': 'layerNames.schWires',
  'sch.buses': 'layerNames.schBuses',
  'sch.junctions': 'layerNames.schJunctions',
  'sch.labels': 'layerNames.schLabels',
  'sch.power': 'layerNames.schPower',
  'sch.symbols': 'layerNames.schSymbols',
  'sch.symbolFill': 'layerNames.schSymbolFill',
  'sch.pins': 'layerNames.schPins',
  'sch.pinText': 'layerNames.schPinText',
  'sch.fields': 'layerNames.schFields',
  'sch.sheets': 'layerNames.schSheets',
  'sch.noConnects': 'layerNames.schNoConnects',
  'sch.texts': 'layerNames.schTexts',
  'sch.shapes': 'layerNames.schShapes',
};

/** Display name of a layer key (a custom stack-up name wins). */
export function layerLabel(t: Translate, key: string, custom: string | null = null): string {
  if (custom !== null && custom !== '') {
    return custom;
  }
  const fixed = FIXED_LAYERS[key];
  if (fixed !== undefined) {
    return t(fixed);
  }
  const numbered = /^(cu\.in|mech\.|user\.)(\d+)$/u.exec(key);
  if (numbered !== null) {
    const n = numbered[2] ?? '';
    const prefix = numbered[1] ?? '';
    if (prefix === 'cu.in') {
      return t('layerNames.copperInner', { n });
    }
    return prefix === 'mech.' ? t('layerNames.mechanical', { n }) : t('layerNames.user', { n });
  }
  return key;
}

/** Translates `key`, or returns `fallback` if there is no translation. */
function optional(t: Translate, key: string, fallback: string): string {
  const text = t(key as MessageKey);
  return text === key ? fallback : text;
}

export function typeLabel(t: Translate, type: string): string {
  return optional(t, `inspector.types.${type}`, type);
}

export function fieldLabel(t: Translate, key: string): string {
  return optional(t, `inspector.fields.${key}`, key);
}

export interface FormatContext {
  readonly t: Translate;
  readonly units: Units;
  readonly locale: string;
}

function number(value: unknown): number {
  return typeof value === 'number' ? value : 0;
}

/** Formats a point (nanometres) in the project units. */
export function formatPoint(ctx: FormatContext, x: number, y: number): string {
  return ctx.t('inspector.point', {
    x: formatLength(x, ctx.units, ctx.locale),
    y: formatLength(y, ctx.units, ctx.locale),
  });
}

/** Formats one inspector value. */
export function formatRow(ctx: FormatContext, row: InspectorRow): string {
  const { t } = ctx;
  const { value } = row;
  switch (row.kind) {
    case 'length':
      return formatLength(number(value), ctx.units, ctx.locale);
    case 'point': {
      const [x, y] = Array.isArray(value) ? (value as unknown[]) : [];
      return formatPoint(ctx, number(x), number(y));
    }
    case 'size': {
      const [w, h] = Array.isArray(value) ? (value as unknown[]) : [];
      return formatSize(number(w), number(h), ctx.units, ctx.locale);
    }
    case 'angle':
      return `${new Intl.NumberFormat(ctx.locale, { maximumFractionDigits: 3 }).format(number(value))}°`;
    case 'bool':
      return value === true ? t('inspector.yes') : t('inspector.no');
    case 'layer':
      return typeof value === 'string' ? layerLabel(t, value) : '';
    case 'layers':
      return Array.isArray(value)
        ? value.map((key) => (typeof key === 'string' ? layerLabel(t, key) : '')).join(', ')
        : '';
    case 'net':
      return typeof value === 'string' ? value : t('inspector.noNet');
    case 'enum':
      return typeof value === 'string' ? optional(t, `inspector.values.${value}`, value) : '';
    case 'count':
      return new Intl.NumberFormat(ctx.locale).format(number(value));
    case 'text':
      return typeof value === 'string' ? value : JSON.stringify(value);
  }
}
