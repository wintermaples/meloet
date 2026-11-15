// アレンジを mount して、名前と札、切りかえ、録音待機、目盛のドラッグ、クリップをえらぶ、横のスクロールを見る。
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView } from "../src/bridge";
import Arrange from "../src/components/Arrange.svelte";
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
    s.arrange = { selectedClip: selected, pianoRollClip: selected };
  });

function pointer(el: HTMLElement, type: string, clientX: number, clientY = 10): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, clientX, clientY, button: 0 }));
  flushSync();
}

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
  it("クリップを押すと selectClip。何もない所は null。えらんであるクリップは送らない", () => {
    show(takes());
    const lanes = q(".lanes")!;
    pointer(lanes, "pointerdown", 100, 30);
    pointer(lanes, "pointerdown", 300, 30);
    pointer(lanes, "pointerdown", 700, 30);
    expect(app.sent).toEqual([
      { type: "selectClip", clip: 1 },
      { type: "selectClip", clip: null },
    ]);
  });

  it("● は録音待機の切りかえ。オンは rec。M と S は使えないまま", () => {
    show(app.stateWith((s) => (s.song.tracks[0]!.armed = true)));
    const arm = q("button[aria-label=録音待機]")!;
    expect(arm.getAttribute("aria-pressed")).toBe("true");
    expect(arm.classList.contains("on")).toBe(true);
    arm.click();
    expect(app.sent).toEqual([{ type: "toggleArm", track: 1 }]);
    expect(q("button[aria-label=ミュート]")!.getAttribute("aria-disabled")).toBe("true");
    expect(q("button[aria-label=ソロ]")!.getAttribute("aria-disabled")).toBe("true");
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

  it("押すだけなら locate", () => {
    show(takes());
    const ruler = q(".ruler.bars")!;
    pointer(ruler, "pointerdown", 300);
    pointer(ruler, "pointerup", 301);
    expect(app.sent).toEqual([{ type: "locate", tick: 9000 }]);
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
