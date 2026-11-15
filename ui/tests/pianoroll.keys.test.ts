// ピアノロールのキー（仕様書 4 章）: 1 ・ 2 ・ 3、↑ ↓ ・ Shift ↑ ↓、← →、Delete、Mod D、Mod A、Mod X ・ C ・ V、Esc のいちばん前の cancelDrag。
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeState } from "../src/bridge/fake";
import * as G from "../src/components/pianoroll/geom";
import { onDragCancel, prView } from "../src/components/pianoroll/view.svelte";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { take } from "./pianoroll-fixture";

const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...mods });

describe("keyAction（ピアノロールのキー）", () => {
  it("1 ・ 2 ・ 3 は道具。文字を打っている最中は効かない", () => {
    expect(keyAction(k("1", { code: "Digit1" }), false, "other")).toEqual({ kind: "tool", tool: "select" });
    expect(keyAction(k("2"), false, "other")).toEqual({ kind: "tool", tool: "draw" });
    expect(keyAction(k("3"), false, "other")).toEqual({ kind: "tool", tool: "erase" });
    expect(keyAction(k("4"), false, "other")).toBeNull();
    expect(keyAction(k("1"), true, "other")).toBeNull();
  });

  it("↑ ↓ は半音、Shift ↑ ↓ は 1 オクターブ、← → はグリッド 1 つ", () => {
    expect(keyAction(k("ArrowUp"), false, "other")).toEqual({ kind: "nudge", steps: 0, keys: 1 });
    expect(keyAction(k("ArrowDown", { shift: true }), false, "other")).toEqual({ kind: "nudge", steps: 0, keys: -12 });
    expect(keyAction(k("ArrowLeft"), false, "other")).toEqual({ kind: "nudge", steps: -1, keys: 0 });
    expect(keyAction(k("ArrowRight", { shift: true }), false, "other")).toBeNull();
    // 文字の枠の中の矢印は、枠の物（ノートの一覧の ↑ ↓ で 1 ずつ）
    expect(keyAction(k("ArrowUp"), true, "other")).toBeNull();
  });

  it("Mod A ・ D ・ X ・ C ・ V。打っている最中の A ・ X ・ C ・ V は枠の物", () => {
    expect(keyAction(k("a", { ctrl: true }), false, "other")).toEqual({ kind: "selectAll" });
    expect(keyAction(k("d", { meta: true }), false, "mac")).toEqual({ kind: "duplicate" });
    expect(keyAction(k("x", { ctrl: true }), false, "other")).toEqual({ kind: "cut" });
    expect(keyAction(k("c", { ctrl: true }), false, "other")).toEqual({ kind: "copy" });
    expect(keyAction(k("v", { ctrl: true }), false, "other")).toEqual({ kind: "paste" });
    expect(keyAction(k("c", { ctrl: true }), true, "other")).toBeNull();
    expect(keyAction(k("d", { ctrl: true }), true, "other")).toEqual({ kind: "duplicate" });
  });
});

describe("handleKeydown（ピアノロールに向けるキー）", () => {
  const n = G.shownOf(take([[0, 480, 60], [480, 480, 62], [960, 240, 64]], 7), 1920);
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
    state.transport = { ...state.transport, position: 1000 };
    sent = [];
    deps = { state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" }, pianoRoll: () => n };
    prView.focus = "pianoRoll";
    prView.tool = "select";
  });
  afterEach(() => {
    prView.focus = "arrange";
    prView.tool = "select";
  });

  it("1 ・ 2 ・ 3 は道具を変える（意図は送らない）", () => {
    expect(press("2").defaultPrevented).toBe(true);
    expect(prView.tool).toBe("draw");
    press("3");
    expect(prView.tool).toBe("erase");
    expect(sent).toEqual([]);
  });

  it("↑ ↓ ・ ← → で選んだノートを動かす（グリッド 1 つ ＝ 1/16 の 120）", () => {
    press("ArrowUp", { shiftKey: true });
    press("ArrowRight");
    expect(sent).toEqual([
      { type: "moveNotes", take: 7, notes: [0, 1], ticks: 0, keys: 12, copy: false },
      { type: "moveNotes", take: 7, notes: [0, 1], ticks: 120, keys: 0, copy: false },
    ]);
  });

  it("Delete はピアノロールなら選んだノート、アレンジならクリップ", () => {
    press("Delete");
    prView.focus = "arrange";
    press("Delete");
    expect(sent).toEqual([{ type: "deleteNotes", take: 7, notes: [0, 1] }, { type: "deleteSelected" }]);
  });

  it("ピアノロールで選んだノートがなければ、Delete は何も消さない（クリップを消さない）", () => {
    state.pianoRoll = { ...state.pianoRoll, selectedNotes: [] };
    press("Delete");
    expect(sent).toEqual([]);
  });

  it("Mod D はすぐうしろに複製（範囲の長さをグリッドに切り上げた分）、Mod A ・ C ・ X ・ V", () => {
    press("d", { ctrlKey: true });
    press("a", { ctrlKey: true });
    press("c", { ctrlKey: true });
    press("x", { ctrlKey: true });
    press("v", { ctrlKey: true });
    expect(sent).toEqual([
      { type: "moveNotes", take: 7, notes: [0, 1], ticks: 960, keys: 0, copy: true },
      { type: "selectAllNotes" },
      { type: "copyNotes", take: 7, notes: [0, 1] },
      { type: "cutNotes", take: 7, notes: [0, 1] },
      // 再生の位置 1000 をグリッド（120）に合わせて 960
      { type: "pasteNotes", at: 960 },
    ]);
  });

  it("ピアノロールがひらいていなければ、ノートのキーは何もしない", () => {
    state.workspace.layout = { ...state.workspace.layout, lower: [] };
    press("ArrowUp");
    press("d", { ctrlKey: true });
    press("2");
    expect(sent).toEqual([]);
    expect(prView.tool).toBe("select");
  });

  it("Esc は、ドラッグ中ならドラッグをやめるだけ（Back を送らない）", () => {
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
