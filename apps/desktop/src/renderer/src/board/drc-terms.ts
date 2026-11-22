/** Terms of the design rules check in the UI language. */
import type { MessageKey, Translate } from '@shared/i18n';

/**
 * The engine names copper items and rule sources in English ("pad R1-2
 * (VCC)", "net class Power"); this gives them in the UI language.
 */
export function translateTerm(t: Translate, value: string): string {
  const item = /^(pad|track|via|zone)(?: ([^\s(]\S*))?(?: \(([^()]*)\))?$/u.exec(value);
  if (item !== null) {
    const [, kind = '', name = '', net] = item;
    const what =
      kind === 'pad'
        ? t('board.drc.items.pad', { name })
        : kind === 'zone' && name !== ''
          ? t('board.drc.items.namedZone', { name })
          : t(`board.drc.items.${kind}` as MessageKey);
    // A pad without a name keeps the engine's word.
    if (kind === 'pad' && name === '') {
      return value;
    }
    return net === undefined ? what : `${what} (${net})`;
  }
  const source = /^(net class|rule) (.+)$/u.exec(value);
  if (source !== null) {
    const [, kind = '', name = ''] = source;
    return kind === 'net class'
      ? t('board.drc.sources.netClass', { name })
      : t('board.drc.sources.rule', { name });
  }
  const fixed: Record<string, MessageKey> = {
    'board constraints': 'board.drc.sources.board',
    'pad clearance': 'board.drc.sources.pad',
    'zone clearance': 'board.drc.sources.zone',
  };
  const key = fixed[value];
  return key === undefined ? value : t(key);
}
