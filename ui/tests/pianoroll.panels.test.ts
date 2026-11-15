// ピアノロールの左右の欄（ノートの一覧、選んだノート）と状態行を mount して見る。見せているテイクは偽物の場面のもの。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TakeNotes } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { runFrameCallbacks } from "../src/gl/slot";
import NoteInspector from "../src/components/pianoroll/NoteInspector.svelte";
import NoteListPanel from "../src/components/pianoroll/NoteListPanel.svelte";
import { dragReading, prView } from "../src/components/pianoroll/view.svelte";
import { attachKeys } from "../src/keys/attach";
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
  it("位置「3.1.000 〜」、音「D4 E4 F4」、長さ「ばらばら」、ベロシティ「77 – 83」", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    expect(text(q('[data-test="inspector-position"]'))).toBe("3.1.000 〜");
    expect(text(q('[data-test="inspector-keys"]'))).toBe("D4 E4 F4");
    expect(text(q('[data-test="inspector-length"]'))).toBe("ばらばら");
    expect(text(q('[data-test="inspector-velocity"]'))).toBe("77 – 83");
  });

  it("1 つだけなら位置に「〜」をつけず、長さとベロシティは値", async () => {
    await scene("pianoRoll", (s) => (s.pianoRoll.selectedNotes = [{ start: 14, end: 15 }]));
    show(NoteInspector);
    expect(text(q('[data-test="inspector-position"]'))).toBe("3.1.000");
    expect(text(q('[data-test="inspector-length"]'))).toBe("0.360");
    expect(text(q('[data-test="inspector-velocity"]'))).toBe("80");
  });

  it("クオンタイズ: テイクがクオンタイズしてあれば「かけてある」。値は pianoRoll.quantize、ボタンはどちらも使える", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    expect(text(q('[data-test="inspector-quantized"]'))).toBe("かけてある");
    const body = text(q('[data-testid="pr-inspector"]'))!;
    expect(body).toContain("グリッド 1/16 三連符");
    expect(body).toContain("強さ 75 %");
    expect(body).toContain("スウィング 0 %");
    expect(body).toContain("そろえる所 はじまり");
    const buttons = qa(".buttons .m-btn");
    expect(buttons.map(text)).toEqual(["かけなおす", "もとの位置へ戻す"]);
    expect(buttons.map((b) => b.getAttribute("aria-disabled"))).toEqual(["false", "false"]);
    expect(buttons[0]!.classList.contains("hi")).toBe(true);
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

// ---- 打ちこみ（仕様書 3 章「ノートの一覧」、モック R3-Lanes） ----

const key = (el: Element, k: string, init: KeyboardEventInit = {}) =>
  el.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...init }));
const field = () => target.querySelector<HTMLInputElement>("input");
function typeIn(value: string): void {
  const input = field()!;
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  flushSync();
}
const settle = async () => {
  await Promise.resolve();
  await Promise.resolve();
  flushSync();
};
const double = (row: number, name: string) =>
  qa(".row")[row]!.querySelector(`[data-field="${name}"]`)!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));

describe("ノートの一覧で値を打ちこむ（モック R3-Lanes）", () => {
  it("欄を 2 回押すと、その欄が入力の枠。Enter で、その行のノート 1 つへ setNotesField", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteListPanel);
    double(2, "velocity");
    flushSync();
    expect(field()!.value).toBe("87");
    expect(document.activeElement).toBe(field());
    typeIn("96");
    expect(field()!.classList.contains("broken")).toBe(false);
    key(field()!, "Enter");
    await settle();
    expect(app.sent).toEqual([{ type: "setNotesField", take: take.id, notes: [2], field: "velocity", value: 96 }]);
    expect(field()).toBeNull();
    expect(document.activeElement).toBe(qa(".row")[2]);
  });

  it("使えない値（200）は「こわれた」の枠のまま。Enter で rejectValue を送り、決めない", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    double(4, "velocity");
    flushSync();
    typeIn("200");
    expect(field()!.classList.contains("broken")).toBe(true);
    expect(field()!.getAttribute("aria-invalid")).toBe("true");
    key(field()!, "Enter");
    flushSync();
    expect(app.sent).toEqual([{ type: "rejectValue", field: "velocity", kept: "101", range: null }]);
    expect(field()!.value).toBe("200");
  });

  it("位置の使えない値は、そのクリップでの範囲の文をつけて rejectValue", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    double(0, "position");
    flushSync();
    typeIn("4.2.000");
    key(field()!, "Enter");
    expect(app.sent).toEqual([{ type: "rejectValue", field: "position", kept: "1.1.000", range: "1.1.000〜4.1.000" }]);
  });

  it("行で Enter を押すと最初の欄（位置）。Tab で右の欄へ、右はしなら次の行の最初の欄（そこで前の欄を決める）", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteListPanel);
    const row = qa(".row")[0]!;
    row.focus();
    key(row, "Enter");
    flushSync();
    expect(field()!.value).toBe("1.1.000");
    key(field()!, "Tab");
    flushSync();
    expect(field()!.value).toBe("C3");
    key(field()!, "Tab");
    flushSync();
    expect(field()!.value).toBe("4.000");
    key(field()!, "Tab");
    flushSync();
    typeIn("71");
    key(field()!, "Tab");
    flushSync();
    expect(field()!.value).toBe("1.1.000");
    expect(field()!.closest(".row")!.getAttribute("data-note")).toBe("1");
    expect(app.sent).toEqual([{ type: "setNotesField", take: take.id, notes: [0], field: "velocity", value: 71 }]);
    key(field()!, "Tab", { shiftKey: true });
    flushSync();
    expect(field()!.closest(".row")!.getAttribute("data-note")).toBe("0");
  });

  it("↑ ↓ で 1 ずつ（Shift で 10）。音は半音、位置は表示のいちばん下のけた", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    double(2, "key");
    flushSync();
    key(field()!, "ArrowUp");
    flushSync();
    expect(field()!.value).toBe("F4");
    key(field()!, "ArrowDown", { shiftKey: true });
    flushSync();
    expect(field()!.value).toBe("G3");
    key(field()!, "Tab", { shiftKey: true });
    flushSync();
    key(field()!, "ArrowUp", { shiftKey: true });
    flushSync();
    expect(field()!.value).toBe("1.1.010");
  });

  it("Esc でやめる（何も送らない）。枠の外を押しても、やめる", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    double(0, "length");
    flushSync();
    typeIn("2.000");
    key(field()!, "Escape");
    await settle();
    expect(field()).toBeNull();
    double(0, "length");
    flushSync();
    field()!.blur();
    flushSync();
    expect(field()).toBeNull();
    expect(app.sent).toEqual([]);
  });

  it("打っている最中のキー（仕様書 4 章）: 一文字のキー、Space、Delete、Backspace、↑ ↓ ← → は枠のもの。ピアノロールには効かない", async () => {
    const { s } = await scene("pianoRoll");
    show(NoteListPanel);
    prView.focus = "pianoRoll";
    const global: unknown[] = [];
    const detach = attachKeys({ state: () => s, send: (c) => global.push(c), nav: { platform: "Linux x86_64" } });
    try {
      double(0, "position");
      flushSync();
      for (const k of ["e", "m", "q", "r", " ", "Delete", "Backspace", "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "1"]) key(field()!, k);
      flushSync();
      expect(global).toEqual([]);
      expect(field()).not.toBeNull();
    } finally {
      detach();
    }
  });
});

describe("選んだノートの欄を打ちこむ", () => {
  it("押すと入力の枠。2 つ以上なら、選んだぜんぶへ（ベロシティはぜんぶ同じ値）", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="inspector-velocity"]')!.click();
    flushSync();
    expect(field()!.value).toBe("77");
    typeIn("90");
    key(field()!, "Enter");
    await settle();
    expect(app.sent).toEqual([{ type: "setNotesField", take: take.id, notes: [14, 15, 16], field: "velocity", value: 90 }]);
    expect(text(q('[data-test="inspector-velocity"]'))).toBe("77 – 83");
  });

  it("位置はいちばん早い位置、音はいちばん低い音、長さ「ばらばら」は、いちばん早いノートの長さを入れておく", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="inspector-position"]')!.click();
    flushSync();
    expect(field()!.value).toBe("3.1.000");
    key(field()!, "Tab");
    flushSync();
    expect(field()!.value).toBe("D4");
    key(field()!, "Tab");
    flushSync();
    expect(field()!.value).toBe("0.360");
    expect(app.sent).toEqual([]);
  });

  it("使えない値: 「ばらばら」の欄は、もとの値として「ばらばら」", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="inspector-length"]')!.click();
    flushSync();
    typeIn("9.000");
    key(field()!, "Enter");
    expect(app.sent).toEqual([{ type: "rejectValue", field: "length", kept: "ばらばら", range: "0.001〜6.240" }]);
  });
});

describe("クオンタイズの欄", () => {
  it("グリッドを押すと一覧（1 小節 ・ 1/2 ・ 1/4 ・ 1/8 ・ 1/16 ・ 1/32）。えらぶと setQuantize（いまの設定ぜんぶ）", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="quantize-grid"]')!.click();
    flushSync();
    const menu = document.querySelector<HTMLElement>('[data-test="quantize-grid-menu"]')!;
    expect(text(menu)).toBe("グリッド 1 小節 1/2 1/4 1/8 1/16 1/32");
    [...menu.querySelectorAll<HTMLButtonElement>("button")].find((b) => text(b) === "1/8")!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setQuantize", division: 8, triplet: false, strength: 75, swing: 0, align: "start" }]);
    expect(document.querySelector('[data-test="quantize-grid-menu"]')).toBeNull();
  });

  it("三連符は、道具の列と同じく別の切りかえ", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    const triplet = q('[data-test="quantize-triplet"]')!;
    expect(triplet.getAttribute("aria-pressed")).toBe("false");
    triplet.click();
    expect(app.sent).toEqual([{ type: "setQuantize", division: 16, triplet: true, strength: 75, swing: 0, align: "start" }]);
  });

  it("そろえる所: はじまり ・ はじまりと終わり", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="quantize-align"]')!.click();
    flushSync();
    const menu = document.querySelector<HTMLElement>('[data-test="quantize-align-menu"]')!;
    expect(text(menu)).toBe("そろえる所 はじまり はじまりと終わり");
    [...menu.querySelectorAll<HTMLButtonElement>("button")][1]!.click();
    expect(app.sent).toEqual([{ type: "setQuantize", division: 16, triplet: false, strength: 75, swing: 0, align: "startAndEnd" }]);
  });

  it("強さ: ドラッグしているあいだは画面だけ。離して setQuantize を 1 つ", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    const bar = q('[data-test="quantize-strength"]')!;
    bar.getBoundingClientRect = () => ({ left: 100, width: 200, top: 0, height: 12, right: 300, bottom: 12, x: 100, y: 0, toJSON: () => ({}) });
    const pointer = (type: string, x: number) =>
      bar.dispatchEvent(Object.assign(new MouseEvent(type, { bubbles: true, clientX: x, button: 0 }), { pointerId: 1 }));
    pointer("pointerdown", 200);
    pointer("pointermove", 220);
    flushSync();
    expect(text(q('[data-test="quantize-strength-value"]'))).toBe("60 %");
    expect(app.sent).toEqual([]);
    pointer("pointermove", 240);
    pointer("pointerup", 240);
    flushSync();
    expect(app.sent).toEqual([{ type: "setQuantize", division: 16, triplet: false, strength: 70, swing: 0, align: "start" }]);
    expect(text(q('[data-test="quantize-strength-value"]'))).toBe("70 %");
  });

  it("かけなおす: 選んだノート（なければ全部）へ quantize。もとの位置へ戻す: unquantize", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="quantize-apply"]')!.click();
    q('[data-test="quantize-restore"]')!.click();
    expect(app.sent).toEqual([
      { type: "quantize", take: take.id, notes: [14, 15, 16] },
      { type: "unquantize", take: take.id, notes: [14, 15, 16] },
    ]);
  });

  it("クオンタイズしていないテイクでは「もとの位置へ戻す」は使えない。「かけてある」も出ない", async () => {
    const { s, take } = await scene("pianoRoll");
    const plain = { ...take, origin: 0 };
    shown.setShown({ clip: shown.shown.clip, track: shown.shown.track, take: plain });
    app.setState({ ...s, pianoRoll: { ...s.pianoRoll, selectedNotes: [] } });
    show(NoteInspector);
    expect(q('[data-test="inspector-quantized"]')).toBeNull();
    expect(q('[data-test="quantize-restore"]')!.getAttribute("aria-disabled")).toBe("true");
    q('[data-test="quantize-restore"]')!.click();
    q('[data-test="quantize-apply"]')!.click();
    expect(app.sent).toEqual([{ type: "quantize", take: plain.id, notes: [] }]);
  });
});

describe("状態行: クオンタイズをかけた ・ 使えない値を打った", () => {
  const left = () => text(q(".message"));

  it("「クオンタイズ 1/16（75 %）を 3 ノートに。うすい枠が、そろえる前の位置。」と「もとの位置へ戻す」（仕様書 6 章）", async () => {
    const { take } = await scene("pianoRoll", (s) => (s.status.message = { id: "quantized", division: 16, triplet: false, strength: 75, count: 3 }));
    show(StatusLine);
    expect(left()).toBe("クオンタイズ 1/16（75 %）を 3 ノートに。うすい枠が、そろえる前の位置。");
    const button = q(".link")!;
    expect(text(button)).toBe("もとの位置へ戻す");
    button.click();
    expect(app.sent).toEqual([{ type: "unquantize", take: take.id, notes: [14, 15, 16] }]);
  });

  it("位置の範囲はクリップで変わる（範囲の文は知らせについてくる）", async () => {
    await scene("pianoRoll", (s) => (s.status.message = { id: "valueRejected", field: "position", kept: "1.1.000", range: "1.1.000〜4.1.000" }));
    show(StatusLine);
    expect(left()).toBe("位置は 1.1.000〜4.1.000 です。もとの 1.1.000 のままです。");
  });
});
