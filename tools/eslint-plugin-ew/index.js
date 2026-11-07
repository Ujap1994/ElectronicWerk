// @ts-check
/**
 * Project-specific ESLint rules for ElectronicWerk.
 *
 * `no-literal-ui-text`: every user-visible text must come from the i18n
 * dictionaries (German and English), so JSX text and user-facing attributes
 * must not contain hard-coded words.
 */

/** Attributes whose value is shown to the user. */
const UI_ATTRIBUTES = new Set(['title', 'aria-label', 'alt', 'placeholder', 'label']);

/** Text that contains at least one letter is considered translatable. */
const HAS_LETTER = /\p{L}/u;

/** @type {import('eslint').Rule.RuleModule} */
const noLiteralUiText = {
  meta: {
    type: 'problem',
    docs: { description: 'Disallow hard-coded user-visible text in JSX; use t() instead.' },
    messages: {
      literal: 'Hard-coded UI text "{{text}}". Add it to the i18n dictionaries and use t().',
    },
    schema: [],
  },
  create(context) {
    /**
     * @param {import('estree').Node} node
     * @param {string} text
     */
    const check = (node, text) => {
      const trimmed = text.trim();
      if (HAS_LETTER.test(trimmed)) {
        context.report({ node, messageId: 'literal', data: { text: trimmed.slice(0, 40) } });
      }
    };
    return {
      /** @param {any} node */
      JSXText(node) {
        check(node, node.value);
      },
      /** @param {any} node */
      JSXAttribute(node) {
        const name = node.name?.name;
        if (typeof name === 'string' && UI_ATTRIBUTES.has(name) && node.value?.type === 'Literal') {
          check(node, String(node.value.value));
        }
      },
    };
  },
};

export default {
  meta: { name: 'eslint-plugin-ew', version: '0.1.0' },
  rules: { 'no-literal-ui-text': noLiteralUiText },
};
