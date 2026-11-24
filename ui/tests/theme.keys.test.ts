// Keys and rows in every theme (the owner, user:2026-11-23): white keys and their rows are lighter than black keys and
// theirs; the dark themes keep the grays they had; rows that cannot be written are row.out alone (user:2026-11-24); the
// meter colors are the same in every theme, on meter grounds of their own; lines have color names of their own (line.*).
// In the dark themes only the lines and gray.6 have values of their own.
import { afterEach, describe, expect, it } from "vitest";
import * as G from "../src/components/pianoroll/geom";
import { paintRoll, paintRollKeys, type RollPicture } from "../src/components/pianoroll/paint";
import { THEME_IDS, palettes, type ThemeId } from "../src/design/themes";
import { colors, type ColorName } from "../src/design/tokens";
import { Batch } from "../src/gl/batch";
import { hexToRgb, rgb, setColorTheme } from "../src/gl/color";
import { paintKeyboard, paintMeter } from "../src/gl/paint";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { GEOM, take } from "./pianoroll-fixture";

const lum = (hex: string) =>
  hexToRgb(hex)
    .map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
    .reduce((sum, c, i) => sum + c * [0.2126, 0.7152, 0.0722][i]!, 0);

const KEYS: readonly ColorName[] = [
  "color.key.white",
  "color.key.black",
  "color.key.meter.white",
  "color.key.meter.black",
  "color.row.white",
  "color.row.black",
  "color.row.out",
];

describe("the key and row colors", () => {
  it("the dark themes keep the grays these places had before they got names of their own", () => {
    const before: Record<string, ColorName> = {
      "color.key.white": "color.gray.5",
      "color.key.black": "color.gray.0",
      "color.key.meter.white": "color.gray.3",
      "color.key.meter.black": "color.gray.0",
      "color.row.white": "color.gray.2",
      "color.row.black": "color.gray.1",
      "color.row.out": "color.gray.0",
    };
    for (const id of ["dark", "pinkDark", "mintDark"] as ThemeId[]) {
      for (const name of KEYS) expect(palettes[id][name], `${id} ${name}`).toBe(palettes[id][before[name]!]);
    }
    expect([colors["color.key.white"], colors["color.key.black"], colors["color.key.meter.white"]]).toEqual(["#3C3C3C", "#000000", "#282828"]);
    expect([colors["color.row.white"], colors["color.row.black"], colors["color.row.out"]]).toEqual(["#212121", "#1A1A1A", "#000000"]);
  });

  it("in every theme a white key is lighter than a black key, a white-key row than a black-key row, and rows that cannot be written are the darkest", () => {
    for (const id of THEME_IDS) {
      const p = palettes[id];
      const l = (n: ColorName) => lum(p[n]);
      expect(l("color.key.white"), id).toBeGreaterThan(l("color.key.black"));
      expect(l("color.key.meter.white"), id).toBeGreaterThan(l("color.key.meter.black"));
      expect(l("color.row.white"), id).toBeGreaterThan(l("color.row.black"));
      expect(l("color.row.black"), id).toBeGreaterThan(l("color.row.out"));
    }
  });

  it("the meter colors are the same in every theme", () => {
    for (const id of THEME_IDS) {
      expect(palettes[id]["color.meter.ok"], id).toBe("#4CD964");
      expect(palettes[id]["color.meter.hot"], id).toBe("#FFD60A");
    }
  });
});

const ratio = (a: string, b: string) => (Math.max(lum(a), lum(b)) + 0.05) / (Math.min(lum(a), lum(b)) + 0.05);
const DARK: readonly ThemeId[] = ["dark", "pinkDark", "mintDark"];
const LIGHT: readonly ThemeId[] = ["light", "pinkLight", "mintLight"];

describe("the dark themes keep their faces (user:2026-11-23)", () => {
  // The values the older names have in the dark themes. gray.6 (button borders) is left out: it has a value of its own here.
  const OLD: readonly ColorName[] = [
    "color.gray.0", "color.gray.1", "color.gray.2", "color.gray.3", "color.gray.4", "color.gray.5", "color.gray.7", "color.gray.8",
    "color.gray.9", "color.gray.10", "color.accent", "color.rec", "color.meter.ok", "color.meter.hot", "color.track.1", "color.track.2",
    "color.track.3", "color.track.4", "color.track.5", "color.tip.bg", "color.tip.fg", "color.key.white", "color.key.black",
    "color.key.meter.white", "color.key.meter.black", "color.row.white", "color.row.black", "color.row.out",
  ];
  const KEPT: Record<string, string> = {
    dark: "#000000 #1A1A1A #212121 #282828 #2F2F2F #3C3C3C #717171 #9A9A9A #D6D6D6 #F2F2F2 #FFB000 #FF6158 #4CD964 #FFD60A #69E4E4 #5ABDF2 #6F8BED #9561C7 #FFD9F1 #F2F2F2 #000000 #3C3C3C #000000 #282828 #000000 #212121 #1A1A1A #000000",
    pinkDark: "#040404 #1E1C1E #262325 #2E292A #343131 #433D3F #787174 #9F999C #D8D6D7 #F3F2F2 #F9B8CF #FF685F #4CD964 #FFD60A #A8EDEC #92C9EA #8B9FDC #9D80BD #FFE9A6 #F3F2F2 #040404 #433D3F #040404 #2E292A #040404 #262325 #1E1C1E #040404",
    mintDark: "#040404 #1B1E1C #212624 #272D2B #2D3432 #3B403D #6B7773 #959C99 #D4D7D6 #F2F3F2 #AAEBD1 #FF6A61 #4CD964 #FFD60A #FFE9A6 #92C9EA #8B9FDC #9E81BE #FEF1F9 #F2F3F2 #040404 #3B403D #040404 #272D2B #040404 #212624 #1B1E1C #040404",
  };

  it("faces, text, states and keys keep their values; the meter grounds are the grays the meters were on (gray.0, gray.4 for the mic)", () => {
    const others = ["color.gray.6", "color.meter.clip", "color.meter.ground", "color.meter.ground.mic", "color.meter.peak"];
    expect(Object.keys(colors).filter((n) => !OLD.includes(n as ColorName) && !n.startsWith("color.line.")).sort()).toEqual(others);
    for (const id of DARK) {
      const p = palettes[id];
      expect(OLD.map((n) => p[n]).join(" "), id).toBe(KEPT[id]);
      expect(p["color.meter.ground"], id).toBe(p["color.gray.0"]);
      expect(p["color.meter.ground.mic"], id).toBe(p["color.gray.4"]);
      expect(p["color.meter.clip"], id).toBe(p["color.rec"]);
      expect(p["color.meter.peak"], id).toBe(p["color.gray.10"]);
    }
  });

  it("button borders: gray.6 is at most about 2.4 : 1 against gray.4, and the on border (accent) keeps 3 : 1 against it", () => {
    expect(colors["color.gray.6"]).toBe("#686868");
    for (const id of DARK) {
      const p = palettes[id];
      expect(ratio(p["color.gray.6"], p["color.gray.4"]), id).toBeGreaterThan(1.8);
      expect(ratio(p["color.gray.6"], p["color.gray.4"]), id).toBeLessThan(2.45);
      expect(ratio(p["color.accent"], p["color.gray.6"]), id).toBeGreaterThanOrEqual(3);
    }
    const light: Record<string, string> = { light: "#A7A7A7", pinkLight: "#B69CA7", mintLight: "#92A9A0" };
    for (const id of LIGHT) expect(palettes[id]["color.gray.6"], id).toBe(light[id]);
  });

  it("the line names reach their kind on the rows of the piano roll in every theme (bar 3, beat 1.5, grid 1.2)", () => {
    for (const id of THEME_IDS) {
      const p = palettes[id];
      for (const row of ["color.row.white", "color.row.black", "color.row.out"] as const) {
        expect(ratio(p["color.line.bar"], p[row]), `${id} ${row}`).toBeGreaterThanOrEqual(3);
        expect(ratio(p["color.line.beat"], p[row]), `${id} ${row}`).toBeGreaterThanOrEqual(1.5);
        expect(ratio(p["color.line.grid"], p[row]), `${id} ${row}`).toBeGreaterThanOrEqual(1.2);
      }
    }
  });
});

// ---- pictures ----

const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8 };
const BOX = { x0: 0, y0: 0, x1: 696, y1: 492 };
const LOCK = { root: 0, mode: "major", lock: true, highlight: true } as const;
const f32 = (hex: string) => hexToRgb(hex).map(Math.fround).join();
const colorOf = (r: number[]) => r.slice(4, 7).join();

function picture(over: Partial<RollPicture> = {}): RollPicture {
  return {
    g: GEOM,
    sizes: SIZES,
    scale: LOCK,
    meter: { num: 4, den: 4 },
    step: TPQ / 4,
    shown: G.shownOf(take([[0, TPQ, 61, 100]]), 0),
    origin: null,
    selected: null,
    live: null,
    drag: null,
    eraseHover: -1,
    ...over,
  };
}

function draw(p: RollPicture, dpr = 1, box = BOX): Batch {
  const out = new Batch();
  out.clip(box.x0, box.y0, box.x1, box.y1);
  paintRoll(out, box, p, null, dpr);
  return out;
}

describe("rows that cannot be written (user:2026-11-24)", () => {
  afterEach(() => setColorTheme("dark"));

  it("row.out alone: no hatch over them; the lines and the note on them are drawn as on other rows", () => {
    const out = draw(picture());
    expect(out.rects().slice(0, 1 + G.rowsShown(GEOM)).every((r) => r[0] === 0 && r[2] === 696)).toBe(true);
    const after = out.rects().slice(1 + G.rowsShown(GEOM));
    expect(after[0]![2]! - after[0]![0]!).toBe(2);
    // The note on C#4 (a row outside the scale).
    expect(after.some((r) => r[1] === 11 * 12 && colorOf(r) === f32(colors["color.gray.10"]))).toBe(true);
  });

  it("rows and keys are read by name when the theme changes; row.out only with scale lock on", () => {
    for (const id of THEME_IDS) {
      setColorTheme(id);
      const p = palettes[id];
      const out = draw(picture());
      const rowIn = (b: Batch, key: number) => b.rects().find((r) => r[0] === 0 && r[1] === (72 - key) * 12 && r[2] === 696 && r[3] === (72 - key) * 12 + 11)!;
      expect(colorOf(rowIn(out, 72)), id).toBe(f32(p["color.row.white"]));
      expect(colorOf(rowIn(out, 61)), id).toBe(f32(p["color.row.out"]));
      expect(colorOf(rowIn(draw(picture({ scale: { ...LOCK, lock: false, highlight: false } })), 61)), id).toBe(f32(p["color.row.black"]));
      expect(colorOf(rowIn(draw(picture({ scale: { ...LOCK, lock: false } })), 61)), id).toBe(f32(p["color.row.black"]));

      const keys = new Batch();
      keys.clip(0, 0, 55, 492);
      paintRollKeys(keys, { x0: 0, y0: 0, x1: 55, y1: 492 }, GEOM, (k) => k === 64, 1);
      const keyAt = (key: number) => keys.rects().find((r) => r[1] === (72 - key) * 12 && r[3] === (72 - key) * 12 + 11)!;
      expect(colorOf(keyAt(72)), id).toBe(f32(p["color.key.white"]));
      expect(colorOf(keyAt(70)), id).toBe(f32(p["color.key.black"]));
      expect(colorOf(keyAt(64)), id).toBe(f32(p["color.accent"]));

      const meter = new Batch();
      meter.clip(0, 0, 400, 40);
      paintKeyboard(meter, { x0: 0, y0: 0, x1: 1 + 29 * 12, y1: 32 }, { w: 7, h: 19 }, 36, 84, () => false);
      const fills = new Set(meter.rects().map(colorOf));
      expect(fills.has(f32(p["color.key.meter.white"])), id).toBe(true);
      expect(fills.has(f32(p["color.key.meter.black"])), id).toBe(true);
    }
  });
});

describe("the grid lines in every theme", () => {
  afterEach(() => setColorTheme("dark"));

  it("the grid lines of the note area are line.bar, line.beat and line.grid of the theme; bar lines 2 device pixels wide, the others 1", () => {
    for (const id of THEME_IDS) {
      setColorTheme(id);
      const p = palettes[id];
      const rects = draw(picture({ scale: { ...LOCK, lock: false } })).rects();
      const line = (x: number, w = 1) => rects.find((r) => r[0] === x && r[2] === x + w && r[1] === 0 && r[3] === 492)!;
      expect(colorOf(line(0, 2)), id).toBe(f32(p["color.line.bar"]));
      expect(colorOf(line(160, 2)), id).toBe(f32(p["color.line.bar"]));
      expect(colorOf(line(40)), id).toBe(f32(p["color.line.beat"]));
      expect(colorOf(line(10)), id).toBe(f32(p["color.line.grid"]));
    }
  });
});

describe("meters draw on their own grounds in every theme (user:2026-11-23)", () => {
  afterEach(() => setColorTheme("dark"));
  const silent = { length: 0, clipped: false };

  it("meter.ground, and meter.ground.mic for the mic meter; in the light themes both are the dark gray.9 of the theme. The clip mark is meter.clip, the peak line meter.peak", () => {
    for (const id of THEME_IDS) {
      setColorTheme(id);
      const p = palettes[id];
      const master = new Batch();
      master.clip(0, 0, 400, 40);
      paintMeter(master, { x0: 0, y0: 0, x1: 132, y1: 12 }, 4, 2, silent, silent);
      expect(master.rects().map(colorOf), id).toEqual([f32(p["color.meter.ground"]), f32(p["color.meter.ground"])]);
      const mic = new Batch();
      mic.clip(0, 0, 400, 40);
      paintMeter(mic, { x0: 0, y0: 0, x1: 200, y1: 8 }, 0, 2, silent, silent, false, { peak: 0 });
      expect(mic.rects().map(colorOf), id).toEqual([f32(p["color.meter.ground.mic"]), f32(p["color.meter.ground.mic"])]);
      if (LIGHT.includes(id)) {
        expect(p["color.meter.ground"], id).toBe(p["color.gray.9"]);
        expect(p["color.meter.ground.mic"], id).toBe(p["color.gray.9"]);
      }
      for (const ground of ["color.meter.ground", "color.meter.ground.mic"] as const) {
        expect(ratio(p["color.meter.ok"], p[ground]), `${id} ${ground}`).toBeGreaterThanOrEqual(3);
        expect(ratio(p["color.meter.hot"], p[ground]), `${id} ${ground}`).toBeGreaterThanOrEqual(3);
        expect(ratio(p["color.meter.clip"], p[ground]), `${id} ${ground}`).toBeGreaterThanOrEqual(3);
      }
      expect(ratio(p["color.meter.peak"], p["color.meter.ground.mic"]), id).toBeGreaterThanOrEqual(3);
      // A clipped level that has fallen: the mark stays at the loud end, on the ground. The mic meter's peak line.
      const fallen = { length: 0.5, clipped: true };
      const marks = new Batch();
      marks.clip(0, 0, 400, 40);
      paintMeter(marks, { x0: 0, y0: 0, x1: 200, y1: 8 }, 0, 2, fallen, silent, false, { peak: 0.75 });
      const of = (name: ColorName) => marks.rects().filter((r) => colorOf(r) === f32(p[name])).map((r) => r.slice(0, 4));
      expect(of("color.meter.clip"), id).toEqual([[198, 0, 200, 4]]);
      expect(of("color.meter.peak"), id).toEqual([[150, 0, 151, 8]]);
    }
  });
});
