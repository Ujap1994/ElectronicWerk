/** Helpers for tests and screenshots of the 2D viewer. */
import { expect, type Locator, type Page } from '@playwright/test';

export type Point = [number, number];

/** The drawn viewer canvas. */
export function viewerCanvas(page: Page): Locator {
  return page.locator('[data-testid="viewer-canvas"][data-ready="true"]');
}

/** Waits until the view stops moving (zoom and fit animations). */
export async function settle(page: Page): Promise<void> {
  const canvas = viewerCanvas(page);
  await canvas.waitFor();
  await expect(async () => {
    const first = await canvas.getAttribute('data-view');
    await page.waitForTimeout(150);
    expect(await canvas.getAttribute('data-view')).toBe(first);
  }).toPass();
}

/** Opens a document from the project tree and waits until it is drawn. */
export async function openDocument(page: Page, name: string): Promise<Locator> {
  await page.locator('[data-testid^="tree-doc:"]', { hasText: name }).first().dblclick();
  await settle(page);
  return viewerCanvas(page);
}

/** Screen position of a design point (nanometres). */
export async function toScreen(canvas: Locator, [x, y]: Point): Promise<Point> {
  const box = await canvas.boundingBox();
  const [cx = 0, cy = 0, scale = 1, flipped = 0] = ((await canvas.getAttribute('data-view')) ?? '')
    .split(',')
    .map(Number);
  const sign = flipped === 1 ? -1 : 1;
  return [
    (box?.x ?? 0) + (box?.width ?? 0) / 2 + (x - cx) * scale * sign,
    (box?.y ?? 0) + (box?.height ?? 0) / 2 - (y - cy) * scale,
  ];
}

/** Zooms in with the mouse wheel at a design point. */
export async function zoomAt(page: Page, at: Point, notches: number): Promise<void> {
  const canvas = viewerCanvas(page);
  const [x, y] = await toScreen(canvas, at);
  await page.mouse.move(x, y);
  for (let i = 0; i < notches; i += 1) {
    await page.mouse.wheel(0, -120);
  }
  await settle(page);
}
