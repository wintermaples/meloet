// Numbers and color choices the surface needs that ui/src/design/tokens.ts does not hold.
// Each one names where it was read: the spec (docs/design/spec.html) or a mock board
// (docs/design/mock/<board>.dc.html). Sizes are px at 100 %; only their ratios are used,
// because every picture is drawn into the rect of its slot.

import type { ColorName } from "../design/tokens";

/** Spec 1 (colors): "color.meter.ok メーターの 0〜72 %", "color.meter.hot メーターの 72 % より上". */
export const METER_HOT_FROM = 0.72;
/** Spec 2 (メーター): "音割れ（上はしに rec の 2px を 2 秒）". */
export const CLIP_HOLD_MS = 2000;

/**
 * Level scale of the mixer strip in A-Mixer: [dB, length from the quiet end].
 * The mock places the labels with `top: N%`; length = 1 - N / 100.
 */
export const DB_SCALE: readonly (readonly [number, number])[] = [
  [6, 1],
  [0, 0.82],
  [-6, 0.62],
  [-12, 0.44],
  [-24, 0.26],
];
/** The mock's last label is −∞ at length 0. A straight line needs a finite end: not in the spec or the mocks. */
export const DB_FLOOR = -60;
/** How fast a level falls on the screen, dB per second. Not in the spec or the mocks. */
export const FALL_DB_PER_S = 20 / 1.7;

/** Telemetry may switch a key that an Event set only when it was sent this long after the Event. */
export const EVENT_WINS_US = 100_000;

/** F-Connect2, the full input meter keyboard. */
export const KEYBOARD = {
  white: { w: 12, h: 32 },
  black: { w: 7, h: 19 },
  frame: "color.gray.6",
  whiteFill: "color.gray.3",
  blackFill: "color.gray.0",
  blackFrame: "color.gray.7",
  down: "color.accent",
} as const satisfies Record<string, ColorName | { w: number; h: number }>;

/** F-Connect2, the bar beside "ベロシティ 87". */
export const VELOCITY = { ground: "color.gray.4", fill: "color.accent", max: 127 } as const;

export const METER = {
  ground: "color.gray.0",
  ok: "color.meter.ok",
  hot: "color.meter.hot",
  clip: "color.rec",
} as const satisfies Record<string, ColorName>;

/** F-Empty / F-Connect1, the bar lines of the arrange area. */
export const ARRANGE_GRID = { line: "color.gray.4" } as const;

/** A-PianoRoll, the frame of the visible range: `top:1px; height:37px` in a strip 39 high. */
export const OVERVIEW = { frame: "color.accent", inset: 1 } as const;
