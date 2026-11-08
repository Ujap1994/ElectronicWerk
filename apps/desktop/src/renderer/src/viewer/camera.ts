/**
 * The 2D camera: maps scene coordinates (nanometres, y up, relative to the
 * scene origin) to CSS pixels (y down) and back.
 */
import type { Box } from './types';

/** Zoom limits in CSS pixels per nanometre. */
export const MIN_SCALE = 1e-7;
export const MAX_SCALE = 0.05;

export interface CameraState {
  readonly cx: number;
  readonly cy: number;
  readonly scale: number;
  readonly flipped: boolean;
}

export class Camera {
  /** Zoom limits of this view (CSS pixels per nanometre). */
  minScale = MIN_SCALE;
  maxScale = MAX_SCALE;
  cx = 0;
  cy = 0;
  /** CSS pixels per nanometre. */
  scale = 1e-5;
  /** View from the bottom (x mirrored). */
  flipped = false;
  width = 1;
  height = 1;

  setViewport(width: number, height: number): void {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
  }

  get state(): CameraState {
    return { cx: this.cx, cy: this.cy, scale: this.scale, flipped: this.flipped };
  }

  set(state: CameraState): void {
    this.cx = state.cx;
    this.cy = state.cy;
    this.scale = this.clamp(state.scale);
    this.flipped = state.flipped;
  }

  /** Clamps a scale to the limits of this view. */
  clamp(scale: number): number {
    return Math.min(this.maxScale, Math.max(this.minScale, scale));
  }

  /**
   * Limits zooming out to a tenth of the size that fits `box`: further out,
   * the design would only be a few pixels large.
   */
  limitTo(box: Box): void {
    const fit = this.fitState(box, 0).scale;
    this.minScale = Math.max(MIN_SCALE, fit / 10);
    this.scale = this.clamp(this.scale);
  }

  private get sign(): number {
    return this.flipped ? -1 : 1;
  }

  toScreen(x: number, y: number): [number, number] {
    return [
      this.width / 2 + (x - this.cx) * this.scale * this.sign,
      this.height / 2 - (y - this.cy) * this.scale,
    ];
  }

  toScene(sx: number, sy: number): [number, number] {
    return [
      this.cx + ((sx - this.width / 2) / this.scale) * this.sign,
      this.cy - (sy - this.height / 2) / this.scale,
    ];
  }

  /** The visible scene rectangle. */
  visible(): Box {
    const hw = this.width / 2 / this.scale;
    const hh = this.height / 2 / this.scale;
    return [this.cx - hw, this.cy - hh, this.cx + hw, this.cy + hh];
  }

  /** Zooms by `factor`, keeping the scene point under (sx, sy) in place. */
  zoomAt(sx: number, sy: number, factor: number): void {
    const [x, y] = this.toScene(sx, sy);
    this.scale = this.clamp(this.scale * factor);
    const [nx, ny] = this.toScene(sx, sy);
    this.cx += x - nx;
    this.cy += y - ny;
  }

  /** Moves the view by a screen distance. */
  pan(dx: number, dy: number): void {
    this.cx -= (dx / this.scale) * this.sign;
    this.cy += dy / this.scale;
  }

  /** The camera state that shows `box` with a margin (fraction of the view). */
  fitState(box: Box, margin = 0.06): CameraState {
    const w = Math.max(box[2] - box[0], 1);
    const h = Math.max(box[3] - box[1], 1);
    const usable = 1 - 2 * margin;
    const scale = Math.min(
      MAX_SCALE,
      Math.max(MIN_SCALE, Math.min((this.width * usable) / w, (this.height * usable) / h)),
    );
    return {
      cx: (box[0] + box[2]) / 2,
      cy: (box[1] + box[3]) / 2,
      scale,
      flipped: this.flipped,
    };
  }

  /** Flips the view (top ↔ bottom), keeping the centre. */
  flip(): void {
    this.flipped = !this.flipped;
  }
}

export function clampScale(scale: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale));
}

/** Interpolates between two camera states (zoom in log space). */
export function interpolate(from: CameraState, to: CameraState, t: number): CameraState {
  const e = t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2;
  const scale = Math.exp(Math.log(from.scale) + (Math.log(to.scale) - Math.log(from.scale)) * e);
  // Keep the zoom anchor stable: interpolate the centre weighted by scale.
  return {
    cx: from.cx + (to.cx - from.cx) * e,
    cy: from.cy + (to.cy - from.cy) * e,
    scale,
    flipped: to.flipped,
  };
}

/** Grid spacing (scene units) that is at least `minPx` CSS pixels wide. */
export function gridStep(base: number, scale: number, minPx = 12): number {
  if (base <= 0) {
    return 0;
  }
  let step = base;
  const factors = [2, 2.5, 2];
  let i = 0;
  while (step * scale < minPx && step < 1e12) {
    step *= factors[i % factors.length] ?? 2;
    i += 1;
  }
  return step;
}

/** A zoom level as a percentage, where 100 % shows the real size. */
export function zoomPercent(scale: number, cssPxPerMm: number): number {
  return (scale * 1e6 * 100) / cssPxPerMm;
}
