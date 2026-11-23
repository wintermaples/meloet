// Screen: track heads (name, reorder, color, delete, instrument name), the shelf, keys, the fake's answers.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INSTRUMENT_GROUPS, type AppState, type Command, type InstrumentKind, type ShelfView } from "../src/bridge";
import { createFakeTransport, fakeScene, FakeBands, reduceTracks, type FakeState } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import InputMeter from "../src/components/InputMeter.svelte";
import RightRail from "../src/components/RightRail.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { emptyLibrary } from "../src/bridge/fakeLibrary";
import { shelfModel, type ShelfLook } from "../src/components/shelfList";
import { moveSlot, shelfStepCommand, shelfUi, trackEdit } from "../src/components/tracks.svelte";
import { handleKeydown, handlePointerdown, type KeyDeps } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...mods });

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  trackEdit.renaming = null;
  Object.assign(shelfUi, { query: "", favoritesOnly: false, open: {}, searchOpen: {}, searchFor: "", keys: false, view: "instruments" });
});

function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
function pointer(el: Element, type: string, clientY: number): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX: 10, clientY, button: 0 }));
  flushSync();
}
function keyOn(el: Element, key: string): KeyboardEvent {
  const ev = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true });
  el.dispatchEvent(ev);
  flushSync();
  return ev;
}

describe("shelf row rules (screen only)", () => {
  const shelf = () => scene("shelfNewTrack").shelf;
  const look = (o: Partial<ShelfLook> = {}): ShelfLook => ({ query: "", favoritesOnly: false, open: {}, searchOpen: {}, searchFor: "", ...o });
  /** Headings and the rows under them (names of built-in instruments). */
  function groups(sh: ShelfView, l: ShelfLook): [string, string[]][] {
    const out: [string, string[]][] = [];
    for (const e of shelfModel("instruments", sh, emptyLibrary(), l).entries) {
      if (e.kind === "heading") out.push([e.id, []]);
      if (e.kind === "row" && e.sound?.kind === "builtin") out.at(-1)![1].push(e.sound.instrument);
    }
    return out;
  }
  const rowsOf = (sh: ShelfView, l: ShelfLook) => groups(sh, l).map((g) => g[1]);

  it("group order and counts. The search text keeps only rows whose name or group name contains it. Groups with nothing to show are hidden with their heading", () => {
    expect(groups(shelf(), look()).map(([g, rows]) => [g, rows.length])).toEqual([
      ["pianoKeys", 11],
      ["guitar", 4],
      ["bass", 4],
      ["drums", 3],
      ["padStrings", 3],
      ["wind", 4],
      ["synth", 2],
    ]);
    expect(rowsOf(shelf(), look({ query: "ベース" }))).toEqual([["fingerBass", "pickBass", "woodBass", "synthBass"]]);
    // If the group name matches, all rows of that group.
    expect(rowsOf(shelf(), look({ query: "弦" }))).toEqual([["strings", "pizzicato", "warmPad"]]);
    expect(rowsOf(shelf(), look({ query: "オルガン" }))).toEqual([
      ["organDrawbar", "organPercussive", "organRock", "organ", "organQuiet", "organChurch", "organReed"],
    ]);
    expect(rowsOf(shelf(), look({ query: "シンセ" }))).toEqual([["synthBass"], ["synthLead", "sine"]]);
  });

  it("favorites only. Instruments not on the shelf are hidden (available only)", () => {
    const s = { ...shelf(), available: shelf().available.filter((i) => i !== "studioKit") };
    expect(rowsOf(s, look({ favoritesOnly: true }))).toEqual([["grandPiano"]]);
    expect(groups({ ...s, available: ["sine"] }, look()).map((g) => g[0])).toEqual(["synth"]);
  });

  it("↑ ↓ move within the visible rows and stop at the ends. Folded groups are skipped", () => {
    const s = scene("shelfNewTrack");
    shelfUi.open = { bass: false };
    shelfUi.view = "instruments";
    const step = (from: InstrumentKind | null, delta: 1 | -1) => {
      s.shelf.selectedSound = from === null ? null : { kind: "builtin", instrument: from };
      const c = shelfStepCommand(s, delta);
      return c?.type === "shelfSelectSound" && c.sound?.kind === "builtin" ? c.sound.instrument : null;
    };
    expect(step(null, 1)).toBe("grandPiano");
    expect(step(null, -1)).toBe("sine");
    expect(step("organ", 1)).toBe("organQuiet");
    expect(step("marimba", 1)).toBe("guitarNylon");
    expect(step("guitarDistortion", 1)).toBe("studioKit");
    // Stops at the ends (the same row is not sent).
    expect(step("grandPiano", -1)).toBeNull();
  });

  it("reorder line and target: the line at the boundary the middle crossed. Lines next to itself do not move it", () => {
    const h = 64;
    expect(moveSlot(0, 4, 0.5 * h + 10, h)).toEqual({ slot: 1, to: 0 });
    expect(moveSlot(0, 4, 0.5 * h + 40, h)).toEqual({ slot: 1, to: 0 });
    expect(moveSlot(0, 4, 0.5 * h + 100, h)).toEqual({ slot: 2, to: 1 });
    expect(moveSlot(3, 4, 3.5 * h - 200, h)).toEqual({ slot: 0, to: 0 });
    expect(moveSlot(1, 4, 9 * h, h)).toEqual({ slot: 4, to: 3 });
  });
});

describe("keys (spec 4)", () => {
  it("F2 works while typing. ↑ ↓ Enter in the shelf work while typing too, Space and F only when not typing", () => {
    expect(keyAction(k("F2"), true, "other")).toEqual({ kind: "renameTrack" });
    expect(keyAction(k("ArrowDown"), true, "other", "shelf")).toEqual({ kind: "shelfStep", delta: 1 });
    expect(keyAction(k("ArrowUp"), false, "other", "shelf")).toEqual({ kind: "shelfStep", delta: -1 });
    expect(keyAction(k("Enter"), true, "other", "shelf")).toEqual({ kind: "shelfConfirm" });
    expect(keyAction(k(" ", { code: "Space" }), false, "other", "shelf")).toEqual({ kind: "shelfSample" });
    expect(keyAction(k(" ", { code: "Space" }), true, "other", "shelf")).toBeNull();
    expect(keyAction(k("f"), false, "other", "shelf")).toEqual({ kind: "shelfFavorite" });
    expect(keyAction(k("f"), true, "other", "shelf")).toBeNull();
    // When keys do not go to the shelf, it works as before.
    expect(keyAction(k(" ", { code: "Space" }), false, "other")).toEqual({ kind: "playStop" });
    expect(keyAction(k("f"), false, "other")).toBeNull();
    expect(keyAction(k("ArrowDown"), false, "other")).toEqual({ kind: "nudge", steps: 0, keys: -1 });
  });

  function deps(state: AppState, sent: Command[]): KeyDeps {
    return { state: () => state, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
  }
  const press = (d: KeyDeps, key: string, init: KeyboardEventInit = {}) => {
    const ev = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...init });
    handleKeydown(ev, d);
    return ev;
  };

  it("when keys go to the shelf: ↓ next row, Enter adds, Space plays the sample, F sets the favorite", () => {
    const s = scene("shelfAudition");
    const sent: Command[] = [];
    const d = deps(s, sent);
    shelfUi.keys = true;
    press(d, "ArrowDown");
    expect(press(d, "Enter").defaultPrevented).toBe(true);
    expect(press(d, " ", { code: "Space" }).defaultPrevented).toBe(true);
    press(d, "f");
    expect(sent).toEqual([
      { type: "shelfSelectSound", sound: { kind: "builtin", instrument: "studioKit" } },
      { type: "shelfConfirm" },
      { type: "playSampleOf", sound: { kind: "builtin", instrument: "synthBass" } },
      { type: "toggleFavoriteSound", sound: { kind: "builtin", instrument: "synthBass" } },
    ]);
  });

  it("pressing outside the shelf takes keys away and Space goes back to play. Pressing the shelf gives them back", () => {
    const s = scene("shelfAudition");
    s.transport.canPlay = true;
    const sent: Command[] = [];
    const d = deps(s, sent);
    const inside = document.createElement("div");
    inside.setAttribute("data-shelf", "");
    const outside = document.createElement("div");
    target.append(inside, outside);
    shelfUi.keys = true;
    handlePointerdown(d, outside);
    expect(shelfUi.keys).toBe(false);
    press(d, " ", { code: "Space" });
    handlePointerdown(d, inside);
    expect(shelfUi.keys).toBe(true);
    expect(sent).toEqual([{ type: "playStop" }]);
  });

  it("Esc: ShelfCancel during try-out, otherwise Back (closes the shelf)", () => {
    const sent: Command[] = [];
    press(deps(scene("shelfAudition"), sent), "Escape");
    press(deps(scene("shelfNewTrack"), sent), "Escape");
    expect(sent).toEqual([{ type: "shelfCancel" }, { type: "back" }]);
  });

  it("F2 opens the name input box of the selected track. Does nothing while recording", () => {
    const s = scene("tracks6");
    press(deps(s, []), "F2");
    expect(trackEdit.renaming).toBe(12);
    trackEdit.renaming = null;
    s.transport.state = "recording";
    press(deps(s, []), "F2");
    expect(trackEdit.renaming).toBeNull();
  });
});

describe("track head (mock R6-Tracks, R7-Arrange6)", () => {
  it("name input box: Enter sends RenameTrack (spaces at both ends removed). Empty or same name is not sent. Esc stops", () => {
    show(Arrange, scene("tracks6"));
    trackEdit.renaming = 12;
    flushSync();
    let field = q("[data-test=track-name-field]") as HTMLInputElement;
    expect(field.value).toBe("ベース");
    field.value = "  ベース A  ";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    keyOn(field, "Enter");
    expect(q("[data-test=track-name-field]")).toBeNull();
    for (const typed of ["   ", "ベース"]) {
      trackEdit.renaming = 12;
      flushSync();
      field = q("[data-test=track-name-field]") as HTMLInputElement;
      field.value = typed;
      field.dispatchEvent(new Event("input", { bubbles: true }));
      keyOn(field, "Enter");
    }
    trackEdit.renaming = 12;
    flushSync();
    field = q("[data-test=track-name-field]") as HTMLInputElement;
    field.value = "やめる名前";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    keyOn(field, "Escape");
    expect(q("[data-test=track-name-field]")).toBeNull();
    expect(app.sent).toEqual([{ type: "renameTrack", track: 12, name: "ベース A" }]);
  });

  it("pressing outside the name input box commits. Nothing is sent after Esc", () => {
    show(Arrange, scene("tracks6"));
    trackEdit.renaming = 12;
    flushSync();
    let field = q("[data-test=track-name-field]") as HTMLInputElement;
    field.value = "外で決める";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    field.dispatchEvent(new FocusEvent("blur"));
    flushSync();
    expect(q("[data-test=track-name-field]")).toBeNull();
    trackEdit.renaming = 12;
    flushSync();
    field = q("[data-test=track-name-field]") as HTMLInputElement;
    field.value = "やめる名前";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    keyOn(field, "Escape");
    field.dispatchEvent(new FocusEvent("blur"));
    expect(app.sent).toEqual([{ type: "renameTrack", track: 12, name: "外で決める" }]);
  });

  it("pressing the name twice opens the input box", () => {
    show(Arrange, scene("tracks6"));
    qa(".track-name")[2]!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    expect(trackEdit.renaming).toBe(13);
  });

  it("head drag: picked up with an accent line. Release sends one MoveTrack. Esc stops. A small move is a plain press", () => {
    show(Arrange, scene("tracks6"));
    const heads = () => qa("[data-test=track-head]");
    // Move 「1 ピアノ」 down by 2 tracks (size.track 64).
    pointer(heads()[0]!, "pointerdown", 100);
    pointer(heads()[0]!, "pointermove", 130);
    expect(heads()[0]!.classList.contains("lifted")).toBe(true);
    pointer(heads()[0]!, "pointermove", 100 + 64 * 2);
    expect(q("[data-test=track-insert]")).not.toBeNull();
    pointer(heads()[0]!, "pointerup", 100 + 64 * 2);
    expect(heads()[0]!.classList.contains("lifted")).toBe(false);
    // Stop with Esc.
    pointer(heads()[3]!, "pointerdown", 300);
    pointer(heads()[3]!, "pointermove", 100);
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(q("[data-test=track-insert]")).toBeNull();
    pointer(heads()[3]!, "pointerup", 100);
    // A move smaller than space.1 (4) is only a press.
    pointer(heads()[4]!, "pointerdown", 300);
    pointer(heads()[4]!, "pointermove", 302);
    pointer(heads()[4]!, "pointerup", 302);
    expect(app.sent).toEqual([
      { type: "moveTrack", track: 11, to: 2 },
      { type: "selectTrack", track: 15, fromArm: false },
    ]);
  });

  it("pressing the instrument name chooses that instrument (19-A). 「＋ 鍵盤のトラック」 adds a new track, 「＋ 歌のトラック」 adds an audio track", () => {
    show(Arrange, scene("tracks6"));
    qa("[data-test=track-instrument]")[1]!.click();
    q("[data-test=add-track]")!.click();
    q("[data-test=add-vocal-track]")!.click();
    expect(text(q("[data-test=add-track]"))).toBe(t("arrange.addKeysTrack"));
    expect(text(q("[data-test=add-vocal-track]"))).toBe(t("arrange.addVocalTrack"));
    expect(app.sent).toEqual([
      { type: "selectInstrument", track: 12 },
      { type: "openShelf", mode: { kind: "newTrack" } },
      { type: "addVocalTrack" },
    ]);
  });

  it("during a replacement try-out, that track's instrument name is the accent 「（名前）（ためし弾き）」", () => {
    show(Arrange, scene("shelfReplace"));
    const names = qa("[data-test=track-instrument]");
    expect(text(names[0]!)).toBe("エレクトリックピアノ（ためし弾き）");
    expect(names[0]!.classList.contains("auditioned")).toBe(true);
    expect(text(names[1]!)).toBe("フィンガーベース");
  });
});

describe("shelf (mock F4-ShelfSample, F4-ShelfAudition, F4-ShelfReplace)", () => {
  it("heading, mode text, search box, all and favorites, groups and rows (favorite stars)", () => {
    show(RightRail, scene("shelfNewTrack"));
    expect(text(q(".head"))).toBe("▾ 棚 音源 31 エフェクト 6 P");
    expect(text(q("[data-test=shelf-mode]"))).toBe("新しいトラックの音源をえらぶ");
    expect((q("[data-test=shelf-search]") as HTMLInputElement).placeholder).toBe("名前 ・ 種類");
    expect(text(q("[data-test=shelf-all]"))).toBe("すべて 31");
    expect(text(q("[data-test=shelf-favorites]"))).toBe("お気に入り 2");
    expect(qa("[data-group]").map(text)).toEqual([
      "▾ ピアノ ・ 鍵盤 11",
      "▾ ギター 4",
      "▾ ベース 4",
      "▾ ドラム 3",
      "▾ パッド ・ 弦 3",
      "▾ 管楽器 4",
      "▾ シンセ 2",
    ]);
    expect(qa("[data-row]").length).toBe(31);
    expect(qa("[aria-pressed=true][aria-label=お気に入り]").map((b) => b.closest("[data-row]")!.getAttribute("data-row"))).toEqual([
      "grandPiano",
      "studioKit",
    ]);
    // In instrument mode the 「音源」 list is in front. 「エフェクト」 can be pressed too (view only; tests/params.shelf.test.ts).
    expect(q("[data-test=shelf-instruments]")!.classList.contains("on")).toBe(true);
    expect((q("[data-test=shelf-effects]") as HTMLButtonElement).disabled).toBe(false);
  });

  it("fold by the group heading (the heading and count stay). Switch to favorites. Search", () => {
    show(RightRail, scene("shelfNewTrack"));
    q("[data-group=pianoKeys]")!.click();
    flushSync();
    expect(text(q("[data-group=pianoKeys]"))).toBe("▸ ピアノ ・ 鍵盤 11");
    expect(qa("[data-row]").length).toBe(20);
    q("[data-test=shelf-favorites]")!.click();
    flushSync();
    expect(qa("[data-group]").map(text)).toEqual(["▸ ピアノ ・ 鍵盤 1", "▾ ドラム 1"]);
    shelfUi.favoritesOnly = false;
    shelfUi.open = {};
    shelfUi.query = "あたたか";
    flushSync();
    expect(qa("[data-row]").map((r) => r.getAttribute("data-row"))).toEqual(["warmPad"]);
  });

  it("pressing a row sends ShelfSelectSound, the star ToggleFavoriteSound, ▶ PlaySampleOf (the row is not chosen)", () => {
    show(RightRail, scene("shelfNewTrack"));
    q("[data-row=organ]")!.click();
    q("[data-row=organ] [aria-label=お気に入り]")!.click();
    q("[data-row=organ] [data-test=shelf-sample]")!.click();
    const organ = { kind: "builtin", instrument: "organ" };
    expect(app.sent).toEqual([
      { type: "shelfSelectSound", sound: organ },
      { type: "toggleFavoriteSound", sound: organ },
      { type: "playSampleOf", sound: organ },
    ]);
  });

  it("in try-out (new track): below the chosen row, the hint and 「トラック 5 を作る Enter」「やめる Esc」", () => {
    show(RightRail, scene("shelfAudition"));
    expect(q("[data-row=synthBass]")!.classList.contains("selected")).toBe(true);
    const box = q("[data-test=shelf-audition]")!;
    expect(box.parentElement!.previousElementSibling).toBe(q("[data-row=synthBass]"));
    expect(text(box)).toBe(`ためし弾き中 ${t("msg.audition")} トラック 5 を作る Enter やめる Esc`);
    q("[data-test=shelf-confirm]")!.click();
    q("[data-test=shelf-cancel]")!.click();
    expect(app.sent).toEqual([{ type: "shelfConfirm" }, { type: "shelfCancel" }]);
  });

  it("replace: 「1 ピアノ の音源を入れかえる」, 「いまの音源」 on the current instrument's row, the button is 「入れかえる Enter」", () => {
    show(RightRail, scene("shelfReplace"));
    expect(text(q("[data-test=shelf-mode]"))).toBe("1 ピアノ の音源を入れかえる");
    expect(text(q("[data-row=grandPiano]"))).toBe("グランドピアノ いまの音源");
    expect(text(q("[data-test=shelf-audition]"))).toBe(`ためし弾き中 ${t("msg.auditionReplace")} 入れかえる Enter やめる Esc`);
  });

  it("while a sample plays: ▶ becomes an accent stop, and 「0:00 / 0:04」 on the right", () => {
    const s = scene("shelfNewTrack");
    s.shelf.soundSample = { sound: { kind: "builtin", instrument: "grandPiano" }, playing: { startedUs: 0, lengthUs: 4_000_000 } };
    show(RightRail, s);
    expect(q("[data-row=grandPiano] [data-test=shelf-sample]")!.classList.contains("playing")).toBe(true);
    expect(text(q("[data-test=shelf-sample-time]"))).toMatch(/^0:0\d \/ 0:04$/);
    q("[data-row=grandPiano] [data-test=shelf-sample]")!.click();
    expect(app.sent).toEqual([{ type: "stopSample" }]);
  });

  it("shelf84 (mock R8-Shelf84): 31 instruments, 7 groups. All but piano, keys and guitar are folded", () => {
    show(RightRail, scene("shelf84"));
    expect(text(q(".head"))).toBe("▾ 棚 音源 31 エフェクト 6 P");
    expect(text(q("[data-test=shelf-all]"))).toBe("すべて 31");
    for (const g of ["bass", "drums", "padStrings", "wind", "synth"]) q(`[data-group=${g}]`)!.click();
    flushSync();
    expect(qa("[data-group]").map(text)).toEqual([
      "▾ ピアノ ・ 鍵盤 11",
      "▾ ギター 4",
      "▸ ベース 4",
      "▸ ドラム 3",
      "▸ パッド ・ 弦 3",
      "▸ 管楽器 4",
      "▸ シンセ 2",
    ]);
    expect(qa("[data-row]").map((r) => r.getAttribute("data-row"))).toEqual([
      ...INSTRUMENT_GROUPS[0]!.instruments,
      ...INSTRUMENT_GROUPS[1]!.instruments,
    ]);
  });

  it("the count in the vertical heading of the closed shelf is the number of instruments on the shelf", () => {
    show(RightRail, scene("tracks6"));
    expect(text(q("[data-band=shelf]"))).toContain("音源 31 ・ エフェクト 6");
    q("[data-band=shelf]")!.click();
    expect(app.sent).toEqual([{ type: "openShelf", mode: { kind: "newTrack" } }]);
  });
});

describe("fake answers (protocol 13)", () => {
  const run = (state: FakeState, c: Command) => reduceTracks(state, new FakeBands(800), c)!;

  it("delete: the track below is selected, numbers are given again, the status line shows the deleted number", () => {
    const s = scene("tracks6");
    const r = run(s, { type: "deleteTrack", track: 12 });
    expect(r.song.tracks.map((x) => [x.number, x.name])).toEqual([
      [1, "ピアノ"],
      [2, "ドラム"],
      [3, "パッド"],
      [4, "シンセ"],
      [5, "ベース 2"],
    ]);
    expect(r.song.playTarget).toBe(13);
    expect(r.notice).toEqual({ id: "trackDeleted", track: { number: 2, name: "ベース" } });
  });

  it("reorder: numbers are given again, colors stay with the track. Colors are only 1 to 5", () => {
    const s = scene("tracks6");
    const r = run(s, { type: "moveTrack", track: 11, to: 2 });
    expect(r.song.tracks.map((x) => [x.number, x.name, x.color])).toEqual([
      [1, "ベース", 2],
      [2, "ドラム", 3],
      [3, "ピアノ", 1],
      [4, "パッド", 4],
      [5, "シンセ", 5],
      [6, "ベース 2", 1],
    ]);
    expect(run(s, { type: "setTrackColor", track: 11, color: 4 }).song.tracks[0]!.color).toBe(4);
    expect(reduceTracks(s, new FakeBands(800), { type: "setTrackColor", track: 11, color: 6 })).toBeNull();
  });

  it("new track: just below the selected track, the group's short name (with a free number from 2 if the name exists), the color of the place", () => {
    const s = scene("tracks6");
    s.shelf.selected = "fingerBass";
    const r = run(s, { type: "shelfConfirm" });
    expect(r.song.tracks.map((x) => [x.number, x.name, x.color])[2]).toEqual([3, "ベース 3", 3]);
    expect(r.song.playTarget).toBe(r.song.tracks[2]!.id);
    expect(r.notice).toEqual({ id: "trackAdded", track: { number: 3, name: "ベース 3" } });
    expect(r.shelf.selected).toBeNull();
  });

  it("flow: open the shelf -> choose (cannot record, the status line shows try-out) -> stop -> close", async () => {
    const transport = createFakeTransport(scene("tracks6"));
    const patches: AppState[] = [];
    let state = (await transport.snapshot()).state;
    await transport.subscribe(
      (m) => {
        if (m.kind !== "patch") return;
        state = { ...state };
        for (const c of m.changes) (state as unknown as Record<string, unknown>)[c.section] = c.value;
        patches.push(state);
      },
      () => {},
    );
    await transport.send({ type: "openShelf", mode: { kind: "replace", track: 11 } });
    expect(state.workspace.layout.rightFront).toBe("shelf");
    expect(state.shelf.currentSound).toEqual({ kind: "builtin", instrument: "grandPiano" });
    await transport.send({ type: "shelfSelect", instrument: "organ" });
    expect(state.transport.canRecord).toBe(false);
    expect(state.status.message).toEqual({ id: "auditionReplace", instrument: "organ", track: { number: 1, name: "ピアノ" }, current: "grandPiano" });
    await transport.send({ type: "shelfConfirm" });
    expect(state.song.tracks[0]!.sound).toEqual({ kind: "builtin", instrument: "organ" });
    expect(state.status.message).toEqual({ id: "instrumentReplaced", track: { number: 1, name: "ピアノ" }, instrument: "organ", lanesRemoved: 0 });
    await transport.send({ type: "toggleBand", band: "shelf" });
    expect(state.workspace.layout.rightOpen).toEqual([]);
    expect(state.shelf.mode).toEqual({ kind: "newTrack" });
    expect(patches.length).toBeGreaterThan(0);
  });
});

describe("status line and input meter (mock F4-ShelfAudition, F4-ShelfReplace, R7-Arrange6)", () => {
  const mountOf = (C: Component<any>, state: AppState) => {
    app.setState(state);
    mounted = mount(C, { target });
    flushSync();
  };

  it("in try-out: the left tag is the accent 「♪ ためし弾き中」, the text is status.auditionNewTrack", () => {
    mountOf(StatusLine, scene("shelfAudition"));
    const badge = q("[data-test=status-badge]")!;
    expect(text(badge)).toBe("♪ ためし弾き中");
    expect(badge.classList.contains("accent")).toBe(true);
    expect(text(q(".message"))).toBe("シンセベース ・ まだトラックには入っていません ・ 入れるには Enter");
  });

  it("replace: status.auditionReplace", () => {
    mountOf(StatusLine, scene("shelfReplace"));
    expect(text(q(".message"))).toBe("エレクトリックピアノ ・ 1 ピアノ の音源は、まだグランドピアノです ・ 入れかえるには Enter");
  });

  it("when the shelf opens and when a heading opens or folds, 「音色 31 ・ …をひらいています」 (spec 6 (Shelf open (headings)))", () => {
    const s = scene("shelf84");
    mountOf(StatusLine, s);
    const say = () => (flushSync(), text(q(".message")));
    const fold = (groups: string[]) => (shelfUi.open = Object.fromEntries(groups.map((g) => [g, false])));
    fold(["bass", "drums", "padStrings", "wind", "synth"]);
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤 と ギター をひらいています");
    fold(["guitar", "bass", "drums", "padStrings", "wind", "synth"]);
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤 をひらいています");
    fold(["drums", "padStrings", "wind", "synth"]);
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤、ギター と ベース をひらいています");
    fold(["padStrings", "wind", "synth"]);
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤、ギター、ベース と ドラム をひらいています");
    fold(["wind", "synth"]);
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤 ほか 4 つ をひらいています");
    fold(["pianoKeys", "guitar", "bass", "drums", "padStrings", "wind", "synth"]);
    expect(say()).toBe("音色 31");
    // When core's notice changes, it is not said again (until the next heading opens or folds).
    app.setState({ ...s, status: { message: { id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: true, notes: null } } });
    expect(say()).toBe("ループ 5–9 ・ 選択なし");
    fold(["bass", "drums", "padStrings", "wind", "synth"]);
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤 と ギター をひらいています");
    app.setState(s);
    // The count is the number of rows shown on the shelf. While searching, 「さがす「オルガン」・ 合う物 7 個」 (mock F5-ShelfSearch).
    fold([]);
    shelfUi.query = "オルガン";
    expect(say()).toBe("さがす「オルガン」・ 合う物 7 個");
    shelfUi.query = "";
    // The effect list shows 「エフェクト 6」, or 「エフェクト 512 ・ リバーブ をひらいています」 when a heading is open.
    shelfUi.view = "effects";
    expect(say()).toBe("エフェクト 6");
    // During try-out, while a sample plays, or when the shelf is closed, the usual text.
    shelfUi.view = "instruments";
    app.setState({ ...s, shelf: { ...s.shelf, soundSample: { sound: { kind: "builtin", instrument: "grandPiano" }, playing: { startedUs: 0, lengthUs: 4_000_000 } } } });
    expect(say()).not.toContain("音色");
    app.setState({ ...s, workspace: { ...s.workspace, layout: { ...s.workspace.layout, rightOpen: [], rightFront: null } } });
    expect(say()).toBe("ループ 5–9 ・ 選択なし");
    // The mute text is added at the end as usual.
    app.setState({ ...s, song: { ...s.song, tracks: s.song.tracks.map((x, i) => ({ ...x, muted: i === 3 })) } });
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤 ほか 6 つ をひらいています ・ 4 パッド はミュート");
  });

  it("● on another track: 「R を押すと 2 ベース に録ります」", () => {
    mountOf(StatusLine, scene("tracks6"));
    expect(text(q(".message"))).toBe("R を押すと 2 ベース に録ります");
  });

  it("notice text: added, deleted, replaced, sample playing", () => {
    const s = scene("tracks6");
    mountOf(StatusLine, s);
    const say = (message: AppState["status"]["message"]) => {
      app.setState({ ...s, status: { message } });
      flushSync();
      return text(q(".message"));
    };
    const track = { number: 3, name: "ドラム" };
    expect(say({ id: "trackAdded", track })).toBe("3 ドラム を足しました。");
    expect(say({ id: "trackDeleted", track })).toBe("3 ドラム を消しました。取り消せます（Ctrl Z）");
    expect(say({ id: "instrumentReplaced", track, instrument: "electroKit", lanesRemoved: 0 })).toBe(
      "3 ドラム の音源を エレクトロキット にしました。取り消せます（Ctrl Z）",
    );
    expect(say({ id: "samplePlaying", instrument: "grandPiano" })).toBe("サンプルを再生中 ・ グランドピアノ。行をえらぶと、鍵盤でためし弾きできます。");
    expect(say({ id: "instrumentLoading", instrument: "grandPiano" })).toBe("グランドピアノを読みこんでいます… 少しだけ待つと、鍵盤で鳴らせます。");
  });

  it("the input meter's play target: usually the selected track, in try-out the accent 「ためし弾き ・ シンセベース」", () => {
    mountOf(InputMeter, scene("tracks6"));
    // jsdom does not measure widths, so the instrument name is in the hidden form (the hiding order is checked in the meter-fit tests).
    expect(text(q("[data-test=meter-target]"))).toMatch(/^2 ベース/);
    app.setState(scene("shelfAudition"));
    flushSync();
    const el = q("[data-test=meter-target]")!;
    expect(text(el)).toBe("ためし弾き ・ シンセベース");
    expect(el.classList.contains("accent")).toBe(true);
  });
});
