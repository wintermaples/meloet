// 区切り2の画面: 曲の名前の一覧、曲の名前の下の文、曲のダイアログ、取り消しの帯札と状態行、Mod のキー。
import { flushSync, mount, tick, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, SongDialogView } from "../src/bridge";
import BandRow from "../src/components/BandRow.svelte";
import SongDialog from "../src/components/SongDialog.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import Transport from "../src/components/Transport.svelte";
import { handleKeydown } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { closeMenu, menuVisible } from "../src/lib/menu";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;

beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1920);
});
afterEach(() => {
  if (menuVisible()) closeMenu();
  if (mounted) unmount(mounted);
  mounted = null;
  document.body.innerHTML = "";
  vi.restoreAllMocks();
});

function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}

const q = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector<T>(sel);
const text = (sel: string) => q(sel)?.textContent?.replace(/\s+/g, " ").trim() ?? null;

describe("曲の名前の下の文", () => {
  it.each([
    ["neverSaved", 0, "新しい曲"],
    ["saved", 14, "保存済み ・ 取り消し 14"],
    ["changed", 12, "取り消し 12"],
  ] as const)("%s", (file, undo, expected) => {
    show(
      Transport,
      app.stateWith((s) => {
        s.song.file = file;
        s.song.undoCount = undo;
      }),
    );
    expect(q('[data-test="transport-song"] .m-lbl')!.textContent).toBe(expected);
  });
});

describe("曲の名前の一覧", () => {
  const openMenu = async () => {
    q<HTMLButtonElement>('[data-test="transport-song"]')!.click();
    flushSync();
    await new Promise((r) => setTimeout(r, 0));
    return q('[data-test="song-menu"]')!;
  };
  const rows = (menu: HTMLElement) => [...menu.querySelectorAll<HTMLButtonElement>("button")];

  it("押すとひらく。行は 新しい曲 Ctrl N ・ ひらく… Ctrl O ・ 保存 Ctrl S。押すと意図を送ってとじる", async () => {
    show(Transport, app.stateWith(() => {}));
    const menu = await openMenu();
    expect(menuVisible()).toBe(true);
    expect(menu.querySelector(".head")).toBeNull();
    expect(rows(menu).map((r) => r.textContent!.replace(/\s+/g, " ").trim())).toEqual(["新しい曲 Ctrl N", "ひらく… Ctrl O", "保存 Ctrl S"]);
    expect(document.activeElement).toBe(rows(menu)[0]);
    rows(menu)[2]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "saveSong" }]);
    expect(q('[data-test="song-menu"]')).toBeNull();
  });

  it("↑ ↓ Enter でえらぶ。Esc（closeMenu）でとじる", async () => {
    show(Transport, app.stateWith(() => {}));
    let menu = await openMenu();
    menu.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    document.activeElement!.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    flushSync();
    expect(app.sent).toEqual([{ type: "openSong" }]);
    menu = await openMenu();
    closeMenu();
    flushSync();
    expect(q('[data-test="song-menu"]')).toBeNull();
  });

  it.each(["recording", "countIn"] as const)("%s のあいだは 3 行とも使えない", async (state) => {
    show(Transport, app.stateWith((s) => (s.transport.state = state)));
    const menu = await openMenu();
    expect(rows(menu).every((r) => r.disabled && r.classList.contains("unusable"))).toBe(true);
    rows(menu)[0]!.click();
    expect(app.sent).toEqual([]);
  });
});

describe("曲のダイアログ", () => {
  const dialog = (d: Partial<SongDialogView>) =>
    app.stateWith((s) => (s.workspace.songDialog = { id: 7, kind: "unsaved", title: "はじめての曲", previous: false, ...d }));
  const buttons = () => [...document.querySelectorAll<HTMLButtonElement>('[data-test="song-dialog"] .foot button')];

  it.each([
    [{ kind: "unsaved" }, "「はじめての曲」に、まだ保存していない変更があります。", ["やめる", "保存しない", "保存する"], ["cancel", "dontSave", "save"]],
    [
      { kind: "cannotSave" },
      "ここには保存できませんでした（書きこむ権利がないか、空きが足りません）。べつの場所をえらんでください。曲は開いたままです。",
      ["やめる", "場所をえらぶ"],
      ["cancel", "chooseLocation"],
    ],
    [
      { kind: "broken", title: "はじめての曲.meloet", previous: true },
      "このファイルは読めませんでした。自動で残してある 1 つ前の状態からなら開けます。",
      ["やめる", "1 つ前を開く"],
      ["cancel", "openPrevious"],
    ],
    [{ kind: "broken", title: "はじめての曲.meloet", previous: false }, "このファイルは読めませんでした。", ["やめる"], ["cancel"]],
  ] as [Partial<SongDialogView>, string, string[], string[]][])("%o", (d, body, labels, answers) => {
    show(SongDialog, dialog(d));
    expect(text('[data-test="song-dialog"] .head')).toBe(d.title ?? "はじめての曲");
    expect(text('[data-test="song-dialog"] .body')).toBe(body);
    expect(buttons().map((b) => b.textContent!.trim())).toEqual(labels);
    expect(buttons().at(-1)!.classList.contains("primary")).toBe(true);
    for (const b of buttons()) b.click();
    expect(app.sent).toEqual(answers.map((answer) => ({ type: "answerSongDialog", dialog: 7, answer })));
  });

  it("Enter は右はしの明るいボタン、Esc は back", () => {
    let state = dialog({ kind: "unsaved" });
    const sent: Command[] = [];
    const deps = { state: () => state, send: (c: Command) => void sent.push(c), nav: { platform: "Linux x86_64" } };
    handleKeydown(new KeyboardEvent("keydown", { key: "Enter", cancelable: true }), deps);
    state = dialog({ kind: "broken", previous: false });
    handleKeydown(new KeyboardEvent("keydown", { key: "Enter", cancelable: true }), deps);
    handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), deps);
    expect(sent).toEqual([
      { type: "answerSongDialog", dialog: 7, answer: "save" },
      { type: "answerSongDialog", dialog: 7, answer: "cancel" },
      { type: "back" },
    ]);
  });
});

describe("取り消し", () => {
  it("帯札: いつも「取り消せる手 / 全部の手」（「0 / 0」「14 / 14」「12 / 14」）", () => {
    show(BandRow, app.stateWith((s) => (s.song.undoCount = 0)));
    expect(text("nav.row")).toContain("0 / 0");
    app.setState(app.stateWith((s) => (s.song.undoCount = 14)));
    flushSync();
    expect(text("nav.row")).toContain("14 / 14");
    expect(text("nav.row")).not.toContain("手");
    app.setState(
      app.stateWith((s) => {
        s.song.undoCount = 12;
        s.song.redoCount = 2;
      }),
    );
    flushSync();
    expect(text("nav.row")).toContain("12 / 14");
  });

  it("状態行: 取り消してある手がある", async () => {
    show(StatusLine, app.stateWith((s) => (s.status.message = { id: "undoPending", count: 2 })));
    await tick();
    expect(text("footer.status")).toContain("2 手ぶん取り消してあります。新しく何かすると、その 2 手はやりなおせなくなります。");
  });
});

describe("Mod のキー", () => {
  const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...mods });

  it("Mod N・O・S・Z・Shift Z。文字を打っている最中も効く。Mac は Cmd", () => {
    for (const typing of [false, true]) {
      expect(keyAction(k("n", { ctrl: true }), typing, "other")).toEqual({ kind: "newSong" });
      expect(keyAction(k("o", { ctrl: true }), typing, "other")).toEqual({ kind: "openSong" });
      expect(keyAction(k("s", { ctrl: true }), typing, "other")).toEqual({ kind: "saveSong" });
      expect(keyAction(k("z", { ctrl: true }), typing, "other")).toEqual({ kind: "undo" });
      expect(keyAction(k("Z", { ctrl: true, shift: true }), typing, "other")).toEqual({ kind: "redo" });
    }
    expect(keyAction(k("s", { meta: true }), false, "mac")).toEqual({ kind: "saveSong" });
    expect(keyAction(k("s", { ctrl: true }), false, "mac")).toBeNull();
    expect(keyAction(k("N", { ctrl: true, shift: true }), false, "other")).toBeNull();
  });

  it("送る意図。録音中とカウントイン中は、新しい曲・ひらく・保存を送らない（ブラウザにも渡さない）", () => {
    let state = app.stateWith(() => {});
    const sent: Command[] = [];
    const deps = { state: () => state, send: (c: Command) => void sent.push(c), nav: { platform: "Linux x86_64" } };
    const key = (key: string, more: KeyboardEventInit = {}) => {
      const ev = new KeyboardEvent("keydown", { key, ctrlKey: true, cancelable: true, ...more });
      handleKeydown(ev, deps);
      return ev;
    };
    for (const l of ["n", "o", "s", "z"]) key(l);
    key("Z", { shiftKey: true });
    expect(sent).toEqual([{ type: "newSong" }, { type: "openSong" }, { type: "saveSong" }, { type: "undo" }, { type: "redo" }]);
    sent.length = 0;
    state = app.stateWith((s) => (s.transport.state = "recording"));
    expect(["n", "o", "s"].map((l) => key(l).defaultPrevented)).toEqual([true, true, true]);
    key("z");
    expect(sent).toEqual([{ type: "undo" }]);
  });
});
