// The dialog in front takes the keys (keys/front.ts): the parts behind it (output chooser, shelf) wait, and plain Enter and
// Esc go to that dialog. A key a part already took (defaultPrevented) is not used again.
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene, fakeState } from "../src/bridge/fake";
import { about, closeAbout, openAbout } from "../src/components/about/about.svelte";
import { closeKeyList, keyList, openKeyList } from "../src/components/keylist/keyList.svelte";
import { topMenus, type ListEntry, type ListRow } from "../src/components/menu/menus";
import { shelfUi } from "../src/components/tracks.svelte";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";

const scene = (name: string) => fakeScene(name)!.state;
const row = (entries: ListEntry[], label: string) => entries.find((e): e is ListRow => e.kind === "row" && e.label === label)!;

describe("keys while a dialog is in front", () => {
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
    sent = [];
    deps = { state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" } };
  });
  afterEach(() => {
    shelfUi.keys = false;
    closeAbout();
    closeKeyList();
  });

  it("UI-SHELL-001: with the output chooser behind the sound settings, ↓ and Enter belong to the settings", () => {
    state.workspace = { ...state.workspace, chooser: { highlighted: null }, settings: { draftOutput: null, advancedOpen: false } };
    press("ArrowDown");
    press("Enter");
    expect(sent).toEqual([{ type: "settingsApply" }]);
  });

  it("UI-SHELL-001: with the output chooser behind a song dialog, Enter answers the dialog", () => {
    state.workspace = { ...state.workspace, chooser: { highlighted: null }, songDialog: { id: 1, kind: "unsaved", title: "はじめての曲", previous: false } };
    press("Enter");
    expect(sent.map((c) => c.type)).toEqual(["answerSongDialog"]);
  });

  it("UI-SHELL-002: the shelf behind the export dialog or 「書き出しました」 does not take Enter or ↓", () => {
    const s = scene("exportDialog");
    state = { ...s, workspace: { ...s.workspace, layout: { ...s.workspace.layout, rightFront: "shelf", rightOpen: ["shelf"] } } };
    shelfUi.keys = true;
    press("ArrowDown");
    press("Enter");
    expect(sent).toEqual([{ type: "startExport" }]);
    const done = scene("exportDone");
    state = { ...done, workspace: { ...done.workspace, layout: { ...done.workspace.layout, rightFront: "shelf", rightOpen: ["shelf"] } } };
    sent.length = 0;
    press("Enter");
    expect(sent).toEqual([{ type: "closeExport" }]);
  });

  it("UI-SHELL-003: Esc in the latency dialog stops measuring, or closes it; Enter does not apply the settings under it", () => {
    state = scene("latency");
    press("Enter");
    expect(sent).toEqual([]);
    press("Escape");
    expect(sent).toEqual([{ type: "stopLatencyMeasure" }]);
    state.workspace.latencyDialog = { measure: { state: "done", measuredMs: 12.4 } };
    sent.length = 0;
    press("Escape");
    expect(sent).toEqual([{ type: "closeLatencyMeasure" }]);
  });

  it("UI-SHELL-005: the key list and About cannot be opened over the export dialog or 「書き出しました」", () => {
    for (const name of ["exportDialog", "exportDone"]) {
      const s = scene(name);
      const help = topMenus(s, { ...deps, state: () => s }, "other")[6]!.entries;
      expect([row(help, "キーの一覧").usable, row(help, "meloet について").usable], name).toEqual([false, false]);
    }
  });

  it("UI-SHELL-005: Enter in the key list or About over the export dialog does not export", () => {
    state = scene("exportDialog");
    openKeyList();
    press("Enter");
    closeKeyList();
    openAbout();
    press("Enter");
    expect(sent).toEqual([]);
    expect([keyList.open, about.open]).toEqual([false, true]);
  });

  it("UI-SHELL #2 (later): on 「くわしい設定」 Enter is the bright button 「閉じる」, Esc goes back one", () => {
    state.workspace = { ...state.workspace, settings: { draftOutput: null, advancedOpen: true } };
    press("Enter");
    press("Escape");
    expect(sent).toEqual([{ type: "settingsCancel" }, { type: "back" }]);
  });

  it("FIX-fix-ui-keys #1: behind a dialog the keys for the parts behind it do nothing; play, save and undo still work", () => {
    const behind: [string, KeyboardEventInit][] = [
      ["l", { code: "KeyL" }],
      ["c", { code: "KeyC" }],
      ["k", { code: "KeyK" }],
      ["Delete", {}],
      ["M", { code: "KeyM", shiftKey: true }],
      ["a", { code: "KeyA", ctrlKey: true }],
    ];
    const still: [string, KeyboardEventInit, Command["type"]][] = [
      [" ", { code: "Space" }, "playStop"],
      ["Home", {}, "goToStart"],
      ["s", { code: "KeyS", ctrlKey: true }, "saveSong"],
      ["z", { code: "KeyZ", ctrlKey: true }, "undo"],
    ];
    state = scene("tracks6");
    for (const [key, init] of behind) {
      sent.length = 0;
      press(key, init);
      expect(sent.length, `${key} with no dialog`).toBe(1);
    }
    const dialogs: [string, (s: AppState) => void][] = [
      ["settings", (s) => (s.workspace.settings = { draftOutput: null, advancedOpen: false })],
      ["song", (s) => (s.workspace.songDialog = { id: 1, kind: "unsaved", title: "はじめての曲", previous: false })],
      ["latency", (s) => (s.workspace.latencyDialog = { measure: { state: "done", measuredMs: 12.4 } })],
      ["about", () => openAbout()],
    ];
    for (const [name, open] of dialogs) {
      state = scene("tracks6");
      open(state);
      sent.length = 0;
      for (const [key, init] of behind) {
        const ev = press(key, init);
        expect(ev.defaultPrevented, `${name}: ${key}`).toBe(init.ctrlKey === true);
      }
      expect(sent, name).toEqual([]);
      for (const [key, init, type] of still) {
        sent.length = 0;
        press(key, init);
        expect(sent.map((c) => c.type), `${name}: ${key}`).toEqual([type]);
      }
      closeAbout();
    }
  });

  it("UI-SHELL-004: a key a part already took is not used again", () => {
    state = scene("exportDialog");
    const ev = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    ev.preventDefault();
    handleKeydown(ev, deps);
    expect(sent).toEqual([]);
  });
});
