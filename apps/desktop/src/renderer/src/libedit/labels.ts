/** Texts of the library editors that do not depend on the app state. */
import type { MessageKey, Translate } from '@shared/i18n';

import type { Finding } from './types';

/** The translated text of a check finding (the engine's English as fallback). */
export function findingText(t: Translate, finding: Finding): string {
  const key = `libedit.checks.codes.${finding.code}` as MessageKey;
  const text = t(key, finding.args);
  return text === key ? finding.message : text;
}
