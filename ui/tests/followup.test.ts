// 第 3 回のあとの直し（docs/history/design-notes.ja.md B-64・65・72〜75）を mount して見る: ベロシティの帯をたたむ、キーが向く帯の印、
// ボタンのキー、ひらいた帯の見出しでたたむ、スケール固定の ▾、ループの範囲と全体図の枠のドラッグ中の Esc、強さ ・ スウィングの棒のキー、用語の説明。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import BandRow from "../src/components/BandRow.svelte";
import LowerBand from "../src/components/LowerBand.svelte";
import PercentSlider from "../src/components/pianoroll/PercentSlider.svelte";
import { cancelDrag, ccLanes, prPanels, prView, velocityLane } from "../src/components/pianoroll/view.svelte";
import RightRail from "../src/components/RightRail.svelte";
import Transport from "../src/components/Transport.svelte";
import { tokensAt, type Scale } from "../src/design/tokens";
import { closeMenu, menuVisible } from "../src/lib/menu";
import { hideTip, tipView } from "../src/lib/tip.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import { take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  takeNotes: (id: number) => Promise.resolve(take([[0, TPQ, 60]], id)),
}));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  prPanels.list = true;
  prPanels.inspector = true;
  prView.focus = "arrange";
  ccLanes.open = [];
  velocityLane.open = true;
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (menuVisible()) closeMenu();
  hideTip();
  if (mounted) unmount(mounted);
  mounted = null;
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.body.innerHTML = "";
});

function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const showRoll = (state: AppState = fakeScene("pianoRoll")!.state) => show(LowerBand, state, { band: "pianoRoll", height: 640 });
const q = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector<T>(sel);
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const click = (sel: string) => {
  q(sel)!.click();
  flushSync();
};

/** el に 0.6 秒マウスを止めたときの用語。 */
function termAt(el: Element): string | null {
  vi.useFakeTimers();
  el.dispatchEvent(new PointerEvent("pointerenter"));
  vi.advanceTimersByTime(600);
  flushSync();
  const found = tipView.value?.term ?? null;
  el.dispatchEvent(new PointerEvent("pointerleave"));
  vi.useRealTimers();
  return found;
}

describe("ベロシティの帯をたたむ（仕様書 3 章の帯の表「ベロシティ」、1 章の例）", () => {
  it("「▾ ベロシティ」でたたむと、CC の札の行の左はしに「▸ ベロシティ」。押すとひらく。覚えている", () => {
    showRoll();
    expect(q('[data-test="pr-velocity"]')).not.toBeNull();
    expect(q('[data-test="pr-velocity-open"]')).toBeNull();
    click('[data-test="pr-velocity-fold"]');
    expect(q('[data-test="pr-velocity"]')).toBeNull();
    const tag = q('[data-test="pr-velocity-open"]')!;
    expect(text(tag)).toBe("▸ベロシティ");
    expect(tag.parentElement!.firstElementChild).toBe(tag);
    expect(velocityLane.open).toBe(false);

    // 帯をたたんでひらきなおしても、たたんだまま。
    unmount(mounted!);
    mounted = null;
    showRoll();
    expect(q('[data-test="pr-velocity"]')).toBeNull();
    click('[data-test="pr-velocity-open"]');
    expect(q('[data-test="pr-velocity"]')).not.toBeNull();
    expect(q('[data-test="pr-velocity-open"]')).toBeNull();
  });

  // 仕様書 1 章の例は 18 行 ・ 24 行。上のメニューの行（150 % で 36）が入って 2 行ずつへった（docs/history/design-notes.ja.md B-59）。
  it("窓 1280×800 で 150 % のとき、ノートは 16 行。たたむと 22 行（たたんだ 72 はノートの場所へ）", () => {
    const scale: Scale = 150;
    const scene = fakeScene("pianoRoll150")!;
    const band = scene.state.workspace.layout.lower.find((l) => l.band === "pianoRoll")!.height;
    const px = tokensAt(scale, 1);
    const n = (name: string) => px[name] as number;
    // 帯の高さから、道具の列、目盛、ベロシティの帯、CC の札の行を引いた残りがノートの場所（Roll.svelte と PianoRoll.svelte の縦の並び）。
    const around = n("size.toolbar") + n("mock.pianoroll.ruler") + n("size.row");
    const rows = (velocity: boolean) => Math.floor((Math.round((band * scale) / 100) - around - (velocity ? n("size.lane.vel") : 0)) / n("size.note"));
    expect(scene.state.workspace.viewportWidth).toBe(1280);
    expect(scene.state.workspace.viewportHeight).toBe(800);
    expect(rows(true)).toBe(16);
    expect(rows(false)).toBe(22);
  });
});

describe("キーが向く帯の印（B-72）", () => {
  it("ピアノロールに向いているときは、道具の列の上辺。アレンジに向いているときは、アレンジの見出しの行", () => {
    const state = fakeScene("pianoRoll")!.state;
    showRoll(state);
    expect(q('[data-test="pr-focus-mark"]')).toBeNull();
    prView.focus = "pianoRoll";
    flushSync();
    expect(q('[data-test="pr-focus-mark"]')).not.toBeNull();
    unmount(mounted!);
    mounted = null;

    show(Arrange, state);
    expect(q('[data-test="arrange-focus-mark"]')).toBeNull();
    prView.focus = "arrange";
    flushSync();
    expect(q('[data-test="arrange-focus-mark"]')).not.toBeNull();
  });

  it("ピアノロールがひらいていなければ、いつもアレンジ", () => {
    prView.focus = "pianoRoll";
    show(Arrange, app.stateWith(() => {}));
    expect(q('[data-test="arrange-focus-mark"]')).not.toBeNull();
  });
});

describe("ボタンのキー（B-73）", () => {
  const ready = (edit: (s: AppState) => void = () => {}) =>
    app.stateWith((s) => {
      s.transport.canPlay = true;
      edit(s);
    });

  it("再生まわり: ⏮ Home、▶ Space、● R、ループ L、メトロノーム C。■ とカウントインには書かない（F-WideArrange）", () => {
    show(Transport, ready());
    const keyOf = (name: string) => text(q(`[data-test="transport-${name}"] .key`));
    expect(["to-start", "play", "stop", "record", "loop", "metronome", "count-in"].map(keyOf)).toEqual([
      "Home",
      "Space",
      null,
      "R",
      "L",
      "C",
      null,
    ]);
  });

  it("ピアノロールの道具: 選ぶ 1 ・ 書く 2 ・ 消す 3", () => {
    showRoll();
    expect(["select", "draw", "erase"].map((tool) => text(q(`[data-test="pr-tool-${tool}"]`)))).toEqual(["選ぶ1", "書く2", "消す3"]);
  });
});

describe("ひらいた帯の見出しを押すとたたむ（B-74）", () => {
  it("「▾ ピアノロール」は toggleBand pianoRoll", () => {
    showRoll();
    click('[data-test="pr-fold"]');
    expect(app.sent).toEqual([{ type: "toggleBand", band: "pianoRoll" }]);
  });

  it("「▾ ミキサー」「▾ 取り消しの履歴」", () => {
    show(LowerBand, app.stateWith(() => {}), { band: "mixer", height: 516 });
    click('[data-test="band-fold"]');
    unmount(mounted!);
    mounted = null;
    show(LowerBand, app.stateWith(() => {}), { band: "history", height: 248 });
    click('[data-test="band-fold"]');
    expect(app.sent).toEqual([
      { type: "toggleBand", band: "mixer" },
      { type: "toggleBand", band: "history" },
    ]);
  });

  it("「▾ 棚」「▾ パラメータ」", () => {
    show(
      RightRail,
      app.stateWith((s) => {
        s.workspace.layout.rightOpen = ["shelf"];
        s.workspace.layout.rightFront = "shelf";
      }),
    );
    expect(text(q('[data-test="rail-fold"]'))).toBe("▾ 棚");
    click('[data-test="rail-fold"]');
    expect(app.sent).toEqual([{ type: "toggleBand", band: "shelf" }]);
  });
});

describe("スケール固定の ▾（B-65）", () => {
  it("ボタンの右に ▾。音階の一覧をひらき、えらぶと lock: true で setMusicalScale", async () => {
    showRoll();
    const lock = q('[data-test="pr-scale-lock"]')!;
    const stub = q('[data-test="pr-scale-menu"]')!;
    expect(lock.nextElementSibling).toBe(stub);
    expect(text(stub)).toBe("▾");
    click('[data-test="pr-scale-menu"]');
    await new Promise((r) => setTimeout(r, 0));
    const menu = q('[data-test="scale-menu"]')!;
    expect(menu).not.toBeNull();
    expect(menuVisible()).toBe(true);
    const rows = [...menu.querySelectorAll<HTMLButtonElement>("button")];
    rows.find((b) => text(b) === "マイナー")!.click();
    flushSync();
    const pr = fakeScene("pianoRoll")!.state.pianoRoll;
    expect(app.sent).toEqual([{ type: "setMusicalScale", root: pr.scale.root, mode: "minor", lock: true }]);
    expect(q('[data-test="scale-menu"]')).toBeNull();
  });
});

describe("ドラッグ中の Esc（仕様書 4 章）", () => {
  it("ループの範囲のドラッグをやめて、前の範囲に戻す", () => {
    const s = app.stateWith(() => {});
    show(Arrange, s);
    const ruler = q(".ruler.bars")!;
    const pointer = (type: string, x: number) => {
      ruler.dispatchEvent(new MouseEvent(type, { bubbles: true, clientX: x, clientY: 10, button: 0 }));
      flushSync();
    };
    pointer("pointerdown", 256);
    pointer("pointermove", 512);
    expect(q(".loop-mark")).not.toBeNull();
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(q(".loop-mark")).toBeNull();
    pointer("pointerup", 512);
    expect(app.sent).toEqual([]);
    expect(cancelDrag()).toBe(false);
  });

  it("強さ ・ スウィングの棒: ドラッグをやめて、動かす前の値に戻す。onchange は送らない", () => {
    const changes: number[] = [];
    show(PercentSlider, app.stateWith(() => {}), { value: 75, label: "強さ", test: "s", onchange: (v: number) => changes.push(v) });
    const bar = q('[data-test="s"]')!;
    bar.getBoundingClientRect = () => ({ left: 0, width: 100, top: 0, height: 12, right: 100, bottom: 12, x: 0, y: 0, toJSON() {} });
    bar.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, clientX: 20, button: 0 }));
    flushSync();
    expect(text(q('[data-test="s-value"]'))).toBe("20 %");
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(text(q('[data-test="s-value"]'))).toBe("75 %");
    bar.dispatchEvent(new MouseEvent("pointerup", { bubbles: true, clientX: 20, button: 0 }));
    expect(changes).toEqual([]);
  });
});

describe("強さ ・ スウィングの棒に Tab で届き、← → で動く（仕様書 4 章）", () => {
  it("tabindex 0。← → で 10、Shift で 1。document のキーの係には渡さない", () => {
    const changes: number[] = [];
    show(PercentSlider, app.stateWith(() => {}), { value: 75, label: "強さ", test: "s", onchange: (v: number) => changes.push(v) });
    const bar = q('[data-test="s"]')!;
    expect(bar.tabIndex).toBe(0);
    let reached = 0;
    const onDoc = () => reached++;
    document.addEventListener("keydown", onDoc);
    const key = (k: string, shiftKey = false) => {
      bar.dispatchEvent(new KeyboardEvent("keydown", { key: k, shiftKey, bubbles: true }));
      flushSync();
    };
    key("ArrowRight");
    key("ArrowLeft", true);
    key("ArrowRight");
    key("ArrowRight");
    document.removeEventListener("keydown", onDoc);
    expect(changes).toEqual([85, 84, 94, 100]);
    expect(reached).toBe(0);
  });
});

describe("用語の説明（仕様書 5 章）", () => {
  it("帯札: アレンジ ・ ピアノロール ・ ミキサーの名前と、ピアノロールの件数の「ノート」", () => {
    // jsdom には幅がないので、件数まで入る幅にする。
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1280);
    show(BandRow, app.stateWith(() => {}));
    const name = (band: string) => q(`[data-band="${band}"] .m-name`)!;
    expect(termAt(name("arrange"))).toBe("アレンジ");
    expect(termAt(name("pianoRoll"))).toBe("ピアノロール");
    expect(termAt(name("mixer"))).toBe("ミキサー");
    const note = [...q('[data-band="pianoRoll"]')!.querySelectorAll("span")].find((s) => s.textContent === "ノート")!;
    expect(termAt(note)).toBe("ノート");
  });

  it("ピアノロール: 見出し、左右の欄の「ノート」、ベロシティ、CC の札", () => {
    showRoll(fakeScene("pianoRollCc")!.state);
    expect(termAt(q('[data-test="pr-fold"] .m-name')!)).toBe("ピアノロール");
    const word = (root: string, w: string) => [...q(root)!.querySelectorAll("span")].find((s) => s.textContent === w)!;
    expect(termAt(word('[data-test="pr-list-fold"]', "ノート"))).toBe("ノート");
    expect(termAt(word('[data-test="pr-inspector-fold"]', "ノート"))).toBe("ノート");
    expect(termAt(word('[data-test="pr-velocity-fold"]', "ベロシティ"))).toBe("ベロシティ");
    expect(termAt(word('[data-test="pr-cc-sustain"]', "サステイン"))).toBe("サステイン");
    expect(termAt(word('[data-test="pr-cc-pitchBend"]', "ピッチベンド"))).toBe("ピッチベンド");
    expect(termAt(word('[data-test="pr-cc-modulation"]', "モジュレーション"))).toBe("モジュレーション");
  });

  it("アレンジ: 見出し、トラックの名前、トラックの dB", () => {
    show(Arrange, app.stateWith(() => {}));
    expect(termAt(q(".title-row .m-name")!)).toBe("アレンジ");
    expect(termAt(q(".track-name")!)).toBe("トラック");
    const db = [...document.querySelectorAll(".track-head .m-num")].find((s) => s.textContent?.endsWith("dB"))!;
    expect(termAt(db)).toBe("dB");
  });
});
