// The menu bar (docs/history/design-notes.ja.md B-59) and the context menu (B-60), F10 and arrows, Esc order, new keys, Space.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, Command } from "../src/bridge";
import { fakeState } from "../src/bridge/fake";
import ContextMenu from "../src/components/menu/ContextMenu.svelte";
import { closeContextMenu, contextMenu, openContextMenu } from "../src/components/menu/contextMenu.svelte";
import MenuBar from "../src/components/menu/MenuBar.svelte";
import { closeMenuBar, menuBar } from "../src/components/menu/menuBar.svelte";
import { topMenus, type ListEntry, type ListRow } from "../src/components/menu/menus";
import { textFieldRows } from "../src/components/menu/textField";
import { shownOf } from "../src/components/pianoroll/geom";
import { DEFAULT_ROW_PX, onDragCancel, prView } from "../src/components/pianoroll/view.svelte";
import { handleKeydown, handleKeyup, type KeyDeps } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { closeMenu, menuOpened, menuVisible } from "../src/lib/menu";
import * as app from "./fake-app.svelte";
import { take } from "./pianoroll-fixture";
import { trackEdit } from "../src/components/tracks.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...mods });
const settle = async () => {
  for (let i = 0; i < 3; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

const CLIP: ClipView = { id: 5, name: "Aメロ", start: 0, end: 16 * TPQ, take: 1, takes: 2, takeId: 7, takeOffset: 0, noteCount: 2, audio: null };

function withClip(edit: (s: AppState) => void = () => {}): AppState {
  const s = fakeState();
  s.song.tracks[0]!.clips = [CLIP];
  s.arrange = { selectedClips: [5], selectedClip: 5, pianoRollClip: 5, clipboardClips: 0, tallTrack: null };
  s.song.undoCount = 2;
  s.transport.canPlay = true;
  edit(s);
  return s;
}
const prOpen = (s: AppState) => (s.workspace.layout.lower = [{ band: "pianoRoll", height: 300 }]);

function depsOf(state: AppState, sent: Command[], extra: Partial<KeyDeps> = {}): KeyDeps {
  return { state: () => state, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0, ...extra };
}
const rows = (entries: ListEntry[]) => entries.filter((e): e is ListRow => e.kind === "row");
const row = (entries: ListEntry[], label: string) => rows(entries).find((r) => r.label === label)!;
const usableOf = (entries: ListEntry[]) => Object.fromEntries(rows(entries).map((r) => [r.label, r.usable]));

afterEach(() => {
  prView.focus = "arrange";
  prView.tool = "select";
  prView.rowPx = DEFAULT_ROW_PX;
  closeMenuBar();
  closeContextMenu();
  if (menuVisible()) closeMenu();
});

describe("menu contents (B-59)", () => {
  it("7 menus, row names, and keys on the right of rows (Linux / Windows)", () => {
    const menus = topMenus(withClip(), depsOf(withClip(), []), "other");
    expect(menus.map((m) => m.label)).toEqual(["ファイル", "編集", "表示", "再生", "ノート", "トラック", "ヘルプ"]);
    const keyed = (i: number) => rows(menus[i]!.entries).map((r) => (r.sub ? `${r.label} ▸` : r.key ? `${r.label} ${r.key}` : r.label));
    expect(keyed(0)).toEqual([
      "新しい曲 Ctrl N",
      "ひらく… Ctrl O",
      "最近ひらいた曲 ▸",
      "保存 Ctrl S",
      "名前をつけて保存… Ctrl Shift S",
      "音のファイルを入れる…",
      "書き出し… Ctrl Shift E",
      "音の設定… Ctrl ,",
      "meloet を終わる Ctrl Q",
    ]);
    expect(keyed(1)).toEqual([
      "取り消す Ctrl Z",
      "やりなおす Ctrl Shift Z",
      "切り取り Ctrl X",
      "コピー Ctrl C",
      "貼りつけ Ctrl V",
      "複製 Ctrl D",
      "消す Delete",
      "ぜんぶ選ぶ Ctrl A",
      "クリップを切る S",
      "クリップをつなげる Ctrl J",
    ]);
    expect(keyed(2)).toEqual([
      "アレンジへ A",
      "ピアノロール E",
      "ミキサー M",
      "取り消しの履歴 H",
      "棚 P",
      "パラメータ U",
      "横に拡大 Ctrl +",
      "横に縮小 Ctrl −",
      "縦に拡大",
      "縦に縮小",
      "拡大率 ▸",
      "テーマ ▸",
    ]);
    expect(keyed(3)).toEqual(["再生 Space", "停止 Space", "録音 R", "先頭へ Home", "ループ L", "メトロノーム C", "カウントイン ▸"]);
    expect(keyed(4)).toEqual(["道具 ▸", "クオンタイズ Q", "もとの位置へ戻す", "グリッド ▸", "三連符", "スケール固定 ▸"]);
    // The spec 3 order. Delete take (B-82) is after the previous take.
    expect(keyed(5)).toEqual([
      "ミュート Shift M",
      "ソロ Shift S",
      "録音待機",
      "次のテイク T",
      "前のテイク Shift T",
      "テイクを消す",
      "鍵盤のトラックを足す…",
      "歌のトラックを足す",
      "名前を変える F2",
      "色 ▸",
      "上へ動かす",
      "下へ動かす",
      "トラックを消す",
    ]);
    expect(keyed(6)).toEqual(["meloet について"]);
  });

  it("Mac uses Cmd and Option", () => {
    const menus = topMenus(withClip(), depsOf(withClip(), []), "mac");
    expect(row(menus[0]!.entries, "名前をつけて保存…").key).toBe("Cmd Shift S");
    expect(rows(row(menus[2]!.entries, "拡大率").sub!).map((r) => r.key)).toEqual(["Cmd Option 1", "Cmd Option 2", "Cmd Option 3"]);
  });

  it("rows of the view bands mark open bands, the arrange has no mark. File, edit and view have 2 dividers each (mock R5-Menus)", () => {
    const s = withClip();
    const menus = topMenus(s, depsOf(s, []), "other");
    const view = menus[2]!.entries;
    expect(row(view, "アレンジへ").checked).toBeUndefined();
    expect(row(view, "ピアノロール").checked).toBe(s.workspace.layout.lower.some((l) => l.band === "pianoRoll"));
    expect(row(view, "ミキサー").checked).toBe(false);
    for (const i of [0, 1, 2]) expect(menus[i]!.entries.filter((e) => e.kind === "separator")).toHaveLength(2);
  });

  it("「書き出し…」 is openExport, same as Mod Shift E. Adding a keyboard track opens the shelf with 「新しいトラック」. Adding a vocal track adds it at once (23-A)", () => {
    const sent: Command[] = [];
    const menus = topMenus(withClip(), depsOf(withClip(), sent), "other");
    const exportRow = row(menus[0]!.entries, "書き出し…");
    expect([exportRow.usable, exportRow.key]).toEqual([true, "Ctrl Shift E"]);
    exportRow.run!();
    row(menus[5]!.entries, "鍵盤のトラックを足す…").run!();
    row(menus[5]!.entries, "歌のトラックを足す").run!();
    expect(sent).toEqual([{ type: "openExport" }, { type: "openShelf", mode: { kind: "newTrack" } }, { type: "addVocalTrack" }]);
  });

  it("with 1 track, move up / down are disabled. Rename opens the input box, same as F2", () => {
    const menus = topMenus(withClip(), depsOf(withClip(), []), "other");
    const track = menus[5]!.entries;
    expect([row(track, "上へ動かす").usable, row(track, "下へ動かす").usable, row(track, "トラックを消す").usable]).toEqual([false, false, true]);
    row(track, "名前を変える").run!();
    expect(trackEdit.renaming).toBe(1);
    trackEdit.renaming = null;
  });

  it("while recording and in count-in, rows that change the song, and the same rows as the song name list, are disabled", () => {
    for (const st of ["recording", "countIn"] as const) {
      const s = withClip((s) => {
        s.transport.state = st;
        s.song.redoCount = 1;
        s.arrange.clipboardClips = 1;
      });
      const menus = topMenus(s, depsOf(s, []), "other");
      expect(usableOf(menus[0]!.entries), st).toMatchObject({
        新しい曲: false,
        "ひらく…": false,
        保存: false,
        "名前をつけて保存…": false,
        "音の設定…": true,
        "meloet を終わる": true,
      });
      expect(usableOf(menus[1]!.entries), st).toMatchObject({
        取り消す: false,
        やりなおす: false,
        切り取り: false,
        コピー: true,
        貼りつけ: false,
        複製: false,
        消す: false,
        ぜんぶ選ぶ: true,
      });
      expect(usableOf(menus[3]!.entries), st).toMatchObject({ 再生: false, 停止: true, ループ: false, メトロノーム: true });
      expect(usableOf(menus[5]!.entries), st).toMatchObject({ ミュート: true, ソロ: true, 録音待機: false, 次のテイク: false, テイクを消す: false });
    }
  });

  it("delete take: DeleteTake of the chosen clip. Disabled with 1 take or no chosen clip", () => {
    const sent: Command[] = [];
    const s = withClip();
    const del = row(topMenus(s, depsOf(s, sent), "other")[5]!.entries, "テイクを消す");
    expect(del.usable).toBe(true);
    expect(del.key).toBeUndefined();
    del.run!();
    expect(sent).toEqual([{ type: "deleteTake", clip: 5 }]);
    const one = withClip((s) => (s.song.tracks[0]!.clips = [{ ...CLIP, takes: 1 }]));
    expect(row(topMenus(one, depsOf(one, []), "other")[5]!.entries, "テイクを消す").usable).toBe(false);
    const none = withClip((s) => (s.arrange = { selectedClips: [], selectedClip: null, pianoRollClip: null, clipboardClips: 0, tallTrack: null }));
    expect(row(topMenus(none, depsOf(none, []), "other")[5]!.entries, "テイクを消す").usable).toBe(false);
  });

  it("usable when stopped. With no step to undo, 「取り消す」 is disabled", () => {
    const s = withClip((s) => (s.arrange.clipboardClips = 1));
    const menus = topMenus(s, depsOf(s, []), "other");
    // Joining clips needs 2 or more keyboard clips chosen on the same track.
    expect(Object.values(usableOf(menus[1]!.entries))).toEqual([true, false, true, true, true, true, true, true, true, false]);
    expect(usableOf(menus[3]!.entries)).toMatchObject({ 再生: true, 停止: false, 録音: true });
  });

  it("the note menu and vertical zoom only when the piano roll is open", () => {
    const closed = withClip();
    let menus = topMenus(closed, depsOf(closed, []), "other");
    expect(Object.values(usableOf(menus[4]!.entries)).every((u) => !u)).toBe(true);
    expect(usableOf(menus[2]!.entries)).toMatchObject({ 縦に拡大: false, 縦に縮小: false });

    const open = withClip((s) => {
      prOpen(s);
      s.pianoRoll.take = 7;
    });
    const shown = () => shownOf(take([[0, TPQ, 60]], 7, { origin: 3 }), 0);
    menus = topMenus(open, depsOf(open, [], { pianoRoll: shown }), "other");
    expect(Object.values(usableOf(menus[4]!.entries)).every((u) => u)).toBe(true);
    expect(usableOf(menus[2]!.entries)).toMatchObject({ 縦に拡大: true, 縦に縮小: true });
  });

  it("on / off rows and choice rows show the current state with the choice mark", () => {
    const s = withClip((s) => {
      prOpen(s);
      s.song.loopOn = true;
      s.transport.countInBars = 2;
      s.workspace.scale = 125;
      s.workspace.unfitScales = [150];
      s.song.tracks[0]!.muted = true;
      s.pianoRoll.grid = { division: 8, triplet: true };
      s.pianoRoll.scale = { root: 9, mode: "minor", lock: true, highlight: true };
    });
    prView.tool = "draw";
    const menus = topMenus(s, depsOf(s, []), "other");
    const checks = (entries: ListEntry[]) => rows(entries).map((r) => `${r.label}${r.checked ? "*" : ""}${r.usable ? "" : "!"}`);
    expect(checks(row(menus[2]!.entries, "拡大率").sub!)).toEqual(["100 %", "125 %*", "150 %!"]);
    expect(rows(row(menus[2]!.entries, "拡大率").sub!).map((r) => r.hint ?? null)).toEqual([null, null, "この画面には入りません"]);
    expect(checks(menus[3]!.entries)).toContain("ループ*");
    expect(checks(menus[3]!.entries)).toContain("メトロノーム");
    expect(checks(row(menus[3]!.entries, "カウントイン").sub!)).toEqual(["なし", "1 小節", "2 小節*"]);
    expect(checks(row(menus[4]!.entries, "道具").sub!)).toEqual(["選ぶ", "書く*", "消す"]);
    expect(checks(row(menus[4]!.entries, "グリッド").sub!)).toEqual(["1 小節", "1/2", "1/4", "1/8*", "1/16", "1/32"]);
    expect(row(menus[4]!.entries, "三連符").checked).toBe(true);
    const lock = row(menus[4]!.entries, "スケール固定");
    expect(lock.checked).toBe(true);
    expect(lock.sub!.map((e) => (e.kind === "heading" ? `[${e.label}]` : e.kind === "row" ? `${e.label}${e.checked ? "*" : ""}` : "-"))).toEqual([
      "オン*",
      "オフ",
      "[種類]",
      "メジャー",
      "マイナー*",
      "ハーモニックマイナー",
      "メロディックマイナー",
      "メジャーペンタトニック",
      "マイナーペンタトニック",
      "ブルース",
      "ドリアン",
      "ミクソリディアン",
      "[主音]",
      ...["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A*", "A#", "B"],
    ]);
    expect(row(menus[5]!.entries, "ミュート").checked).toBe(true);
    expect(row(menus[5]!.entries, "ソロ").checked).toBe(false);
    expect(row(menus[5]!.entries, "録音待機").checked).toBe(true);
  });

  it("最近ひらいた曲 ▸: the songs newest first with their place, then 一覧を消す; only a disabled まだありません when empty (A5)", () => {
    const sent: Command[] = [];
    const s = withClip((s) => {
      s.recent = [
        { id: 4, name: "はじめての曲", place: "~/ミュージック/はじめての曲/" },
        { id: 2, name: "夜のピアノ", place: "~/ミュージック/夜のピアノ/" },
      ];
    });
    const sub = () => row(topMenus(s, depsOf(s, sent), "other")[0]!.entries, "最近ひらいた曲").sub!;
    expect(sub().map((e) => (e.kind === "row" ? `${e.label}|${e.detail ?? ""}${e.usable ? "" : "!"}` : "-"))).toEqual([
      "はじめての曲|~/ミュージック/はじめての曲/",
      "夜のピアノ|~/ミュージック/夜のピアノ/",
      "-",
      "一覧を消す|",
    ]);
    row(sub(), "夜のピアノ").run!();
    row(sub(), "一覧を消す").run!();
    expect(sent).toEqual([{ type: "openRecent", id: 2 }, { type: "clearRecent" }]);

    // While recording, the songs cannot be opened (as 「ひらく…」); the list can still be cleared.
    s.transport.state = "recording";
    expect(rows(sub()).map((r) => r.usable)).toEqual([false, false, true]);

    s.recent = [];
    const empty = topMenus(s, depsOf(s, sent), "other")[0]!.entries;
    expect(row(empty, "最近ひらいた曲").usable).toBe(true);
    expect(row(empty, "最近ひらいた曲").sub!).toEqual([{ kind: "row", label: "まだありません", usable: false }]);
  });

  it("pressing a row takes the same path as the key (edit goes where keys go)", () => {
    const sent: Command[] = [];
    const s = withClip((s) => {
      prOpen(s);
      s.pianoRoll.take = 7;
      s.pianoRoll.selectedNotes = [{ start: 0, end: 1 }];
    });
    const shown = () => shownOf(take([[0, TPQ, 60]], 7), 0);
    const menus = () => topMenus(s, depsOf(s, sent, { pianoRoll: shown }), "other");
    row(menus()[1]!.entries, "コピー").run!();
    prView.focus = "pianoRoll";
    row(menus()[1]!.entries, "コピー").run!();
    row(menus()[0]!.entries, "名前をつけて保存…").run!();
    row(menus()[5]!.entries, "ミュート").run!();
    row(menus()[5]!.entries, "録音待機").run!();
    row(row(menus()[4]!.entries, "グリッド").sub!, "1/4").run!();
    row(row(menus()[2]!.entries, "拡大率").sub!, "150 %").run!();
    expect(sent).toEqual([
      { type: "copyClips", clips: [5] },
      { type: "copyNotes", take: 7, notes: [0] },
      { type: "saveSongAs" },
      { type: "toggleMute", track: 1 },
      { type: "selectTrack", track: 1, fromArm: true },
      { type: "setGrid", division: 4, triplet: false },
      { type: "setScale", percent: 150 },
    ]);
    row(menus()[2]!.entries, "縦に拡大").run!();
    expect(prView.rowPx).toBe(16);
    row(row(menus()[4]!.entries, "道具").sub!, "消す").run!();
    expect(prView.tool).toBe("erase");
  });

  it("quantize in the note menu goes to the piano roll even if the arrange was pressed last", () => {
    const sent: Command[] = [];
    const s = withClip((s) => {
      prOpen(s);
      s.pianoRoll.take = 7;
    });
    const shown = () => shownOf(take([[0, TPQ, 60]], 7), 0);
    row(topMenus(s, depsOf(s, sent, { pianoRoll: shown }), "other")[4]!.entries, "クオンタイズ").run!();
    expect(sent).toEqual([{ type: "quantize", take: 7, notes: [] }]);
  });
});

describe("new keys (chapter 4 and B-59, 70, 71)", () => {
  it("Mod Shift S, Mod Q, F10, Shift M / Shift S (no effect while typing; the owner's decision)", () => {
    expect(keyAction(k("S", { ctrl: true, shift: true }), false, "other")).toEqual({ kind: "saveSongAs" });
    expect(keyAction(k("q", { ctrl: true }), true, "other")).toEqual({ kind: "quit" });
    expect(keyAction(k("q", { meta: true }), false, "mac")).toEqual({ kind: "quit" });
    expect(keyAction(k("F10"), false, "other")).toEqual({ kind: "menuBar" });
    expect(keyAction(k("M", { shift: true }), false, "other")).toEqual({ kind: "trackMute" });
    expect(keyAction(k("S", { shift: true, code: "KeyS" }), false, "other")).toEqual({ kind: "trackSolo" });
    expect(keyAction(k("M", { shift: true }), true, "other")).toBeNull();
    expect(keyAction(k("S", { shift: true, code: "KeyS" }), true, "other")).toBeNull();
    expect(keyAction(k("m", { shift: true, code: "KeyM" }), false, "other", "outputChooser")).toEqual({ kind: "trackMute" });
    expect(keyAction(k("q"), false, "other")).toEqual({ kind: "quantize" });
  });

  const press = (deps: KeyDeps, key: string, init: KeyboardEventInit = {}) => {
    const ev = new KeyboardEvent("keydown", { key, cancelable: true, ...init });
    handleKeydown(ev, deps);
    return ev;
  };

  it("sent intents. Save as is not sent while recording (mute is sent; A-14)", () => {
    const sent: Command[] = [];
    let s = withClip();
    const deps = depsOf(s, sent, { state: () => s });
    expect(press(deps, "S", { ctrlKey: true, shiftKey: true }).defaultPrevented).toBe(true);
    press(deps, "q", { ctrlKey: true });
    press(deps, "M", { shiftKey: true });
    press(deps, "S", { shiftKey: true });
    expect(sent).toEqual([{ type: "saveSongAs" }, { type: "quit" }, { type: "toggleMute", track: 1 }, { type: "toggleSolo", track: 1 }]);
    sent.length = 0;
    s = withClip((s) => (s.transport.state = "recording"));
    expect(press(deps, "S", { ctrlKey: true, shiftKey: true }).defaultPrevented).toBe(true);
    press(deps, "M", { shiftKey: true });
    press(deps, "S", { shiftKey: true });
    expect(sent).toEqual([{ type: "toggleMute", track: 1 }, { type: "toggleSolo", track: 1 }]);
  });

  it("when keys go to the arrange: Mod A, C, X, V, D go to clips", () => {
    const sent: Command[] = [];
    const s = withClip((s) => (s.arrange.clipboardClips = 2));
    const deps = depsOf(s, sent);
    for (const l of ["a", "c", "x", "v", "d"]) expect(press(deps, l, { ctrlKey: true }).defaultPrevented).toBe(true);
    expect(sent).toEqual([
      { type: "selectAllClips" },
      { type: "copyClips", clips: [5] },
      { type: "cutClips", clips: [5] },
      { type: "pasteClips", to: null },
      { type: "duplicateClips", clips: [5] },
    ]);
  });

  it("not sent when there is nothing to paste or no clip chosen", () => {
    const sent: Command[] = [];
    const s = withClip((s) => (s.arrange = { selectedClips: [], selectedClip: null, pianoRollClip: null, clipboardClips: 0, tallTrack: null }));
    const deps = depsOf(s, sent);
    for (const l of ["c", "x", "v", "d"]) press(deps, l, { ctrlKey: true });
    expect(sent).toEqual([]);
  });

  it("Space: does not press the focused button (stops both keydown and keyup). Also when disabled", () => {
    document.body.innerHTML = `<button>x</button><input type="text">`;
    const button = document.querySelector("button")!;
    button.focus();
    const sent: Command[] = [];
    const deps = depsOf(fakeState(), sent);
    expect(press(deps, " ", { code: "Space" }).defaultPrevented).toBe(true);
    const up = new KeyboardEvent("keyup", { key: " ", code: "Space", cancelable: true });
    handleKeyup(up);
    expect(up.defaultPrevented).toBe(true);
    expect(sent).toEqual([]);

    // Inside a text box, it goes in as a space.
    document.querySelector("input")!.focus();
    expect(press(deps, " ", { code: "Space" }).defaultPrevented).toBe(false);
    const up2 = new KeyboardEvent("keyup", { key: " ", code: "Space", cancelable: true });
    handleKeyup(up2);
    expect(up2.defaultPrevented).toBe(false);
    document.body.innerHTML = "";
  });
});

describe("Esc order (spec 3)", () => {
  const esc = (deps: KeyDeps) => handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), deps);

  it("drag → menu bar → context menu → other lists → Rust back", () => {
    const sent: Command[] = [];
    const deps = depsOf(fakeState(), sent);
    let dragging = true;
    const off = onDragCancel(() => (dragging ? ((dragging = false), true) : false));
    menuBar.open = 2;
    openContextMenu(10, 10, [{ kind: "row", label: "menu.copy", usable: true, run: () => {} }]);
    let other = true;
    const forget = menuOpened(() => (other = false));

    esc(deps);
    expect(dragging).toBe(false);
    expect(menuBar.open).toBe(2);
    esc(deps);
    expect(menuBar.open).toBeNull();
    expect(contextMenu.open).not.toBeNull();
    esc(deps);
    expect(contextMenu.open).toBeNull();
    expect(other).toBe(true);
    esc(deps);
    expect(other).toBe(false);
    expect(sent).toEqual([]);
    esc(deps);
    expect(sent).toEqual([{ type: "back" }]);
    off();
    forget();
  });
});

describe("menu bar (mount)", () => {
  let mounted: Record<string, unknown> | null = null;
  beforeEach(() => {
    app.reset();
    app.setState(withClip());
    mounted = mount(MenuBar, { target: document.body });
    flushSync();
  });
  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = null;
    document.body.innerHTML = "";
  });

  const names = () => [...document.querySelectorAll<HTMLButtonElement>('[data-test="menubar"] button')];
  const list = () => document.querySelector<HTMLElement>('[data-test="menubar-list"]');
  const listRows = () => [...list()!.querySelectorAll<HTMLButtonElement>(":scope > .item > button")];
  const keydown = (key: string) => {
    const ev = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true });
    (document.activeElement ?? document.body).dispatchEvent(ev);
    flushSync();
    return ev;
  };

  it("pressing a name opens just below; while open, hovering another name switches. Pressing again closes", async () => {
    expect(names().map((b) => b.textContent)).toEqual(["ファイル", "編集", "表示", "再生", "ノート", "トラック", "ヘルプ"]);
    names()[0]!.click();
    await settle();
    expect(list()).not.toBeNull();
    expect(names()[0]!.classList.contains("open")).toBe(true);
    expect(listRows()[0]!.textContent).toContain("新しい曲");
    names()[2]!.dispatchEvent(new PointerEvent("pointerenter"));
    await settle();
    expect(listRows()[0]!.textContent).toContain("アレンジへ");
    names()[2]!.click();
    await settle();
    expect(list()).toBeNull();
  });

  it("pressing a row closes first, then acts", async () => {
    names()[0]!.click();
    await settle();
    listRows()[4]!.click();
    await settle();
    expect(app.sent).toEqual([{ type: "saveSongAs" }]);
    expect(list()).toBeNull();
  });

  it("a song of 最近ひらいた曲 ▸ shows its place in thin text; pressing it closes, then opens it (A5)", async () => {
    app.setState({ ...withClip(), recent: [{ id: 7, name: "夜のピアノ", place: "~/ミュージック/夜のピアノ/" }] });
    flushSync();
    names()[0]!.click();
    await settle();
    listRows()[2]!.dispatchEvent(new PointerEvent("pointerenter"));
    await settle();
    const sub = document.querySelector('[data-test="menubar-list-sub"]')!;
    const song = sub.querySelector<HTMLButtonElement>(".item > button")!;
    expect(song.querySelector(".label")!.textContent).toBe("夜のピアノ");
    const place = song.querySelector('[data-test="menu-detail"]')!;
    expect(place.textContent).toBe("~/ミュージック/夜のピアノ/");
    expect(place.classList.contains("m-lbl")).toBe(true);
    song.click();
    await settle();
    expect(app.sent).toEqual([{ type: "openRecent", id: 7 }]);
    expect(list()).toBeNull();
  });

  it("pressing outside closes", async () => {
    names()[1]!.click();
    await settle();
    document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    await settle();
    expect(list()).toBeNull();
  });

  it("F10 opens the first menu. ← → neighbors, ↑ ↓ rows, → the child list, ← back, Enter acts", async () => {
    const deps: KeyDeps = { state: () => app.current.state, send: app.send, nav: { platform: "Linux x86_64" } };
    const off = (() => {
      const on = (ev: KeyboardEvent) => handleKeydown(ev, deps);
      document.addEventListener("keydown", on);
      return () => document.removeEventListener("keydown", on);
    })();
    keydown("F10");
    await settle();
    expect(menuBar.open).toBe(0);
    expect(document.activeElement?.textContent).toContain("新しい曲");
    keydown("ArrowDown");
    expect(document.activeElement?.textContent).toContain("ひらく…");
    keydown("ArrowLeft");
    await settle();
    // The rightmost is 「ヘルプ」. Focus on its first usable row 「meloet について」.
    expect(menuBar.open).toBe(6);
    expect(document.activeElement?.textContent).toContain("meloet について");
    keydown("ArrowRight");
    await settle();
    expect(menuBar.open).toBe(0);
    keydown("ArrowRight");
    await settle();
    keydown("ArrowRight");
    await settle();
    expect(menuBar.open).toBe(2);
    // View: vertical zoom in / out is disabled as the piano roll is closed. To the row 「拡大率 ▸」 (before 「テーマ ▸」).
    for (let i = 0; i < 8; i++) keydown("ArrowDown");
    expect(document.activeElement?.textContent).toContain("拡大率");
    keydown("ArrowRight");
    await settle();
    expect(document.activeElement?.textContent).toContain("100 %");
    expect(document.querySelector('[data-test="menubar-list-sub"]')).not.toBeNull();
    keydown("ArrowLeft");
    await settle();
    expect(document.querySelector('[data-test="menubar-list-sub"]')).toBeNull();
    expect(document.activeElement?.textContent).toContain("拡大率");
    keydown("ArrowRight");
    await settle();
    keydown("ArrowDown");
    keydown("Enter");
    await settle();
    expect(app.sent).toEqual([{ type: "setScale", percent: 125 }]);
    expect(menuBar.open).toBeNull();

    keydown("F10");
    await settle();
    expect(menuBar.open).toBe(0);
    keydown("Escape");
    await settle();
    expect(menuBar.open).toBeNull();
    off();
  });

  it("hovering ▸ with the mouse opens the child list on the right, hovering another row closes it", async () => {
    names()[3]!.click();
    await settle();
    const countIn = listRows().find((b) => b.textContent?.includes("カウントイン"))!;
    countIn.dispatchEvent(new PointerEvent("pointerenter"));
    await settle();
    const sub = document.querySelector('[data-test="menubar-list-sub"]')!;
    expect(sub.textContent).toContain("なし");
    expect(sub.parentElement!.classList.contains("sub")).toBe(true);
    listRows()[0]!.dispatchEvent(new PointerEvent("pointerenter"));
    await settle();
    expect(document.querySelector('[data-test="menubar-list-sub"]')).toBeNull();
  });
});

describe("context menu (mount)", () => {
  let mounted: Record<string, unknown> | null = null;
  beforeEach(() => {
    app.reset();
    app.setState(fakeState());
    mounted = mount(ContextMenu, { target: document.body });
    flushSync();
  });
  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = null;
    document.body.innerHTML = "";
    vi.restoreAllMocks();
  });
  const menu = () => document.querySelector<HTMLElement>('[data-test="context-menu"]');

  it("opens at the pressed place; pressing a row closes first, then acts. Disabled rows cannot be pressed", async () => {
    const ran: string[] = [];
    openContextMenu(30, 40, [
      { kind: "row", label: "menu.cut", key: "Ctrl X", usable: false, run: () => ran.push("cut") },
      { kind: "separator" },
      { kind: "row", label: "menu.copy", key: "Ctrl C", usable: true, run: () => ran.push("copy") },
    ]);
    await settle();
    const popup = menu()!.parentElement!;
    expect(popup.style.left).toBe("30px");
    expect(popup.style.top).toBe("40px");
    const buttons = [...menu()!.querySelectorAll("button")];
    expect(buttons.map((b) => b.textContent?.replace(/\s+/g, " ").trim())).toEqual(["切り取り Ctrl X", "コピー Ctrl C"]);
    expect(buttons[0]!.disabled).toBe(true);
    expect(menu()!.querySelector('[role="separator"]')).not.toBeNull();
    buttons[1]!.click();
    await settle();
    expect(ran).toEqual(["copy"]);
    expect(menu()).toBeNull();
  });

  it("↑ ↓ and Enter", async () => {
    const ran: string[] = [];
    openContextMenu(0, 0, [
      { kind: "row", label: "menu.cut", usable: true, run: () => ran.push("cut") },
      { kind: "row", label: "menu.copy", usable: true, run: () => ran.push("copy") },
    ]);
    await settle();
    const key = (k: string) => (document.activeElement ?? document.body).dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));
    key("ArrowDown");
    key("ArrowDown");
    key("Enter");
    await settle();
    expect(ran).toEqual(["copy"]);
  });

  it("pressing outside closes", async () => {
    openContextMenu(0, 0, [{ kind: "row", label: "menu.copy", usable: true, run: () => {} }]);
    await settle();
    document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    await settle();
    expect(menu()).toBeNull();
  });

  it("moves inside when it would go out of the window", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      const left = parseFloat(this.parentElement?.style.left ?? "0") || 0;
      const top = parseFloat(this.parentElement?.style.top ?? "0") || 0;
      return { left, top, right: left + 220, bottom: top + 48, width: 220, height: 48, x: left, y: top, toJSON: () => ({}) } as DOMRect;
    });
    openContextMenu(window.innerWidth - 100, window.innerHeight - 10, [
      { kind: "row", label: "menu.cut", usable: true, run: () => {} },
      { kind: "row", label: "menu.copy", usable: true, run: () => {} },
    ]);
    await settle();
    expect(menu()!.style.transform).toBe("translate(-120px, -38px)");
  });

  it("the WebKit menu is never shown. Text boxes get cut, copy, paste, select all", async () => {
    const div = document.createElement("div");
    document.body.append(div);
    const ev = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 5, clientY: 6 });
    div.dispatchEvent(ev);
    expect(ev.defaultPrevented).toBe(true);
    await settle();
    expect(menu()).toBeNull();

    const input = document.createElement("input");
    input.value = "abc";
    document.body.append(input);
    input.focus();
    input.setSelectionRange(0, 2);
    input.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 5, clientY: 6 }));
    await settle();
    const buttons = [...menu()!.querySelectorAll("button")];
    expect(buttons.map((b) => [b.textContent?.replace(/\s+/g, " ").trim(), b.disabled])).toEqual([
      ["切り取り Ctrl X", false],
      ["コピー Ctrl C", false],
      ["貼りつけ Ctrl V", true],
      ["ぜんぶ選ぶ Ctrl A", false],
    ]);
    buttons[3]!.click();
    await settle();
    expect(document.activeElement).toBe(input);
    expect([input.selectionStart, input.selectionEnd]).toEqual([0, 3]);
  });
});

describe("rows of the text box menu", () => {
  it("with nothing selected, cut and copy are disabled. Read-only boxes cannot cut", () => {
    const input = document.createElement("input");
    input.value = "abc";
    document.body.append(input);
    input.setSelectionRange(1, 1);
    const mod = (x: string) => `Ctrl ${x}`;
    expect(textFieldRows(input, mod).map((r) => r.kind === "row" && r.usable)).toEqual([false, false, false, true]);
    input.readOnly = true;
    input.setSelectionRange(0, 3);
    expect(textFieldRows(input, mod).map((r) => r.kind === "row" && r.usable)).toEqual([false, true, false, true]);
    input.remove();
  });
});
