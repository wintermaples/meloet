// ピアノロールの左右の欄（ノートの一覧、選んだノート）と状態行を mount して見る。見せているテイクは偽物の場面のもの。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TakeNotes } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { runFrameCallbacks } from "../src/gl/slot";
import NoteInspector from "../src/components/pianoroll/NoteInspector.svelte";
import NoteListPanel from "../src/components/pianoroll/NoteListPanel.svelte";
import { dragReading } from "../src/components/pianoroll/view.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

const BAR = 1920;
const at = (bar: number, beat: number, tick: number) => (bar - 1) * BAR + (beat - 1) * 480 + tick;

// jsdom は大きさを測らない。一覧の見えている高さを 200（行 10 本ぶん）にする。
const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientHeight");
beforeAll(() => Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => 200 }));
afterAll(() => had && Object.defineProperty(proto, "clientHeight", had));

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
  dragReading.text = null;
});

const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

/** 偽物の場面を状態にし、その場面の見せているテイクを shown に入れる。 */
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

function show(component: Component): void {
  mounted = mount(component, { target });
  flushSync();
}

describe("ノートの一覧（モック A-PianoRoll の左の欄）", () => {
  it("見出しの行と、ノートごとの行（位置 ・ 音 ・ 長さ ・ Vel）", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    expect(text(q(".head"))).toBe("位置 音 長さ Vel");
    const rows = qa(".row");
    expect(rows.slice(0, 5).map(text)).toEqual(["1.1.000 C3 4.000 70", "1.1.000 G3 4.000 64", "1.1.000 E4 1.000 87", "1.2.000 G4 1.000 92", "1.3.000 C5 1.240 101"]);
  });

  it("選んだノート（D4 E4 F4）の行は selected", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    const viewport = q('[data-test="note-list-rows"]')!;
    viewport.scrollTop = 200;
    viewport.dispatchEvent(new Event("scroll"));
    flushSync();
    const picked = qa(".row.selected").map(text);
    expect(picked).toEqual(["3.1.000 D4 0.360 80", "3.1.360 E4 0.360 83", "3.2.240 F4 0.240 77"]);
  });

  it("行を押すとそのノートを選ぶ（replace）。Shift で足す（add）", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteListPanel);
    qa(".row")[2]!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    qa(".row")[3]!.dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
    expect(app.sent).toEqual([
      { type: "selectNotes", take: take.id, notes: [2], mode: "replace" },
      { type: "selectNotes", take: take.id, notes: [3], mode: "add" },
    ]);
  });

  it("ノートが 1 万あっても、作る行は見えているぶんだけ。スクロールすると、その所の行", async () => {
    await scene("pianoRoll");
    const n = 10_000;
    const big: TakeNotes = {
      id: 999,
      length: n * 120,
      origin: 0,
      starts: Uint32Array.from({ length: n }, (_, i) => i * 120),
      lengths: new Uint32Array(n).fill(120),
      keys: new Uint8Array(n).fill(60),
      velocities: new Uint8Array(n).fill(64),
      froms: new Uint32Array(n),
      controlTicks: new Uint32Array(0),
      controlKinds: new Uint8Array(0),
      controlNumbers: new Uint8Array(0),
      controlValues: new Int16Array(0),
    };
    const s = app.stateWith(() => {});
    const clip = { ...s.song.tracks[0]!.clips[0]!, id: 1, start: 0, end: n * 120, takeId: 999 };
    shown.setShown({ clip: { ...clip, take: 1, takes: 1, takeOffset: 0, noteCount: n, name: "x" }, track: s.song.tracks[0]!, take: big });
    app.setState(s);
    show(NoteListPanel);
    expect(qa(".row").length).toBeLessThanOrEqual(12);
    const viewport = q('[data-test="note-list-rows"]')!;
    expect(q(".rows")!.style.height).toBe(`${n * 20}px`);
    viewport.scrollTop = 5000 * 20;
    viewport.dispatchEvent(new Event("scroll"));
    flushSync();
    expect(qa(".row")[0]!.dataset.note).toBe("5000");
    expect(qa(".row").length).toBeLessThanOrEqual(12);
  });
});

describe("選んだノート（モック A-PianoRoll の右の欄）", () => {
  it("位置「3.1.000 〜」、音「D4 E4 F4」、長さ「ばらばら」、ベロシティ「77 – 83」、数 3", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    expect(text(q('[data-test="inspector-position"]'))).toBe("3.1.000 〜");
    expect(text(q('[data-test="inspector-keys"]'))).toBe("D4 E4 F4");
    expect(text(q('[data-test="inspector-length"]'))).toBe("ばらばら");
    expect(text(q('[data-test="inspector-velocity"]'))).toBe("77 – 83");
    expect(text(q(".count"))).toBe("3");
  });

  it("1 つだけなら位置に「〜」をつけず、長さとベロシティは値", async () => {
    await scene("pianoRoll", (s) => (s.pianoRoll.selectedNotes = [{ start: 14, end: 15 }]));
    show(NoteInspector);
    expect(text(q('[data-test="inspector-position"]'))).toBe("3.1.000");
    expect(text(q('[data-test="inspector-length"]'))).toBe("0.360");
    expect(text(q('[data-test="inspector-velocity"]'))).toBe("80");
  });

  it("クオンタイズ: テイクがクオンタイズしてあれば「かけてある」。値は pianoRoll.quantize、ボタンは使えない", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    expect(text(q('[data-test="inspector-quantized"]'))).toBe("かけてある");
    const body = text(q('[data-testid="pr-inspector"]'))!;
    expect(body).toContain("グリッド 1/16");
    expect(body).toContain("強さ 75 %");
    expect(body).toContain("スウィング 0 %");
    expect(body).toContain("そろえる所 はじまり");
    const buttons = qa(".buttons .m-btn");
    expect(buttons.map(text)).toEqual(["かけなおす", "もとの位置へ戻す"]);
    expect(buttons.every((b) => b.getAttribute("aria-disabled") === "true")).toBe(true);
  });

  it("長さをそろえる ・ ベロシティをまとめて変える は、たたんだ「使えない」見出し", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    const folded = qa(".hd.folded");
    expect(folded.map(text)).toEqual(["▸ 長さをそろえる", "▸ ベロシティをまとめて変える"]);
    expect(folded.every((b) => b.getAttribute("aria-disabled") === "true")).toBe(true);
  });

  it("固定を押すと、オン ・ オフを入れかえる意図を送る（Rust の状態を見せるだけで、自分では変えない）", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    const lock = q('[data-test="inspector-lock"]')!;
    expect(text(lock)).toBe("オン");
    expect(lock.classList.contains("on")).toBe(true);
    lock.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setMusicalScale", root: 0, mode: "major", lock: false }]);
    expect(text(lock)).toBe("オン");
  });

  it("音階を押すと一覧: 見出し「種類」の下に メジャー ・ マイナー、「主音」の下に C 〜 B。えらぶと setMusicalScale", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    const button = q('[data-test="inspector-scale"]')!;
    expect(text(button)).toBe("C メジャー");
    button.click();
    flushSync();
    const menu = document.querySelector<HTMLElement>('[data-test="scale-menu"]')!;
    expect(text(menu)).toBe("種類 メジャー マイナー 主音 C C# D D# E F F# G G# A A# B");
    const checked = [...menu.querySelectorAll('[aria-checked="true"]')].map(text);
    expect(checked).toEqual(["メジャー", "C"]);
    const a = [...menu.querySelectorAll<HTMLButtonElement>("button")].find((b) => text(b) === "A")!;
    a.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setMusicalScale", root: 9, mode: "major", lock: true }]);
    expect(document.querySelector('[data-test="scale-menu"]')).toBeNull();
  });
});

describe("状態行（ピアノロールのまわり）", () => {
  const left = () => text(q(".message"));

  it("選んだノート: 「選択 3 ノート ・ D4 E4 F4 ・ クオンタイズ済み（もとの位置へ戻せます）」（モック A-PianoRoll）", async () => {
    await scene("pianoRoll");
    show(StatusLine);
    expect(left()).toBe("選択 3 ノート ・ D4 E4 F4 ・ クオンタイズ済み（もとの位置へ戻せます）");
  });

  it("クオンタイズしていなければ、うしろは出さない。ループがオンなら前に「ループ 5–9」", async () => {
    await scene("pianoRoll", (s) => {
      s.status.message = { id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: false, notes: { count: 6, keys: [60, 62, 64, 65], quantized: false } };
    });
    show(StatusLine);
    expect(left()).toBe("ループ 5–9 ・ 選択 6 ノート ・ C4 D4 E4 F4");
  });

  it("音階の外に書こうとした（仕様書 6 章）", async () => {
    await scene("pianoRoll", (s) => (s.status.message = { id: "outOfScale", key: 63, root: 0, mode: "major" }));
    show(StatusLine);
    expect(left()).toBe("D# は C メジャーの外の音です（スケール固定 ・ オン）");
  });

  it("ドラッグしている間は、その場の読み値を左の文の代わりに", async () => {
    await scene("pianoRoll");
    show(StatusLine);
    dragReading.text = "+0.240 ・ +2 半音";
    flushSync();
    expect(left()).toBe("+0.240 ・ +2 半音");
    dragReading.text = null;
    flushSync();
    expect(left()).toContain("選択 3 ノート");
  });

  it("ピアノロールをひらいて再生中: 「1 ピアノ ・ Aメロ ・ テイク 2 / 3 ・ 鳴っている音 C3 E3 G3 C4」（モック F-PRPlay）", async () => {
    await scene("pianoRollPlay", (s) => (s.transport.position = at(4, 1, 240)));
    show(StatusLine);
    expect(left()).toBe("1 ピアノ ・ Aメロ ・ テイク 2 / 3");
    runFrameCallbacks();
    flushSync();
    expect(left()).toBe("1 ピアノ ・ Aメロ ・ テイク 2 / 3 ・ 鳴っている音 C3 E3 G3 C4");
  });

  it("録音中は仕様書 6 章「録音中」の文のまま", async () => {
    await scene("pianoRollRec");
    show(StatusLine);
    expect(left()).toBe("1 ピアノ へ録音 ・ 止めると テイク 3 として残ります");
  });
});
