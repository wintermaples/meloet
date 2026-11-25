// The mounted note area while something is held or the song moves on: no page turn under a held pointer (UI-ARRANGE-004), a drag
// dropped when another take is shown (UI-ROLL-003), no edit started while recording or counting in (UI-ROLL-008, UI-ARRANGE-013),
// and the fit on opening for a clip of another song with the same id (UI-ROLL-007).
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TakeNotes } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import Roll from "../src/components/pianoroll/Roll.svelte";
import { prPanels, prView, pxPerTick } from "../src/components/pianoroll/view.svelte";
import { runFrameCallbacks } from "../src/gl/slot";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

// jsdom does not measure sizes: the note area is 668 × 440.
const proto = HTMLElement.prototype;
const hadW = Object.getOwnPropertyDescriptor(proto, "clientWidth");
const hadH = Object.getOwnPropertyDescriptor(proto, "clientHeight");
beforeAll(() => {
  Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => 668 });
  Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => 440 });
});
afterAll(() => {
  if (hadW) Object.defineProperty(proto, "clientWidth", hadW);
  if (hadH) Object.defineProperty(proto, "clientHeight", hadH);
});

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  prView.shownClip = null;
  prView.tool = "select";
  prView.follow = false;
  prPanels.list = true;
  prPanels.inspector = true;
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  prView.tool = "select";
});

async function open(edit?: (s: AppState) => void): Promise<{ s: AppState; take: TakeNotes }> {
  const s = fakeScene("pianoRoll")!.state;
  edit?.(s);
  const track = s.song.tracks[0]!;
  const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
  const take = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
  app.setState(s);
  shown.setShown({ clip, track, take });
  mounted = mount(Roll, { target });
  flushSync();
  return { s, take };
}

const q = (sel: string) => target.querySelector<HTMLElement>(sel)!;
const pointer = (el: Element, type: string, init: PointerEventInit) =>
  el.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, ...init }));
const playingAt = (tick: number) =>
  app.setTelemetry(app.telemetryWith({ position: { tick, ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 } }));

describe("while playing, a held pointer is not paged away from (UI-ARRANGE-004)", () => {
  it("no page turn while a pointer is held in the piano roll; paging goes on at the first frame after release", async () => {
    await open((s) => (s.transport = { ...s.transport, state: "playing" }));
    expect(prView.follow).toBe(true);
    const before = prView.scrollTick;
    playingAt(before + (3 * 668) / pxPerTick(100));
    pointer(q(".area"), "pointerdown", { clientX: 300, clientY: 5 });
    runFrameCallbacks();
    expect(prView.scrollTick).toBe(before);
    pointer(q(".area"), "pointerup", { clientX: 300, clientY: 5 });
    runFrameCallbacks();
    expect(prView.scrollTick).toBeGreaterThan(before);
  });
});

describe("a drag of the take shown at the press (UI-ROLL-003)", () => {
  it("is dropped when another take is shown before release: nothing is written into the new take", async () => {
    const { take } = await open();
    prView.tool = "draw";
    pointer(q(".area"), "pointerdown", { clientX: 300, clientY: 5 });
    shown.setShown({ clip: shown.shown.clip, track: shown.shown.track, take: { ...take, id: take.id + 100 } });
    flushSync();
    pointer(q(".area"), "pointerup", { clientX: 300, clientY: 5 });
    expect(app.sent.filter((c) => c.type === "addNote" || c.type === "addNotes")).toEqual([]);
  });
});

describe("while counting in or recording (UI-ROLL-008, UI-ARRANGE-013)", () => {
  it("the draw tool writes nothing and plays nothing", async () => {
    await open((s) => (s.transport = { ...s.transport, state: "countIn" }));
    prView.tool = "draw";
    pointer(q(".area"), "pointerdown", { clientX: 300, clientY: 5 });
    pointer(q(".area"), "pointerup", { clientX: 300, clientY: 5 });
    expect(app.sent).toEqual([]);
  });

  it("the ruler shows no new loop range and sends nothing", async () => {
    await open((s) => (s.transport = { ...s.transport, state: "recording" }));
    expect(target.querySelector(".loop-mark")).toBeNull();
    const ruler = q('[data-test="pr-ruler"]');
    const bar = 4 * TPQ * pxPerTick(100);
    pointer(ruler, "pointerdown", { clientX: bar + 2 });
    pointer(ruler, "pointermove", { clientX: 3 * bar - 2 });
    flushSync();
    expect(target.querySelector(".loop-mark")).toBeNull();
    pointer(ruler, "pointerup", { clientX: 3 * bar - 2 });
    expect(app.sent).toEqual([]);
  });
});

describe("opening a clip of another song with the same id (UI-ROLL-007)", () => {
  it("fits again (zoom and scroll to the clip); the same song's clip keeps the view", async () => {
    const { s, take } = await open();
    const fitted = { scrollTick: prView.scrollTick, quarterPx: prView.quarterPx };
    prView.scrollTick = fitted.scrollTick + 50 * 4 * TPQ;
    prView.quarterPx = 320;
    // The same song: an edit gives the clip again, with the same id.
    shown.setShown({ clip: { ...shown.shown.clip! }, track: shown.shown.track, take });
    flushSync();
    expect(prView.scrollTick).toBe(fitted.scrollTick + 50 * 4 * TPQ);
    // Another song (a new serial) whose clip has the same id.
    const next = fakeScene("pianoRoll")!.state;
    next.song = { ...next.song, serial: s.song.serial + 1 };
    const track = next.song.tracks[0]!;
    const clip = track.clips.find((c) => c.id === next.arrange.pianoRollClip)!;
    expect(clip.id).toBe(shown.shown.clip!.id);
    app.setState(next);
    shown.setShown({ clip, track, take });
    flushSync();
    expect({ scrollTick: prView.scrollTick, quarterPx: prView.quarterPx }).toEqual(fitted);
  });
});
