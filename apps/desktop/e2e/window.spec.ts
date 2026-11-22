/** The app's windows start maximized (the maintainer's wish). */
import { expect, test } from '@playwright/test';

import { launchApp } from './fixtures';

test('starts maximized', async () => {
  // Without a window manager (the Linux runners' virtual screen) there is
  // nothing that maximizes a window.
  test.skip(process.platform === 'linux', 'needs a window manager');
  const launched = await launchApp({ env: { EW_WINDOW_MAXIMIZE: '1' } });
  try {
    await expect
      .poll(() =>
        launched.app.evaluate(({ BrowserWindow }) =>
          BrowserWindow.getAllWindows().map((window) => window.isMaximized()),
        ),
      )
      .toEqual([true]);
  } finally {
    await launched.close();
  }
});
