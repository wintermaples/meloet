// Keys and rows in every theme (the owner, user:2026-11-23): white keys and their rows are lighter than black keys and
// theirs; the dark themes keep the grays they had; rows that cannot be written get a hatch of gray.7 lines (drawn by
// the hatch program of gl/renderer.ts, one quad a row); the meter colors are the same in every theme.
import { afterEach, describe, expect, it, vi } from "vitest";
import * as G from "../src/components/pianoroll/geom";
import { paintRoll, paintRollKeys, type RollPicture } from "../src/components/pianoroll/paint";
import { THEME_IDS, palettes, type ThemeId } from "../src/design/themes";
import { colors, type ColorName } from "../src/design/tokens";
import { Batch } from "../src/gl/batch";
import { hexToRgb, rgb, setColorTheme } from "../src/gl/color";
import { paintKeyboard } from "../src/gl/paint";
import { Renderer } from "../src/gl/renderer";
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

// ---- pictures ----

const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8, hatch: 6 };
const BOX = { x0: 0, y0: 0, x1: 696, y1: 492 };
const LOCK = { root: 0, mode: "major", lock: true } as const;
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

describe("the hatch on rows that cannot be written", () => {
  afterEach(() => setColorTheme("dark"));

  it("one per row outside the scale, over the whole row (the line below left free), and only when scale lock is on", () => {
    const out = draw(picture());
    const hatches = out.hatches();
    const outKeys: number[] = [];
    for (let key = GEOM.topKey; key > GEOM.topKey - G.rowsShown(GEOM); key--) if (!G.writable(key, LOCK)) outKeys.push(key);
    expect(outKeys.length).toBeGreaterThan(10);
    expect(hatches.map((h) => h.slice(1, 5))).toEqual(outKeys.map((k) => [0, (72 - k) * 12, 696, (72 - k) * 12 + 11]));
    expect(hatches.every((h) => h[5] === 6)).toBe(true);
    expect(draw(picture({ scale: { ...LOCK, lock: false } })).hatches()).toEqual([]);
    expect(draw(picture({ scale: { root: 7, mode: "minor", lock: true } })).hatches().length).toBe(outKeys.length);
  });

  it("drawn after every row and before the lines and notes", () => {
    const out = draw(picture());
    const at = out.hatches()[0]![0]!;
    expect(out.hatches().every((h) => h[0] === at)).toBe(true);
    // Before them: the ground and the rows (as wide as the area). Right after them: a grid line.
    const before = out.rects().slice(0, at / 30);
    expect(before.length).toBe(1 + G.rowsShown(GEOM));
    expect(before.every((r) => r[0] === 0 && r[2] === 696)).toBe(true);
    const after = out.rects().slice(at / 30);
    expect(after[0]![2]! - after[0]![0]!).toBe(1);
    // The note on C#4 (a row outside the scale) is still drawn, above the hatch.
    expect(after.some((r) => r[1] === 11 * 12 && colorOf(r) === f32(colors["color.gray.10"]))).toBe(true);
  });

  it("the lines are tied to the song and the row: tick 0 at the row top is on a line; device pixels at other scales", () => {
    for (const scrollTick of [0, 7, 1000]) {
      const g = { ...GEOM, scrollTick };
      for (const h of draw(picture({ g })).hatches()) {
        const originX = Math.round(-scrollTick * g.ppt);
        expect((originX + h[2]! + h[6]!) % h[5]!, `${scrollTick}`).toBe(0);
        expect(h[6]).toBeGreaterThanOrEqual(0);
        expect(h[6]).toBeLessThan(h[5]!);
      }
    }
    const big = draw(picture({ sizes: { ...SIZES, hatch: 8 } }), 1.5, { x0: 0, y0: 0, x1: 1044, y1: 738 }).hatches();
    expect(big.every((h) => h[5] === 12)).toBe(true);
  });

  it("its color is gray.7 of the theme, and so are rows and keys: read by name when the theme changes", () => {
    for (const id of THEME_IDS) {
      setColorTheme(id);
      const p = palettes[id];
      const out = draw(picture());
      for (const h of out.hatches()) expect(h.slice(7).map(Math.fround).join(), id).toBe(f32(p["color.gray.7"]));
      const rowIn = (b: Batch, key: number) => b.rects().find((r) => r[0] === 0 && r[1] === (72 - key) * 12 && r[2] === 696 && r[3] === (72 - key) * 12 + 11)!;
      expect(colorOf(rowIn(out, 72)), id).toBe(f32(p["color.row.white"]));
      expect(colorOf(rowIn(out, 61)), id).toBe(f32(p["color.row.out"]));
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

describe("hatches in a picture (Batch)", () => {
  it("cut by the clip box; remember the floats of rectangles before them; reset and sameAs see them", () => {
    const b = new Batch();
    b.clip(0, 0, 50, 100);
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    b.hatch(-20, 10, 80, 22, rgb("color.gray.7"), 6, -1000, 10);
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    const [h] = b.hatches();
    expect(h!.slice(0, 7)).toEqual([30, 0, 10, 50, 22, 6, (((1000 - 10) % 6) + 6) % 6]);
    b.clip(0, 0, 50, 5);
    b.hatch(0, 10, 50, 22, rgb("color.gray.7"), 6, 0, 10);
    expect(b.hatches()).toHaveLength(1);

    const make = (pitch: number, color: ColorName = "color.gray.7") => {
      const c = new Batch();
      c.clip(0, 0, 100, 100);
      c.hatch(0, 0, 100, 12, rgb(color), pitch, 0, 0);
      return c;
    };
    expect(make(6).sameAs(make(6))).toBe(true);
    expect(make(6).sameAs(make(8))).toBe(false);
    expect(make(6).sameAs(make(6, "color.gray.8"))).toBe(false);
    expect(make(6).sameAs(new Batch())).toBe(false);
    b.reset();
    expect(b.hatches()).toEqual([]);
  });
});

describe("the renderer draws rectangles, then the hatch, then later rectangles", () => {
  afterEach(() => vi.restoreAllMocks());

  it("with the hatch program and its (pitch, shift); a picture without hatches is one draw, as before", () => {
    const calls: string[] = [];
    let program = 0;
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(function () {
      return new Proxy(
        {},
        {
          get: (_, name: string) => {
            if (name === "getShaderParameter" || name === "getProgramParameter") return () => true;
            if (name === "createProgram") return () => ({ n: ++program });
            if (typeof name === "string" && /^[A-Z_0-9]+$/.test(name)) return name;
            return (...args: unknown[]) => {
              if (name === "useProgram") calls.push(`useProgram ${(args[0] as { n: number }).n}`);
              if (name === "drawArrays") calls.push(`drawArrays ${args.join(" ")}`);
              if (name === "uniform2i") calls.push(`uniform2i ${args.slice(1).join(" ")}`);
              return {};
            };
          },
        },
      ) as never;
    });
    const r = new Renderer(document.createElement("canvas"), false);
    calls.length = 0;
    const b = new Batch();
    b.clip(0, 0, 100, 100);
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    b.hatch(0, 0, 100, 11, rgb("color.gray.7"), 6, 2, 0);
    b.hatch(0, 24, 100, 35, rgb("color.gray.7"), 6, 2, 24);
    b.rect(0, 0, 10, 10, rgb("color.gray.10"));
    r.draw(b, 100, 100);
    // Programs: 1 wave, 2 hatch, 3 rectangles (made in that order in setUp).
    expect(calls).toEqual([
      "drawArrays TRIANGLES 0 6",
      "useProgram 2",
      "uniform2i 6 4",
      "drawArrays TRIANGLE_STRIP 0 4",
      "uniform2i 6 4",
      "drawArrays TRIANGLE_STRIP 0 4",
      "useProgram 3",
      "drawArrays TRIANGLES 6 6",
    ]);
    calls.length = 0;
    b.reset();
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    r.draw(b, 100, 100);
    expect(calls).toEqual(["drawArrays TRIANGLES 0 12"]);
  });
});
