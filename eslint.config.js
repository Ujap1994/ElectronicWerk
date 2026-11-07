// @ts-check
import eslint from '@eslint/js';
import comments from '@eslint-community/eslint-plugin-eslint-comments/configs';
import eslintReact from '@eslint-react/eslint-plugin';
import prettier from 'eslint-config-prettier';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';

import ewPlugin from './tools/eslint-plugin-ew/index.js';

export default tseslint.config(
  {
    // Large local data folders and generated output are never linted.
    ignores: [
      'KiCAD-Files/**',
      'DipTrace-Files/**',
      'Muster-Projekte/**',
      '**/node_modules/**',
      '**/out/**',
      '**/dist/**',
      '**/release/**',
      '**/target/**',
      '**/coverage/**',
      '**/test-results/**',
      '**/playwright-report/**',
      'docs/.vitepress/cache/**',
      'docs/.vitepress/dist/**',
    ],
  },
  eslint.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  ...tseslint.configs.stylisticTypeChecked,
  comments.recommended,
  {
    languageOptions: {
      parserOptions: {
        projectService: { allowDefaultProject: ['*.js', 'scripts/*.mjs', 'tools/*/*.js'] },
        tsconfigRootDir: import.meta.dirname,
      },
    },
    linterOptions: { reportUnusedDisableDirectives: 'error' },
    rules: {
      // Every suppression must explain itself.
      '@eslint-community/eslint-comments/require-description': 'error',
      '@eslint-community/eslint-comments/no-unlimited-disable': 'error',
      '@typescript-eslint/consistent-type-imports': 'error',
      '@typescript-eslint/explicit-module-boundary-types': 'error',
      '@typescript-eslint/no-import-type-side-effects': 'error',
      '@typescript-eslint/switch-exhaustiveness-check': 'error',
      '@typescript-eslint/restrict-template-expressions': ['error', { allowNumber: true }],
      eqeqeq: ['error', 'always'],
      'no-console': 'error',
      'prefer-const': 'error',
    },
  },
  {
    // Node scripts and tooling.
    files: ['scripts/**/*.mjs', 'tools/**/*.js', '*.js'],
    languageOptions: { globals: globals.node },
    rules: { 'no-console': 'off' },
  },
  {
    // React UI: hooks rules, React best practices, no hard-coded UI text.
    files: ['apps/desktop/src/renderer/**/*.{ts,tsx}'],
    ...eslintReact.configs['strict-type-checked'],
    languageOptions: { globals: globals.browser },
  },
  {
    files: ['apps/desktop/src/renderer/**/*.{ts,tsx}'],
    plugins: { 'react-hooks': reactHooks, ew: ewPlugin },
    rules: {
      ...reactHooks.configs.flat['recommended-latest'].rules,
      'ew/no-literal-ui-text': 'error',
    },
  },
  {
    // Tests may use non-null assertions and loose typing on fixtures.
    files: ['**/*.test.ts', 'apps/desktop/e2e/**/*.ts'],
    rules: {
      '@typescript-eslint/no-non-null-assertion': 'off',
      '@typescript-eslint/explicit-module-boundary-types': 'off',
    },
  },
  {
    // Plain JavaScript files are not type-checked.
    files: ['**/*.js', '**/*.mjs'],
    ...tseslint.configs.disableTypeChecked,
  },
  {
    files: ['**/*.js', '**/*.mjs'],
    rules: { '@typescript-eslint/explicit-module-boundary-types': 'off' },
  },
  prettier,
);
