// Round 12, A8: 「いいとこ取り」 of a vocal clip on the screen: the pieces a drag makes, the band under the track (opened
// from the clip's menu), the drag over a take's row (the screen first, one setComp on release, Esc), 「もとに戻す」, and
// what the fake does with the intents.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, AudioClipView, ClipView, Command, CompPiece, TrackView } from "../src/bridge";
import { COMP_CLIP, createFakeTransport, fakeScene } from "../src/bridge/fake";
import { fakeAudioClip, fakeVocalClip } from "../src/bridge/fakeAudio";
import { fakeCompValid, fakeCompWindow, fakeTakes } from "../src/bridge/fakeComp";
import Arrange from "../src/components/Arrange.svelte";
import { arrangeRows } from "../src/components/automation/autoGeom";
import * as C from "../src/components/comp/comp";
import { closeComp, compBand, openComp, openCompOf } from "../src/components/comp/compState.svelte";
import { waveParts } from "../src/components/arrange/wave";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import TrackHeads from "../src/components/TrackHeads.svelte";
import type { GlRect, GlSlot } from "../src/gl/slot";
import { t } from "../src/text";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  audioPeaks: () => new Promise(() => {}),
  moreAudioPeaks: () => new Promise(() => {}),
  peekAudioPeaks: () => null,
}));
// What each box asks the WebGL side to draw, by its box.
const drawn = new Map<HTMLElement, GlSlot>();
vi.mock("../src/gl/slot", async (original) => {
  const real = await original<typeof import("../src/gl/slot")>();
  return {
    ...real,
    glSlot: (node: HTMLElement, slot: GlSlot) => {
      drawn.set(node, slot);
      const held = real.glSlot(node, slot);
      return {
        update(next: GlSlot) {
          drawn.set(node, next);
          held.update(next);
        },
        destroy() {
          drawn.delete(node);
          held.destroy();
        },
      };
    },
  };
});

const BAR = 4 * TPQ;
const CLIP = 61;
/** The ids of the clip's three takes, in their order. */
const [T1, T2, T3] = [62, 63, 64];
const pieces = (...list: [number, number][]): CompPiece[] => list.map(([from, take]) => ({ from, take }));

describe("the pieces a drag makes", () => {
  const whole = pieces([0, T3]);
  it("a take chosen for a range leaves what was before and after it", () => {
    expect(C.chooseTake(whole, 4 * BAR, BAR, 2 * BAR, T1)).toEqual(pieces([0, T3], [BAR, T1], [2 * BAR, T3]));
    // From the start, to the end, all of it; backwards is the same range.
    expect(C.chooseTake(whole, 4 * BAR, 0, BAR, T1)).toEqual(pieces([0, T1], [BAR, T3]));
    expect(C.chooseTake(whole, 4 * BAR, 3 * BAR, 4 * BAR, T1)).toEqual(pieces([0, T3], [3 * BAR, T1]));
    expect(C.chooseTake(whole, 4 * BAR, 0, 4 * BAR, T1)).toEqual(pieces([0, T1]));
    expect(C.chooseTake(whole, 4 * BAR, 2 * BAR, BAR, T1)).toEqual(pieces([0, T3], [BAR, T1], [2 * BAR, T3]));
    // Outside the clip it is put inside; no length changes nothing.
    expect(C.chooseTake(whole, 4 * BAR, -BAR, 9 * BAR, T2)).toEqual(pieces([0, T2]));
    expect(C.chooseTake(whole, 4 * BAR, BAR, BAR, T1)).toEqual(whole);
  });

  it("neighbours of one take become one piece, and a piece covered whole goes", () => {
    const three = pieces([0, T1], [BAR, T3], [3 * BAR, T2]);
    // The middle piece taken by the first one's take: two pieces.
    expect(C.chooseTake(three, 4 * BAR, BAR, 3 * BAR, T1)).toEqual(pieces([0, T1], [3 * BAR, T2]));
    // Over a seam: the piece before is shorter, the one after starts later.
    expect(C.chooseTake(three, 4 * BAR, BAR / 2, 2 * BAR, T2)).toEqual(pieces([0, T1], [BAR / 2, T2], [2 * BAR, T3], [3 * BAR, T2]));
    // The take that already sounds there: nothing new.
    expect(C.chooseTake(three, 4 * BAR, BAR, 2 * BAR, T3)).toEqual(three);
    // Everything the fake and Rust take as a comp.
    for (const made of [C.chooseTake(three, 4 * BAR, BAR / 2, 2 * BAR, T2), C.chooseTake(three, 4 * BAR, 0, 4 * BAR - 1, T3)]) {
      expect(fakeCompValid(made, 4 * BAR, [T1, T2, T3])).toBe(true);
    }
  });

  it("the band has a row for each of the newest eight takes, the newest first", () => {
    const takes = Array.from({ length: 10 }, (_, i) => ({ takeId: 100 + i, number: i + 1 }) as AudioClipView["takes"][number]);
    expect(C.rowTakes(takes).map((x) => x.number)).toEqual([10, 9, 8, 7, 6, 5, 4, 3]);
    expect(C.rowTakes(takes.slice(0, 2)).map((x) => x.number)).toEqual([2, 1]);
    const d = { headH: 24, rowH: 48, pad: 4, hair: 1 };
    expect(C.bandHeight(d, 3)).toBe(24 + 3 * 48);
    expect([0, 23, 24, 71, 72, 167, 168].map((y) => C.rowAt(d, 3, y))).toEqual([-1, -1, 0, 0, 1, 2, -1]);
  });

  it("a comp cut to a clip with other edges (the fake's copy of Rust's rule)", () => {
    const comp = pieces([0, T1], [100, T2], [300, T3]);
    expect(fakeCompWindow(comp, 0, 300)).toEqual(pieces([0, T1], [100, T2]));
    expect(fakeCompWindow(comp, 0, 301)).toEqual(comp);
    expect(fakeCompWindow(comp, 40, 360)).toEqual(pieces([0, T1], [60, T2], [260, T3]));
    expect(fakeCompWindow(comp, 100, 300)).toEqual(pieces([0, T2], [200, T3]));
    expect(fakeCompWindow(comp, 350, 50)).toEqual(pieces([0, T3]));
    expect(fakeCompWindow(comp, -50, 450)).toEqual(pieces([0, T1], [150, T2], [350, T3]));
    expect(fakeCompWindow(pieces([0, T1], [100, T2], [200, T1]), 0, 100)).toEqual(pieces([0, T1]));
    expect(fakeCompWindow([], 5, 10)).toEqual([]);
  });
});

// ---- The band in the arrange ----

const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => 1000 }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  closeComp();
  drawn.clear();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  contextMenu.open = null;
  target.remove();
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target, props: {} });
  flushSync();
}

const rows = () => (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");
const row = (label: string) => rows().find((r) => r.label === label)!;

/**
 * Row 1 「ピアノ」; row 2 「歌」 with 「歌 1」 over bars 1–4 (three takes, the third shown) and 「ひとつ」 over bars 5–6 (one take).
 * Rows are 64 high, a bar 64 wide: the grid in view is the bars.
 */
const song = (comp: CompPiece[] = [], edit: (s: AppState) => void = () => {}) =>
  app.stateWith((s) => {
    const base = s.song.tracks[0]!;
    const file = { id: 7, file: "歌.wav", sampleRate: 48000, bits: 24, channels: 1, frames: 8 * 48000 };
    const audio = fakeAudioClip(file);
    audio.takes = fakeTakes(T1, [file, file, file], audio);
    const clip: ClipView = { ...fakeVocalClip(CLIP, "歌 1", 0, 4 * BAR, audio), take: 3, takes: 3, takeId: T3, comp };
    const vocal: TrackView = {
      ...base,
      id: 3,
      number: 2,
      name: "歌",
      kind: { kind: "vocal", input: "mono1", inputReady: true, armed: false, monitor: false },
      clips: [clip, fakeVocalClip(71, "ひとつ", 4 * BAR, 6 * BAR, fakeAudioClip(file))],
    };
    s.song.tracks = [{ ...base, clips: [] }, vocal];
    s.arrange = { selectedClips: [CLIP], selectedClip: CLIP, pianoRollClip: null, clipboardClips: 0, clipboardKind: null, tallTrack: null };
    edit(s);
  });

function rightClick(x: number, y: number): void {
  target.querySelector(".lanes")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 2 }));
  flushSync();
}

const band = () => target.querySelector<HTMLElement>("[data-test=comp-band]");
/** A pointer event on the band; y from the band's top (the heading is 24 high, then rows of 48, the newest take first). */
function pointer(type: string, x: number, y: number, more: PointerEventInit = {}): void {
  band()!.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, clientX: x, clientY: y, ...more }));
  flushSync();
}
const ROW_T3 = 24 + 10;
const ROW_T1 = 24 + 2 * 48 + 10;
/** The accent parts of the band, as [x, y, w] in px, and the gray.7 lines. */
const bandRects = () => (drawn.get(band()!) as Extract<GlSlot, { kind: "rects" }>).rects;
const accent = () => bandRects().filter((r: GlRect) => r.color === "color.accent.ink").map((r) => [r.x, r.y, r.w]);
const frames = () => bandRects().filter((r: GlRect) => r.color === "color.gray.7").length;

describe("the row of the clip's menu", () => {
  it("opens the band under the track, a row for each take, and closes it again", () => {
    show(Arrange, song());
    expect(band()).toBeNull();
    rightClick(20, 64 + 30);
    expect([row("menu.comp").usable, row("menu.comp").checked]).toEqual([true, false]);
    row("menu.comp").run();
    flushSync();
    expect(band()).not.toBeNull();
    expect(app.sent.filter((c) => c.type === "setComp")).toEqual([]);
    expect(band()!.querySelector("[data-test=comp-clip]")!.textContent).toBe("歌 1 ・ テイク 3");
    // With no comp the shown take is chosen all along: one accent bar, in the first row, as wide as the clip.
    expect(accent()).toEqual([[0, 24 + 4, 256]]);
    // The rows below the track move down by the band: its heading and three rows.
    const s = app.current.state!;
    const d = { trackH: 64, laneH: 48, rowH: 24 };
    expect(arrangeRows(s.song.tracks, [], d, null, { track: 3, height: C.bandHeight({ headH: 24, rowH: 48 }, 3) }).height).toBe(2 * 64 + 24 + 3 * 48);

    contextMenu.open = null;
    rightClick(20, 64 + 30);
    expect(row("menu.comp").checked).toBe(true);
    row("menu.comp").run();
    flushSync();
    expect(band()).toBeNull();
  });

  it("is disabled, with the reason, for a clip of one take", () => {
    show(Arrange, song());
    rightClick(64 * 4 + 20, 64 + 30);
    expect(app.sent.at(-1)).toEqual({ type: "selectClip", clip: 71 });
    expect([row("menu.comp").usable, row("menu.comp").hint]).toEqual([false, t("comp.needsTakes")]);
  });

  it("a clip with a comp keeps its takes: 「テイクを消す」「このテイク以外を消す」「逆にする」「ノーマライズ」 are disabled, with the reason", () => {
    show(Arrange, song(pieces([0, T1], [BAR, T3])));
    rightClick(20, 64 + 30);
    for (const label of ["menu.deleteTake", "menu.deleteOtherTakes", "menu.reverse", "menu.normalize"]) {
      expect([label, row(label).usable, row(label).hint]).toEqual([label, false, t("comp.notWhileComp")]);
    }
    // Another take can still be shown: that takes the comp away (Rust, one step).
    expect(row("menu.nextTake").usable).toBe(true);
    contextMenu.open = null;
    show(Arrange, song());
    rightClick(20, 64 + 30);
    expect([row("menu.deleteTake").usable, row("menu.reverse").usable, row("menu.deleteTake").hint]).toEqual([true, true, undefined]);
    expect([row("menu.deleteOtherTakes").usable, row("menu.deleteOtherTakes").hint]).toEqual([true, undefined]);
  });

  it("the head side has the heading and the takes' names; ▾ closes the band", () => {
    openComp(CLIP);
    show(TrackHeads, song());
    const head = target.querySelector("[data-test=comp-head]")!;
    expect([...head.querySelectorAll("[data-test=comp-take]")].map((n) => n.textContent)).toEqual(["テイク 3", "テイク 2", "テイク 1"]);
    expect(head.querySelector("[data-test=comp-close]")!.textContent!.replace(/\s+/g, " ").trim()).toBe("▾ いいとこ取り");
    head.querySelector<HTMLElement>("[data-test=comp-close]")!.click();
    flushSync();
    expect(compBand.clip).toBeNull();
    expect(target.querySelector("[data-test=comp-head]")).toBeNull();
  });

  it("the band goes when its clip is gone or has one take left", () => {
    openComp(CLIP);
    const s = song();
    expect(openCompOf(s)?.takes.map((x) => x.takeId)).toEqual([T3, T2, T1]);
    expect(openCompOf(s, 71)).toBeNull();
    expect(openCompOf(s, 999)).toBeNull();
    show(Arrange, s);
    expect(band()).not.toBeNull();
    app.setState(song([], (x) => (x.song.tracks[1]!.clips = [])));
    flushSync();
    expect(band()).toBeNull();
  });
});

describe("the drag over a take's row", () => {
  const open = (comp: CompPiece[] = [], edit: (s: AppState) => void = () => {}) => {
    openComp(CLIP);
    show(Arrange, song(comp, edit));
    app.sent.length = 0;
  };

  it("only the screen changes first; one setComp on release, its ends on the bars in view", () => {
    open();
    pointer("pointerdown", 70, ROW_T1);
    pointer("pointermove", 100, ROW_T1);
    pointer("pointermove", 130, ROW_T1);
    expect(app.sent).toEqual([]);
    // Take 1 is accent over bar 2, take 3 before and after it; what was chosen before has a gray.7 frame.
    expect(accent()).toEqual([
      [0, 24 + 4, 64],
      [128, 24 + 4, 128],
      [64, 24 + 2 * 48 + 4, 64],
    ]);
    expect(frames()).toBe(4);
    pointer("pointerup", 130, ROW_T1);
    expect(app.sent).toEqual([{ type: "setComp", clip: CLIP, span: { start: 0, end: 4 * BAR }, pieces: pieces([0, T3], [BAR, T1], [2 * BAR, T3]) }]);
    expect(frames()).toBe(0);
  });

  it("with Shift the ends are where the pointer is", () => {
    open();
    pointer("pointerdown", 70, ROW_T1, { shiftKey: true });
    pointer("pointermove", 130, ROW_T1, { shiftKey: true });
    pointer("pointerup", 130, ROW_T1, { shiftKey: true });
    const perPx = BAR / 64;
    expect(app.sent).toEqual([{ type: "setComp", clip: CLIP, span: { start: 0, end: 4 * BAR }, pieces: pieces([0, T3], [70 * perPx, T1], [130 * perPx, T3]) }]);
  });

  it("Esc stops the drag and nothing is sent", () => {
    open();
    pointer("pointerdown", 70, ROW_T1);
    pointer("pointermove", 200, ROW_T1);
    expect(accent()).toHaveLength(3);
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(accent()).toEqual([[0, 24 + 4, 256]]);
    pointer("pointerup", 200, ROW_T1);
    expect(app.sent).toEqual([]);
    expect(cancelDrag()).toBe(false);
  });

  it("neighbours of one take are sent as one piece; a drag past the clip stops at its ends", () => {
    open(pieces([0, T1], [BAR, T3], [3 * BAR, T2]));
    expect(band()!.querySelector("[data-test=comp-clear]")!.getAttribute("aria-disabled")).toBeNull();
    // Take 1 over bars 2 and 3, where take 3 was.
    pointer("pointerdown", 64 + 4, ROW_T1);
    pointer("pointermove", 190, ROW_T1);
    pointer("pointerup", 190, ROW_T1);
    expect(app.sent).toEqual([{ type: "setComp", clip: CLIP, span: { start: 0, end: 4 * BAR }, pieces: pieces([0, T1], [3 * BAR, T2]) }]);
    app.sent.length = 0;
    // Take 3 from bar 4 to far past the clip's end, backwards from there to before its start.
    pointer("pointerdown", 250, ROW_T3);
    pointer("pointermove", 900, ROW_T3);
    pointer("pointermove", -50, ROW_T3);
    pointer("pointerup", -50, ROW_T3);
    expect(app.sent).toEqual([{ type: "setComp", clip: CLIP, span: { start: 0, end: 4 * BAR }, pieces: pieces([0, T3]) }]);
  });

  it("nothing is sent for a press without a move, a range the take already has, a press beside the clip, or while recording", () => {
    open(pieces([0, T1], [BAR, T3]));
    pointer("pointerdown", 70, ROW_T1);
    pointer("pointerup", 70, ROW_T1);
    pointer("pointerdown", 70, ROW_T3);
    pointer("pointermove", 200, ROW_T3);
    pointer("pointerup", 200, ROW_T3);
    pointer("pointerdown", 300, ROW_T1);
    pointer("pointermove", 400, ROW_T1);
    pointer("pointerup", 400, ROW_T1);
    // On the heading.
    pointer("pointerdown", 70, 10);
    pointer("pointermove", 200, 10);
    pointer("pointerup", 200, 10);
    expect(app.sent).toEqual([]);
    open([], (s) => (s.transport.state = "recording"));
    pointer("pointerdown", 70, ROW_T1);
    pointer("pointermove", 200, ROW_T1);
    pointer("pointerup", 200, ROW_T1);
    expect(app.sent).toEqual([]);
  });

  it("「もとに戻す」 sends clearComp; without a comp, and while recording, it is disabled", () => {
    open(pieces([0, T1], [BAR, T3]));
    const clear = () => band()!.querySelector<HTMLElement>("[data-test=comp-clear]")!;
    expect(clear().textContent).toBe("もとに戻す");
    clear().click();
    expect(app.sent).toEqual([{ type: "clearComp", clip: CLIP }]);
    open();
    expect(clear().getAttribute("aria-disabled")).toBe("true");
    clear().click();
    open(pieces([0, T1], [BAR, T3]), (s) => (s.transport.state = "countIn"));
    expect(clear().getAttribute("aria-disabled")).toBe("true");
    clear().click();
    expect(app.sent).toEqual([]);
  });
});

describe("the clip in the arrange shows what it plays", () => {
  it("one wave part for a clip with no comp; one for each piece, of its take, with a comp", () => {
    const plain = song().song.tracks[1]!.clips[0]!;
    expect(waveParts(plain).map((p) => [p.audio, p.range])).toEqual([[7, null]]);
    const s = song(pieces([0, T1], [BAR, T3]));
    const clip = s.song.tracks[1]!.clips[0]!;
    clip.audio!.takes[0] = { ...clip.audio!.takes[0]!, audio: 8, firstFrame: 100, takeOffset: -TPQ };
    const parts = waveParts(clip);
    expect(parts.map((p) => [p.audio, p.range])).toEqual([
      [8, { from: 0, to: BAR }],
      [7, { from: BAR, to: 4 * BAR }],
    ]);
    expect([parts[0]!.shape.timing.firstFrame, parts[0]!.shape.timing.takeOffset, parts[0]!.shape.endFrame]).toEqual([100, -TPQ, 100 + 8 * 48000]);
    expect(waveParts({ ...clip, audio: null })).toEqual([]);
  });
});

// ---- The fake ----

const vocalOf = (s: AppState) => s.song.tracks.find((x) => x.clips.some((c) => c.id === COMP_CLIP))!;
const clipOf = (s: AppState) => vocalOf(s).clips.find((c) => c.id === COMP_CLIP)!;

async function opened(edit: (s: AppState) => void = () => {}) {
  const state = fakeScene("compTakes")!.state;
  edit(state);
  const f = createFakeTransport(state);
  const send = async (c: Command) => {
    await f.send(c);
    return (await f.snapshot()).state as AppState;
  };
  return { send, now: async () => (await f.snapshot()).state as AppState };
}

describe("the fake (one step each, Rust's rules)", () => {
  const [A, B, D] = [COMP_CLIP + 1, COMP_CLIP + 2, COMP_CLIP + 3];
  const span = { start: 4 * BAR, end: 8 * BAR };

  it("the scene compTakes: three takes, the third shown, and the comp of the round's mock", async () => {
    const { now } = await opened();
    const clip = clipOf(await now());
    expect([clip.take, clip.takes, clip.takeId, clip.audio!.takes.map((x) => x.takeId)]).toEqual([3, 3, D, [A, B, D]]);
    expect(clip.comp).toEqual(pieces([0, A], [BAR + TPQ, D], [3 * BAR, B]));
    expect(fakeCompValid(clip.comp, 4 * BAR, [A, B, D])).toBe(true);
  });

  it("setComp and clearComp are one step each; what does not fit does nothing", async () => {
    const { send, now } = await opened();
    const before = await now();
    const steps = before.song.undoCount;
    const made = pieces([0, B], [2 * BAR, A]);
    let s = await send({ type: "setComp", clip: COMP_CLIP, span, pieces: made });
    expect([clipOf(s).comp, s.song.undoCount, s.song.file]).toEqual([made, steps + 1, "changed"]);
    expect([clipOf(s).takeId, clipOf(s).audio!.takes]).toEqual([D, clipOf(before).audio!.takes]);
    // The same again, other edges, an old take id, an old clip id, pieces that are no comp.
    const nothing: Command[] = [
      { type: "setComp", clip: COMP_CLIP, span, pieces: made },
      { type: "setComp", clip: COMP_CLIP, span: { start: 4 * BAR, end: 7 * BAR }, pieces: pieces([0, A]) },
      { type: "setComp", clip: COMP_CLIP, span, pieces: pieces([0, 5]) },
      { type: "setComp", clip: 999, span, pieces: pieces([0, A]) },
      { type: "setComp", clip: COMP_CLIP, span, pieces: pieces([BAR, A]) },
      { type: "setComp", clip: COMP_CLIP, span, pieces: pieces([0, A], [BAR, A]) },
      { type: "setComp", clip: COMP_CLIP, span, pieces: pieces([0, A], [4 * BAR, B]) },
      { type: "setComp", clip: COMP_CLIP, span, pieces: [] },
      { type: "clearComp", clip: 999 },
      { type: "deleteTake", clip: COMP_CLIP },
      { type: "deleteOtherTakes", clip: COMP_CLIP },
      { type: "reverseClips", clips: [COMP_CLIP] },
      { type: "normalizeClips", clips: [COMP_CLIP] },
    ];
    for (const command of nothing) {
      s = await send(command);
      expect([command, clipOf(s).comp, s.song.undoCount]).toEqual([command, made, steps + 1]);
    }
    s = await send({ type: "clearComp", clip: COMP_CLIP });
    expect([clipOf(s).comp, s.song.undoCount]).toEqual([[], steps + 2]);
    s = await send({ type: "clearComp", clip: COMP_CLIP });
    expect(s.song.undoCount).toBe(steps + 2);
    // The shown take whole is what the clip plays now: no step.
    s = await send({ type: "setComp", clip: COMP_CLIP, span, pieces: pieces([0, D]) });
    expect([clipOf(s).comp, s.song.undoCount]).toEqual([[], steps + 2]);
  });

  it("another take takes the comp away in the same step; a trim cuts it", async () => {
    const { send, now } = await opened();
    const steps = (await now()).song.undoCount;
    let s = await send({ type: "trimClip", clip: COMP_CLIP, edge: "start", ticks: 2 * BAR });
    expect([clipOf(s).start, clipOf(s).comp]).toEqual([6 * BAR, pieces([0, D], [BAR, B])]);
    expect(clipOf(s).audio!.takes.map((x) => x.takeOffset)).toEqual([-2 * BAR, -2 * BAR, -2 * BAR]);
    s = await send({ type: "trimClip", clip: COMP_CLIP, edge: "end", ticks: -BAR });
    expect(clipOf(s).comp).toEqual(pieces([0, D]));
    s = await send({ type: "stepTake", delta: 1 });
    expect([clipOf(s).take, clipOf(s).takeId, clipOf(s).comp, clipOf(s).audio!.audio]).toEqual([1, A, [], 66]);
    expect(s.status.message).toMatchObject({ id: "takeSwitched", take: 1, takes: 3 });
    s = await send({ type: "stepTake", delta: -1 });
    expect([clipOf(s).take, clipOf(s).takeId]).toEqual([3, D]);
    expect(s.song.undoCount).toBeGreaterThan(steps);
  });

  it("while recording nothing changes", async () => {
    const { send, now } = await opened((s) => (s.transport.state = "recording"));
    const before = await now();
    await send({ type: "setComp", clip: COMP_CLIP, span, pieces: pieces([0, A]) });
    const s = await send({ type: "clearComp", clip: COMP_CLIP });
    expect(s.song).toEqual(before.song);
  });
});
