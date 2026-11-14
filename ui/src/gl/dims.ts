// What the surface needs from ui/src/design/tokens.ts, under the names the GL code uses, and
// which spec color plays which role. Sizes are px at 100 %; only their ratios are used, because
// every picture is drawn into the rect of its slot.

import { mock, plain, type ColorName } from "../design/tokens";

export const METER_HOT_FROM = plain["meter.hot-from"].value / 100;
export const CLIP_HOLD_MS = plain["meter.clip-hold"].value * 1000;

/** Level scale of the mixer strip: [dB, length from the quiet end]. */
export const DB_SCALE: readonly (readonly [number, number])[] = plain["mock.mixer.scale"].value.map(
  ([db, top]) => [db, 1 - top / 100] as const,
);
/** The mock's last label is −∞ at length 0. A straight line needs a finite end: not in the spec or the mocks. */
export const DB_FLOOR = -60;
/** How fast a level falls on the screen, dB per second. Not in the spec or the mocks. */
export const FALL_DB_PER_S = 20 / 1.7;

/** Telemetry may switch a key that an Event set only when it was sent this long after the Event. */
export const EVENT_WINS_US = 100_000;

/** F-Connect2 (a keyboard is there) and F-Connect1 (none: `dim`). */
export const KEYBOARD = {
  white: { w: mock["mock.input-meter.key-full-w"].value, h: mock["mock.input-meter.keys-full-h"].value },
  black: { w: mock["mock.input-meter.black-key-w"].value, h: mock["mock.input-meter.black-key-h"].value },
  whiteFill: "color.gray.3",
  blackFill: "color.gray.0",
  down: "color.accent",
  lit: { frame: "color.gray.6", blackFrame: "color.gray.7" },
  dim: { frame: "color.gray.5", blackFrame: "color.gray.6" },
} as const satisfies Record<string, ColorName | { w: number; h: number } | { frame: ColorName; blackFrame: ColorName }>;

/** F-Connect2, the bar beside "ベロシティ 87". `max` is the top MIDI velocity. */
export const VELOCITY = { ground: "color.gray.4", fill: "color.accent", max: 127 } as const;

export const METER = {
  ground: "color.gray.0",
  ok: "color.meter.ok",
  hot: "color.meter.hot",
  clip: "color.rec",
} as const satisfies Record<string, ColorName>;

/** F-Empty / F-Connect1, the bar lines of the arrange area. */
export const ARRANGE_GRID = { line: "color.gray.4" } as const;

/** A-PianoRoll, the frame of the visible range. */
export const OVERVIEW = { frame: "color.accent", inset: mock["mock.range-frame.inset"].value } as const;
