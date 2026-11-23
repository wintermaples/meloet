// Mounts the piano roll's left and right panels (note list, selected notes) and the status line. The shown take comes from the fake scene.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TakeNotes } from "../src/bridge";
import { createFakeTransport, fakeScene, keyCounts } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { runFrameCallbacks } from "../src/gl/slot";
import NoteInspector from "../src/components/pianoroll/NoteInspector.svelte";
import NoteListPanel from "../src/components/pianoroll/NoteListPanel.svelte";
import { dragReading, prView } from "../src/components/pianoroll/view.svelte";
import { attachKeys } from "../src/keys/attach";
import StatusLine from "../src/components/StatusLine.svelte";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

const BAR = 4 * TPQ;
const at = (bar: number, beat: number, tick: number) => (bar - 1) * BAR + (beat - 1) * TPQ + tick;

// jsdom does not measure sizes. Set the list's visible height to 200 (10 rows).
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

/** Makes the fake scene the state and puts the scene's shown take into shown. */
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

describe("note list (left panel of mock A-PianoRoll)", () => {
  it("the heading row and one row per note (position, pitch, length, Vel)", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    expect(text(q(".head"))).toBe("位置 音 長さ Vel");
    const rows = qa(".row");
    expect(rows.slice(0, 5).map(text)).toEqual(["1.1.0000 C3 4.0000 70", "1.1.0000 G3 4.0000 64", "1.1.0000 E4 1.0000 87", "1.2.0000 G4 1.0000 92", "1.3.0000 C5 1.0960 101"]);
  });

  it("rows of selected notes (D4 E4 F4) are selected", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    const viewport = q('[data-test="note-list-rows"]')!;
    viewport.scrollTop = 200;
    viewport.dispatchEvent(new Event("scroll"));
    flushSync();
    const picked = qa(".row.selected").map(text);
    expect(picked).toEqual(["3.1.0000 D4 0.1440 80", "3.1.1440 E4 0.1440 83", "3.2.0960 F4 0.0960 77"]);
  });

  it("pressing a row selects that note (replace). Shift adds (add)", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteListPanel);
    qa(".row")[2]!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    qa(".row")[3]!.dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
    expect(app.sent).toEqual([
      { type: "selectNotes", take: take.id, notes: [2], mode: "replace" },
      { type: "selectNotes", take: take.id, notes: [3], mode: "add" },
    ]);
  });

  it("even with 10,000 notes, only visible rows are made. Scrolling gives the rows there", async () => {
    await scene("pianoRoll");
    const n = 10_000;
    const big: TakeNotes = {
      id: 999,
      length: (n * TPQ) / 4,
      origin: 0,
      starts: Uint32Array.from({ length: n }, (_, i) => (i * TPQ) / 4),
      lengths: new Uint32Array(n).fill(TPQ / 4),
      keys: new Uint8Array(n).fill(60),
      velocities: new Uint8Array(n).fill(64),
      froms: new Uint32Array(n),
      controlTicks: new Uint32Array(0),
      controlKinds: new Uint8Array(0),
      controlNumbers: new Uint8Array(0),
      controlValues: new Int16Array(0),
    };
    const s = app.stateWith(() => {});
    const clip = { ...s.song.tracks[0]!.clips[0]!, id: 1, start: 0, end: (n * TPQ) / 4, takeId: 999 };
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

describe("selected notes (right panel of mock A-PianoRoll)", () => {
  it("position「3.1.0000 〜」, pitch「D4 E4 F4」, length「ばらばら」, velocity「77 – 83」", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    expect(text(q('[data-test="inspector-position"]'))).toBe("3.1.0000 〜");
    expect(text(q('[data-test="inspector-keys"]'))).toBe("D4 E4 F4");
    expect(text(q('[data-test="inspector-length"]'))).toBe("ばらばら");
    expect(text(q('[data-test="inspector-velocity"]'))).toBe("77 – 83");
  });

  it("with only one, no「〜」after the position; length and velocity are values", async () => {
    await scene("pianoRoll", (s) => (s.pianoRoll.selectedNotes = [{ start: 14, end: 15 }]));
    show(NoteInspector);
    expect(text(q('[data-test="inspector-position"]'))).toBe("3.1.0000");
    expect(text(q('[data-test="inspector-length"]'))).toBe("0.1440");
    expect(text(q('[data-test="inspector-velocity"]'))).toBe("80");
  });

  it("quantize: if the take is quantized,「かけてある」. Values are pianoRoll.quantize; both buttons can be used", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    expect(text(q('[data-test="inspector-quantized"]'))).toBe("かけてある");
    const body = text(q('[data-testid="pr-inspector"]'))!;
    expect(body).toContain("グリッド 1/16 三連符");
    expect(body).toContain("強さ 75 %");
    expect(body).toContain("スウィング 0 %");
    expect(body).toContain("そろえる所 はじまり");
    const buttons = qa(".buttons .m-btn");
    expect(buttons.map(text)).toEqual(["かけなおすQ", "もとの位置へ戻す"]);
    expect(text(q('[data-test="quantize-key"]'))).toBe("Q");
    expect(buttons.map((b) => b.getAttribute("aria-disabled"))).toEqual(["false", "false"]);
    expect(buttons[0]!.classList.contains("hi")).toBe(true);
  });

  it("pressing lock sends an intent to toggle on/off (it only shows the Rust state and does not change it itself)", async () => {
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

  it("pressing the scale opens a list: major and minor under the heading「種類」, C to B under「主音」. Choosing sends setMusicalScale", async () => {
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

describe("status line (around the piano roll)", () => {
  const left = () => text(q(".message"));

  it("selected notes:「選択 3 ノート ・ D4 E4 F4 ・ クオンタイズ済み（もとの位置へ戻せます）」(mock A-PianoRoll)", async () => {
    await scene("pianoRoll");
    show(StatusLine);
    expect(left()).toBe("選択 3 ノート ・ D4 E4 F4 ・ クオンタイズ済み（もとの位置へ戻せます）");
  });

  it("not quantized: the end part is not shown. With loop on,「ループ 5–9」in front", async () => {
    await scene("pianoRoll", (s) => {
      s.status.message = { id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: false, notes: { count: 6, keys: [60, 62, 64, 65], quantized: false, names: keyCounts([60, 62, 64, 65, 67, 69]) } };
    });
    show(StatusLine);
    expect(left()).toBe("ループ 5–9 ・ 選択 6 ノート ・ C4 D4 E4 F4 …");
  });

  it("tried to write outside the scale (spec 6)", async () => {
    await scene("pianoRoll", (s) => (s.status.message = { id: "outOfScale", key: 63, root: 0, mode: "major" }));
    show(StatusLine);
    expect(left()).toBe("D# は C メジャーの外の音です（スケール固定 ・ オン）");
  });

  it("while dragging, the reading in place replaces the left text", async () => {
    await scene("pianoRoll");
    show(StatusLine);
    dragReading.text = "+0.0960 ・ +2 半音";
    flushSync();
    expect(left()).toBe("+0.0960 ・ +2 半音");
    dragReading.text = null;
    flushSync();
    expect(left()).toContain("選択 3 ノート");
  });

  it("piano roll open while playing:「1 ピアノ ・ Aメロ ・ テイク 2 / 3 ・ 鳴っている音 C3 E3 G3 C4」(mock F-PRPlay)", async () => {
    await scene("pianoRollPlay", (s) => (s.transport.position = at(4, 1, TPQ / 2)));
    show(StatusLine);
    expect(left()).toBe("1 ピアノ ・ Aメロ ・ テイク 2 / 3");
    runFrameCallbacks();
    flushSync();
    expect(left()).toBe("1 ピアノ ・ Aメロ ・ テイク 2 / 3 ・ 鳴っている音 C3 E3 G3 C4");
  });

  it("piano roll open while recording:「1 ピアノ へ録音 ・ テイク 3 ・ 押している音 C3 E3 G3 C4」(spec 6 (Recording, piano roll))", async () => {
    await scene("pianoRollRec");
    show(StatusLine);
    expect(left()).toBe("1 ピアノ へ録音 ・ テイク 3");
    app.setTelemetry(app.telemetryWith({ keys: [48, 52, 55, 60] }));
    flushSync();
    expect(left()).toBe("1 ピアノ へ録音 ・ テイク 3 ・ 押している音 C3 E3 G3 C4");
  });

  it("held notes, like the input meter, apply key on/off without waiting for the meter. On release「押している音」is not shown", async () => {
    await scene("pianoRollRec");
    show(StatusLine);
    app.emit({ type: "note", on: true, note: 64, velocity: 90, tUs: 0 });
    app.emit({ type: "note", on: true, note: 60, velocity: 90, tUs: 0 });
    flushSync();
    expect(left()).toBe("1 ピアノ へ録音 ・ テイク 3 ・ 押している音 C4 E4");
    app.emit({ type: "note", on: false, note: 60, velocity: 0, tUs: 0 });
    app.emit({ type: "note", on: false, note: 64, velocity: 0, tUs: 0 });
    flushSync();
    expect(left()).toBe("1 ピアノ へ録音 ・ テイク 3");
  });

  it("with the piano roll closed, the text of spec 6 (Recording) as it is", async () => {
    await scene("pianoRollRec", (s) => (s.workspace.layout.lower = []));
    show(StatusLine);
    app.setTelemetry(app.telemetryWith({ keys: [48, 52] }));
    flushSync();
    expect(left()).toBe("1 ピアノ へ録音 ・ 止めると テイク 3 として残ります");
  });

  it("with 5 or more selected,「…」after the first 4 (spec 6 (Notes selected))", async () => {
    await scene("pianoRoll", (s) => {
      s.status.message = { id: "summary", loopBars: null, nothingSelected: false, notes: { count: 7, keys: [48, 52, 55, 60], quantized: false, names: keyCounts([48, 52, 55, 60, 64, 67, 72]) } };
    });
    show(StatusLine);
    expect(left()).toBe("選択 7 ノート ・ C3 E3 G3 C4 …");
  });

  it("with 4, no「…」", async () => {
    await scene("pianoRoll", (s) => {
      s.status.message = { id: "summary", loopBars: null, nothingSelected: false, notes: { count: 4, keys: [48, 52, 55, 60], quantized: false, names: keyCounts([48, 52, 55, 60]) } };
    });
    show(StatusLine);
    expect(left()).toBe("選択 4 ノート ・ C3 E3 G3 C4");
  });
});

describe("status line (piano roll where there is no clip, mock F-PREmpty)", () => {
  const left = () => text(q(".message"));
  const empty = (edit?: (s: AppState) => void) => {
    const s = fakeScene("pianoRollEmpty")!.state;
    edit?.(s);
    app.setState(s);
    shown.setShown({ clip: null, track: null, take: null });
  };

  it("the left while stopped is「1 ピアノ ・ クリップなし」", () => {
    empty();
    show(StatusLine);
    expect(left()).toBe("1 ピアノ ・ クリップなし");
  });

  it("with loop on,「ループ 5–9」in front", () => {
    empty((s) => (s.status.message = { id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: true, notes: null }));
    show(StatusLine);
    expect(left()).toBe("ループ 5–9 ・ 1 ピアノ ・ クリップなし");
  });

  it("other notices stay as they are", () => {
    empty((s) => (s.status.message = { id: "clipsNotPasted" }));
    show(StatusLine);
    expect(left()).toBe("ほかのクリップと重なるので、貼りつけませんでした。");
  });

  it("could not write outside the clip (B-82)", () => {
    empty((s) => (s.status.message = { id: "notesNotWritten" }));
    show(StatusLine);
    expect(left()).toBe("ほかのクリップと重なるので、書けませんでした。");
  });

  it("not shown when the piano roll is closed", () => {
    empty((s) => (s.workspace.layout.lower = []));
    show(StatusLine);
    expect(left()).toBe("選択なし");
  });
});

// ---- typing values (spec 3 (note list), mock R3-Lanes) ----

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

describe("typing values in the note list (mock R3-Lanes)", () => {
  it("pressing a field twice makes it an input box. Enter sends setNotesField to the one note of that row", async () => {
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

  it("a value not readable as a number (「あ」) keeps the broken border. Enter sends rejectValue and does not decide", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    double(4, "velocity");
    flushSync();
    typeIn("あ");
    expect(field()!.classList.contains("broken")).toBe(true);
    expect(field()!.getAttribute("aria-invalid")).toBe("true");
    key(field()!, "Enter");
    flushSync();
    expect(app.sent).toEqual([{ type: "rejectValue", field: "velocity", kept: "101", range: null }]);
    expect(field()!.value).toBe("あ");
  });

  it("a value out of range (200) is not broken; it becomes 127 and is decided, then clampValue (B-68)", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteListPanel);
    double(4, "velocity");
    flushSync();
    typeIn("200");
    expect(field()!.classList.contains("broken")).toBe(false);
    key(field()!, "Enter");
    await settle();
    expect(app.sent).toEqual([
      { type: "setNotesField", take: take.id, notes: [4], field: "velocity", value: 127 },
      { type: "clampValue", field: "velocity", value: "127", range: null },
    ]);
  });

  it("position: outside the clip gives the clip range text; outside only by beats gives the beat range (B-68)", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteListPanel);
    double(0, "position");
    flushSync();
    typeIn("4.2.0000");
    key(field()!, "Enter");
    await settle();
    expect(app.sent).toEqual([
      { type: "setNotesField", take: take.id, notes: [0], field: "start", value: at(4, 1, 0) },
      { type: "clampValue", field: "position", value: "4.1.0000", range: "1.1.0000〜4.1.0000" },
    ]);
    app.sent.length = 0;
    double(0, "position");
    flushSync();
    typeIn("2.7.0000");
    key(field()!, "Enter");
    await settle();
    expect(app.sent).toEqual([
      { type: "setNotesField", take: take.id, notes: [0], field: "start", value: at(2, 4, 0) },
      { type: "clampValue", field: "beat", value: "2.4.0000", range: "1〜4" },
    ]);
  });

  it("Enter on a row goes to the first field (position). Tab goes to the field on the right; at the right end, to the first field of the next row (deciding the field before)", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteListPanel);
    const row = qa(".row")[0]!;
    row.focus();
    key(row, "Enter");
    flushSync();
    expect(field()!.value).toBe("1.1.0000");
    key(field()!, "Tab");
    flushSync();
    expect(field()!.value).toBe("C3");
    key(field()!, "Tab");
    flushSync();
    expect(field()!.value).toBe("4.0000");
    key(field()!, "Tab");
    flushSync();
    typeIn("71");
    key(field()!, "Tab");
    flushSync();
    expect(field()!.value).toBe("1.1.0000");
    expect(field()!.closest(".row")!.getAttribute("data-note")).toBe("1");
    expect(app.sent).toEqual([{ type: "setNotesField", take: take.id, notes: [0], field: "velocity", value: 71 }]);
    key(field()!, "Tab", { shiftKey: true });
    flushSync();
    expect(field()!.closest(".row")!.getAttribute("data-note")).toBe("0");
  });

  it("↑ ↓ by 1 (Shift by 10; pitch by 12 semitones with Shift). Pitch moves by semitones, position by the last shown digit", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    double(2, "key");
    flushSync();
    key(field()!, "ArrowUp");
    flushSync();
    expect(field()!.value).toBe("F4");
    key(field()!, "ArrowDown", { shiftKey: true });
    flushSync();
    expect(field()!.value).toBe("F3");
    key(field()!, "Tab", { shiftKey: true });
    flushSync();
    key(field()!, "ArrowUp", { shiftKey: true });
    flushSync();
    expect(field()!.value).toBe("1.1.0010");
  });

  it("Esc cancels (sends nothing). Pressing outside the box also cancels", async () => {
    await scene("pianoRoll");
    show(NoteListPanel);
    double(0, "length");
    flushSync();
    typeIn("2.0000");
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

  it("keys while typing (spec 4): one-letter keys, Space, Delete, Backspace, ↑ ↓ ← → belong to the box. They do not act on the piano roll", async () => {
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

describe("typing in the selected notes fields", () => {
  it("pressing makes an input box. With 2 or more, it goes to all selected (velocity is the same value for all)", async () => {
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

  it("position puts the earliest position, pitch the lowest pitch, and length「ばらばら」the length of the earliest note", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="inspector-position"]')!.click();
    flushSync();
    expect(field()!.value).toBe("3.1.0000");
    key(field()!, "Tab");
    flushSync();
    expect(field()!.value).toBe("D4");
    key(field()!, "Tab");
    flushSync();
    expect(field()!.value).toBe("0.1440");
    expect(app.sent).toEqual([]);
  });

  it("unusable value: a「ばらばら」field keeps「ばらばら」as the old value", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="inspector-length"]')!.click();
    flushSync();
    typeIn("x");
    key(field()!, "Enter");
    expect(app.sent).toEqual([{ type: "rejectValue", field: "length", kept: "ばらばら", range: "0.0001〜6.0960" }]);
  });

  it("a length out of range (9.0000) becomes the longest, 6.0960, and is decided", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="inspector-length"]')!.click();
    flushSync();
    typeIn("9.0000");
    key(field()!, "Enter");
    await settle();
    expect(app.sent).toEqual([
      { type: "setNotesField", take: take.id, notes: [14, 15, 16], field: "length", value: 6 * TPQ + TPQ / 2 },
      { type: "clampValue", field: "length", value: "6.0960", range: "0.0001〜6.0960" },
    ]);
  });
});

describe("quantize fields", () => {
  it("pressing the grid opens a list (「1 小節」, 1/2, 1/4, 1/8, 1/16, 1/32). Choosing sends setQuantize (all current settings)", async () => {
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

  it("triplets are a separate toggle, as in the toolbar", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    const triplet = q('[data-test="quantize-triplet"]')!;
    expect(triplet.getAttribute("aria-pressed")).toBe("false");
    triplet.click();
    expect(app.sent).toEqual([{ type: "setQuantize", division: 16, triplet: true, strength: 75, swing: 0, align: "start" }]);
  });

  it("what to align: start, start and end", async () => {
    await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="quantize-align"]')!.click();
    flushSync();
    const menu = document.querySelector<HTMLElement>('[data-test="quantize-align-menu"]')!;
    expect(text(menu)).toBe("そろえる所 はじまり はじまりと終わり");
    [...menu.querySelectorAll<HTMLButtonElement>("button")][1]!.click();
    expect(app.sent).toEqual([{ type: "setQuantize", division: 16, triplet: false, strength: 75, swing: 0, align: "startAndEnd" }]);
  });

  it("strength: while dragging only the screen moves. Release sends one setQuantize", async () => {
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

  it("quantize again: quantize on the selected notes (all if none). Back to old position: unquantize", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteInspector);
    q('[data-test="quantize-apply"]')!.click();
    q('[data-test="quantize-restore"]')!.click();
    expect(app.sent).toEqual([
      { type: "quantize", take: take.id, notes: [14, 15, 16] },
      { type: "unquantize", take: take.id, notes: [14, 15, 16] },
    ]);
  });

  it("a take that is not quantized cannot use「もとの位置へ戻す」.「かけてある」is not shown either", async () => {
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

describe("status line: quantized, typed an unusable value", () => {
  const left = () => text(q(".message"));

  it("「クオンタイズ 1/16（75 %）を 3 ノートに。うすい枠が、そろえる前の位置。」and「もとの位置へ戻す」(spec 6)", async () => {
    const { take } = await scene("pianoRoll", (s) => (s.status.message = { id: "quantized", division: 16, triplet: false, strength: 75, count: 3 }));
    show(StatusLine);
    expect(left()).toBe("クオンタイズ 1/16（75 %）を 3 ノートに。うすい枠が、そろえる前の位置。");
    const button = q(".link")!;
    expect(text(button)).toBe("もとの位置へ戻す");
    button.click();
    expect(app.sent).toEqual([{ type: "unquantize", take: take.id, notes: [14, 15, 16] }]);
  });

  it("typed something unreadable in the note position field: the form of spec 6 (Not readable as a position)", async () => {
    await scene("pianoRoll", (s) => (s.status.message = { id: "valueRejected", field: "position", kept: "1.1.000", range: "1.1.000〜4.1.000" }));
    show(StatusLine);
    expect(left()).toBe("位置は「5」「5.2」「5.2.240」の形で打ちます。もとの 1.1.000 のままです。");
  });

  it("typed something unreadable in the note length field: the form of spec 6 (Not readable as a number)", async () => {
    await scene("pianoRoll", (s) => (s.status.message = { id: "valueRejected", field: "length", kept: "0.240", range: "0.030〜4.000" }));
    show(StatusLine);
    expect(left()).toBe("長さは数字で打ちます。もとの 0.240 のままです。");
  });

  it("typed something unreadable in the note velocity field: the spec 6 example as it is", async () => {
    await scene("pianoRoll", (s) => (s.status.message = { id: "valueRejected", field: "velocity", kept: "77", range: null }));
    show(StatusLine);
    expect(left()).toBe("ベロシティは数字で打ちます。もとの 77 のままです。");
  });

  it("typed something unreadable in the pitch field (typed as a note name): the range text form as it is", async () => {
    await scene("pianoRoll", (s) => (s.status.message = { id: "valueRejected", field: "key", kept: "C4", range: "C-1〜G9" }));
    show(StatusLine);
    expect(left()).toBe("音は C-1〜G9 です。もとの C4 のままです。");
  });

  it("typed something unreadable in the play position field (B-61, does not use the range text form)", async () => {
    await scene("pianoRoll", (s) => (s.status.message = { id: "valueRejected", field: "playPosition", kept: "1. 1. 000", range: null }));
    show(StatusLine);
    expect(left()).toBe("位置は「5」「5.2」「5.2.240」の形で打ちます。もとの 1. 1. 000 のままです。");
  });
});
