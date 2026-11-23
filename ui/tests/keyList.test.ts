// The key list dialog and the link to the user guide (round 12 G1).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene, fakeState } from "../src/bridge/fake";
import { about, closeAbout, openAbout } from "../src/components/about/about.svelte";
import KeyListDialog from "../src/components/keylist/KeyListDialog.svelte";
import { closeKeyList, keyList, keyListBack, openKeyList } from "../src/components/keylist/keyList.svelte";
import { keyNames, topMenus, type ListRow } from "../src/components/menu/menus";
import StatusLine from "../src/components/StatusLine.svelte";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import { ACTION_ROWS, KEY_GROUPS, keyListGroups, OTHER_ROWS } from "../src/keys/keyList";
import { t } from "../src/text";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let host: HTMLElement | null = null;
let comp: ReturnType<typeof mount> | null = null;
beforeEach(() => app.reset());
afterEach(() => {
  if (comp) unmount(comp);
  comp = null;
  host?.remove();
  host = null;
  closeKeyList();
  closeAbout();
  vi.restoreAllMocks();
});

function show(component: Component<any>, props: Record<string, unknown> = {}): void {
  host = document.createElement("div");
  document.body.append(host);
  comp = mount(component, { target: host, props });
  flushSync();
}
const q = (sel: string) => host!.querySelector<HTMLElement>(sel);
const all = (sel: string) => [...host!.querySelectorAll<HTMLElement>(sel)];
const words = (el: Element | null) => (el?.textContent ?? "").replace(/\s+/g, " ").trim();
const click = (el: HTMLElement | null) => {
  el!.click();
  flushSync();
};
const type = (el: HTMLElement | null, text: string) => {
  const input = el as HTMLInputElement;
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  flushSync();
};
const linux = keyNames("other");
const mac = keyNames("mac");
const find = (label: string, names = linux) =>
  keyListGroups(names)
    .flatMap((g) => g.rows.map((r) => ({ group: g.group, ...r })))
    .filter((r) => r.label === label);

describe("the table (keyList.ts)", () => {
  it("the groups of the spec, in its order, all with rows", () => {
    expect(keyListGroups(linux).map((g) => g.label)).toEqual(["曲とファイル", "再生と録音", "帯", "アレンジ", "ピアノロール", "ミキサー", "表示"]);
    expect(KEY_GROUPS).toHaveLength(7);
  });

  it("every row has words and keys; the count covers the spec 4 table", () => {
    const rows = keyListGroups(linux).flatMap((g) => g.rows);
    expect(rows.length).toBe(Object.values(ACTION_ROWS).flat().length + OTHER_ROWS.length);
    expect(rows.length).toBeGreaterThanOrEqual(60);
    for (const r of rows) {
      expect(r.label, JSON.stringify(r)).not.toBe("");
      expect(r.keys, r.label).not.toBe("");
    }
  });

  it("keys are written as the menus write them (Ctrl on Linux, Cmd and Option on Mac)", () => {
    expect(find("保存")[0]!.keys).toBe("Ctrl S");
    expect(find("保存", mac)[0]!.keys).toBe("Cmd S");
    expect(find("やりなおす")[0]!.keys).toBe("Ctrl Shift Z");
    expect(find("拡大率")[0]!.keys).toBe("Ctrl Alt 1 ／ 2");
    expect(find("拡大率", mac)[0]!.keys).toBe("Cmd Option 1 ／ 2");
    expect(find("次のマーカーへ ／ 前のマーカーへ", mac)[0]!.keys).toBe("Option → ／ Option ←");
    expect(find("再生 ／ 止める")[0]).toMatchObject({ group: "play", keys: "Space" });
    expect(find("クオンタイズをかけなおす")[0]).toMatchObject({ group: "pianoRoll", keys: "Q" });
    expect(find("次のテイク ／ 前のテイク")[0]!.keys).toBe("T ／ Shift T");
  });

  it("cut, copy, paste and the like are in both the arrange and the piano roll", () => {
    expect(find("コピー").map((r) => r.group)).toEqual(["arrange", "pianoRoll"]);
  });

  it("the keys of round 12 wave 2 are listed (Shift R, Mod K, Mod R)", () => {
    expect(find("さっきのを残す")[0]).toMatchObject({ group: "play", keys: "Shift R" });
    expect(find("キーで弾く")[0]).toMatchObject({ group: "play", keys: "Ctrl K" });
    expect(find("クリップをくり返す")[0]).toMatchObject({ group: "arrange", keys: "Ctrl R" });
  });

  it("the filter looks at the words, the keys and the group name; case does not matter", () => {
    expect(keyListGroups(linux, "ループ").map((g) => [g.label, g.rows.map((r) => r.label)])).toEqual([["再生と録音", ["ループ"]]]);
    const mixer = keyListGroups(linux, "ミキサー");
    expect(mixer.find((g) => g.group === "mixer")!.rows.length).toBe(ACTION_ROWS.bypass.length + OTHER_ROWS.filter((r) => r.group === "mixer").length);
    expect(keyListGroups(linux, "ctrl shift s").flatMap((g) => g.rows.map((r) => r.label))).toEqual(["名前をつけて保存…"]);
    expect(keyListGroups(linux, "  ")).toHaveLength(7);
    expect(keyListGroups(linux, "そんなキーはない")).toEqual([]);
  });
});

describe("the dialog", () => {
  it("heading, filter box, every group open, the guide link", () => {
    openKeyList();
    show(KeyListDialog, { platform: "other" });
    expect(words(q(".head"))).toBe("キーの一覧 閉じるEsc");
    expect((q("[data-test=keylist-search]") as HTMLInputElement).placeholder).toBe("さがす…");
    expect(all("[data-test=keylist-group]").map(words)).toEqual(KEY_GROUPS.map((g) => `▾ ${t(`keyList.group.${g}`)}`));
    expect(all("[data-test=keylist-row]").length).toBe(keyListGroups(linux).flatMap((g) => g.rows).length);
    expect(words(all("[data-test=keylist-row]")[0]!)).toBe("新しい曲 Ctrl N");
    expect(words(q("[data-test=keylist-guide]"))).toBe("使い方のページをひらく");
  });

  it("typing in the box filters; groups with a match open even when folded", () => {
    openKeyList();
    show(KeyListDialog, { platform: "other" });
    click(q("[data-group=play]"));
    expect(words(q("[data-group=play]"))).toBe("▸ 再生と録音");
    expect(all("[data-test=keylist-row]").map(words)).not.toContain("ループ L");
    type(q("[data-test=keylist-search]"), "ループ");
    expect(all("[data-test=keylist-group]").map(words)).toEqual(["▾ 再生と録音"]);
    expect(all("[data-test=keylist-row]").map(words)).toEqual(["ループ L"]);
    type(q("[data-test=keylist-search]"), "");
    expect(words(q("[data-group=play]"))).toBe("▸ 再生と録音");
  });

  it("opening again starts with nothing typed and nothing folded", () => {
    openKeyList();
    keyList.query = "x";
    keyList.folded = ["play"];
    closeKeyList();
    openKeyList();
    expect([keyList.query, keyList.folded]).toEqual(["", []]);
  });

  it("the link sends openUserGuide (Rust opens the page; the address is not sent from the screen)", () => {
    openKeyList();
    show(KeyListDialog, { platform: "other" });
    click(q("[data-test=keylist-guide]"));
    expect(app.sent).toEqual([{ type: "openUserGuide" }]);
    expect(keyList.open).toBe(true);
  });

  it("closes with the close buttons", () => {
    openKeyList();
    show(KeyListDialog, { platform: "other" });
    click(q("[data-test=keylist-close]"));
    expect(keyList.open).toBe(false);
    openKeyList();
    flushSync();
    click(q("[data-test=keylist-close-head]"));
    expect(keyList.open).toBe(false);
  });

  it("the status line says the dialog is open", () => {
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1920);
    app.setState(fakeScene("scanFinished")!.state);
    openKeyList();
    show(StatusLine);
    expect(words(q(".status .message"))).toBe("キーの一覧 をひらいています");
  });
});

describe("Esc (same turn as the about dialog)", () => {
  const esc = (deps: KeyDeps) => handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), deps);

  it("closes; when closed, goes to Rust's back", () => {
    const s = fakeState();
    const sent: Command[] = [];
    const deps: KeyDeps = { state: () => s, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
    openKeyList();
    esc(deps);
    expect(keyList.open).toBe(false);
    expect(sent).toEqual([]);
    esc(deps);
    expect(sent).toEqual([{ type: "back" }]);
  });

  it("in the filter box, the first Esc only leaves the box", () => {
    const s = fakeState();
    const sent: Command[] = [];
    const deps: KeyDeps = { state: () => s, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
    openKeyList();
    show(KeyListDialog, { platform: "other" });
    q("[data-test=keylist-search]")!.focus();
    esc(deps);
    expect(keyList.open).toBe(true);
    expect(document.activeElement).not.toBe(q("[data-test=keylist-search]"));
    esc(deps);
    expect(keyList.open).toBe(false);
    expect(sent).toEqual([]);
  });

  it("keyListBack does nothing when not open", () => {
    expect(keyListBack()).toBe(false);
  });
});

describe("Help menu", () => {
  const deps = (s: AppState, sent: Command[]): KeyDeps => ({ state: () => s, send: (c) => sent.push(c), playTick: () => 0 });
  const help = (s: AppState, sent: Command[] = []) => topMenus(s, deps(s, sent), "other")[6]!.entries;
  const rowOf = (s: AppState, label: string, sent: Command[] = []) =>
    help(s, sent).find((e) => e.kind === "row" && e.label === label) as ListRow;

  it("「キーの一覧」 opens the dialog; disabled while another dialog is open", () => {
    const s = fakeState();
    expect(rowOf(s, "キーの一覧").usable).toBe(true);
    rowOf(s, "キーの一覧").run!();
    expect(keyList.open).toBe(true);
    // One screen dialog at a time, so the Esc order stays single.
    expect(rowOf(s, "meloet について").usable).toBe(false);
    closeKeyList();
    openAbout();
    expect(about.open).toBe(true);
    expect(rowOf(s, "キーの一覧").usable).toBe(false);
    closeAbout();
    s.workspace.settings = { draftOutput: null, advancedOpen: false } as unknown as AppState["workspace"]["settings"];
    expect(rowOf(s, "キーの一覧").usable).toBe(false);
  });

  it("「使い方をひらく」 sends openUserGuide", () => {
    const s = fakeState();
    const sent: Command[] = [];
    const row = rowOf(s, "使い方をひらく", sent);
    expect(row.usable).toBe(true);
    row.run!();
    expect(sent).toEqual([{ type: "openUserGuide" }]);
  });
});
