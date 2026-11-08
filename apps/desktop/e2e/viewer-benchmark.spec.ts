/**
 * Viewer benchmark (ROADMAP section 6): pans and zooms the board of a large
 * design and measures the frame rate and the CPU time per frame.
 *
 * Runs only when EW_PERF_PROJECT points to a project, for example one made
 * with `ewerk synth L <dir>`. EW_PERF_MIN_FPS sets the frame-rate budget,
 * which is checked only on a hardware GPU (software renderers in CI virtual
 * machines are far slower). The results are written to
 * test-results/viewer-benchmark.json.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, test } from '@playwright/test';

import { APP_DIR, launchApp, queueDialogAnswer, type LaunchedApp } from './fixtures';

const PROJECT = process.env['EW_PERF_PROJECT'] ?? '';
const MIN_FPS = Number(process.env['EW_PERF_MIN_FPS'] ?? '50');
/** CPU time to prepare one frame; independent of the GPU. */
const MAX_CPU_MS = 4;
const SOFTWARE = /swiftshader|llvmpipe|softpipe|basic render|warp|microsoft/iu;

interface Run {
  readonly mode: string;
  readonly fps: number;
  readonly p95FrameMs: number;
  readonly cpuMs: number;
  readonly p95CpuMs: number;
}

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

test('pans and zooms a large board smoothly', async () => {
  test.skip(PROJECT === '', 'set EW_PERF_PROJECT to a large project');
  test.setTimeout(240_000);
  launched = await launchApp();
  const { app, page } = launched;
  await page.setViewportSize({ width: 1600, height: 1000 });
  await queueDialogAnswer(app, PROJECT);
  await page.getByTestId('open-project').click();
  await page.getByTestId('workspace').waitFor({ timeout: 60_000 });
  const started = Date.now();
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Board' }).first().dblclick();
  const canvas = page.locator('[data-testid="viewer-canvas"][data-ready="true"]');
  await canvas.waitFor({ timeout: 120_000 });
  const openMs = Date.now() - started;
  await page.getByTestId('viewer-fit').click();
  await page.waitForTimeout(500);

  const renderer = await page.evaluate(() => {
    const gl = document.createElement('canvas').getContext('webgl2');
    if (gl === null) {
      return 'none';
    }
    const info = gl.getExtension('WEBGL_debug_renderer_info');
    const name: unknown = gl.getParameter(
      info === null ? gl.RENDERER : info.UNMASKED_RENDERER_WEBGL,
    );
    return String(name);
  });

  const runs: Run[] = [];
  for (const mode of ['pan', 'zoom']) {
    const run = await page.evaluate(async (m): Promise<Omit<Run, 'mode'>> => {
      const target = document.querySelector<HTMLCanvasElement>('[data-testid="viewer-canvas"]');
      if (target === null) {
        throw new Error('no canvas');
      }
      const rect = target.getBoundingClientRect();
      const times: number[] = [];
      const cpu: number[] = [];
      await new Promise<void>((resolve) => {
        let n = 0;
        const step = (time: number): void => {
          times.push(time);
          cpu.push(Number(target.dataset['frameMs'] ?? '0'));
          n += 1;
          const dir = n <= 120 ? 1 : -1;
          const at = { clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2 };
          target.dispatchEvent(
            new WheelEvent(
              'wheel',
              m === 'pan'
                ? { ...at, deltaX: 9 * dir, deltaY: 5 * dir, bubbles: true, cancelable: true }
                : { ...at, deltaY: -4 * dir, ctrlKey: true, bubbles: true, cancelable: true },
            ),
          );
          if (n < 240) {
            requestAnimationFrame(step);
          } else {
            resolve();
          }
        };
        requestAnimationFrame(step);
      });
      const frames = times
        .slice(11)
        .map((time, i) => time - (times[i + 10] ?? time))
        .sort((a, b) => a - b);
      const cpuSorted = cpu.slice(10).sort((a, b) => a - b);
      const avg = (list: number[]): number => list.reduce((a, b) => a + b, 0) / list.length;
      const p95 = (list: number[]): number => list[Math.floor(list.length * 0.95)] ?? 0;
      return {
        fps: 1000 / avg(frames),
        p95FrameMs: p95(frames),
        cpuMs: avg(cpuSorted),
        p95CpuMs: p95(cpuSorted),
      };
    }, mode);
    runs.push({ mode, ...run });
  }

  const report = { project: PROJECT, renderer, openMs, runs };
  mkdirSync(join(APP_DIR, 'test-results'), { recursive: true });
  writeFileSync(
    join(APP_DIR, 'test-results', 'viewer-benchmark.json'),
    JSON.stringify(report, null, 2),
  );
  test.info().annotations.push({ type: 'benchmark', description: JSON.stringify(report) });
  for (const run of runs) {
    process.stdout.write(
      `viewer ${run.mode}: ${run.fps.toFixed(1)} fps (p95 frame ${run.p95FrameMs.toFixed(1)} ms), ` +
        `CPU ${run.cpuMs.toFixed(2)} ms (p95 ${run.p95CpuMs.toFixed(2)} ms) on ${renderer}\n`,
    );
    expect(run.p95CpuMs, `${run.mode}: CPU time per frame`).toBeLessThan(MAX_CPU_MS);
    if (!SOFTWARE.test(renderer)) {
      expect(run.fps, `${run.mode}: frame rate on ${renderer}`).toBeGreaterThanOrEqual(MIN_FPS);
    }
  }
});
