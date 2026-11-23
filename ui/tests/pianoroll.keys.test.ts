// Piano roll keys (spec 4): 1, 2, 3, ↑ ↓, Shift ↑ ↓, ← →, Delete, Mod D, Mod A, Mod X / C / V, and cancelDrag first on Esc.
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeState } from "../src/bridge/fake";
import * as G from "../src/components/pianoroll/geom";
import { onDragCancel, prView } from "../src/components/pianoroll/view.svelte";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { copied, noteCopied } from "../src/state/copied";
import { take } from "./pianoroll-fixture";

const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...mods });

describe("keyAction (piano roll keys)", () => {
  it("1, 2, 3 are tools. No effect while typing", () => {
    expect(keyAction(k("1", { code: "Digit1" }), false, "other")).toEqual({ kind: "tool", tool: "select" });
    expect(keyAction(k("2"), false, "other")).toEqual({ kind: "tool", tool: "draw" });
    expect(keyAction(k("3"), false, "other")).toEqual({ kind: "tool", tool: "erase" });
    expect(keyAction(k("4"), false, "other")).toBeNull();
    expect(keyAction(k("1"), true, "other")).toBeNull();
  });

  it("↑ ↓ is a semitone, Shift ↑ ↓ an octave, ← → one grid step", () => {
    expect(keyAction(k("ArrowUp"), false, "other")).toEqual({ kind: "nudge", steps: 0, keys: 1 });
    expect(keyAction(k("ArrowDown", { shift: true }), false, "other")).toEqual({ kind: "nudge", steps: 0, keys: -12 });
    expect(keyAction(k("ArrowLeft"), false, "other")).toEqual({ kind: "nudge", steps: -1, keys: 0 });
    expect(keyAction(k("ArrowRight", { shift: true }), false, "other")).toBeNull();
    // Arrows inside a text box belong to the box (↑ ↓ in the note list go by 1)
    expect(keyAction(k("ArrowUp"), true, "other")).toBeNull();
  });

  it("Mod A, D, X, C, V. A, X, C, V while typing belong to the box", () => {
    expect(keyAction(k("a", { ctrl: true }), false, "other")).toEqual({ kind: "selectAll" });
    expect(keyAction(k("d", { meta: true }), false, "mac")).toEqual({ kind: "duplicate" });
    expect(keyAction(k("x", { ctrl: true }), false, "other")).toEqual({ kind: "cut" });
    expect(keyAction(k("c", { ctrl: true }), false, "other")).toEqual({ kind: "copy" });
    expect(keyAction(k("v", { ctrl: true }), false, "other")).toEqual({ kind: "paste" });
    expect(keyAction(k("c", { ctrl: true }), true, "other")).toBeNull();
    expect(keyAction(k("d", { ctrl: true }), true, "other")).toEqual({ kind: "duplicate" });
  });
});

describe("handleKeydown (keys that go to the piano roll)", () => {
  const n = G.shownOf(take([[0, TPQ, 60], [TPQ, TPQ, 62], [2 * TPQ, TPQ / 2, 64]], 7), 4 * TPQ);
  let state: AppState;
  let sent: Command[];
  let deps: KeyDeps;
  const press = (key: string, init: KeyboardEventInit = {}) => {
    const ev = new KeyboardEvent("keydown", { key, cancelable: true, ...init });
    handleKeydown(ev, deps);
    return ev;
  };

  beforeEach(() => {
    state = fakeState();
    state.workspace.layout = { ...state.workspace.layout, lower: [{ band: "pianoRoll", height: 640 }] };
    state.pianoRoll = { ...state.pianoRoll, take: 7, selectedNotes: [{ start: 0, end: 2 }], grid: { division: 16, triplet: false }, clipboardNotes: 2 };
    state.transport = { ...state.transport, position: 2 * TPQ + TPQ / 12 };
    sent = [];
    deps = { state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" }, pianoRoll: () => n };
    prView.focus = "pianoRoll";
    prView.tool = "select";
  });
  afterEach(() => {
    prView.focus = "arrange";
    prView.tool = "select";
  });

  it("1, 2, 3 change the tool (no intent sent)", () => {
    expect(press("2").defaultPrevented).toBe(true);
    expect(prView.tool).toBe("draw");
    press("3");
    expect(prView.tool).toBe("erase");
    expect(sent).toEqual([]);
  });

  it("↑ ↓ and ← → move the selected notes (one grid step = TPQ / 4 for 1/16)", () => {
    press("ArrowUp", { shiftKey: true });
    press("ArrowRight");
    expect(sent).toEqual([
      { type: "moveNotes", take: 7, notes: [0, 1], ticks: 0, keys: 12, copy: false },
      { type: "moveNotes", take: 7, notes: [0, 1], ticks: TPQ / 4, keys: 0, copy: false },
    ]);
  });

  it("Delete: selected notes in the piano roll, clips in the arrange", () => {
    press("Delete");
    prView.focus = "arrange";
    press("Delete");
    expect(sent).toEqual([{ type: "deleteNotes", take: 7, notes: [0, 1] }, { type: "deleteSelected" }]);
  });

  it("with no selected notes in the piano roll, Delete deletes nothing (not the clip)", () => {
    state.pianoRoll = { ...state.pianoRoll, selectedNotes: [] };
    press("Delete");
    expect(sent).toEqual([]);
  });

  it("Mod D copies right after (the range length rounded up to the grid), Mod A, C, X, V", () => {
    press("d", { ctrlKey: true });
    press("a", { ctrlKey: true });
    press("c", { ctrlKey: true });
    press("x", { ctrlKey: true });
    press("v", { ctrlKey: true });
    expect(sent).toEqual([
      { type: "moveNotes", take: 7, notes: [0, 1], ticks: 2 * TPQ, keys: 0, copy: true },
      { type: "selectAllNotes" },
      { type: "copyNotes", take: 7, notes: [0, 1] },
      { type: "cutNotes", take: 7, notes: [0, 1] },
      // Play position 2 * TPQ + TPQ / 12 snapped to the grid (TPQ / 4) gives 2 * TPQ
      { type: "pasteNotes", at: 2 * TPQ },
    ]);
  });

  it("with the piano roll closed, note keys do nothing", () => {
    state.workspace.layout = { ...state.workspace.layout, lower: [] };
    press("ArrowUp");
    press("d", { ctrlKey: true });
    press("2");
    expect(sent).toEqual([]);
    expect(prView.tool).toBe("select");
  });

  it("Esc during a drag only cancels the drag (no Back sent)", () => {
    let dragging = true;
    const stop = onDragCancel(() => {
      if (!dragging) return false;
      dragging = false;
      return true;
    });
    press("Escape");
    expect(sent).toEqual([]);
    press("Escape");
    expect(sent).toEqual([{ type: "back" }]);
    stop();
  });
});

describe("Mod V pastes what was copied last (notes or clips)", () => {
  // The shown clip is [4 * TPQ, 12 * TPQ) in the song.
  const n = G.shownOf(take([[0, TPQ, 60]], 7), 4 * TPQ, { from: 4 * TPQ, to: 12 * TPQ });
  let state: AppState;
  let sent: Command[];
  let deps: KeyDeps;
  let playTick: number;
  const press = (key: string, init: KeyboardEventInit = {}) => handleKeydown(new KeyboardEvent("keydown", { key, cancelable: true, ...init }), deps);

  beforeEach(() => {
    state = fakeState();
    state.workspace.layout = { ...state.workspace.layout, lower: [{ band: "pianoRoll", height: 640 }] };
    state.pianoRoll = { ...state.pianoRoll, take: 7, selectedNotes: [], grid: { division: 16, triplet: false }, clipboardNotes: 2 };
    state.arrange = { ...state.arrange, clipboardClips: 1 };
    playTick = 6 * TPQ + TPQ / 12;
    sent = [];
    deps = { state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" }, pianoRoll: () => n, playTick: () => playTick };
    prView.focus = "arrange";
  });
  afterEach(() => {
    copied.last = null;
    prView.focus = "arrange";
  });

  it("copy and cut of notes or clips are remembered when they are sent", () => {
    noteCopied({ type: "copyNotes", take: 7, notes: [0] });
    expect(copied.last).toBe("notes");
    noteCopied({ type: "cutClips", clips: [3] });
    expect(copied.last).toBe("clips");
    noteCopied({ type: "cutNotes", take: 7, notes: [0] });
    expect(copied.last).toBe("notes");
    noteCopied({ type: "copyClips", clips: [3] });
    expect(copied.last).toBe("clips");
    noteCopied({ type: "undo" });
    expect(copied.last).toBe("clips");
  });

  it("notes last: into the shown clip, even when the keys point at the arrange", () => {
    copied.last = "notes";
    press("v", { ctrlKey: true });
    // Inside the clip: the play line on the grid (TPQ / 4).
    expect(sent).toEqual([{ type: "pasteNotes", at: 6 * TPQ }]);
  });

  it("notes last, the play line outside the clip: at the clip's head", () => {
    copied.last = "notes";
    playTick = 20 * TPQ;
    press("v", { ctrlKey: true });
    playTick = TPQ;
    prView.focus = "pianoRoll";
    press("v", { ctrlKey: true });
    // The clip's end is outside too.
    playTick = 12 * TPQ;
    press("v", { ctrlKey: true });
    expect(sent).toEqual([
      { type: "pasteNotes", at: 4 * TPQ },
      { type: "pasteNotes", at: 4 * TPQ },
      { type: "pasteNotes", at: 4 * TPQ },
    ]);
  });

  it("clips last: clips, even when the keys point at the piano roll", () => {
    copied.last = "clips";
    prView.focus = "pianoRoll";
    press("v", { ctrlKey: true });
    expect(sent).toEqual([{ type: "pasteClips", to: null }]);
  });

  it("notes last with the piano roll closed, or nothing copied yet: as the keys point", () => {
    copied.last = "notes";
    state.workspace.layout = { ...state.workspace.layout, lower: [] };
    press("v", { ctrlKey: true });
    copied.last = null;
    state.workspace.layout = { ...state.workspace.layout, lower: [{ band: "pianoRoll", height: 640 }] };
    press("v", { ctrlKey: true });
    prView.focus = "pianoRoll";
    press("v", { ctrlKey: true });
    expect(sent).toEqual([{ type: "pasteClips", to: null }, { type: "pasteClips", to: null }, { type: "pasteNotes", at: 6 * TPQ }]);
  });

  it("notes last, the piano roll showing no clip: nothing", () => {
    copied.last = "notes";
    state.arrange = { ...state.arrange, pianoRollClip: null };
    deps = { ...deps, pianoRoll: () => null };
    press("v", { ctrlKey: true });
    expect(sent).toEqual([]);
  });

  it("the shown take not fetched yet: the clip's range comes from the song", () => {
    copied.last = "notes";
    const clip = { id: 90, name: "A", start: 8 * TPQ, end: 16 * TPQ, take: 1, takes: 1, takeId: 91, takeOffset: 0, noteCount: 0, audio: null };
    state.song = { ...state.song, tracks: state.song.tracks.map((t, i) => (i === 0 ? { ...t, clips: [clip] } : t)) };
    state.arrange = { ...state.arrange, pianoRollClip: clip.id };
    deps = { ...deps, pianoRoll: () => null };
    playTick = clip.end + 5 * TPQ;
    press("v", { ctrlKey: true });
    expect(sent).toEqual([{ type: "pasteNotes", at: clip.start }]);
  });
});
