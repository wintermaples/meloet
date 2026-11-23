// Mute sets on the screen (B-97-3, B-97-5): 「ミュートのセット ▾」 at the head of the mixer band, its list, the name box, the row of
// 「トラック」; and 「再生 ▸ アルペジオ」 (B-98-6), the same on / off as the input meter's button.
import { flushSync, mount, tick, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { MUTE_SET_NAME_MAX } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { arpToggle } from "../src/components/arpTarget";
import LowerBand from "../src/components/LowerBand.svelte";
import { closeContextMenu, contextMenu, listEntries, type MenuEntry, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { topMenus, type ListEntry, type ListRow } from "../src/components/menu/menus";
import MuteSetBox from "../src/components/mixer/MuteSetBox.svelte";
import { closeMuteSetBox, muteSetBox, muteSetBoxResult, muteSetEntries, muteSetName } from "../src/components/mixer/muteSets.svelte";
import { dragReading } from "../src/components/pianoroll/view.svelte";
import type { KeyDeps } from "../src/keys/attach";
import { t } from "../src/text";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => structuredClone(fakeScene(name)!.state);
const busy = (s: AppState): AppState => ({ ...s, transport: { ...s.transport, state: "recording" } });
const rowsOf = (entries: readonly MenuEntry[]) => entries.filter((e): e is MenuRow => e.kind === "row");
const shape = (entries: readonly MenuEntry[]) =>
  entries.map((e) => (e.kind === "separator" ? "---" : `${e.text ?? t(e.label)}${e.checked ? " ✓" : ""}${e.sub ? " ▸" : ""}${e.usable ? "" : " (off)"}`));

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
  closeContextMenu();
  closeMuteSetBox();
  flushSync();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}

describe("the wording", () => {
  it("uses 「ミュートのセット」 and has the same keys in English", () => {
    expect([t("mixer.muteSets"), t("menu.muteSets"), t("mixer.muteSetSave"), t("mixer.muteSetRename"), t("mixer.muteSetDelete")]).toEqual([
      "ミュートのセット ▾",
      "ミュートのセット",
      "いまのミュートを残す…",
      "名前を変える",
      "消す",
    ]);
    expect(t("mixer.muteSetsFull", { max: 64 })).toBe("セットは 64 個までです");
    expect(t("menu.arp")).toBe("アルペジオ");
  });
});

describe("the name of a set", () => {
  it("is trimmed, 1 to 64 characters, with no control characters", () => {
    expect(muteSetName("  サビ  ")).toBe("サビ");
    expect(muteSetName("   ")).toBeNull();
    expect(muteSetName("a".repeat(MUTE_SET_NAME_MAX))).toBe("a".repeat(MUTE_SET_NAME_MAX));
    expect(muteSetName("a".repeat(MUTE_SET_NAME_MAX + 1))).toBeNull();
    expect(muteSetName("🎹".repeat(MUTE_SET_NAME_MAX))).toBe("🎹".repeat(MUTE_SET_NAME_MAX));
    expect(muteSetName("a\tb")).toBeNull();
  });
});

describe("the list", () => {
  it("one row per set (checked when as now), a separator, then save, rename ▸ and delete ▸", () => {
    const s = scene("mixerMuteSets");
    const entries = muteSetEntries(s, () => {}, null);
    expect(shape(entries)).toEqual([
      "ドラムとベースだけ",
      "シンセなし ✓",
      "ピアノだけ",
      "---",
      "いまのミュートを残す…",
      "名前を変える ▸",
      "消す ▸",
    ]);
    // Rows of sets are choice rows: the mark is there, empty unless as now.
    expect(rowsOf(entries).slice(0, 3).map((r) => r.checked)).toEqual([false, true, false]);
    for (const i of [5, 6]) expect(shape((entries[i] as MenuRow).sub!)).toEqual(["ドラムとベースだけ", "シンセなし", "ピアノだけ"]);
  });

  it("with no sets has only the save row", () => {
    const s = scene("mixer");
    expect(s.song.muteSets).toEqual([]);
    expect(shape(muteSetEntries(s, () => {}, null))).toEqual(["いまのミュートを残す…"]);
  });

  it("with 64 sets the save row is disabled with the reason", () => {
    const entries = muteSetEntries(scene("mixerMuteSetsFull"), () => {}, null);
    const save = rowsOf(entries).find((r) => r.label === "mixer.muteSetSave")!;
    expect([save.usable, save.hint]).toEqual([false, "セットは 64 個までです"]);
    expect(rowsOf(entries)).toHaveLength(64 + 3);
  });

  it("every row is disabled while counting in or recording", () => {
    for (const state of ["recording", "countIn"] as const) {
      const s = scene("mixerMuteSets");
      const entries = muteSetEntries({ ...s, transport: { ...s.transport, state } }, () => {}, null);
      expect(rowsOf(entries).every((r) => !r.usable)).toBe(true);
      expect(rowsOf(entries.flatMap((e) => (e.kind === "row" && e.sub ? e.sub : []))).every((r) => !r.usable)).toBe(true);
    }
  });

  it("a set's row recalls it, 「消す ▸」 deletes, 「名前を変える ▸」 and the save row open the name box", () => {
    const sent: Command[] = [];
    const entries = muteSetEntries(scene("mixerMuteSets"), (c) => sent.push(c), { x: 10, y: 20 });
    const rows = rowsOf(entries);
    rows[0]!.run();
    rowsOf(rows[5]!.sub!)[1]!.run();
    expect(sent).toEqual([
      { type: "recallMuteSet", name: "ドラムとベースだけ" },
      { type: "deleteMuteSet", name: "シンセなし" },
    ]);
    rows[3]!.run();
    expect(muteSetBox.open).toEqual({ kind: "save", at: { x: 10, y: 20 } });
    rowsOf(rows[4]!.sub!)[2]!.run();
    expect(muteSetBox.open).toEqual({ kind: "rename", from: "ピアノだけ", at: { x: 10, y: 20 } });
  });
});

describe("what Enter does in the name box", () => {
  const s = scene("mixerMuteSets");
  it("saves a new name as a new set, and a name taken over that set", () => {
    expect(muteSetBoxResult(s, { kind: "save" }, " サビ ")).toEqual({
      command: { type: "saveMuteSet", name: "サビ", overwrite: false },
      reading: "Enter で「サビ」を残します",
    });
    expect(muteSetBoxResult(s, { kind: "save" }, "シンセなし")).toEqual({
      command: { type: "saveMuteSet", name: "シンセなし", overwrite: true },
      reading: "Enter で「シンセなし」を上書きします",
    });
    expect(muteSetBoxResult(s, { kind: "save" }, "  ")).toEqual({ command: null, reading: null });
  });

  it("renames to a free name; the name of another set is refused with the reason; the same name is nothing", () => {
    const box = { kind: "rename", from: "ピアノだけ" } as const;
    expect(muteSetBoxResult(s, box, "ピアノとドラム")).toEqual({
      command: { type: "renameMuteSet", name: "ピアノだけ", to: "ピアノとドラム" },
      reading: "Enter で名前を「ピアノとドラム」に変えます",
    });
    expect(muteSetBoxResult(s, box, "シンセなし")).toEqual({ command: null, reading: "「シンセなし」はもうあります" });
    expect(muteSetBoxResult(s, box, "ピアノだけ ")).toEqual({ command: null, reading: null });
  });
});

describe("the name box (mounted)", () => {
  const field = () => document.querySelector<HTMLInputElement>('[data-test="mute-set-name"]');
  const type = (text: string) => {
    field()!.value = text;
    field()!.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
  };
  const key = (k: string) => {
    field()!.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));
    flushSync();
  };

  it("Enter sends one saveMuteSet and closes; the status line says what Enter does while it is open", async () => {
    show(MuteSetBox, scene("mixerMuteSets"));
    muteSetBox.open = { kind: "save", at: { x: 0, y: 0 } };
    flushSync();
    await tick();
    expect(field()!.value).toBe("");
    type("シンセなし");
    expect(dragReading.text).toBe("Enter で「シンセなし」を上書きします");
    key("Enter");
    expect(app.sent).toEqual([{ type: "saveMuteSet", name: "シンセなし", overwrite: true }]);
    expect(field()).toBeNull();
    expect(dragReading.text).toBeNull();
  });

  it("a rename starts with the old name; a name taken stays open, marked; Esc closes without sending", async () => {
    show(MuteSetBox, scene("mixerMuteSets"));
    muteSetBox.open = { kind: "rename", from: "ピアノだけ", at: { x: 0, y: 0 } };
    flushSync();
    await tick();
    expect(field()!.value).toBe("ピアノだけ");
    type("シンセなし");
    key("Enter");
    expect(field()!.classList.contains("broken")).toBe(true);
    expect(dragReading.text).toBe("「シンセなし」はもうあります");
    key("Escape");
    expect(field()).toBeNull();
    expect(app.sent).toEqual([]);
  });

  it("closes when recording starts", async () => {
    const s = scene("mixerMuteSets");
    show(MuteSetBox, s);
    muteSetBox.open = { kind: "save", at: { x: 0, y: 0 } };
    flushSync();
    app.setState(busy(s));
    flushSync();
    expect(muteSetBox.open).toBeNull();
  });
});

describe("the button at the head of the mixer band", () => {
  it("opens the list under it", () => {
    show(LowerBand, scene("mixerMuteSets"), { band: "mixer", height: 516 });
    const button = document.querySelector<HTMLElement>('[data-test="mute-sets-button"]')!;
    expect(button.textContent?.trim()).toBe("ミュートのセット ▾");
    button.click();
    expect(shape(contextMenu.open!.entries)).toEqual(shape(muteSetEntries(scene("mixerMuteSets"), () => {}, null)));
  });
});

describe("the menu bar", () => {
  const deps = (state: AppState, sent: Command[]): KeyDeps => ({ state: () => state, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 });
  const menu = (state: AppState, label: string, sent: Command[] = []) => topMenus(state, deps(state, sent), "other").find((m) => m.label === label)!.entries;
  const row = (entries: ListEntry[], label: string) => entries.find((e): e is ListRow => e.kind === "row" && e.label === label)!;

  it("「トラック ▸ ミュートのセット ▸」 holds the same list", () => {
    const s = scene("mixerMuteSets");
    const sub = row(menu(s, "トラック"), "ミュートのセット").sub!;
    const same = listEntries(muteSetEntries(s, () => {}, null));
    expect(sub.map((e) => (e.kind === "row" ? [e.label, e.checked, e.usable, e.sub?.length] : e.kind))).toEqual(
      same.map((e) => (e.kind === "row" ? [e.label, e.checked, e.usable, e.sub?.length] : e.kind)),
    );
  });

  it("「再生 ▸ アルペジオ」 is checked while on and sends what the input meter's button sends", () => {
    for (const name of ["arpOn", "keepPlayedReady"]) {
      const s = scene(name);
      const sent: Command[] = [];
      const r = row(menu(s, "再生", sent), "アルペジオ");
      const track = s.song.tracks.find((x) => x.id === s.song.keysTarget)!;
      expect([r.checked, r.usable, r.key]).toEqual([track.arp.on, true, undefined]);
      r.run!();
      expect(sent).toEqual([arpToggle(s)!]);
      expect(sent[0]).toEqual({ type: "setArp", track: track.id, arp: { ...track.arp, on: !track.arp.on } });
    }
  });

  it("「再生 ▸ アルペジオ」 is disabled when the button is: no keys track, counting in or recording", () => {
    const s = scene("arpOn");
    for (const state of [busy(s), { ...s, song: { ...s.song, keysTarget: null } }]) {
      expect(arpToggle(state)).toBeNull();
      expect(row(menu(state, "再生"), "アルペジオ").usable).toBe(false);
    }
  });
});
