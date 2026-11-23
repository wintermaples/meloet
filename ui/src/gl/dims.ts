// What the surface needs from ui/src/design/tokens.ts, under the names the GL code uses, and
// which spec color plays which role. Sizes are px at 100 %.

import { meterFall, meterScale, mock, part, plain, type ColorName } from "../design/tokens";

export const METER_HOT_FROM = plain["meter.hot-from"].value / 100;
export const CLIP_HOLD_MS = plain["meter.clip-hold"].value * 1000;
/** Width of the clip mark at the loud end, px at 100 %. */
export const CLIP_MARK = part["part.meter.clip"].value;

/**
 * Spec 2, "Scale marks of meters and faders": [dB, length 0..1 from the quiet end], loudest first, straight
 * lines between. Below the last point the length is 0 (−∞).
 */
export const DB_SCALE: readonly (readonly [number, number])[] = meterScale.map(
  ([db, percent]) => [db, percent / 100] as const,
);
export const DB_FLOOR = DB_SCALE[DB_SCALE.length - 1]![0];
/** How fast a level falls on the screen, dB per second. */
export const FALL_DB_PER_S = meterFall.fallDb / meterFall.fallSeconds;

/** Telemetry may switch a key that an Event set only when it was sent this long after the Event. */
export const EVENT_WINS_US = 100_000;

const FULL_H = mock["mock.input-meter.keys-full-h"].value;
const FULL_BLACK_H = mock["mock.input-meter.black-key-h"].value;
const SLIM_W = mock["mock.input-meter.key-slim-w"].value;

/**
 * Key sizes of the three forms of the input meter keyboard. `narrow` is the full meter with too
 * little width (spec 1: thin keys (width 8); black keys from F-MeterLongName, the 150 % full example).
 * Slim black keys: spec 3 (width 5, height 7).
 */
export const KEY_FORMS = {
  full: {
    white: { w: mock["mock.input-meter.key-full-w"].value, h: FULL_H },
    black: { w: mock["mock.input-meter.black-key-w"].value, h: FULL_BLACK_H },
  },
  narrow: {
    white: { w: SLIM_W, h: FULL_H },
    black: { w: mock["mock.input-meter.black-key-narrow-w"].value, h: FULL_BLACK_H },
  },
  slim: {
    white: { w: SLIM_W, h: mock["mock.input-meter.keys-slim-h"].value },
    black: { w: part["part.meter.black-key-slim-w"].value, h: part["part.meter.black-key-slim-h"].value },
  },
} as const;
export type KeyForm = keyof typeof KEY_FORMS;

/** F-Connect2 (a keyboard is there) and F-Connect1 (none: `dim`). The keys have colors of their own (user:2026-11-23). */
export const KEYBOARD = {
  whiteFill: "color.key.meter.white",
  blackFill: "color.key.meter.black",
  down: "color.accent",
  lit: { frame: "color.gray.6", blackFrame: "color.gray.7" },
  dim: { frame: "color.gray.5", blackFrame: "color.gray.6" },
} as const satisfies Record<string, ColorName | { frame: ColorName; blackFrame: ColorName }>;

/** F-Connect2, the bar beside 「ベロシティ 87」. `max` is the top MIDI velocity. */
export const VELOCITY = { ground: "color.gray.4", fill: "color.accent", max: 127 } as const;

export const METER = {
  ground: "color.gray.0",
  ok: "color.meter.ok",
  hot: "color.meter.hot",
  clip: "color.rec",
} as const satisfies Record<string, ColorName>;

/** Spec 3, "Scale marks of the mic meter" (38-B) and "Background and line of the mic meter": the dB in the middle, the ground, the white line of the 1 s peak. */
export const MIC_MID_DB = -12;
export const MIC_METER = { ground: "color.gray.4", peak: "color.gray.10" } as const satisfies Record<string, ColorName>;

/** F-Empty / F-Connect1, the bar lines of the arrange area. */
/** Bar lines alone are gray.4 (Main). With beat lines (B-61), as in the piano roll: bars gray.6, beats gray.4. */
export const ARRANGE_GRID = { line: "color.gray.4", bar: "color.gray.6", beat: "color.gray.4" } as const;

/**
 * The wave of a vocal clip (spec 2 "Audio clip", spec 3 (93-3, 93-4), F7-AudioEdit): bars of gray.8, the line of
 * silence gray.5 in a tall row, the lines of the fades gray.10. A clip whose file is not found: ground gray.2,
 * top line gray.7, no wave.
 */
export const WAVE = {
  bar: "color.gray.8",
  mid: "color.gray.5",
  fade: "color.gray.10",
  missingGround: "color.gray.2",
  missingTop: "color.gray.7",
} as const satisfies Record<string, ColorName>;

/** A-PianoRoll, the frame of the visible range. */
export const OVERVIEW = { frame: "color.accent", inset: mock["mock.range-frame.inset"].value } as const;
