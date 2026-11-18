// 第 3 回の続きの区切り1 を mount して見る: 縦の − ＋、左の鍵盤の列を押して鳴らす、目盛を押す ・ ドラッグする、
// 右クリックの一覧（中身は contextMenu.open に入る。描くのは App の ContextMenu）、ベロシティをまとめて変える。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TakeNotes } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { closeContextMenu, contextMenu } from "../src/components/menu/contextMenu.svelte";
import * as G from "../src/components/pianoroll/geom";
import NoteInspector from "../src/components/pianoroll/NoteInspector.svelte";
import Roll from "../src/components/pianoroll/Roll.svelte";
import { cancelDrag, DEFAULT_ROW_PX, inspectorFolds, prView, pxPerTick } from "../src/components/pianoroll/view.svelte";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  prView.rowPx = DEFAULT_ROW_PX;
  prView.topKey = 72;
  prView.scrollTick = 0;
  inspectorFolds.velocityAll = false;
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  closeContextMenu();
  target.remove();
});

const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

async function scene(name: string, edit?: (s: AppState) => void): Promise<{ s: AppState; take: TakeNotes }> {
  const s = fakeScene(name)!.state;
  edit?.(s);
  const track = s.song.tracks[0]!;
  const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
  const take = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
  app.setState(s);
  shown.setShown({ clip, track, take });
  return { s, take };
}

function show(component: Component<any>): void {
  mounted = mount(component, { target, props: {} });
  flushSync();
}

const pointer = (el: Element, type: string, init: PointerEventInit) =>
  el.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, ...init }));

describe("縦の − ＋（B-63）", () => {
  it("目盛の左の角。押すと 1 段。はしの段では押せない", async () => {
    await scene("pianoRoll");
    show(Roll);
    const zoomIn = q<HTMLButtonElement>('[data-test="pr-row-zoom-in"]')!;
    const zoomOut = q<HTMLButtonElement>('[data-test="pr-row-zoom-out"]')!;
    expect(zoomIn.closest(".ruler-gap")).not.toBeNull();
    expect([text(zoomOut), text(zoomIn)]).toEqual(["−", "＋"]);
    expect([zoomOut.getAttribute("aria-label"), zoomIn.getAttribute("aria-label")]).toEqual(["縦に縮小", "縦に拡大"]);
    zoomIn.click();
    flushSync();
    expect(prView.rowPx).toBe(16);
    for (let i = 0; i < 4; i++) zoomIn.click();
    flushSync();
    expect(prView.rowPx).toBe(24);
    expect(zoomIn.disabled).toBe(true);
    for (let i = 0; i < 5; i++) zoomOut.click();
    flushSync();
    expect(prView.rowPx).toBe(8);
    expect(zoomOut.disabled).toBe(true);
  });

  it("Mod Shift ＋ ホイールでも（上へ回すと拡大）", async () => {
    await scene("pianoRoll");
    show(Roll);
    q(".middle")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: -100, ctrlKey: true, shiftKey: true }));
    flushSync();
    expect(prView.rowPx).toBe(16);
    // Shift ＋ ホイールを横に変える環境（deltaX に来る）でも。
    q(".middle")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaX: 100, ctrlKey: true, shiftKey: true }));
    flushSync();
    expect(prView.rowPx).toBe(12);
  });
});

describe("左の鍵盤の列を押して鳴らす（B-62）", () => {
  it("押している間だけ。なぞると次の鍵に入れかえ、離すと止める。強さは 100", async () => {
    await scene("pianoRoll");
    show(Roll);
    const keys = q('[data-test="pr-keys"]')!;
    pointer(keys, "pointerdown", { clientY: 5 });
    pointer(keys, "pointermove", { clientY: 8 });
    pointer(keys, "pointermove", { clientY: 12 + 5 });
    pointer(keys, "pointerup", { clientY: 12 + 5 });
    expect(app.sent).toEqual([
      { type: "audition", keys: [72], velocity: 100 },
      { type: "audition", keys: [71], velocity: 100 },
      { type: "audition", keys: [], velocity: 0 },
    ]);
  });

  it("ベロシティ固定がオンならその値。録音中も鳴らす", async () => {
    await scene("pianoRollRec", (s) => (s.keyboards.velocityFixed = { on: true, value: 64 }));
    show(Roll);
    const keys = q('[data-test="pr-keys"]')!;
    pointer(keys, "pointerdown", { clientY: 5 });
    pointer(keys, "pointerup", { clientY: 5 });
    expect(app.sent).toEqual([
      { type: "audition", keys: [72], velocity: 64 },
      { type: "audition", keys: [], velocity: 0 },
    ]);
  });
});

describe("ピアノロールの目盛（B-61）", () => {
  it("押すと locate（グリッドに寄せる。Shift で寄せない）", async () => {
    const { s } = await scene("pianoRoll");
    show(Roll);
    const ruler = q('[data-test="pr-ruler"]')!;
    const ppt = pxPerTick(100);
    const meter = { num: s.song.meterNum, den: s.song.meterDen };
    const step = G.gridTicks(s.pianoRoll.grid, meter);
    const x = (step * 2 + step * 0.4) * ppt;
    pointer(ruler, "pointerdown", { clientX: x });
    pointer(ruler, "pointerup", { clientX: x });
    pointer(ruler, "pointerdown", { clientX: x, shiftKey: true });
    pointer(ruler, "pointerup", { clientX: x, shiftKey: true });
    expect(app.sent).toEqual([
      { type: "locate", tick: step * 2 },
      { type: "locate", tick: Math.round(step * 2.4) },
    ]);
  });

  it("ドラッグはループの範囲（小節の単位）。ドラッグ中の Esc で前の範囲に戻し、何も送らない", async () => {
    const { s } = await scene("pianoRoll");
    show(Roll);
    const ruler = q('[data-test="pr-ruler"]')!;
    const bar = 1920 * pxPerTick(100);
    expect(s.song.meterNum).toBe(4);
    pointer(ruler, "pointerdown", { clientX: bar + 2 });
    pointer(ruler, "pointermove", { clientX: 3 * bar - 2 });
    flushSync();
    expect(q(".loop-mark")).not.toBeNull();
    pointer(ruler, "pointerup", { clientX: 3 * bar - 2 });
    expect(app.sent).toEqual([{ type: "setLoop", fromBar: 2, toBar: 4 }]);
    app.sent.length = 0;
    pointer(ruler, "pointerdown", { clientX: bar + 2 });
    pointer(ruler, "pointermove", { clientX: 3 * bar - 2 });
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(q(".loop-mark")).toBeNull();
    pointer(ruler, "pointerup", { clientX: 3 * bar - 2 });
    expect(app.sent).toEqual([]);
  });
});

describe("右クリックの一覧（B-60）", () => {
  it("ノートの場所の空いた所: 貼りつけ ・ ぜんぶ選ぶ。WebKit の一覧は出さない", async () => {
    await scene("pianoRoll", (s) => (s.pianoRoll.clipboardNotes = 2));
    show(Roll);
    const e = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 40, clientY: 30 });
    q(".area")!.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(true);
    const open = contextMenu.open!;
    expect([open.x, open.y]).toEqual([40, 30]);
    expect(open.entries.map((r) => (r.kind === "row" ? `${r.label}${r.usable ? "" : " ×"}` : "／"))).toEqual(["menu.paste", "menu.selectAll"]);
    expect(prView.focus).toBe("pianoRoll");
  });
});

describe("ベロシティをまとめて変える（B-67）", () => {
  it("ひらくと「ベロシティ」の入力の枠と「そろえる」。ばらばらなら空。Enter かボタンで matchVelocity", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteInspector);
    const head = q('[data-test="inspector-velocity-all"]')!;
    expect(text(head)).toBe("▸ ベロシティをまとめて変える");
    expect(q('[data-test="inspector-velocity-all-field"]')).toBeNull();
    head.click();
    flushSync();
    expect(text(head)).toBe("▾ ベロシティをまとめて変える");
    const input = q<HTMLInputElement>('[data-test="inspector-velocity-all-field"]')!;
    const button = q('[data-test="inspector-match-velocity"]')!;
    expect(input.value).toBe("");
    expect(text(button)).toBe("そろえる");
    expect(button.getAttribute("aria-disabled")).toBe("false");
    input.value = "100";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
    expect(app.sent).toEqual([{ type: "matchVelocity", take: take.id, notes: [14, 15, 16], value: 100 }]);
    app.sent.length = 0;
    input.value = "300";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    button.click();
    expect(app.sent).toEqual([
      { type: "matchVelocity", take: take.id, notes: [14, 15, 16], value: 127 },
      { type: "clampValue", field: "velocity", value: "127", range: null },
    ]);
    app.sent.length = 0;
    input.value = "あ";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    expect(input.classList.contains("broken")).toBe(true);
    button.click();
    expect(app.sent).toEqual([{ type: "rejectValue", field: "velocity", kept: "77 – 83", range: null }]);
  });

  it("みな同じ値ならその値。ノートを選んでいなければ使えない。ひらいたのを覚えている", async () => {
    await scene("pianoRoll", (s) => (s.pianoRoll.selectedNotes = [{ start: 14, end: 15 }]));
    inspectorFolds.velocityAll = true;
    show(NoteInspector);
    expect(q<HTMLInputElement>('[data-test="inspector-velocity-all-field"]')!.value).toBe("80");
    unmount(mounted!);
    mounted = null;
    await scene("pianoRoll", (s) => (s.pianoRoll.selectedNotes = []));
    show(NoteInspector);
    const input = q<HTMLInputElement>('[data-test="inspector-velocity-all-field"]')!;
    expect(input.disabled).toBe(true);
    expect(q('[data-test="inspector-match-velocity"]')!.getAttribute("aria-disabled")).toBe("true");
    q('[data-test="inspector-match-velocity"]')!.click();
    expect(app.sent).toEqual([]);
  });
});
