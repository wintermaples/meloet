// The color themes: switching them changes the CSS variables and the colors WebGL reads, and draws every surface
// once again with the contexts it has. The themes here are made up for the test (every color of every theme is
// different), so the checks hold whatever the real values in design/themes.ts are.
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeState } from "../src/bridge/fake";
import { readFileSync } from "node:fs";
import { topMenus, type ListEntry, type ListRow } from "../src/components/menu/menus";
import { cursorCss } from "../src/components/pianoroll/cursor";
import { DEFAULT_THEME, THEME_IDS, palettes, type Palette, type ThemeId } from "../src/design/themes";
import { colors, cssVariables, type ColorName } from "../src/design/tokens";
import type { Batch } from "../src/gl/batch";
import { colorTheme, hexToRgb, rgb, setColorTheme } from "../src/gl/color";
import { Engine } from "../src/gl/engine";
import { contextsMade, resetPool } from "../src/gl/pool";
import { Renderer } from "../src/gl/renderer";
import { glRegion, glSlot, type GlSlot } from "../src/gl/slot";
import type { RegionName } from "../src/gl/budget";
import { t } from "../src/text/ja";

vi.mock("../src/design/themes", async (importOriginal) => {
  const real = await importOriginal<typeof import("../src/design/themes")>();
  const hh = (n: number) => n.toString(16).padStart(2, "0").toUpperCase();
  const names = Object.keys(real.palettes.dark);
  const made = (id: string, k: number) =>
    Object.fromEntries(
      names.map((name, j) => {
        const base = id.endsWith("Light") ? 0xc0 : 0x10;
        return [name, `#${hh(base + k)}${hh(base + j)}${hh(base + 0x10 + j)}`];
      }),
    );
  const palettes = Object.fromEntries(real.THEME_IDS.map((id, k) => [id, k === 0 ? real.palettes.dark : made(id, k)]));
  return { ...real, palettes };
});

const NAMES = Object.keys(colors) as ColorName[];
const valuesOf = (p: Palette) => new Set(NAMES.map((n) => p[n].toUpperCase()));
/** The colors as getComputedStyle writes them. */
const cssRgbOf = (p: Palette) =>
  new Set(
    NAMES.map((n) => {
      const [r, g, b] = hexToRgb(p[n]).map((c) => Math.round(c * 255));
      return `rgb(${r}, ${g}, ${b})`;
    }),
  );
/** The batch keeps colors as 32-bit floats. */
const f32Of = (p: Palette) => new Set(NAMES.map((n) => hexToRgb(p[n]).map(Math.fround).join()));

describe("the made-up themes", () => {
  it("dark is the spec's table; no color of one theme is in another", () => {
    expect(palettes[DEFAULT_THEME]).toBe(colors);
    const seen = new Set<string>();
    for (const id of THEME_IDS) {
      const v = valuesOf(palettes[id]);
      if (id !== DEFAULT_THEME) expect(v.size, id).toBe(NAMES.length);
      for (const c of v) {
        expect(seen.has(c), `${id} ${c}`).toBe(false);
        seen.add(c);
      }
    }
  });
});

describe("no color is written outside design/", () => {
  const sources = import.meta.glob(["../src/**/*.ts", "../src/**/*.svelte", "!../src/design/**", "!../src/bridge/fake*.ts", "!../src/bridge/generated/**"], {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>;

  it("the spec's table (tokens.ts `colors`) is read only through a theme; no #RRGGBB in the code", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(100);
    const bad: string[] = [];
    for (const [file, text] of Object.entries(sources)) {
      if (/import\s*\{[^}]*\bcolors\b[^}]*\}\s*from\s*["'][./]*design\/tokens["']/.test(text)) bad.push(`${file}: colors`);
      for (const m of text.matchAll(/["'`]#[0-9a-fA-F]{6}\b/g)) bad.push(`${file}: ${m[0]}`);
    }
    expect(bad).toEqual([]);
  });
});

describe("CSS variables", () => {
  it("the color variables are those of the theme; the other values do not change", () => {
    const dark = cssVariables(100, 1);
    for (const id of THEME_IDS) {
      const v = cssVariables(100, 1, palettes[id]);
      for (const n of NAMES) expect(v[`--${n.replaceAll(".", "-")}`]).toBe(palettes[id][n]);
      for (const [k, value] of Object.entries(v)) if (!k.startsWith("--color-")) expect(value, k).toBe(dark[k]);
    }
  });
});

// ---- WebGL ----

class FakeResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

function fakePage() {
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal("requestAnimationFrame", (f: FrameRequestCallback) => frames.push(f));
  vi.stubGlobal("cancelAnimationFrame", () => {});
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(() => {
    return new Proxy(
      {},
      {
        get: (_, name: string) => {
          if (name === "getShaderParameter" || name === "getProgramParameter") return () => true;
          return typeof name === "string" && /^[A-Z_0-9]+$/.test(name) ? 1 : () => ({});
        },
      },
    ) as never;
  });
  /** `canvas`: the renderer of the context that drew. */
  const drawn: { canvas: Renderer; rects: number[][] }[] = [];
  const draw = Renderer.prototype.draw;
  vi.spyOn(Renderer.prototype, "draw").mockImplementation(function (this: Renderer, batch: Batch, width: number, height: number) {
    drawn.push({ canvas: this, rects: batch.rects() });
    draw.call(this, batch, width, height);
  });
  const frame = () => {
    for (const f of frames.splice(0)) f(0);
  };
  return { drawn, frame, frames };
}

function box(rect: DOMRect, parent: HTMLElement = document.body): HTMLElement {
  const el = parent.appendChild(document.createElement("div"));
  el.getBoundingClientRect = () => rect;
  return el;
}

const actions: { destroy(): void }[] = [];
function region(node: HTMLElement, name: RegionName): HTMLElement {
  actions.push(glRegion(node, name));
  return node;
}
function slot(node: HTMLElement, s: GlSlot): void {
  actions.push(glSlot(node, s));
}

describe("WebGL surfaces", () => {
  afterEach(() => {
    for (const a of actions.splice(0)) a.destroy();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    document.body.innerHTML = "";
    setColorTheme(DEFAULT_THEME);
  });

  it("rgb reads the colors of the theme set; the same theme again keeps what it read", () => {
    expect(colorTheme()).toBe("dark");
    const before = rgb("color.gray.4");
    expect(setColorTheme("dark")).toBe(false);
    expect(rgb("color.gray.4")).toBe(before);
    expect(setColorTheme("pinkLight")).toBe(true);
    for (const n of NAMES) expect(rgb(n)).toEqual(hexToRgb(palettes.pinkLight[n]));
    expect(rgb("color.gray.4")).not.toEqual(before);
  });

  it("a new theme draws every surface once, in its colors only, with no new context", () => {
    resetPool();
    const { drawn, frame } = fakePage();
    const engine = new Engine();
    const meterHost = region(box(new DOMRect(400, 0, 200, 30)), "masterMeter");
    slot(box(new DOMRect(420, 10, 120, 6), meterHost), { kind: "meter", source: "master", gap: 1 });
    slot(region(box(new DOMRect(0, 40, 349, 32)), "pianoRollKeys"), { kind: "keyboard", size: "full", firstNote: 36, lastNote: 84 });
    slot(region(box(new DOMRect(0, 80, 640, 200)), "arrangeLanes"), {
      kind: "rects",
      rects: [
        { x: 0, y: 0, w: 100, h: 40, color: "color.track.2" },
        { x: 10, y: 2, w: 20, h: 2, color: "color.gray.10" },
      ],
    });
    slot(region(box(new DOMRect(0, 300, 640, 40)), "overview"), { kind: "overview" });
    frame();
    const surfaces = new Set(drawn.map((d) => d.canvas)).size;
    expect(surfaces).toBe(4);
    const made = contextsMade();
    const darkDraws = drawn.splice(0);
    const darkColors = f32Of(palettes.dark);
    for (const d of darkDraws) for (const r of d.rects) expect(darkColors.has(r.slice(4).join())).toBe(true);

    for (const id of ["light", "mintDark"] as ThemeId[]) {
      engine.setTheme(id);
      engine.setTheme(id);
      frame();
      expect(drawn).toHaveLength(surfaces);
      expect(new Set(drawn.map((d) => d.canvas)).size).toBe(surfaces);
      const mine = f32Of(palettes[id]);
      for (const d of drawn) for (const r of d.rects) expect(mine.has(r.slice(4).join()), `${id} ${r.slice(4).join()}`).toBe(true);
      expect(drawn.flatMap((d) => d.rects).length).toBe(darkDraws.flatMap((d) => d.rects).length);
      drawn.length = 0;
      // Nothing more until something changes.
      engine.invalidate();
      frame();
      expect(drawn).toHaveLength(0);
    }
    expect(contextsMade()).toBe(made);
    engine.destroy();
  });
});

// ---- the menu ----

const rows = (entries: readonly ListEntry[]) => entries.filter((e): e is ListRow => e.kind === "row");

describe("「表示 ▸ テーマ ▸」", () => {
  it("after 「拡大率 ▸」 in 「表示」; the 6 themes in order, a mark on the one chosen; a row sends setTheme", () => {
    for (const chosen of THEME_IDS) {
      const state: AppState = fakeState();
      state.workspace.theme = chosen;
      const sent: Command[] = [];
      const menus = topMenus(state, { state: () => state, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 }, "other");
      const view = rows(menus[2]!.entries);
      const at = view.findIndex((r) => r.label === "テーマ");
      const theme = view[at]!;
      expect(view[at - 1]!.label).toBe("拡大率");
      const sub = rows(theme.sub!);
      expect(sub.map((r) => r.label)).toEqual(["ダーク", "ライト", "ピンク ・ ダーク", "ピンク ・ ライト", "ミント ・ ダーク", "ミント ・ ライト"]);
      expect(sub.map((r) => r.label)).toEqual(THEME_IDS.map((id) => t(`theme.${id}`)));
      expect(sub.filter((r) => r.checked).map((r) => r.label)).toEqual([t(`theme.${chosen}`)]);
      expect(sub.every((r) => r.usable && r.checked !== undefined && r.key === undefined)).toBe(true);
      for (const r of sub) r.run!();
      expect(sent).toEqual(THEME_IDS.map((id) => ({ type: "setTheme", theme: id })));
    }
  });
});

// ---- the cursor (a picture with colors in it) ----

describe("pencil and eraser", () => {
  it("are drawn in the colors of the theme", () => {
    for (const kind of ["pencil", "eraser"] as const) {
      for (const id of THEME_IDS) {
        const css = decodeURIComponent(cursorCss(kind, 125, id));
        expect(css).toContain(`fill="${palettes[id]["color.gray.10"]}"`);
        expect(css).toContain(`stroke="${palettes[id]["color.gray.0"]}"`);
      }
    }
    expect(cursorCss("crosshair" as never, 100, "light")).toBe("crosshair");
  });
});

// ---- the whole screen ----

vi.mock("../src/gl/Surface.svelte", () => ({ default: () => {} }));

const settle = async () => {
  for (let i = 0; i < 5; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

/** Every color getComputedStyle gives for the page, and the elements that have it. */
function pageColors(): Map<string, string[]> {
  const props = [
    "color",
    "background-color",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "outline-color",
    "fill",
    "stroke",
    "caret-color",
    "box-shadow",
  ];
  const out = new Map<string, string[]>();
  for (const el of [document.documentElement, ...document.querySelectorAll("*")]) {
    const st = getComputedStyle(el);
    for (const p of props) {
      for (const m of st.getPropertyValue(p).matchAll(/rgba?\([^)]*\)|#[0-9a-fA-F]{3,8}\b/g)) {
        const list = out.get(m[0]) ?? [];
        list.push(`${el.tagName.toLowerCase()}.${(el.getAttribute("class") ?? "").split(" ")[0]} ${p}`);
        out.set(m[0], list);
      }
    }
  }
  return out;
}

describe("the whole screen (?scene=pianoRoll&theme=pinkDark), then 「ミント ・ ライト」", () => {
  let app: ReturnType<typeof mount>;
  let style: HTMLStyleElement;
  let state: () => AppState;
  let send: (c: Command) => Promise<number | null>;
  beforeAll(async () => {
    const appState = await import("../src/state/app.svelte");
    state = () => appState.current.state!;
    send = appState.send;
    style = document.head.appendChild(document.createElement("style"));
    // The parts' own <style> is not in the test page; the shared one is put in by hand.
    const here = decodeURIComponent(new URL(import.meta.url).pathname);
    style.textContent = new TextDecoder().decode(readFileSync(`${here.slice(0, here.lastIndexOf("/"))}/../src/components/common.css`));
    history.replaceState(null, "", "/?scene=pianoRoll&scale=100&theme=pinkDark");
    const App = (await import("../src/App.svelte")).default;
    app = mount(App, { target: document.body });
    await settle();
  });
  afterAll(() => {
    unmount(app);
    style.remove();
    history.replaceState(null, "", "/");
  });

  const rootVars = () => {
    const st = document.documentElement.style;
    return Object.fromEntries(NAMES.map((n) => [n, st.getPropertyValue(`--${n.replaceAll(".", "-")}`)]));
  };

  it("starts in the theme the scene was asked in", () => {
    expect(state().workspace.theme).toBe("pinkDark");
    expect(rootVars()).toEqual({ ...palettes.pinkDark });
    const [r, g, b] = hexToRgb(palettes.pinkDark["color.gray.1"]).map((c) => Math.round(c * 255));
    expect(getComputedStyle(document.body).backgroundColor).toBe(`rgb(${r}, ${g}, ${b})`);
  });

  it("after the switch no color of the theme before is left on the page, and the page is not made again", async () => {
    const body = document.querySelector(".app");
    expect(body).not.toBeNull();
    const before = pageColors();
    const old = cssRgbOf(palettes.pinkDark);
    expect([...before.keys()].some((c) => old.has(c))).toBe(true);

    await send({ type: "setTheme", theme: "mintLight" });
    await settle();
    expect(state().workspace.theme).toBe("mintLight");
    expect(document.querySelector(".app")).toBe(body);
    expect(rootVars()).toEqual({ ...palettes.mintLight });
    const after = pageColors();
    const left = [...after].filter(([c]) => old.has(c));
    expect(left).toEqual([]);
    const mine = cssRgbOf(palettes.mintLight);
    expect([...after.keys()].some((c) => mine.has(c))).toBe(true);
  });
});
