/**
 * Stability: opening and closing documents again and again must not let
 * the memory of the user interface grow.
 */
import { expect, test } from '@playwright/test';

import { createProjectWithWizard, launchApp } from './fixtures';

test('opening and closing documents does not leak memory', async () => {
  test.setTimeout(600_000);
  const launched = await launchApp();
  const { page } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await createProjectWithWizard(page, 'Leak Demo', 'demo');
  const cdp = await page.context().newCDPSession(page);
  const heap = async (): Promise<number> => {
    await cdp.send('HeapProfiler.collectGarbage');
    await page.waitForTimeout(200);
    await cdp.send('HeapProfiler.collectGarbage');
    const usage = (await cdp.send('Runtime.getHeapUsage')) as { usedSize: number };
    return usage.usedSize / 1e6;
  };
  const board = page.locator('[data-testid^="tree-doc:"]', { hasText: 'Leak Demo' }).first();
  const sheet = page.locator('[data-testid^="tree-doc:"]', { hasText: 'Root' }).first();
  const samples: number[] = [];
  for (let round = 0; round < 30; round += 1) {
    for (const doc of [board, sheet]) {
      await doc.dblclick();
      await expect(page.locator('[data-testid="viewer-canvas"]').first()).toBeVisible();
      await page.waitForTimeout(150);
    }
    // Close every tab but the details.
    for (let i = 0; i < 5; i += 1) {
      const close = page.locator('[data-testid^="tab-close"]').first();
      if ((await close.count()) === 0) break;
      await close.click();
    }
    if (round % 5 === 4) {
      samples.push(await heap());
    }
  }
  await launched.close();
  // A few hundred kilobytes of caches are fine; a leak grows with every round.
  const first = samples[0] ?? 0;
  const last = samples.at(-1) ?? 0;
  expect(last - first, `heap ${samples.map((s) => s.toFixed(1)).join(' → ')} MB`).toBeLessThan(3);
});
