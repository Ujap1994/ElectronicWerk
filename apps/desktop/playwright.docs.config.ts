import { defineConfig } from '@playwright/test';

/** Screenshot generation for the documentation (not part of the test suite). */
export default defineConfig({
  testDir: './e2e/docs',
  outputDir: './test-results/docs',
  timeout: 120_000,
  expect: { timeout: 15_000 },
  workers: 1,
  reporter: 'list',
});
