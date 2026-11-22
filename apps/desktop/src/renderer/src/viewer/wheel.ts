/**
 * Mouse wheel input of the 2D views: tells a wheel from a trackpad and turns
 * a wheel event into a zoom factor.
 *
 * Wheels differ a lot: a classic wheel sends one event per notch (Chromium:
 * `wheelDeltaY` ±120, `deltaY` about ±100 or ±53), high-resolution wheels
 * and some drivers send several smaller events per notch, and macOS speeds
 * fast turns up (`deltaY` grows, `wheelDeltaY` stays 120 per notch, since
 * Blink makes it from the raw notch count; for a trackpad it is three times
 * the pixel delta). The zoom therefore follows the amount the wheel turned —
 * a notch is one zoom step however many events it comes in — and a burst of
 * events cannot run the zoom far ahead of what is on the screen.
 */

import type { WheelZoomSetting } from '@shared/settings';

/** Zoom factor per wheel notch for each setting. */
export const WHEEL_STEPS: Readonly<Record<WheelZoomSetting, number>> = {
  fine: 1.08,
  normal: 1.15,
  coarse: 1.3,
};

let step = WHEEL_STEPS.normal;

/** Sets the zoom step of every view (from the settings). */
export function setWheelZoom(setting: WheelZoomSetting): void {
  step = WHEEL_STEPS[setting];
}

/** The zoom factor per wheel notch. */
export function wheelStep(): number {
  return step;
}

/** The parts of a `WheelEvent` the views use. */
export interface WheelInput {
  readonly deltaMode: number;
  readonly deltaX: number;
  readonly deltaY: number;
  /** Chromium's legacy delta: 120 per notch of a wheel. */
  readonly wheelDeltaY?: number | undefined;
  readonly timeStamp: number;
}

export type WheelDevice = 'mouse' | 'trackpad';

const DOM_DELTA_PIXEL = 0;
/** Pixels per notch when only `deltaY` is known. */
const PIXELS_PER_NOTCH = 100;
/** Lines per notch (Firefox, some Linux setups). */
const LINES_PER_NOTCH = 3;
/** Events closer than this belong to one gesture. */
const GESTURE_MS = 150;
/** At most this many steps of zoom may be waiting to be shown. */
export const MAX_AHEAD_STEPS = 3;

/** Reads `wheelDeltaY` (not in the DOM types any more, still in Chromium). */
export function inputOf(event: WheelEvent): WheelInput {
  const legacy = (event as WheelEvent & { wheelDeltaY?: number }).wheelDeltaY;
  return {
    deltaMode: event.deltaMode,
    deltaX: event.deltaX,
    deltaY: event.deltaY,
    wheelDeltaY: typeof legacy === 'number' ? legacy : undefined,
    timeStamp: event.timeStamp,
  };
}

/** Whether a single event looks like a wheel. */
function looksLikeWheel(input: WheelInput): boolean {
  if (input.deltaMode !== DOM_DELTA_PIXEL) {
    return true;
  }
  if (input.deltaX !== 0 || input.deltaY === 0) {
    return false;
  }
  const legacy = input.wheelDeltaY;
  if (legacy !== undefined && legacy !== 0) {
    // A notch (or a fixed part of one) is a multiple of 120 / 8 = 15;
    // trackpads report three times their pixel delta.
    if (Math.abs(Math.abs(legacy) - 3 * Math.abs(input.deltaY)) < 0.01) {
      return false;
    }
    return Math.abs(legacy) % 15 === 0;
  }
  return Number.isInteger(input.deltaY) && Math.abs(input.deltaY) >= 50;
}

/**
 * Tells a wheel from a trackpad. A trackpad gesture keeps its device: its
 * momentum events can look like a notch now and then.
 */
export class WheelClassifier {
  private last: { readonly device: WheelDevice; readonly time: number } | null = null;

  classify(input: WheelInput): WheelDevice {
    let device: WheelDevice = looksLikeWheel(input) ? 'mouse' : 'trackpad';
    const last = this.last;
    if (last?.device === 'trackpad' && input.timeStamp - last.time < GESTURE_MS) {
      device = 'trackpad';
    }
    this.last = { device, time: input.timeStamp };
    return device;
  }
}

/** How many notches an event of a wheel turned (signed, down positive). */
export function notches(input: WheelInput): number {
  if (input.deltaMode !== DOM_DELTA_PIXEL) {
    // Lines (or pages, counted as one notch each).
    return input.deltaMode === 1 ? input.deltaY / LINES_PER_NOTCH : Math.sign(input.deltaY);
  }
  const legacy = input.wheelDeltaY;
  if (legacy !== undefined && legacy !== 0) {
    return -legacy / 120;
  }
  return input.deltaY / PIXELS_PER_NOTCH;
}

/**
 * The zoom factor of one wheel event (> 1 zooms in). Each event counts for
 * at least a quarter and at most two notches, so slow turns still zoom and
 * fast, accelerated ones do not jump.
 */
export function wheelZoomFactor(input: WheelInput, perNotch = step): number {
  const n = notches(input);
  if (n === 0) {
    return 1;
  }
  const amount = Math.min(2, Math.max(0.25, Math.abs(n)));
  return perNotch ** (n < 0 ? amount : -amount);
}

/**
 * The new zoom target: the factor applied to the target still being
 * animated, but never more than a few steps away from the scale shown now.
 */
export function nextZoomTarget(
  shown: number,
  pending: number | null,
  factor: number,
  perNotch = step,
): number {
  const target = (pending ?? shown) * factor;
  const limit = perNotch ** MAX_AHEAD_STEPS;
  return Math.min(shown * limit, Math.max(shown / limit, target));
}
