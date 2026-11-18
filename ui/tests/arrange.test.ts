// アレンジを mount して、名前と札、切りかえ、録音待機、目盛のドラッグ、クリップをえらぶ、横のスクロールを見る。
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView } from "../src/bridge";
import { arrangeOf } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const BAR = 1920;
const WIDTH = 1000;

// jsdom は大きさを測らない。時間の列の幅を 1000 にする。
const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() =>
  Object.defineProperty(proto, "clientWidth", {
    configurable: true,
    get: () => WIDTH,
  }),
);
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});

function show(state: AppState): void {
  app.setState(state);
  mounted = mount(Arrange, { target });
  flushSync();
}

/** 部品をつけたまま、状態だけを入れかえる。 */
function show2(state: AppState): void {
  app.setState(state);
  flushSync();
}

const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

function clip(id: number, name: string, fromBar: number, toBar: number, take = 1, takes = 1): ClipView {
  return {
    id,
    name,
    start: (fromBar - 1) * BAR,
    end: (toBar - 1) * BAR,
    take,
    takes,
    takeId: 100 + id,
    takeOffset: 0,
    noteCount: 0,
  };
}

/** モック F-Takes: 「Aメロ」はテイク 3 つのうち 2 つ目で、えらんである。 */
const takes = (selected: number | null = 3) =>
  app.stateWith((s) => {
    s.song.tracks[0]!.clips = [clip(1, "イントロ", 1, 5), clip(3, "Aメロ", 5, 9, 2, 3)];
    s.arrange = arrangeOf(selected);
  });

function pointer(el: HTMLElement, type: string, clientX: number, clientY = 10, more: MouseEventInit = {}): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX, clientY, button: 0, ...more }));
  flushSync();
}

/** トラック 3 本（偽物の場面 arrangeSelect と同じ形）。selected のクリップを選んである。 */
const three = (selected: number[] = [], clicked: number | null = null) =>
  app.stateWith((s) => {
    const base = s.song.tracks[0]!;
    s.song.tracks = [
      { ...base, clips: [clip(1, "イントロ", 1, 5), clip(3, "Aメロ", 5, 9, 2, 3)] },
      { ...base, id: 2, number: 2, name: "ベース", color: 2, muted: true, clips: [clip(41, "ベース A", 5, 9)] },
      { ...base, id: 3, number: 3, name: "ドラム", color: 3, soloed: true, clips: [clip(43, "8ビート", 1, 9)] },
    ];
    s.song.playTarget = 2;
    s.arrange = { selectedClips: selected, selectedClip: clicked, pianoRollClip: clicked, clipboardClips: 1 };
  });

const rows = () => (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");

describe("クリップの名前と札（仕様書 2 章「テイクの印と切りかえ」、モック F-Takes）", () => {
  it("名前はクリップの中。テイクが 2 つ以上なら名前の前に「2/3」。えらんだクリップの札は accent", () => {
    show(takes());
    const labels = qa(".clip-label");
    expect(labels.map(text)).toEqual(["イントロ", "2/3 Aメロ"]);
    expect(labels[1]!.style.left).toBe("257px");
    expect(labels[1]!.style.top).toBe("5px");
    expect(q(".take-tag")!.classList.contains("selected")).toBe(true);
  });

  it("幅が 28 より狭いクリップは、6×6 の四角だけ", () => {
    show(
      app.stateWith((s) => {
        s.song.tracks[0]!.clips = [{ ...clip(1, "サビ", 1, 2, 2, 3), end: 400 }];
      }),
    );
    expect(text(q(".clip-label"))).toBe("");
    expect(q(".take-square")).not.toBeNull();
  });

  it("えらんだクリップのすぐ上に切りかえの札。◂ ▸ で stepTake", () => {
    show(takes());
    const sw = q(".switcher")!;
    expect(text(sw)).toBe("◂ テイク 2 / 3 ▸ T");
    expect([sw.style.left, sw.style.top, sw.style.height]).toEqual(["256px", "-21px", "24px"]);
    expect(qa(".switcher .m-num").map(text)).toEqual(["2", "3", "T"]);
    pointer(sw, "pointerdown", 260, -10);
    expect(app.sent).toEqual([]);
    (sw.querySelector("[aria-label=前のテイク]") as HTMLButtonElement).click();
    (sw.querySelector("[aria-label=次のテイク]") as HTMLButtonElement).click();
    expect(app.sent).toEqual([
      { type: "stepTake", delta: -1 },
      { type: "stepTake", delta: 1 },
    ]);
  });

  it("テイクが 1 つのクリップや、何もえらんでいないときは、切りかえの札なし", () => {
    show(takes(1));
    expect(q(".switcher")).toBeNull();
  });
});

describe("押す: クリップをえらぶ、録音待機", () => {
  it("クリップを押すと selectClip。何もない所は、動かさずに離すと null。えらんであるクリップは送らない", () => {
    show(takes());
    const lanes = q(".lanes")!;
    pointer(lanes, "pointerdown", 100, 30);
    pointer(lanes, "pointerup", 100, 30);
    pointer(lanes, "pointerdown", 300, 30);
    pointer(lanes, "pointerup", 300, 30);
    pointer(lanes, "pointerdown", 700, 30);
    pointer(lanes, "pointerup", 700, 30);
    expect(app.sent).toEqual([
      { type: "selectClip", clip: 1 },
      { type: "selectClip", clip: null },
    ]);
  });

  it("● は、そのトラックを選ぶ（録音待機は、いつも選んだトラック）。オンは rec。M ・ S も押せて、オンは on（accent）", () => {
    show(three());
    const arm = qa("button[aria-label=録音待機]");
    expect(arm.map((b) => b.getAttribute("aria-pressed"))).toEqual(["false", "true", "false"]);
    expect(arm.map((b) => b.classList.contains("on"))).toEqual([false, true, false]);
    arm[0]!.click();
    const mute = qa("button[aria-label=ミュート]");
    const solo = qa("button[aria-label=ソロ]");
    expect(mute.map((b) => b.classList.contains("on"))).toEqual([false, true, false]);
    expect(solo.map((b) => b.classList.contains("on"))).toEqual([false, false, true]);
    expect(mute[1]!.getAttribute("aria-pressed")).toBe("true");
    expect(mute[0]!.hasAttribute("aria-disabled")).toBe(false);
    mute[0]!.click();
    solo[2]!.click();
    expect(app.sent).toEqual([
      { type: "selectTrack", track: 1, fromArm: true },
      { type: "toggleMute", track: 1 },
      { type: "toggleSolo", track: 3 },
    ]);
  });

  it("録音中 ・ カウントイン中は M だけ「使えない」（core が受けない）。S ・ ● は押せる", () => {
    for (const state of ["recording", "countIn"] as const) {
      const s = three();
      s.transport.state = state;
      (mounted ? show2 : show)(s);
      const mute = qa("button[aria-label=ミュート]");
      expect(mute.map((b) => b.getAttribute("aria-disabled"))).toEqual(["true", "true", "true"]);
      expect(mute.map((b) => b.classList.contains("live"))).toEqual([false, false, false]);
      expect(mute[0]!.tabIndex).toBe(-1);
      mute[1]!.click();
      qa("button[aria-label=ソロ]")[0]!.click();
      qa("button[aria-label=録音待機]")[1]!.click();
      expect(app.sent).toEqual([
        { type: "toggleSolo", track: 1 },
        { type: "selectTrack", track: 2, fromArm: true },
      ]);
      expect(qa("button[aria-label=ソロ]")[0]!.classList.contains("live")).toBe(true);
      app.sent.length = 0;
    }
  });
});

describe("選んだトラック（B-70）", () => {
  it("playTarget の見出しは selected（地 gray.4）。見出しを押すと selectTrack、M ・ S ・ ● の上や、選んであるトラックは送らない", () => {
    show(three());
    const heads = qa("[data-test=track-head]");
    expect(heads.map((h) => h.classList.contains("selected"))).toEqual([false, true, false]);
    pointer(heads[0]!, "pointerdown", 10, 10);
    pointer(heads[1]!, "pointerdown", 10, 10);
    pointer(qa("button[aria-label=ミュート]")[2]!, "pointerdown", 10, 10);
    expect(app.sent).toEqual([{ type: "selectTrack", track: 1, fromArm: false }]);
  });
});

describe("クリップを何個も選ぶ ・ 動かす（B-71）", () => {
  it("選んだクリップはぜんぶ「選んだ」の札。テイクの切りかえの札は selectedClip だけ", () => {
    show(three([3, 41], 41));
    expect(qa(".take-tag.selected").length).toBe(1);
    expect(q(".switcher")).toBeNull();
    show2(three([3, 41], 3));
    expect(q(".switcher")).not.toBeNull();
  });

  it("Shift ＋ 押すと selectClips add。選んである中の 1 つを動かさずに離すと、それだけを selectClip", () => {
    show(three([3], 3));
    const lanes = q(".lanes")!;
    pointer(lanes, "pointerdown", 300, 100, { shiftKey: true });
    pointer(lanes, "pointerup", 300, 100, { shiftKey: true });
    expect(app.sent).toEqual([{ type: "selectClips", clips: [41], mode: "add" }]);
    app.sent.length = 0;
    show2(three([3, 41], 41));
    const lanes2 = q(".lanes")!;
    pointer(lanes2, "pointerdown", 300, 30);
    pointer(lanes2, "pointerup", 300, 30);
    expect(app.sent).toEqual([{ type: "selectClip", clip: 3 }]);
  });

  it("空いた所から囲むと selectClips（Shift で add）。枠は accent の box-frame。Esc でやめる", () => {
    show(three([1], 1));
    const lanes = q(".lanes")!;
    // 行 2（ベース）の 3 小節目の空き → 行 1 の 6 小節目まで。イントロ、Aメロ、ベース A にかかる。
    pointer(lanes, "pointerdown", 140, 100);
    pointer(lanes, "pointermove", 330, 30);
    const box = q("[data-test=arrange-box]")!;
    expect([box.style.left, box.style.top, box.style.width, box.style.height]).toEqual(["140px", "30px", "190px", "70px"]);
    pointer(lanes, "pointerup", 330, 30);
    expect(q("[data-test=arrange-box]")).toBeNull();
    pointer(lanes, "pointerdown", 140, 100, { shiftKey: true });
    pointer(lanes, "pointermove", 330, 30, { shiftKey: true });
    pointer(lanes, "pointerup", 330, 30, { shiftKey: true });
    pointer(lanes, "pointerdown", 140, 100);
    pointer(lanes, "pointermove", 330, 30);
    expect(cancelDrag()).toBe(true);
    pointer(lanes, "pointerup", 330, 30);
    expect(app.sent).toEqual([
      { type: "selectClips", clips: [1, 3, 41], mode: "replace" },
      { type: "selectClips", clips: [1, 3, 41], mode: "add" },
    ]);
  });

  it("選んであるクリップをつかむと、選んだぜんぶを moveClips。もとの場所はぜんぶ gray.7 の枠", () => {
    show(three([3, 41], 41));
    const lanes = q(".lanes")!;
    pointer(lanes, "pointerdown", 300, 100);
    pointer(lanes, "pointermove", 364, 100);
    expect(qa(".drag-clip").length).toBe(1);
    expect(qa(".clip-label").filter((l) => l.style.transform).map(text)).toEqual(["2/3 Aメロ", "ベース A"]);
    pointer(lanes, "pointerup", 364, 100);
    expect(app.sent).toEqual([{ type: "moveClips", clips: [3, 41], ticks: BAR, tracks: 0, copy: false }]);
  });

  it("選ばれていないクリップをつかむと、それだけを選んで動かす", () => {
    show(three([3, 41], 41));
    const lanes = q(".lanes")!;
    pointer(lanes, "pointerdown", 100, 170);
    pointer(lanes, "pointermove", 100, 100);
    pointer(lanes, "pointerup", 100, 100);
    expect(app.sent[0]).toEqual({ type: "selectClip", clip: 43 });
    // ドラムの 8ビート（1–9）をベースの行へ: ベース A と重なるので送らない。
    expect(app.sent).toHaveLength(1);
  });
});

describe("目盛（ヤチヨさんの決め: ループは目盛のドラッグ）", () => {
  it("ドラッグすると、その間は見た目だけ先に動き、離すと setLoop", () => {
    show(takes());
    const ruler = q(".ruler.bars")!;
    pointer(ruler, "pointerdown", 256);
    pointer(ruler, "pointermove", 512);
    expect(text(q(".head .row:nth-child(2)"))).toBe("マーカー ・ ループ 5–9");
    expect([q(".loop-mark")!.style.left, q(".loop-mark")!.style.width]).toEqual(["256px", "256px"]);
    pointer(ruler, "pointerup", 512);
    expect(app.sent).toEqual([{ type: "setLoop", fromBar: 5, toBar: 9 }]);
  });

  it("押すだけなら locate。目盛に見えている線（ふだんは小節）に吸いつき、Shift なら吸いつかない（B-61）", () => {
    show(takes());
    const ruler = q(".ruler.bars")!;
    pointer(ruler, "pointerdown", 300);
    pointer(ruler, "pointerup", 301);
    pointer(ruler, "pointerdown", 300, 10, { shiftKey: true });
    pointer(ruler, "pointerup", 301, 10, { shiftKey: true });
    expect(app.sent).toEqual([
      { type: "locate", tick: 9600 },
      { type: "locate", tick: 9000 },
    ]);
    expect(q(".loop-mark")).toBeNull();
  });

  it("ループがオンなら、目盛の上辺に accent と「マーカー ・ ループ 5–9」。オフなら出さない", () => {
    const on = takes();
    on.song.loopRange = { start: 4 * BAR, end: 8 * BAR };
    on.song.loopOn = true;
    show(on);
    expect(text(q(".head .row:nth-child(2)"))).toBe("マーカー ・ ループ 5–9");
    expect(q(".loop-mark")).not.toBeNull();
    expect(text(q(".loop-range"))).toBe("5–9");
    app.setState({ ...on, song: { ...on.song, loopOn: false } });
    flushSync();
    expect(text(q(".head .row:nth-child(2)"))).toBe("マーカー");
    expect(q(".loop-mark")).toBeNull();
  });
});

describe("右クリックの一覧（B-60）", () => {
  const ctx = (el: HTMLElement, x: number, y: number, more: MouseEventInit = {}) => {
    const ev = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 2, ...more });
    el.dispatchEvent(ev);
    flushSync();
    return ev;
  };
  afterEach(() => (contextMenu.open = null));

  it("クリップ: 選ばれていなければ選んでから。行と右のキー。テイクが 1 つなら次 ・ 前のテイクは使えない", () => {
    show(three([3], 3));
    const ev = ctx(q(".lanes")!, 300, 100);
    expect(ev.defaultPrevented).toBe(true);
    expect(app.sent).toEqual([{ type: "selectClip", clip: 41 }]);
    expect(contextMenu.open?.entries.map((e) => (e.kind === "row" ? e.label : "-"))).toEqual([
      "menu.openInPianoRoll",
      "menu.cut",
      "menu.copy",
      "menu.paste",
      "menu.duplicate",
      "menu.delete",
      "-",
      "menu.nextTake",
      "menu.prevTake",
      "menu.deleteTake",
    ]);
    expect(rows().map((r) => r.key)).toEqual(["E", "Ctrl X", "Ctrl C", "Ctrl V", "Ctrl D", "Delete", "T", "Shift T", undefined]);
    expect(rows().map((r) => r.usable)).toEqual([true, true, true, true, true, true, false, false, false]);
  });

  it("クリップの行は、押したときの選択に向く。選んであるクリップは選びなおさない", () => {
    show(three([3, 41], 41));
    ctx(q(".lanes")!, 300, 30);
    expect(app.sent).toEqual([]);
    for (const r of rows()) r.run();
    expect(app.sent).toEqual([
      { type: "toggleBand", band: "pianoRoll" },
      { type: "cutClips", clips: [3, 41] },
      { type: "copyClips", clips: [3, 41] },
      { type: "pasteClips", to: null },
      { type: "duplicateClips", clips: [3, 41] },
      { type: "deleteSelected" },
      { type: "stepTake", delta: 1 },
      { type: "stepTake", delta: -1 },
      { type: "deleteTake", clip: 3 },
    ]);
  });

  it("録音中は、曲を変える行が使えない（ピアノロールでひらく ・ コピーは使える）", () => {
    const s = three([3], 3);
    s.transport.state = "recording";
    show(s);
    ctx(q(".lanes")!, 300, 30);
    expect(rows().map((r) => r.usable)).toEqual([true, false, true, false, false, false, false, false, false]);
  });

  it("空いた所: 貼りつけ。押した所をグリッド（1 小節）に寄せ、Shift なら寄せない", () => {
    show(three());
    ctx(q(".lanes")!, 600, 100);
    expect(rows().map((r) => [r.label, r.key, r.usable])).toEqual([["menu.paste", "Ctrl V", true]]);
    rows()[0]!.run();
    ctx(q(".lanes")!, 600, 100, { shiftKey: true });
    rows()[0]!.run();
    expect(app.sent).toEqual([
      { type: "pasteClips", to: { track: 2, tick: 9 * BAR } },
      { type: "pasteClips", to: { track: 2, tick: 18000 } },
    ]);
  });

  it("トラックの見出し: ミュート ・ ソロ ・ 録音待機。選ばれていなければ selectTrack してから", () => {
    show(three());
    ctx(qa("[data-test=track-head]")[0]!, 10, 10);
    expect(app.sent).toEqual([{ type: "selectTrack", track: 1, fromArm: false }]);
    expect(rows().map((r) => [r.label, r.key])).toEqual([
      ["menu.mute", "Shift M"],
      ["menu.solo", "Shift S"],
      ["menu.arm", undefined],
    ]);
    for (const r of rows()) r.run();
    expect(app.sent.slice(1)).toEqual([
      { type: "toggleMute", track: 1 },
      { type: "toggleSolo", track: 1 },
      { type: "selectTrack", track: 1, fromArm: true },
    ]);
  });
});

describe("キーが向く帯の印（B-72）", () => {
  it("見出しの行と目盛の上を横いっぱいに（帯の箱のすぐ下に 1 つ）", () => {
    show(takes());
    const mark = q("[data-test=arrange-focus-mark]")!;
    expect(mark.parentElement!.classList.contains("arrange")).toBe(true);
  });
});

describe("横のスクロール", () => {
  it("Shift＋ホイールと横のホイールで、目盛とクリップがいっしょに動く。0 より左へは行かない", () => {
    const s = takes();
    s.song.tracks[0]!.clips.push(clip(5, "サビ", 30, 34));
    show(s);
    const time = q(".time")!;
    time.dispatchEvent(
      new WheelEvent("wheel", {
        bubbles: true,
        cancelable: true,
        deltaY: 640,
        shiftKey: true,
      }),
    );
    flushSync();
    expect(text(q(".bar-number"))).toBe("11");
    expect(qa(".clip-label")).toHaveLength(0);
    time.dispatchEvent(
      new WheelEvent("wheel", {
        bubbles: true,
        cancelable: true,
        deltaX: -2000,
      }),
    );
    flushSync();
    expect(text(q(".bar-number"))).toBe("1");
    expect(qa(".clip-label")).toHaveLength(2);
  });
});

describe("録っているあいだ（モック F-ArrangeRec）", () => {
  const recording = () =>
    app.stateWith((s) => {
      s.song.tracks[0]!.clips = [clip(1, "イントロ", 1, 5), clip(3, "ピアノ", 5, 7)];
      s.transport.state = "recording";
      s.transport.position = 4 * BAR;
      s.transport.recording = {
        id: 1,
        track: 1,
        from: 4 * BAR,
        take: 2,
        clip: 3,
        lap: 0,
      };
    });

  it("クリップの名前の所は「● 録音中 テイク 2」。下になったクリップの名前は出さない。線は rec", () => {
    show(recording());
    expect(qa(".clip-label").map(text)).toEqual(["イントロ", "● 録音中 テイク 2"]);
    expect(q(".clip-label.recording")).not.toBeNull();
    expect(q(".play-line")!.classList.contains("rec")).toBe(true);
    expect(q(".rec-tail")!.dataset.glSlot).toBe("rects");
    expect(q(".rec-tail")!.parentElement!.dataset.glRegion).toBe("recordingClip");
  });

  it("止まっていれば線は accent で、transport.position の所", () => {
    show(takes());
    const line = q(".play-line")!;
    expect(line.classList.contains("rec")).toBe(false);
    expect(line.style.transform).toBe("translateX(0px)");
    app.setState({
      ...takes(),
      transport: { ...takes().transport, position: 4 * BAR },
    });
    flushSync();
    expect(line.style.transform).toBe("translateX(256px)");
  });
});

describe("全体図（高さ 136 未満）", () => {
  it("クリップの線（rects）と、見えている範囲の枠と、再生の位置の線", () => {
    const s = takes();
    s.workspace.layout = {
      ...s.workspace.layout,
      arrangeOverview: true,
      arrangeHeight: 40,
    };
    show(s);
    expect(q(".overview-slot")!.dataset.glRegion).toBe("overview");
    expect(qa(".overview-slot [data-gl-slot]").map((e) => e.dataset.glSlot)).toEqual(["rects", "overview"]);
    expect(q(".overview .play-line")).not.toBeNull();
  });
});
