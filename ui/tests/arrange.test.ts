// Mounts the arrange and checks names and tabs, take switching, record arm, dragging the ruler, choosing clips, and horizontal scroll.
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView } from "../src/bridge";
import { arrangeOf } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const BAR = 4 * TPQ;
const WIDTH = 1000;

// jsdom does not measure sizes. Set the width of the time column to 1000.
const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() =>
  Object.defineProperty(proto, "clientWidth", {
    configurable: true,
    get: () => WIDTH,
  }),
);
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

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
});

function show(state: AppState): void {
  app.setState(state);
  mounted = mount(Arrange, { target });
  flushSync();
}

/** Swaps only the state while the component stays mounted. */
function show2(state: AppState): void {
  app.setState(state);
  flushSync();
}

const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

function clip(id: number, name: string, fromBar: number, toBar: number, take = 1, takes = 1): ClipView {
  return {
    id,
    name,
    start: (fromBar - 1) * BAR,
    end: (toBar - 1) * BAR,
    take,
    takes,
    takeId: 100 + id,
    takeOffset: 0,
    noteCount: 0,
    audio: null,
    comp: [],
  };
}

/** mock F-Takes: 「Aメロ」 is take 2 of 3, and is selected. */
const takes = (selected: number | null = 3) =>
  app.stateWith((s) => {
    s.song.tracks[0]!.clips = [clip(1, "イントロ", 1, 5), clip(3, "Aメロ", 5, 9, 2, 3)];
    s.arrange = arrangeOf(selected);
  });

function pointer(el: HTMLElement, type: string, clientX: number, clientY = 10, more: MouseEventInit = {}): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX, clientY, button: 0, ...more }));
  flushSync();
}

/** 3 tracks (the same shape as the fake scene arrangeSelect). The `selected` clip is selected. */
const three = (selected: number[] = [], clicked: number | null = null) =>
  app.stateWith((s) => {
    const base = s.song.tracks[0]!;
    s.song.tracks = [
      { ...base, clips: [clip(1, "イントロ", 1, 5), clip(3, "Aメロ", 5, 9, 2, 3)] },
      { ...base, id: 2, number: 2, name: "ベース", color: 2, muted: true, clips: [clip(41, "ベース A", 5, 9)] },
      { ...base, id: 3, number: 3, name: "ドラム", color: 3, soloed: true, clips: [clip(43, "8ビート", 1, 9)] },
    ];
    s.song.playTarget = 2;
    s.song.keysTarget = 2;
    s.arrange = { selectedClips: selected, selectedClip: clicked, pianoRollClip: clicked, clipboardClips: 1, tallTrack: null };
  });

const rows = () => (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");

describe("clip name and tabs (spec 2 (Take mark and switch), mock F-Takes)", () => {
  it("the name is inside the clip. With 2 or more takes, 「2/3」 before the name. The selected clip's tab is accent", () => {
    show(takes());
    const labels = qa(".clip-label");
    expect(labels.map(text)).toEqual(["イントロ", "2/3 Aメロ"]);
    expect(labels[1]!.style.left).toBe("257px");
    expect(labels[1]!.style.top).toBe("5px");
    expect(q(".take-tag")!.classList.contains("selected")).toBe(true);
  });

  it("a clip narrower than 28 shows only a 6×6 square", () => {
    show(
      app.stateWith((s) => {
        s.song.tracks[0]!.clips = [{ ...clip(1, "サビ", 1, 2, 2, 3), end: (5 * TPQ) / 6 }];
      }),
    );
    expect(text(q(".clip-label"))).toBe("");
    expect(q(".take-square")).not.toBeNull();
  });

  it("the switch tab is just above the selected clip. ◂ ▸ send stepTake", () => {
    show(takes());
    const sw = q(".switcher")!;
    expect(text(sw)).toBe("◂ テイク 2 / 3 ▸ T");
    expect([sw.style.left, sw.style.top, sw.style.height]).toEqual(["256px", "-21px", "24px"]);
    expect(qa(".switcher .m-num").map(text)).toEqual(["2", "3", "T"]);
    pointer(sw, "pointerdown", 260, -10);
    expect(app.sent).toEqual([]);
    (sw.querySelector("[aria-label=前のテイク]") as HTMLButtonElement).click();
    (sw.querySelector("[aria-label=次のテイク]") as HTMLButtonElement).click();
    expect(app.sent).toEqual([
      { type: "stepTake", delta: -1 },
      { type: "stepTake", delta: 1 },
    ]);
  });

  it("no switch tab for a clip with 1 take, or when nothing is selected", () => {
    show(takes(1));
    expect(q(".switcher")).toBeNull();
  });
});

describe("press: choose a clip, record arm", () => {
  it("pressing a clip sends selectClip. An empty place sends null when released without moving. A selected clip is not sent", () => {
    show(takes());
    const lanes = q(".lanes")!;
    pointer(lanes, "pointerdown", 100, 30);
    pointer(lanes, "pointerup", 100, 30);
    pointer(lanes, "pointerdown", 300, 30);
    pointer(lanes, "pointerup", 300, 30);
    pointer(lanes, "pointerdown", 700, 30);
    pointer(lanes, "pointerup", 700, 30);
    expect(app.sent).toEqual([
      { type: "selectClip", clip: 1 },
      { type: "selectClip", clip: null },
    ]);
  });

  it("● selects that track (record arm always follows the selected track). On is rec. M and S can be pressed too, on is on (accent)", () => {
    show(three());
    const arm = qa("button[aria-label=録音待機]");
    expect(arm.map((b) => b.getAttribute("aria-pressed"))).toEqual(["false", "true", "false"]);
    expect(arm.map((b) => b.classList.contains("on"))).toEqual([false, true, false]);
    arm[0]!.click();
    const mute = qa("button[aria-label=ミュート]");
    const solo = qa("button[aria-label=ソロ]");
    expect(mute.map((b) => b.classList.contains("on"))).toEqual([false, true, false]);
    expect(solo.map((b) => b.classList.contains("on"))).toEqual([false, false, true]);
    expect(mute[1]!.getAttribute("aria-pressed")).toBe("true");
    expect(mute[0]!.hasAttribute("aria-disabled")).toBe(false);
    mute[0]!.click();
    solo[2]!.click();
    expect(app.sent).toEqual([
      { type: "selectTrack", track: 1, fromArm: true },
      { type: "toggleMute", track: 1 },
      { type: "toggleSolo", track: 3 },
    ]);
  });

  it("M, S and ● can be pressed while recording and during count-in (A-14 made mute accepted)", () => {
    for (const state of ["recording", "countIn"] as const) {
      const s = three();
      s.transport.state = state;
      (mounted ? show2 : show)(s);
      const mute = qa("button[aria-label=ミュート]");
      expect(mute.map((b) => b.getAttribute("aria-disabled"))).toEqual([null, null, null]);
      expect(mute.map((b) => b.classList.contains("live"))).toEqual([true, true, true]);
      mute[1]!.click();
      qa("button[aria-label=ソロ]")[0]!.click();
      qa("button[aria-label=録音待機]")[1]!.click();
      expect(app.sent).toEqual([
        { type: "toggleMute", track: 2 },
        { type: "toggleSolo", track: 1 },
        { type: "selectTrack", track: 2, fromArm: true },
      ]);
      expect(qa("button[aria-label=ソロ]")[0]!.classList.contains("live")).toBe(true);
      app.sent.length = 0;
    }
  });
});

describe("selected track (B-70)", () => {
  it("the playTarget head is selected (bg gray.4). Pressing and releasing a head sends selectTrack; not over M, S, ● or for the selected track", () => {
    show(three());
    const heads = qa("[data-test=track-head]");
    expect(heads.map((h) => h.classList.contains("selected"))).toEqual([false, true, false]);
    for (const el of [heads[0]!, heads[1]!, qa("button[aria-label=ミュート]")[2]!]) {
      pointer(el, "pointerdown", 10, 10);
      pointer(el, "pointerup", 10, 10);
    }
    expect(app.sent).toEqual([{ type: "selectTrack", track: 1, fromArm: false }]);
  });
});

describe("select and move many clips (B-71)", () => {
  it("all selected clips have the selected tab. The take switch tab is only for selectedClip", () => {
    show(three([3, 41], 41));
    expect(qa(".take-tag.selected").length).toBe(1);
    expect(q(".switcher")).toBeNull();
    show2(three([3, 41], 3));
    expect(q(".switcher")).not.toBeNull();
  });

  it("Shift + press sends selectClips add. Releasing one of the selected clips without moving sends selectClip for it alone", () => {
    show(three([3], 3));
    const lanes = q(".lanes")!;
    pointer(lanes, "pointerdown", 300, 100, { shiftKey: true });
    pointer(lanes, "pointerup", 300, 100, { shiftKey: true });
    expect(app.sent).toEqual([{ type: "selectClips", clips: [41], mode: "add" }]);
    app.sent.length = 0;
    show2(three([3, 41], 41));
    const lanes2 = q(".lanes")!;
    pointer(lanes2, "pointerdown", 300, 30);
    pointer(lanes2, "pointerup", 300, 30);
    expect(app.sent).toEqual([{ type: "selectClip", clip: 3 }]);
  });

  it("a box drawn from an empty place sends selectClips (add with Shift). The frame is the accent box-frame. Esc stops it", () => {
    show(three([1], 1));
    const lanes = q(".lanes")!;
    // From the empty bar 3 of row 2 (bass) to bar 6 of row 1. Covers the intro, 「Aメロ」 and 「ベース A」.
    pointer(lanes, "pointerdown", 140, 100);
    pointer(lanes, "pointermove", 330, 30);
    const box = q("[data-test=arrange-box]")!;
    expect([box.style.left, box.style.top, box.style.width, box.style.height]).toEqual(["140px", "30px", "190px", "70px"]);
    pointer(lanes, "pointerup", 330, 30);
    expect(q("[data-test=arrange-box]")).toBeNull();
    pointer(lanes, "pointerdown", 140, 100, { shiftKey: true });
    pointer(lanes, "pointermove", 330, 30, { shiftKey: true });
    pointer(lanes, "pointerup", 330, 30, { shiftKey: true });
    pointer(lanes, "pointerdown", 140, 100);
    pointer(lanes, "pointermove", 330, 30);
    expect(cancelDrag()).toBe(true);
    pointer(lanes, "pointerup", 330, 30);
    expect(app.sent).toEqual([
      { type: "selectClips", clips: [1, 3, 41], mode: "replace" },
      { type: "selectClips", clips: [1, 3, 41], mode: "add" },
    ]);
  });

  it("grabbing a selected clip sends moveClips for all selected. Every old place gets a gray.7 frame", () => {
    show(three([3, 41], 41));
    const lanes = q(".lanes")!;
    pointer(lanes, "pointerdown", 300, 100);
    pointer(lanes, "pointermove", 364, 100);
    expect(qa(".drag-clip").length).toBe(1);
    expect(qa(".clip-label").filter((l) => l.style.transform).map(text)).toEqual(["2/3 Aメロ", "ベース A"]);
    pointer(lanes, "pointerup", 364, 100);
    expect(app.sent).toEqual([{ type: "moveClips", clips: [3, 41], ticks: BAR, tracks: 0, copy: false }]);
  });

  it("grabbing a clip that is not selected selects and moves only it", () => {
    show(three([3, 41], 41));
    const lanes = q(".lanes")!;
    pointer(lanes, "pointerdown", 100, 170);
    pointer(lanes, "pointermove", 100, 100);
    pointer(lanes, "pointerup", 100, 100);
    expect(app.sent[0]).toEqual({ type: "selectClip", clip: 43 });
    // The drum 8-beat (1–9) to the bass row: it overlaps 「ベース A」, so nothing is sent.
    expect(app.sent).toHaveLength(1);
  });
});

describe("ruler (the owner's decision: the loop is a drag on the ruler)", () => {
  it("while dragging, only the look moves first; releasing sends setLoop", () => {
    show(takes());
    const ruler = q(".ruler.bars")!;
    pointer(ruler, "pointerdown", 256);
    pointer(ruler, "pointermove", 512);
    expect(text(q(".head .row:nth-child(2)"))).toBe("マーカー ・ ループ 5–9");
    expect([q(".loop-mark")!.style.left, q(".loop-mark")!.style.width]).toEqual(["256px", "256px"]);
    pointer(ruler, "pointerup", 512);
    expect(app.sent).toEqual([{ type: "setLoop", fromBar: 5, toBar: 9 }]);
  });

  it("a plain press sends locate. Snaps to the lines shown on the ruler (bars by default); with Shift it does not snap (B-61)", () => {
    show(takes());
    const ruler = q(".ruler.bars")!;
    pointer(ruler, "pointerdown", 300);
    pointer(ruler, "pointerup", 301);
    pointer(ruler, "pointerdown", 300, 10, { shiftKey: true });
    pointer(ruler, "pointerup", 301, 10, { shiftKey: true });
    expect(app.sent).toEqual([
      { type: "locate", tick: 5 * BAR },
      { type: "locate", tick: (300 * BAR) / 64 },
    ]);
    expect(q(".loop-mark")).toBeNull();
  });

  it("with the loop on, accent on the top edge of the ruler and 「マーカー ・ ループ 5–9」. Not shown when off", () => {
    const on = takes();
    on.song.loopRange = { start: 4 * BAR, end: 8 * BAR };
    on.song.loopOn = true;
    show(on);
    expect(text(q(".head .row:nth-child(2)"))).toBe("マーカー ・ ループ 5–9");
    expect(q(".loop-mark")).not.toBeNull();
    expect(text(q(".loop-range"))).toBe("5–9");
    app.setState({ ...on, song: { ...on.song, loopOn: false } });
    flushSync();
    expect(text(q(".head .row:nth-child(2)"))).toBe("マーカー");
    expect(q(".loop-mark")).toBeNull();
  });
});

describe("context menu (B-60)", () => {
  const ctx = (el: HTMLElement, x: number, y: number, more: MouseEventInit = {}) => {
    const ev = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 2, ...more });
    el.dispatchEvent(ev);
    flushSync();
    return ev;
  };
  afterEach(() => (contextMenu.open = null));

  it("clip: selects it first if not selected. Rows and keys on the right. With 1 take, next and previous take are disabled", () => {
    show(three([3], 3));
    const ev = ctx(q(".lanes")!, 300, 100);
    expect(ev.defaultPrevented).toBe(true);
    expect(app.sent).toEqual([{ type: "selectClip", clip: 41 }]);
    expect(contextMenu.open?.entries.map((e) => (e.kind === "row" ? e.label : "-"))).toEqual([
      "menu.openInPianoRoll",
      "-",
      "menu.cut",
      "menu.copy",
      "menu.paste",
      "menu.duplicate",
      "menu.repeatClips",
      "menu.delete",
      "-",
      "menu.splitHere",
      "menu.join",
      "menu.renameClip",
      "-",
      "menu.nextTake",
      "menu.prevTake",
      "menu.deleteTake",
      "menu.deleteOtherTakes",
      "-",
      "menu.bounceClips",
    ]);
    expect(rows().map((r) => r.key)).toEqual(["E", "Ctrl X", "Ctrl C", "Ctrl V", "Ctrl D", "Ctrl R", "Delete", undefined, "Ctrl J", "F2", "T", "Shift T", undefined, undefined, undefined]);
    // Join needs 2 or more selected (the right click selected only 41 again).
    expect(rows().map((r) => r.usable)).toEqual([true, true, true, true, true, true, true, true, false, true, false, false, false, false, true]);
  });

  it("the clip rows act on the selection at the time of the press. A selected clip is not selected again", () => {
    show(three([3, 41], 41));
    ctx(q(".lanes")!, 300, 30);
    expect(app.sent).toEqual([]);
    for (const r of rows()) r.run();
    expect(app.sent).toEqual([
      { type: "toggleBand", band: "pianoRoll" },
      { type: "cutClips", clips: [3, 41] },
      { type: "copyClips", clips: [3, 41] },
      { type: "pasteClips", to: null },
      { type: "duplicateClips", clips: [3, 41] },
      { type: "deleteSelected" },
      // Cut here: the right-clicked place (bar 4.69) snaps to the lines shown on the ruler (bars). Only the right-clicked clip is cut.
      { type: "splitClipAt", clip: 3, tick: 5 * BAR },
      { type: "joinClips", clips: [3, 41] },
      { type: "stepTake", delta: 1 },
      { type: "stepTake", delta: -1 },
      { type: "deleteTake", clip: 3 },
      { type: "deleteOtherTakes", clip: 3 },
      // 「音のクリップにする」 (A7; clips of two tracks: disabled, and Rust does nothing with them).
      { type: "bounceToAudio", clips: [3, 41] },
    ]);
  });

  it("while recording, rows that change the song are disabled (open in piano roll and copy still work)", () => {
    const s = three([3], 3);
    s.transport.state = "recording";
    show(s);
    ctx(q(".lanes")!, 300, 30);
    expect(rows().map((r) => r.usable)).toEqual([true, false, true, false, false, false, false, false, false, false, false, false, false, false, false]);
  });

  it("empty place: paste. The pressed place snaps to the grid (1 bar); not with Shift", () => {
    show(three());
    ctx(q(".lanes")!, 600, 100);
    expect(rows().map((r) => [r.label, r.key, r.usable])).toEqual([
      ["menu.paste", "Ctrl V", true],
      ["menu.createClipHere", undefined, true],
    ]);
    rows()[0]!.run();
    ctx(q(".lanes")!, 600, 100, { shiftKey: true });
    rows()[0]!.run();
    // Make a clip here: the pressed tick as it is (Rust snaps it to the bar).
    rows()[1]!.run();
    expect(app.sent).toEqual([
      { type: "pasteClips", to: { track: 2, tick: 9 * BAR } },
      { type: "pasteClips", to: { track: 2, tick: (600 * BAR) / 64 } },
      { type: "createClip", track: 2, tick: (600 * BAR) / 64 },
    ]);
  });

  it("track head (mock R6-Tracks): mute, solo, record arm / bounce (A7) / name, duplicate (A3), color, up, down / delete. Sends selectTrack first if not selected", () => {
    show(three());
    ctx(qa("[data-test=track-head]")[0]!, 10, 10);
    expect(app.sent).toEqual([{ type: "selectTrack", track: 1, fromArm: false }]);
    expect(rows().map((r) => [r.label, r.key, r.usable, r.checked])).toEqual([
      ["menu.openInPianoRoll", "E", true, undefined],
      ["menu.mute", "Shift M", true, false],
      ["menu.solo", "Shift S", true, false],
      ["menu.arm", undefined, true, true],
      ["menu.bounceTrack", undefined, true, undefined],
      ["menu.renameTrack", "F2", true, undefined],
      ["menu.duplicateTrack", undefined, true, undefined],
      ["menu.trackColor", undefined, true, undefined],
      ["menu.moveTrackUp", undefined, false, undefined],
      ["menu.moveTrackDown", undefined, true, undefined],
      ["menu.deleteTrack", undefined, true, undefined],
    ]);
    expect(contextMenu.open!.entries.map((e) => e.kind)).toEqual(["row", "separator", "row", "row", "row", "separator", "row", "separator", "row", "row", "row", "row", "row", "separator", "row"]);
    const colors = rows().find((r) => r.label === "menu.trackColor")!.sub!.filter((e) => e.kind === "row");
    expect(colors.map((r) => [r.text, r.swatch, r.checked])).toEqual([
      ["色 1", 1, true],
      ["色 2", 2, false],
      ["色 3", 3, false],
      ["色 4", 4, false],
      ["色 5", 5, false],
    ]);
    for (const r of rows().slice(1, 4)) r.run();
    colors[2]!.run();
    for (const r of rows().slice(8).filter((r) => r.usable)) r.run();
    expect(app.sent.slice(1)).toEqual([
      { type: "toggleMute", track: 1 },
      { type: "toggleSolo", track: 1 },
      { type: "selectTrack", track: 1, fromArm: true },
      { type: "setTrackColor", track: 1, color: 3 },
      { type: "moveTrack", track: 1, to: 1 },
      { type: "deleteTrack", track: 1 },
    ]);
  });
});

describe("mark of the band that keys go to (B-72)", () => {
  it("across the whole head row and ruler (one, just below the band's box)", () => {
    show(takes());
    const mark = q("[data-test=arrange-focus-mark]")!;
    expect(mark.parentElement!.classList.contains("arrange")).toBe(true);
  });
});

describe("horizontal scroll", () => {
  it("Shift+wheel and the horizontal wheel move the ruler and clips together. Never left of 0", () => {
    const s = takes();
    s.song.tracks[0]!.clips.push(clip(5, "サビ", 30, 34));
    show(s);
    const time = q(".time")!;
    time.dispatchEvent(
      new WheelEvent("wheel", {
        bubbles: true,
        cancelable: true,
        deltaY: 640,
        shiftKey: true,
      }),
    );
    flushSync();
    expect(text(q(".bar-number"))).toBe("11");
    expect(qa(".clip-label")).toHaveLength(0);
    time.dispatchEvent(
      new WheelEvent("wheel", {
        bubbles: true,
        cancelable: true,
        deltaX: -2000,
      }),
    );
    flushSync();
    expect(text(q(".bar-number"))).toBe("1");
    expect(qa(".clip-label")).toHaveLength(2);
  });
});

describe("while recording (mock F-ArrangeRec)", () => {
  const recording = () =>
    app.stateWith((s) => {
      s.song.tracks[0]!.clips = [clip(1, "イントロ", 1, 5), clip(3, "ピアノ", 5, 7)];
      s.transport.state = "recording";
      s.transport.position = 4 * BAR;
      s.transport.recording = {
        id: 1,
        track: 1,
        from: 4 * BAR,
        take: 2,
        clip: 3,
        lap: 0,
        overdub: false,
      };
    });

  it("the clip name place shows 「● 録音中 テイク 2」. Names of clips underneath are hidden. The line is rec", () => {
    show(recording());
    expect(qa(".clip-label").map(text)).toEqual(["イントロ", "● 録音中 テイク 2"]);
    expect(q(".clip-label.recording")).not.toBeNull();
    expect(q(".play-line")!.classList.contains("rec")).toBe(true);
    expect(q(".rec-tail")!.dataset.glSlot).toBe("rects");
    expect(q(".rec-tail")!.parentElement!.dataset.glRegion).toBe("recordingClip");
  });

  it("when stopped, the line is accent at transport.position", () => {
    show(takes());
    const line = q(".play-line")!;
    expect(line.classList.contains("rec")).toBe(false);
    expect(line.style.transform).toBe("translateX(0px)");
    app.setState({
      ...takes(),
      transport: { ...takes().transport, position: 4 * BAR },
    });
    flushSync();
    expect(line.style.transform).toBe("translateX(256px)");
  });
});

describe("overview (height under 136)", () => {
  it("clip lines (rects), the frame of the visible range, and the play position line", () => {
    const s = takes();
    s.workspace.layout = {
      ...s.workspace.layout,
      arrangeOverview: true,
      arrangeHeight: 40,
    };
    show(s);
    expect(q(".overview-slot")!.dataset.glRegion).toBe("overview");
    expect(qa(".overview-slot [data-gl-slot]").map((e) => e.dataset.glSlot)).toEqual(["rects", "overview"]);
    expect(q(".overview .play-line")).not.toBeNull();
  });
});
