// Keys and rows in every theme (the owner, user:2026-11-23): white keys and their rows are lighter than black keys and
// theirs; the dark themes keep the grays they had; rows that cannot be written get a hatch of row.hatch lines (drawn by
// the hatch program of gl/renderer.ts, one quad a row); the meter colors are the same in every theme, on meter grounds
// of their own; lines have color names of their own (line.*). In the dark themes only the lines, gray.6 and the hatch
// have values of their own.
import { afterEach, describe, expect, it, vi } from "vitest";
import * as G from "../src/components/pianoroll/geom";
import { paintRoll, paintRollKeys, type RollPicture } from "../src/components/pianoroll/paint";
import { THEME_IDS, palettes, type ThemeId } from "../src/design/themes";
import { colors, type ColorName } from "../src/design/tokens";
import { Batch } from "../src/gl/batch";
import { hexToRgb, rgb, setColorTheme } from "../src/gl/color";
import { paintKeyboard, paintMeter } from "../src/gl/paint";
import { HATCH_WIDTH, Renderer, hatchCover } from "../src/gl/renderer";
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
    const others = ["color.gray.6", "color.meter.clip", "color.meter.ground", "color.meter.ground.mic", "color.meter.peak", "color.row.hatch"];
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

const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8, hatch: 12 };
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

describe("the hatch on rows that cannot be written", () => {
  afterEach(() => setColorTheme("dark"));

  it("one per row outside the scale, over the whole row (the line below left free), and only when scale lock is on", () => {
    const out = draw(picture());
    const hatches = out.hatches();
    const outKeys: number[] = [];
    for (let key = GEOM.topKey; key > GEOM.topKey - G.rowsShown(GEOM); key--) if (!G.writable(key, LOCK)) outKeys.push(key);
    expect(outKeys.length).toBeGreaterThan(10);
    expect(hatches.map((h) => h.slice(1, 5))).toEqual(outKeys.map((k) => [0, (72 - k) * 12, 696, (72 - k) * 12 + 11]));
    expect(hatches.every((h) => h[5] === 12)).toBe(true);
    expect(draw(picture({ scale: { ...LOCK, lock: false } })).hatches()).toEqual([]);
    expect(draw(picture({ scale: { root: 7, mode: "minor", lock: true, highlight: true } })).hatches().length).toBe(outKeys.length);
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
    // 150 % (part.row.hatch 18 CSS px) on a screen of device pixel ratio 1.5.
    const big = draw(picture({ sizes: { ...SIZES, hatch: 18 } }), 1.5, { x0: 0, y0: 0, x1: 1044, y1: 738 }).hatches();
    expect(big.every((h) => h[5] === 27)).toBe(true);
  });

  it("its color is row.hatch of the theme, and so are rows and keys: read by name when the theme changes", () => {
    for (const id of THEME_IDS) {
      setColorTheme(id);
      const p = palettes[id];
      const out = draw(picture());
      expect(out.hatches().length, id).toBeGreaterThan(10);
      for (const h of out.hatches()) expect(h.slice(7).map(Math.fround).join(), id).toBe(f32(p["color.row.hatch"]));
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

describe("the hatch color and the grid lines in every theme", () => {
  afterEach(() => setColorTheme("dark"));

  it("row.hatch: just 3 : 1 against row.out in the dark themes (weaker than gray.7), gray.7 in the light themes", () => {
    expect(colors["color.row.hatch"]).toBe("#5A5A5A");
    for (const id of DARK) {
      const p = palettes[id];
      expect(ratio(p["color.row.hatch"], p["color.row.out"]), id).toBeGreaterThanOrEqual(3);
      expect(ratio(p["color.row.hatch"], p["color.row.out"]), id).toBeLessThan(3.05);
      expect(lum(p["color.row.hatch"]), id).toBeLessThan(lum(p["color.gray.7"]));
    }
    for (const id of LIGHT) expect(palettes[id]["color.row.hatch"], id).toBe(palettes[id]["color.gray.7"]);
  });

  it("the grid lines of the note area are line.bar, line.beat and line.grid of the theme", () => {
    for (const id of THEME_IDS) {
      setColorTheme(id);
      const p = palettes[id];
      const rects = draw(picture({ scale: { ...LOCK, lock: false } })).rects();
      const line = (x: number) => rects.find((r) => r[0] === x && r[2] === x + 1 && r[1] === 0 && r[3] === 492)!;
      expect(colorOf(line(0)), id).toBe(f32(p["color.line.bar"]));
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

describe("the cover of a hatch line (smooth edges, user:2026-11-23)", () => {
  it("1 on the line, 0 halfway between lines, a little on the pixels next to the line; a line is thin", () => {
    const pitch = 12;
    for (const shift of [0, 5, 11]) {
      const on = (pitch - shift) % pitch;
      expect(hatchCover(on, 0, pitch, shift)).toBe(1);
      expect(hatchCover(on + pitch, 3 * pitch, pitch, shift)).toBe(1);
      expect(hatchCover(on + pitch / 2, 0, pitch, shift)).toBe(0);
      const next = hatchCover(on + 1, 0, pitch, shift);
      expect(next).toBeGreaterThan(0);
      expect(next).toBeLessThan(0.5);
      expect(hatchCover(on, 1, pitch, shift)).toBe(next);
      expect(hatchCover(on + 2, 0, pitch, shift)).toBe(0);
    }
    // Rising to the right: one step right and one step up is the same line.
    expect(hatchCover(5, 7, pitch, 0)).toBe(1);
    expect(hatchCover(6, 6, pitch, 0)).toBe(1);
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
    const sources: string[] = [];
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
              if (name === "enable" || name === "disable") calls.push(`${name} ${String(args[0])}`);
              if (name === "shaderSource") sources.push(String(args[1]));
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
    b.hatch(0, 0, 100, 11, rgb("color.gray.7"), 12, 2, 0);
    b.hatch(0, 24, 100, 35, rgb("color.gray.7"), 12, 2, 24);
    b.rect(0, 0, 10, 10, rgb("color.gray.10"));
    r.draw(b, 100, 100);
    // Programs: 1 wave, 2 hatch, 3 rectangles (made in that order in setUp). Only the hatches blend.
    expect(calls).toEqual([
      "drawArrays TRIANGLES 0 6",
      "enable BLEND",
      "useProgram 2",
      "uniform2i 12 10",
      "drawArrays TRIANGLE_STRIP 0 4",
      "uniform2i 12 10",
      "drawArrays TRIANGLE_STRIP 0 4",
      "disable BLEND",
      "useProgram 3",
      "drawArrays TRIANGLES 6 6",
    ]);
    // The hatch shader works out the cover as hatchCover does.
    const hatchShader = sources.find((src) => src.includes("u_hatch"))!;
    expect(hatchShader).toContain(`const float REACH = ${(HATCH_WIDTH / 2 + 0.5).toFixed(6)};`);
    expect(hatchShader).toContain("clamp(REACH - float(min(u, u_hatch.x - u)) * 0.70710678, 0.0, 1.0)");
    expect(hatchShader).toContain("o_color = vec4(u_color, cover);");
    calls.length = 0;
    b.reset();
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    r.draw(b, 100, 100);
    expect(calls).toEqual(["drawArrays TRIANGLES 0 12"]);
  });
});
