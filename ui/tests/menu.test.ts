// 上のメニューの行（design-notes B-59）と右クリックの一覧（B-60）、F10 と矢印、Esc の順、新しいキー、Space。
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

const CLIP: ClipView = { id: 5, name: "Aメロ", start: 0, end: 16 * TPQ, take: 1, takes: 2, takeId: 7, takeOffset: 0, noteCount: 2, vocal: false };

function withClip(edit: (s: AppState) => void = () => {}): AppState {
  const s = fakeState();
  s.song.tracks[0]!.clips = [CLIP];
  s.arrange = { selectedClips: [5], selectedClip: 5, pianoRollClip: 5, clipboardClips: 0 };
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

describe("メニューの中身（B-59）", () => {
  it("7 つのメニューと行の名前、行の右のキー（Linux ・ Windows）", () => {
    const menus = topMenus(withClip(), depsOf(withClip(), []), "other");
    expect(menus.map((m) => m.label)).toEqual(["ファイル", "編集", "表示", "再生", "ノート", "トラック", "ヘルプ"]);
    const keyed = (i: number) => rows(menus[i]!.entries).map((r) => (r.sub ? `${r.label} ▸` : r.key ? `${r.label} ${r.key}` : r.label));
    expect(keyed(0)).toEqual([
      "新しい曲 Ctrl N",
      "ひらく… Ctrl O",
      "保存 Ctrl S",
      "名前をつけて保存… Ctrl Shift S",
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
    ]);
    expect(keyed(3)).toEqual(["再生 Space", "停止 Space", "録音 R", "先頭へ Home", "ループ L", "メトロノーム C", "カウントイン ▸"]);
    expect(keyed(4)).toEqual(["道具 ▸", "クオンタイズ Q", "もとの位置へ戻す", "グリッド ▸", "三連符", "スケール固定 ▸"]);
    // 仕様書 3 章の順。テイクを消す（B-82）は前のテイクのうしろ。
    expect(keyed(5)).toEqual([
      "ミュート Shift M",
      "ソロ Shift S",
      "録音待機",
      "次のテイク T",
      "前のテイク Shift T",
      "テイクを消す",
      "トラックを足す",
      "名前を変える F2",
      "色 ▸",
      "上へ動かす",
      "下へ動かす",
      "トラックを消す",
    ]);
    expect(keyed(6)).toEqual(["meloet について"]);
  });

  it("Mac は Cmd と Option", () => {
    const menus = topMenus(withClip(), depsOf(withClip(), []), "mac");
    expect(row(menus[0]!.entries, "名前をつけて保存…").key).toBe("Cmd Shift S");
    expect(rows(row(menus[2]!.entries, "拡大率").sub!).map((r) => r.key)).toEqual(["Cmd Option 1", "Cmd Option 2", "Cmd Option 3"]);
  });

  it("表示の帯の行は、ひらいている帯に印がつき、アレンジへは印なし。ファイル ・ 編集 ・ 表示に区切りが 2 本ずつ（モック R5-Menus）", () => {
    const s = withClip();
    const menus = topMenus(s, depsOf(s, []), "other");
    const view = menus[2]!.entries;
    expect(row(view, "アレンジへ").checked).toBeUndefined();
    expect(row(view, "ピアノロール").checked).toBe(s.workspace.layout.lower.some((l) => l.band === "pianoRoll"));
    expect(row(view, "ミキサー").checked).toBe(false);
    for (const i of [0, 1, 2]) expect(menus[i]!.entries.filter((e) => e.kind === "separator")).toHaveLength(2);
  });

  it("まだない物（書き出し）は使えない。トラックを足すは棚を「新しいトラック」でひらく", () => {
    const sent: Command[] = [];
    const menus = topMenus(withClip(), depsOf(withClip(), sent), "other");
    expect(row(menus[0]!.entries, "書き出し…").usable).toBe(false);
    row(menus[5]!.entries, "トラックを足す").run!();
    expect(sent).toEqual([{ type: "openShelf", mode: { kind: "newTrack" } }]);
  });

  it("トラック 1 本では、上へ ・ 下へ動かすは使えない。名前を変えるは F2 と同じく入力の枠をひらく", () => {
    const menus = topMenus(withClip(), depsOf(withClip(), []), "other");
    const track = menus[5]!.entries;
    expect([row(track, "上へ動かす").usable, row(track, "下へ動かす").usable, row(track, "トラックを消す").usable]).toEqual([false, false, true]);
    row(track, "名前を変える").run!();
    expect(trackEdit.renaming).toBe(1);
    trackEdit.renaming = null;
  });

  it("録音中とカウントイン中は、曲を変える行と曲の名前の一覧と同じ行が使えない", () => {
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

  it("テイクを消す: えらんだクリップの DeleteTake。テイクが 1 つ ・ クリップをえらんでいないときは使えない", () => {
    const sent: Command[] = [];
    const s = withClip();
    const del = row(topMenus(s, depsOf(s, sent), "other")[5]!.entries, "テイクを消す");
    expect(del.usable).toBe(true);
    expect(del.key).toBeUndefined();
    del.run!();
    expect(sent).toEqual([{ type: "deleteTake", clip: 5 }]);
    const one = withClip((s) => (s.song.tracks[0]!.clips = [{ ...CLIP, takes: 1 }]));
    expect(row(topMenus(one, depsOf(one, []), "other")[5]!.entries, "テイクを消す").usable).toBe(false);
    const none = withClip((s) => (s.arrange = { selectedClips: [], selectedClip: null, pianoRollClip: null, clipboardClips: 0 }));
    expect(row(topMenus(none, depsOf(none, []), "other")[5]!.entries, "テイクを消す").usable).toBe(false);
  });

  it("止まっているときは使える。取り消す手がなければ「取り消す」は使えない", () => {
    const s = withClip((s) => (s.arrange.clipboardClips = 1));
    const menus = topMenus(s, depsOf(s, []), "other");
    expect(Object.values(usableOf(menus[1]!.entries))).toEqual([true, false, true, true, true, true, true, true]);
    expect(usableOf(menus[3]!.entries)).toMatchObject({ 再生: true, 停止: false, 録音: true });
  });

  it("ノートのメニューと縦の拡大は、ピアノロールがひらいているときだけ", () => {
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

  it("オン ・ オフの行とえらぶ行は、選ぶ印でいまの状態を見せる", () => {
    const s = withClip((s) => {
      prOpen(s);
      s.song.loopOn = true;
      s.transport.countInBars = 2;
      s.workspace.scale = 125;
      s.workspace.unfitScales = [150];
      s.song.tracks[0]!.muted = true;
      s.pianoRoll.grid = { division: 8, triplet: true };
      s.pianoRoll.scale = { root: 9, mode: "minor", lock: true };
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
      "[主音]",
      ...["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A*", "A#", "B"],
    ]);
    expect(row(menus[5]!.entries, "ミュート").checked).toBe(true);
    expect(row(menus[5]!.entries, "ソロ").checked).toBe(false);
    expect(row(menus[5]!.entries, "録音待機").checked).toBe(true);
  });

  it("行を押すと、キーと同じ道を通る（編集はキーが向いている方へ）", () => {
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

  it("ノートのメニューのクオンタイズは、最後に押したのがアレンジでもピアノロールへ", () => {
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

describe("新しいキー（4 章と B-59 ・ 70 ・ 71）", () => {
  it("Mod Shift S、Mod Q、F10、Shift M ・ Shift S（文字を打っている最中は効かない。ヤチヨさんの決め 2026-11-18）", () => {
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

  it("送る意図。録音中は名前をつけて保存を送らない（ミュートは送る。A-14）", () => {
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

  it("アレンジにキーが向いているとき: Mod A ・ C ・ X ・ V ・ D はクリップへ", () => {
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

  it("貼りつける物がない ・ 選んだクリップがないときは送らない", () => {
    const sent: Command[] = [];
    const s = withClip((s) => (s.arrange = { selectedClips: [], selectedClip: null, pianoRollClip: null, clipboardClips: 0 }));
    const deps = depsOf(s, sent);
    for (const l of ["c", "x", "v", "d"]) press(deps, l, { ctrlKey: true });
    expect(sent).toEqual([]);
  });

  it("Space: フォーカスのあるボタンを押さない（keydown と keyup の両方を止める）。使えないときも", () => {
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

    // 文字の枠の中では、空白として入る。
    document.querySelector("input")!.focus();
    expect(press(deps, " ", { code: "Space" }).defaultPrevented).toBe(false);
    const up2 = new KeyboardEvent("keyup", { key: " ", code: "Space", cancelable: true });
    handleKeyup(up2);
    expect(up2.defaultPrevented).toBe(false);
    document.body.innerHTML = "";
  });
});

describe("Esc の順（仕様書 3 章）", () => {
  const esc = (deps: KeyDeps) => handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), deps);

  it("ドラッグ → 上のメニュー → 右クリックの一覧 → ほかの一覧 → Rust の back", () => {
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

describe("上のメニューの行（mount）", () => {
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

  it("名前を押すとすぐ下にひらき、ひらいている間はほかの名前に乗ると切りかわる。もう一度押すととじる", async () => {
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

  it("行を押すと、とじてから行う", async () => {
    names()[0]!.click();
    await settle();
    listRows()[3]!.click();
    await settle();
    expect(app.sent).toEqual([{ type: "saveSongAs" }]);
    expect(list()).toBeNull();
  });

  it("外を押すと、とじる", async () => {
    names()[1]!.click();
    await settle();
    document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    await settle();
    expect(list()).toBeNull();
  });

  it("F10 で最初のメニュー。← → でとなり、↑ ↓ で行、→ で子の一覧、← で戻る、Enter で行う", async () => {
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
    // いちばん右は「ヘルプ」。最初の使える行「meloet について」にフォーカス。
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
    // 表示: 縦に拡大 ・ 縮小はピアノロールがとじているので使えない。いちばん下の「拡大率 ▸」へ。
    for (let i = 0; i < 10; i++) keydown("ArrowDown");
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

  it("マウスで ▸ に乗ると子の一覧が右にひらき、ほかの行に乗るととじる", async () => {
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

describe("右クリックの一覧（mount）", () => {
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

  it("押した所にひらき、行を押すと、とじてから行う。使えない行は押せない", async () => {
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

  it("↑ ↓ と Enter", async () => {
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

  it("外を押すと、とじる", async () => {
    openContextMenu(0, 0, [{ kind: "row", label: "menu.copy", usable: true, run: () => {} }]);
    await settle();
    document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    await settle();
    expect(menu()).toBeNull();
  });

  it("窓からはみ出すときは、内がわへずらす", async () => {
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

  it("WebKit の一覧は、どこでも出さない。文字の枠では、切り取り ・ コピー ・ 貼りつけ ・ ぜんぶ選ぶ", async () => {
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

describe("文字の枠の一覧の行", () => {
  it("選んでいなければ切り取り ・ コピーは使えない。読むだけの枠は切り取れない", () => {
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
