import { afterEach, describe, expect, it, vi } from "vitest";
import { FakeBands, SCENES, encodeTelemetry, fakeScene, fakeState, unfitScales } from "../src/bridge/fake";
import { decodeTelemetry, keyIsDown } from "../src/bridge/telemetry";
import type { AppState, Command } from "../src/bridge";
import { bandCountsShown, bandTabWidth, toolbarParts, transportParts, type TransportParts, type TransportWidths } from "../src/components/fit";
import { PeakHold, dbNumber, msText, peakNumberText, peakText, textParts } from "../src/components/format";
import { mock, tokensAt } from "../src/design/tokens";
import { handleKeydown, handlePointerdown } from "../src/keys/attach";
import { hideTip, term, tipVisible } from "../src/lib/tip.svelte";

// docs/ is outside ui/, so read it with Node's fs, as design-sources.ts does.
const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): { readFileSync(p: URL, e: "utf8"): string } } }).process.getBuiltinModule("node:fs");
const here = import.meta.url;
const readMock = (board: string) => fs.readFileSync(new URL(`../../docs/design/mock/${board}.dc.html`, here), "utf8");

/** A transport without keys (key width 0): the set of the other items. */
function withoutKeys(room: number, w: Omit<TransportWidths, "keys">, stubs = {}): Omit<TransportParts, "keys"> {
  const { keys: _keys, ...rest } = transportParts(room, { ...w, keys: 0 }, stubs);
  return rest;
}

describe("how the transport hides things (spec 1 (Rules for the scale) (1), measured widths): keep ▾", () => {
  const w = { base: 800, songName: 144, load: 60, countIn: 227, positionLabels: 20 };
  const stubs = { songName: 40, countIn: 20 };
  const all = w.base + w.songName + w.load + w.countIn + w.positionLabels;
  const at = (room: number) => withoutKeys(room, w, stubs);

  it("the song name and the count-in lose less width, by the ▾ and the metronome ▾", () => {
    expect(at(all - 1)).toEqual({ songName: false, load: true, countIn: true, positionLabels: true });
    const afterName = all - w.songName + stubs.songName;
    expect(at(afterName)).toEqual({ songName: false, load: true, countIn: true, positionLabels: true });
    expect(at(afterName - 1)).toEqual({ songName: false, load: false, countIn: true, positionLabels: true });
    const afterLoad = afterName - w.load;
    expect(at(afterLoad - 1)).toEqual({ songName: false, load: false, countIn: false, positionLabels: true });
    expect(at(afterLoad - w.countIn + stubs.countIn)).toEqual({ songName: false, load: false, countIn: false, positionLabels: true });
    expect(at(afterLoad - w.countIn + stubs.countIn - 1)).toEqual({ songName: false, load: false, countIn: false, positionLabels: false });
  });
});

describe("how the transport hides things (spec 1 (Rules for the scale) (1), measured widths)", () => {
  const w = { base: 800, songName: 144, load: 60, countIn: 227, positionLabels: 20 };
  const all = w.base + w.songName + w.load + w.countIn + w.positionLabels;
  const at = (room: number) => withoutKeys(room, w);

  it("show all when they fit", () => {
    expect(at(all)).toEqual({ songName: true, load: true, countIn: true, positionLabels: true });
  });

  it("hide the song name, CPU load, count-in and position label in that order", () => {
    expect(at(all - 1)).toEqual({ songName: false, load: true, countIn: true, positionLabels: true });
    expect(at(all - w.songName - 1)).toEqual({ songName: false, load: false, countIn: true, positionLabels: true });
    expect(at(all - w.songName - w.load - 1)).toEqual({ songName: false, load: false, countIn: false, positionLabels: true });
    expect(at(w.base)).toEqual({ songName: false, load: false, countIn: false, positionLabels: false });
    expect(at(10)).toEqual({ songName: false, load: false, countIn: false, positionLabels: false });
  });
});

describe("keys at the right end of buttons (spec 4, B-73): hidden before anything else", () => {
  // The widths are examples (rough sizes at 100 % and 150 %). In the real screen, that Main, F-PR720, F-PR150 hide the same as before was checked by shooting the fake scenes.
  const cases = [
    { name: "Main（100 %）", w: { base: 905, songName: 156, load: 64, countIn: 160, positionLabels: 60 }, keys: 146 },
    { name: "F-PR150（150 %）", w: { base: 1180, songName: 228, load: 92, countIn: 238, positionLabels: 86 }, keys: 219 },
  ];
  const stubs = { songName: 40, countIn: 20 };

  it.each(cases)("$name: at every width, shows the same as the form without keys. Keys show only when everything else fits", ({ w, keys }) => {
    const all = w.base + w.songName + w.load + w.countIn + w.positionLabels;
    for (let room = 0; room <= all + keys + 10; room += 1) {
      const got = transportParts(room, { ...w, keys }, stubs);
      const { keys: shownKeys, ...rest } = got;
      expect(rest, `room ${room}`).toEqual(withoutKeys(room, w, stubs));
      expect(shownKeys, `room ${room}`).toBe(room >= all + keys);
    }
  });

  it("piano roll toolbar: keys, then the clip text. The clip text hides only when it does not fit even without keys", () => {
    const w = { base: 700, keys: 40, info: 300 };
    expect(toolbarParts(1040, w)).toEqual({ keys: true, info: true });
    expect(toolbarParts(1039, w)).toEqual({ keys: false, info: true });
    expect(toolbarParts(1000, w)).toEqual({ keys: false, info: true });
    expect(toolbarParts(999, w)).toEqual({ keys: false, info: false });
    expect(toolbarParts(10, w)).toEqual({ keys: false, info: false });
  });
});

describe("band tab counts (spec 3: drop from the counts of the right bands)", () => {
  const tabs = [
    bandTabWidth("アレンジ", "トラック 1", "A"),
    bandTabWidth("ピアノロール", "ノート 0", "E"),
    bandTabWidth("ミキサー", "マスター 0.0 dB", "M"),
    bandTabWidth("取り消しの履歴", "0 手", "H"),
  ];
  const full = tabs.reduce((s, t) => s + t.fixed + t.count, 0);

  it("all when they fit", () => {
    expect(bandCountsShown(full, 100, tabs)).toEqual([true, true, true, true]);
  });
  it("drop from the right. The name and key parts are not dropped", () => {
    expect(bandCountsShown(full - 1, 100, tabs)).toEqual([true, true, true, false]);
    expect(bandCountsShown(full - tabs[3]!.count - 1, 100, tabs)).toEqual([true, true, false, false]);
    expect(bandCountsShown(1, 100, tabs)).toEqual([false, false, false, false]);
  });
  it("counts the right-end hint part and the scale too", () => {
    expect(bandCountsShown(full + 100, 100, tabs, 100)).toEqual([true, true, true, true]);
    expect(bandCountsShown(full, 150, tabs)).toContain(false);
  });
});

describe("how numbers are shown", () => {
  it("dB in 0.1 steps, minus is −", () => {
    expect(dbNumber(0)).toBe("0.0");
    expect(dbNumber(-0.01)).toBe("0.0");
    expect(dbNumber(-4.84)).toBe("−4.8");
    expect(peakText(1)).toBe("0.0 dB");
    expect(peakText(10 ** (-8.4 / 20))).toBe("−8.4 dB");
    expect(peakText(10 ** (-89.9 / 20))).toBe("−89.9 dB");
    expect(peakText(10 ** (-90.1 / 20))).toBe("−∞ dB");
    expect(peakText(0)).toBe("−∞ dB");
    expect(peakNumberText(10 ** (-6.2 / 20))).toBe("−6.2");
    expect(peakNumberText(0)).toBe("−∞");
  });
  it("the largest value in the last 1 second", () => {
    const h = new PeakHold(1000);
    h.push(0.5, 0);
    h.push(0.2, 500);
    expect(h.max(900)).toBe(0.5);
    expect(h.max(1200)).toBe(0.2);
    expect(h.max(1600)).toBe(0);
  });
  it("latency ms: under 20 ms with 1 decimal, 20 ms and up as an integer (「約 53 ms」「約 13.5 ms」in spec 6)", () => {
    expect(msText(13.3333)).toBe("13.3");
    expect(msText(13.54)).toBe("13.5");
    expect(msText(13)).toBe("13.0");
    expect(msText(5)).toBe("5.0");
    expect(msText(19.96)).toBe("20");
    expect(msText(20)).toBe("20");
    expect(msText(53.3333)).toBe("53");
  });
  it("text holes", () => {
    expect(textParts("status.bufferMismatch", { actual: 1024, requested: 256 })).toEqual([
      { text: "バッファ ", hole: null },
      { text: "1024", hole: "actual" },
      { text: " ・ 頼んだ数 ", hole: null },
      { text: "256", hole: "requested" },
    ]);
  });
});

describe("mock.* (sizes only in the mock)", () => {
  // Checks only what this screen frame added (mock.* of other parts are in those parts' tests).
  const mine = Object.entries(mock).filter(([k]) => /^mock\.(transport|arrange|track|overview)\./.test(k));
  it("there is something to check", () => expect(mine.length).toBe(13));
  it.each(mine)("the value of %s is in the mock, and the scaled value is an integer (rounded)", (_, tok) => {
    const html = readMock(tok.board);
    expect(html).toContain(`${tok.value}px`);
    for (const s of [100, 125, 150] as const) expect(Number.isInteger(tokensAt(s, 1)[_] as number)).toBe(true);
  });
});

describe("fake scenes", () => {
  // Gives the same numbers as crates/meloet-core/src/layout.rs.
  it("window 800, piano roll: slim input meter, arrange 40, the roll takes the rest", () => {
    const l = fakeScene("pianoRoll")!.state.workspace.layout;
    expect(l).toMatchObject({ meter: "slim", arrangeHeight: 40, arrangeOverview: true, showBackHint: true });
    expect(l.lower).toEqual([{ band: "pianoRoll", height: 800 - 24 - 48 - 24 - 24 - 24 - 40 }]);
  });
  it("in a window of 800 the mixer makes the arrange 40 and takes the rest. History is 248 and the rest goes to the arrange", () => {
    expect(fakeScene("mixer")!.state.workspace.layout).toMatchObject({ arrangeHeight: 40, lower: [{ band: "mixer", height: 656 - 40 }] });
    expect(fakeScene("history")!.state.workspace.layout).toMatchObject({ arrangeHeight: 656 - 248, lower: [{ band: "history", height: 248 }] });
  });
  // spec 3 (Examples of splitting the height). order is oldest first. Heights are the arrange, then the bands (from the top).
  it.each([
    { order: ["pianoRoll"], fits: 476, grows: 944, rows: 1040, at800: [40, 616], at1080: [176, 760] },
    { order: ["mixer"], fits: 700, grows: 844, rows: 940, at800: [40, 616], at1080: [276, 660] },
    { order: ["history"], fits: 432, grows: 432, rows: 528, at800: [408, 248], at1080: [688, 248] },
    { order: ["mixer", "pianoRoll"], fits: 992, grows: 1604, rows: 1700, at800: null, at1080: [40, 380, 516] },
    { order: ["history", "mixer"], fits: 948, grows: 1092, rows: 1188, at800: null, at1080: [40, 648, 248] },
  ] as const)("縦の分けかたの例: $order", ({ order, fits, grows, rows, at800, at1080 }) => {
    const at = (height: number) => {
      const b = new FakeBands(height);
      for (const band of order) b.toggle(band);
      return b.layout();
    };
    const heights = (l: ReturnType<typeof at>) => [l.arrangeHeight, ...l.lower.map((x) => x.height)];
    expect(at(fits).lower).toHaveLength(order.length);
    expect(at(fits).arrangeHeight).toBe(40);
    if (order.length > 1) expect(at(fits - 1).lower).toHaveLength(1);
    expect(at(grows).arrangeHeight).toBe(40);
    expect(at(grows + 1).arrangeHeight).toBe(41);
    expect(at(rows)).toMatchObject({ arrangeHeight: 136, arrangeOverview: false });
    expect(at(rows - 1).arrangeOverview).toBe(true);
    if (at800) expect(heights(at(800))).toEqual(at800);
    else expect(at(800).lower).toHaveLength(1);
    expect(heights(at(1080))).toEqual(at1080);
    expect(at(1080).meter).toBe("slim");
  });
  it("a taller window keeps the band set, and no height shrinks", () => {
    for (const order of [["pianoRoll"], ["mixer", "pianoRoll"], ["pianoRoll", "mixer"], ["history", "mixer"], ["pianoRoll", "history"]] as const) {
      const b = new FakeBands(944);
      for (const band of order) b.toggle(band);
      let last = b.layout();
      for (let h = 945; h <= 2000; h++) {
        expect(b.setHeight(h)).toBeNull();
        const l = b.layout();
        expect(l.lower.map((x) => x.band)).toEqual(last.lower.map((x) => x.band));
        expect(l.arrangeHeight).toBeGreaterThanOrEqual(last.arrangeHeight);
        l.lower.forEach((x, i) => expect(x.height).toBeGreaterThanOrEqual(last.lower[i]!.height));
        last = l;
      }
    }
  });
  it("tells which band was folded and why. If a third band pushes one out and another is folded because it still does not fit, the later one", () => {
    const low = new FakeBands(800);
    expect(low.toggle("mixer")).toBeNull();
    expect(low.toggle("pianoRoll")).toEqual({ band: "mixer", why: "tooLow" });
    const tall = new FakeBands(1080);
    tall.toggle("history");
    tall.toggle("mixer");
    expect(tall.toggle("pianoRoll")).toEqual({ band: "history", why: "third" });
    const mid = new FakeBands(960);
    mid.toggle("history");
    mid.toggle("mixer");
    expect(mid.toggle("pianoRoll")).toEqual({ band: "mixer", why: "tooLow" });
    expect(tall.setHeight(967)).toBe("mixer");
    expect(tall.setHeight(2000)).toBeNull();
    expect(tall.layout().lower.map((x) => x.band)).toEqual(["pianoRoll"]);
  });
  it("scales that do not fit: on a 1200×700 screen only 150. 100 always fits", () => {
    expect(unfitScales({ width: 1200, height: 700 })).toEqual([150]);
    expect(unfitScales({ width: 1280, height: 672 })).toEqual([]);
    expect(unfitScales({ width: 800, height: 400 })).toEqual([125, 150]);
    const zoom = fakeScene("zoom")!.state.workspace;
    expect(zoom).toMatchObject({ scale: 125, unfitScales: [150] });
  });
  it("scene with a folded band: only the piano roll opens, and the status line tells", () => {
    const s = fakeScene("bandFolded")!.state;
    expect(s.workspace.layout.lower.map((x) => x.band)).toEqual(["pianoRoll"]);
    expect(s.status.message).toEqual({ id: "bandFoldedToOpen", opened: "pianoRoll", folded: "mixer" });
  });
  it("with only the arrange, the input meter is full and the arrange takes the rest", () => {
    expect(fakeScene("empty")!.state.workspace.layout).toMatchObject({ meter: "full", arrangeHeight: 800 - 24 - 48 - 52 - 24 - 24, lower: [] });
  });
  it("the scale is decided after turning into 100 % px", () => {
    const l = fakeScene("empty", 125, { width: 1280, height: 800 })!.state.workspace.layout;
    expect(l.arrangeHeight).toBe(640 - 24 - 48 - 52 - 24 - 24);
  });
  it("at most 2 lower bands. If they do not fit, the older one folds", () => {
    const b = new FakeBands(800);
    b.toggle("pianoRoll");
    b.toggle("history");
    expect(b.layout().lower.map((x) => x.band)).toEqual(["pianoRoll", "history"]);
    b.toggle("history");
    b.toggle("mixer");
    expect(b.layout().lower.map((x) => x.band)).toEqual(["mixer"]);
    const tall = new FakeBands(1400);
    tall.toggle("mixer");
    tall.toggle("history");
    tall.toggle("pianoRoll");
    expect(tall.layout().lower.map((x) => x.band)).toEqual(["pianoRoll", "history"]);
  });
  it("in the right panel, the one opened later is in front", () => {
    const b = new FakeBands(800);
    b.toggle("shelf");
    b.toggle("params");
    expect(b.layout()).toMatchObject({ rightOpen: ["shelf", "params"], rightFront: "params" });
    b.back();
    expect(b.layout().rightFront).toBe("shelf");
  });
  it("every scene can be made", () => {
    for (const name of Object.keys(SCENES)) expect(fakeScene(name), name).not.toBeNull();
    expect(fakeScene("nope")).toBeNull();
    const c2 = fakeScene("connect2")!.state;
    expect(c2.outputs.list.map((o) => o.unusable)).toEqual([
      null,
      null,
      "switchedByPc",
      ...Array(7).fill("stoppedByPc"),
    ]);
    expect(c2.workspace.chooser).toEqual({ highlighted: "out-headphones" });
  });
  it("the telemetry binary can be read with telemetry.ts", () => {
    const m = SCENES.connect3!.meters;
    const t = decodeTelemetry(encodeTelemetry(m, 7, 1234, 1));
    expect([60, 64, 67].every((n) => keyIsDown(t, n))).toBe(true);
    expect(keyIsDown(t, 61)).toBe(false);
    expect(t).toMatchObject({ seq: 7, sentUs: 1234, lastVelocity: 87, sustain: 127, modulation: 12, loadPermille: 60 });
    expect(t.masterPeak[0]).toBeCloseTo(10 ** (-8.4 / 20), 5);
    expect(t.trackPeaks).toHaveLength(2);
  });
});

describe("connecting keys to the screen", () => {
  let sent: Command[] = [];
  let state: AppState = fakeState();
  const deps = { state: () => state, send: (c: Command) => void sent.push(c), nav: { platform: "Linux x86_64" } };
  const key = (k: string, more: KeyboardEventInit = {}) => {
    const ev = new KeyboardEvent("keydown", { key: k, cancelable: true, ...more });
    handleKeydown(ev, deps);
    return ev;
  };
  afterEach(() => {
    sent = [];
    state = fakeState();
    hideTip();
    document.body.innerHTML = "";
    vi.useRealTimers();
  });

  it("A, E, M, H, P, U, Mod+, and Mod+Alt+number. P opening a closed shelf uses「新しいトラック」", () => {
    for (const k of ["a", "e", "m", "h", "p", "u"]) key(k);
    key(",", { ctrlKey: true });
    key("2", { ctrlKey: true, altKey: true, code: "Digit2" });
    expect(sent).toEqual([
      { type: "goArrange" },
      { type: "toggleBand", band: "pianoRoll" },
      { type: "toggleBand", band: "mixer" },
      { type: "toggleBand", band: "history" },
      { type: "openShelf", mode: { kind: "newTrack" } },
      { type: "toggleBand", band: "params" },
      { type: "openSettings", advanced: false },
      { type: "setScale", percent: 125 },
    ]);
  });

  const withOutput = (canRecord: boolean) => {
    state = { ...state, transport: { ...state.transport, canPlay: true, canRecord } };
  };

  it("transport keys: Space, R, Home, L, C, T, Shift T, Delete", () => {
    withOutput(true);
    const evs = [key(" ", { code: "Space" }), key("r"), key("Home"), key("l"), key("c"), key("t"), key("T", { shiftKey: true }), key("Delete")];
    expect(evs.every((e) => e.defaultPrevented)).toBe(true);
    expect(sent).toEqual([
      { type: "playStop" },
      { type: "record" },
      { type: "goToStart" },
      { type: "toggleLoop" },
      { type: "toggleMetronome" },
      { type: "stepTake", delta: 1 },
      { type: "stepTake", delta: -1 },
      { type: "deleteSelected" },
    ]);
  });

  it("while there is no output yet, Space and R do nothing. Others work", () => {
    // Even when disabled, Space does not press the focused button (only play and stop).
    expect(key(" ", { code: "Space" }).defaultPrevented).toBe(true);
    expect(key("r", { code: "KeyR" }).defaultPrevented).toBe(false);
    expect(sent).toEqual([]);
    key("Home");
    key("l");
    key("c");
    expect(sent).toEqual([{ type: "goToStart" }, { type: "toggleLoop" }, { type: "toggleMetronome" }]);
  });

  it("R sends record even with no armed track (Rust gives the reason)", () => {
    withOutput(false);
    key("r");
    expect(sent).toEqual([{ type: "record" }]);
  });

  it("while running, Space can stop even if the output is gone", () => {
    state = { ...state, transport: { ...state.transport, state: "playing", canPlay: false } };
    key(" ", { code: "Space" });
    expect(sent).toEqual([{ type: "playStop" }]);
  });

  it("Home while typing goes back to the start and moves the cursor in the box too", () => {
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    expect(key("Home").defaultPrevented).toBe(false);
    key(" ", { code: "Space" });
    key("Delete");
    expect(sent).toEqual([{ type: "goToStart" }]);
  });

  it("one-letter keys do not work while typing text", () => {
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    key("m");
    expect(sent).toEqual([]);
  });

  it("Esc: text box → term tip → Rust back", () => {
    vi.useFakeTimers();
    const span = document.createElement("span");
    const input = document.createElement("input");
    document.body.append(span, input);
    term(span, "ループ");
    span.dispatchEvent(new PointerEvent("pointerenter"));
    vi.advanceTimersByTime(600);
    input.focus();
    expect(tipVisible()).toBe(true);

    key("Escape");
    expect(document.activeElement).not.toBe(input);
    expect(tipVisible()).toBe(true);
    key("Escape");
    expect(tipVisible()).toBe(false);
    expect(sent).toEqual([]);
    key("Escape");
    expect(sent).toEqual([{ type: "back" }]);
  });

  it("↑ ↓ Enter in the output chooser, Enter in sound settings", () => {
    state = { ...state, workspace: { ...state.workspace, chooser: { highlighted: null } } };
    key("ArrowDown");
    key("ArrowUp");
    key("Enter");
    state = fakeState();
    state = { ...state, workspace: { ...state.workspace, settings: { draftOutput: null, advancedOpen: false } } };
    key("Enter");
    expect(sent).toEqual([
      { type: "chooserMove", delta: 1 },
      { type: "chooserMove", delta: -1 },
      { type: "chooserConfirm" },
      { type: "settingsApply" },
    ]);
  });

  it("「音が出ました」goes away on a key or a mouse button", () => {
    state = { ...state, workspace: { ...state.workspace, hint: "soundCameOut" } };
    key("Shift");
    expect(sent).toEqual([]);
    key("x");
    handlePointerdown(deps);
    expect(sent).toEqual([{ type: "dismissHint" }, { type: "dismissHint" }]);
  });
});
