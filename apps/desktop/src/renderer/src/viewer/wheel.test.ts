import { describe, expect, it } from 'vitest';

import {
  MAX_AHEAD_STEPS,
  WheelClassifier,
  nextZoomTarget,
  notches,
  wheelZoomFactor,
  type WheelInput,
} from './wheel';

const at = (time: number, values: Partial<WheelInput>): WheelInput => ({
  deltaMode: 0,
  deltaX: 0,
  deltaY: 0,
  timeStamp: time,
  ...values,
});

describe('wheel input', () => {
  it('tells wheels from trackpads', () => {
    const wheel = new WheelClassifier();
    // Windows: one event of 120 per notch.
    expect(wheel.classify(at(0, { deltaY: 100, wheelDeltaY: -120 }))).toBe('mouse');
    // A high-resolution wheel: a notch in parts.
    expect(wheel.classify(at(500, { deltaY: 33.3333, wheelDeltaY: -45 }))).toBe('mouse');
    // Lines (Firefox, Linux).
    expect(wheel.classify(at(900, { deltaMode: 1, deltaY: 3 }))).toBe('mouse');
    // macOS: the pixel delta is accelerated, the notch count is not.
    expect(wheel.classify(at(1500, { deltaY: 4.000244140625, wheelDeltaY: -120 }))).toBe('mouse');
    expect(notches(at(0, { deltaY: 4.000244140625, wheelDeltaY: -120 }))).toBe(1);
    const pad = new WheelClassifier();
    // macOS trackpad: three times the pixel delta, often sideways too.
    expect(pad.classify(at(0, { deltaY: 7, wheelDeltaY: -21 }))).toBe('trackpad');
    expect(pad.classify(at(10, { deltaX: 2, deltaY: 5 }))).toBe('trackpad');
    // A momentum event that looks like a notch stays part of the gesture.
    expect(pad.classify(at(20, { deltaY: 40, wheelDeltaY: -120 }))).toBe('trackpad');
    // After a pause it is a wheel again.
    expect(pad.classify(at(1000, { deltaY: 100, wheelDeltaY: -120 }))).toBe('mouse');
  });

  it('counts notches however they arrive', () => {
    expect(notches(at(0, { deltaY: 100, wheelDeltaY: -120 }))).toBe(1);
    expect(notches(at(0, { deltaY: -100, wheelDeltaY: 120 }))).toBe(-1);
    expect(notches(at(0, { deltaY: 50, wheelDeltaY: -60 }))).toBe(0.5);
    expect(notches(at(0, { deltaMode: 1, deltaY: 3 }))).toBe(1);
    expect(notches(at(0, { deltaY: 200 }))).toBe(2);
  });

  it('zooms one step per notch, in parts for parts of a notch', () => {
    const step = 1.15;
    const notch = wheelZoomFactor(at(0, { deltaY: -100, wheelDeltaY: 120 }), step);
    expect(notch).toBeCloseTo(step);
    // Two halves zoom as much as one notch.
    const half = wheelZoomFactor(at(0, { deltaY: -50, wheelDeltaY: 60 }), step);
    expect(half * half).toBeCloseTo(step);
    // Zooming out is the inverse.
    expect(wheelZoomFactor(at(0, { deltaY: 100, wheelDeltaY: -120 }), step)).toBeCloseTo(1 / step);
    // A tiny, slow turn still zooms a little; an accelerated one at most two steps.
    expect(wheelZoomFactor(at(0, { deltaY: -4, wheelDeltaY: 12 }), step)).toBeCloseTo(step ** 0.25);
    expect(wheelZoomFactor(at(0, { deltaY: -900, wheelDeltaY: 1080 }), step)).toBeCloseTo(
      step ** 2,
    );
  });

  it('never runs far ahead of the view', () => {
    const step = 1.15;
    let pending: number | null = null;
    for (let i = 0; i < 20; i += 1) {
      pending = nextZoomTarget(1, pending, step, step);
    }
    expect(pending).toBeCloseTo(step ** MAX_AHEAD_STEPS);
    expect(nextZoomTarget(1, null, 1 / step, step)).toBeCloseTo(1 / step);
  });
});
