// アレンジの横の拡大 ・ 縮小（− ＋、Mod ＋ ホイール）、横の棒、全体図の「見えている範囲の枠」（仕様書 3 章、モック R3-Arrange・A-PianoRoll）。
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView } from "../src/bridge";
import Arrange from "../src/components/Arrange.svelte";
import { arrangeView, DEFAULT_ARRANGE_BAR, prView } from "../src/components/pianoroll/view.svelte";
import { watchSlots, type GlRect, type GlSlot } from "../src/gl/slot";
import HScrollbar from "../src/lib/HScrollbar.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const BAR = 4 * TPQ;
const WIDTH = 1000;

const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
const hadH = Object.getOwnPropertyDescriptor(proto, "clientHeight");
beforeAll(() => {
  Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => WIDTH });
  Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => 39 });
});
afterAll(() => {
  if (had) Object.defineProperty(proto, "clientWidth", had);
  if (hadH) Object.defineProperty(proto, "clientHeight", hadH);
});

const slots = new Map<HTMLElement, GlSlot>();
let stopWatching: () => void = () => {};
let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  slots.clear();
  stopWatching = watchSlots({
    attach: (n, s) => slots.set(n, s),
    update: (n, s) => slots.set(n, s),
    detach: (n) => slots.delete(n),
  });
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  stopWatching();
  arrangeView.barPx = DEFAULT_ARRANGE_BAR;
  Object.assign(prView, { scrollTick: 0, areaWidth: 0, follow: true, focus: "arrange" });
});

const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

function clip(id: number, fromBar: number, toBar: number): ClipView {
  return { id, name: `c${id}`, start: (fromBar - 1) * BAR, end: (toBar - 1) * BAR, take: 1, takes: 1, takeId: 100 + id, takeOffset: 0, noteCount: 0, vocal: false };
}

function show(state: AppState): void {
  app.setState(state);
  mounted = mount(Arrange, { target });
  flushSync();
}

const longSong = () => app.stateWith((s) => (s.song.tracks[0]!.clips = [clip(1, 1, 5), clip(2, 30, 34)]));

describe("アレンジの見出しの行の − ＋ と「グリッド 1 小節」（モック R3-Arrange）", () => {
  it("見出しの行は「アレンジ − ＋ グリッド 1 小節」", () => {
    show(longSong());
    expect(text(q(".head .row"))).toBe("アレンジ − ＋ グリッド 1 小節");
    expect(qa(".zoom-btn").map((b) => b.getAttribute("aria-label"))).toEqual(["横に縮小", "横に拡大"]);
  });

  it("＋ で 1 小節 64 → 91 px。目盛の番号もそれに合わせて動く。− で戻る", () => {
    show(longSong());
    qa(".zoom-btn")[1]!.click();
    flushSync();
    expect(arrangeView.barPx).toBe(91);
    expect(qa(".bar-number")[1]!.style.left).toBe("91px");
    qa(".zoom-btn")[0]!.click();
    flushSync();
    expect(qa(".bar-number")[1]!.style.left).toBe("64px");
  });

  it("縮小して番号が重なるときは間引く（1 小節 16 px なら 2 小節おき）", () => {
    arrangeView.barPx = 16;
    show(longSong());
    expect(qa(".bar-number").slice(0, 3).map(text)).toEqual(["1", "3", "5"]);
  });

  it("Ctrl ＋ ホイールで、カーソルの所を中心に拡大する（その所のティックが画面の同じ所に残る）", () => {
    show(longSong());
    const time = q(".time")!;
    // 曲の頭から 640 px（11 小節目の頭）にカーソル。左はしは 0。
    time.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: -100, ctrlKey: true, clientX: 640 }));
    flushSync();
    expect(arrangeView.barPx).toBe(91);
    // 11 小節目の頭（10 × 91 = 910 px）が、まだ 640 px の所にある: 左はしは 270 px。
    const eleven = qa(".bar-number").find((b) => text(b) === "11")!;
    expect(parseFloat(eleven.style.left)).toBeCloseTo(640, 5);
  });

  it("アレンジを押すと、Mod ＋ ・ Mod − の行き先がアレンジになる", () => {
    prView.focus = "pianoRoll";
    show(longSong());
    q(".lanes")!.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0 }));
    expect(prView.focus).toBe("arrange");
  });
});

describe("横の棒（仕様書 2 章「スクロールの棒」を横にした物）", () => {
  it("入りきる曲では出さない。入りきらなければトラックの行のいちばん下に重ねる", () => {
    show(app.stateWith((s) => (s.song.tracks[0]!.clips = [clip(1, 1, 5)])));
    expect(q('[data-testid="hscrollbar"]')).toBeNull();
    unmount(mounted!);
    mounted = null;
    show(longSong());
    const bar = q('[data-testid="hscrollbar"]')!;
    expect(bar.parentElement!.classList.contains("lanes-box")).toBe(true);
    // 右のはし: 34 小節目の頭（33 × 64 = 2112 px）が画面のまん中まで。中身は 2112 − 500 + 1000 = 2612。
    expect(Number(bar.dataset.total)).toBeCloseTo(2612, 5);
    expect(bar.dataset.visible).toBe(String(WIDTH));
  });

  it("つまみの外を押すと 1 画面ぶん送る（次の再生まで追わない）", () => {
    show(longSong());
    const bar = q('[data-testid="hscrollbar"]')!;
    bar.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: 900 }));
    flushSync();
    expect(q('[data-testid="hscrollbar"]')!.dataset.offset).toBe("1000");
    expect(text(q(".bar-number"))).toBe("16");
  });

  it("HScrollbar だけ: つまみは長さに比例し、いちばん短くて 8。ドラッグで動かす", () => {
    const seen: number[] = [];
    app.setState(app.stateWith(() => {}));
    mounted = mount(HScrollbar, { target, props: { total: 100_000, visible: 1000, offset: 0, onscroll: (o: number) => seen.push(o) } });
    flushSync();
    const thumb = q('[data-testid="hscrollbar-thumb"]')!;
    expect(thumb.style.width).toBe("10px");
    thumb.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: 0 }));
    thumb.dispatchEvent(new MouseEvent("pointermove", { bubbles: true, clientX: 100 }));
    expect(seen.at(-1)).toBeCloseTo(10_000, 5);
    unmount(mounted);
    mounted = mount(HScrollbar, { target, props: { total: 1_000_000, visible: 1000, offset: 0, onscroll: () => {} } });
    flushSync();
    expect(q('[data-testid="hscrollbar-thumb"]')!.style.width).toBe("8px");
  });
});

describe("全体図の「見えている範囲の枠」", () => {
  const overview = (pianoRoll: boolean) =>
    app.stateWith((s) => {
      s.song.tracks[0]!.clips = [clip(1, 1, 5), clip(2, 30, 34)];
      s.workspace.layout = { ...s.workspace.layout, arrangeOverview: true, arrangeHeight: 40, lower: pianoRoll ? [{ band: "pianoRoll", height: 600 }] : [] };
    });
  const accent = () => {
    const rects = [...slots.values()].flatMap((s) => (s.kind === "rects" ? s.rects : [])) as GlRect[];
    return rects.filter((r) => r.color === "color.accent");
  };

  it("ピアノロールがひらいていなければ、今までどおり全体図いっぱいの枠（overview の slot）", () => {
    show(overview(false));
    expect(qa(".overview-slot [data-gl-slot]").map((e) => e.dataset.glSlot)).toEqual(["rects", "overview"]);
    expect(accent()).toEqual([]);
  });

  it("ピアノロールがひらいていれば、ピアノロールで見えている範囲（A-PianoRoll: 1〜5 小節 → x 0〜256）", () => {
    Object.assign(prView, { scrollTick: 0, areaWidth: 1280, quarterPx: 80 });
    show(overview(true));
    expect(qa(".overview-slot [data-gl-slot]").map((e) => e.dataset.glSlot)).toEqual(["rects"]);
    const top = accent()[0]!;
    expect(top.x).toBe(0);
    expect(top.w).toBe(256);
  });

  it("枠をドラッグすると、ピアノロールが横に送られる（手で動かしたので、次の再生まで追わない）", () => {
    Object.assign(prView, { scrollTick: 0, areaWidth: 1280, quarterPx: 80 });
    show(overview(true));
    const slot = q(".overview-slot")!;
    slot.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: 100 }));
    slot.dispatchEvent(new MouseEvent("pointermove", { bubbles: true, clientX: 164 }));
    slot.dispatchEvent(new MouseEvent("pointerup", { bubbles: true }));
    flushSync();
    expect(prView.scrollTick).toBeCloseTo(BAR, 5);
    expect(prView.follow).toBe(false);
    expect(accent()[0]!.x).toBe(64);
  });

  it("ピアノロールが全体図の外まで送られたら、全体図を送って枠を見せる", () => {
    Object.assign(prView, { scrollTick: 0, areaWidth: 1280, quarterPx: 80 });
    show(overview(true));
    prView.scrollTick = 20 * BAR;
    flushSync();
    const top = accent()[0]!;
    expect(top.x).toBeGreaterThanOrEqual(0);
    expect(top.x + top.w).toBeLessThanOrEqual(WIDTH);
  });
});

describe("トラックの行を縦に送る（仮の形、user:2026-11-20）", () => {
  /** トラック 6 本（行の高さ 64 × 6 = 384）。見えている高さは、上の beforeAll の 39。 */
  const six = (target = 100) =>
    app.stateWith((s) => {
      const first = s.song.tracks[0]!;
      s.song.tracks = Array.from({ length: 6 }, (_, i) => ({ ...first, id: 100 + i, number: i + 1, clips: i === 0 ? [clip(1, 1, 5)] : [] }));
      s.song.playTarget = target;
    });
  const wheel = (el: Element, init: WheelEventInit) => {
    const e = new WheelEvent("wheel", { bubbles: true, cancelable: true, ...init });
    el.dispatchEvent(e);
    flushSync();
    return e;
  };
  const tops = () => [q(".lanes-view")!.scrollTop, q(".heads-view")!.scrollTop];

  it("ふつうのホイールで、見出しの列とレーンを同じだけ送る。下のはしで止まる。上の行は、送る箱の外", () => {
    show(six());
    expect(tops()).toEqual([0, 0]);
    const e = wheel(q(".time")!, { deltaY: 30 });
    expect(e.defaultPrevented).toBe(true);
    expect(tops()).toEqual([30, 30]);
    wheel(q(".head")!, { deltaY: 50 });
    expect(tops()).toEqual([80, 80]);
    wheel(q(".time")!, { deltaY: 10000 });
    expect(tops()).toEqual([384 + 32 - 39, 384 + 32 - 39]);
    wheel(q(".time")!, { deltaY: -10000 });
    expect(tops()).toEqual([0, 0]);
    for (const sel of [".ruler", ".tempo-head", ".title-row"]) expect(q(sel)!.closest(".lanes-view, .heads-view"), sel).toBeNull();
    expect(q(".heads-view")!.dataset.glRegion).toBe("trackMeters");
    expect(q(".lanes-view")!.hasAttribute("data-gl-clip")).toBe(true);
  });

  it("Shift ＋ ホイールは横、Mod ＋ ホイールは拡大のまま（縦には送らない）", () => {
    show(six());
    wheel(q(".time")!, { deltaY: 40, shiftKey: true });
    expect(tops()).toEqual([0, 0]);
    wheel(q(".time")!, { deltaY: -100, ctrlKey: true });
    expect(tops()).toEqual([0, 0]);
  });

  it("右はしに縦の棒（入りきるときは出さない）。棒で送れる", () => {
    show(six());
    const bar = q("[data-test=arrange-vscrollbar]")!;
    expect(bar.parentElement!.classList.contains("lanes-box")).toBe(true);
    bar.getBoundingClientRect = () => new DOMRect(0, 0, 8, 39);
    bar.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, cancelable: true, button: 0, clientY: 38 }));
    flushSync();
    expect(tops()[0]).toBeGreaterThan(0);
    expect(tops()[0]).toBe(tops()[1]);
    unmount(mounted!);
    mounted = null;
    const h = Object.getOwnPropertyDescriptor(proto, "clientHeight")!;
    Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => 1000 });
    try {
      show(six());
      expect(q("[data-test=arrange-vscrollbar]")).toBeNull();
      expect(wheel(q(".time")!, { deltaY: 30 }).defaultPrevented).toBe(false);
    } finally {
      Object.defineProperty(proto, "clientHeight", h);
    }
  });

  it("えらんだトラックが変わったら、そのトラックが見える所まで送る。同じトラックのままなら送らない", () => {
    show(six());
    app.setState(six(105));
    flushSync();
    // 6 本めの行は 320〜384。見えている 39 より高いので、行の上に合わせる。
    expect(tops()).toEqual([320, 320]);
    wheel(q(".time")!, { deltaY: -100 });
    expect(tops()).toEqual([220, 220]);
    app.setState(six(105));
    flushSync();
    expect(tops()).toEqual([220, 220]);
    app.setState(six(100));
    flushSync();
    expect(tops()).toEqual([0, 0]);
  });
});
