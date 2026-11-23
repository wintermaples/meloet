// 「鳴っている音を光らせる」 in the note area (round 12 E1): while playing, the sounding notes of the shown clip are painted in the track's
// color on the small pianoRollLive canvas (the `.glow-box` slot), whose box is as tall as the note area and as wide as the lit notes. The
// slot is asked to draw again only when the set changes, never every frame. Off with the setting, on a muted track, and when stopped.
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TakeNotes } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import Roll from "../src/components/pianoroll/Roll.svelte";
import { prPanels, prView } from "../src/components/pianoroll/view.svelte";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { runFrameCallbacks, watchSlots, type GlSlot } from "../src/gl/slot";
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
const slots = new Map<HTMLElement, GlSlot>();
let updates = 0;
let stopWatch: () => void = () => {};
beforeEach(() => {
  app.reset();
  prView.focus = "arrange";
  prView.shownClip = null;
  prView.drums = false;
  prView.follow = false;
  prPanels.list = true;
  prPanels.inspector = true;
  slots.clear();
  updates = 0;
  stopWatch = watchSlots({
    attach: (node, slot) => slots.set(node, slot),
    update: (node, slot) => {
      slots.set(node, slot);
      if (node.dataset.test === "pr-glow") updates += 1;
    },
    detach: (node) => slots.delete(node),
  });
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  stopWatch();
});

/** 1.3.240 of the scene pianoRollGlow, where C3 G3 and C5 sound. */
const AT = 2 * TPQ + TPQ / 2;

function at(tick: number, state: AppState["transport"]["state"] = "playing"): void {
  app.setTelemetry(app.telemetryWith({ position: { tick, ticksPerUs: 0, state, countInBeatsLeft: 0, lap: 0 } }));
}

async function play(edit?: (s: AppState) => void): Promise<AppState> {
  const s = fakeScene("pianoRollGlow")!.state;
  edit?.(s);
  const track = s.song.tracks[0]!;
  const clip = track.clips[0]!;
  const take: TakeNotes = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
  app.setState(s);
  at(AT);
  shown.setShown({ clip, track, take });
  mounted = mount(Roll, { target });
  flushSync();
  return s;
}

const box = () => target.querySelector<HTMLElement>("[data-test=pr-glow]")!;
const size = () => [parseFloat(box().style.width || "0"), parseFloat(box().style.height || "0")];

/** The faces painted by the glow slot (the 2 px gray.10 edge of each note left out). */
function faces(): string[] {
  const slot = slots.get(box())!;
  if (slot.kind !== "paint") throw new Error(slot.kind);
  const [w, h] = size();
  const out = new Batch();
  out.clip(0, 0, w!, h!);
  slot.paint(out, { x0: 0, y0: 0, x1: w!, y1: h! }, { dpr: 1, scale: 100, isDown: () => false });
  return out.rects().filter((r) => r[2]! - r[0]! > 2).map((r) => [r[4], r[5], r[6]].join(","));
}
const is = (name: Parameters<typeof rgb>[0]) => Array.from(Float32Array.from(rgb(name))).join(",");

describe("the lit notes while playing", () => {
  it("C3 G3 and C5 in the track's color, in a box as tall as the note area that starts at the notes", async () => {
    await play();
    runFrameCallbacks();
    const [w, h] = size();
    expect(h).toBe(440);
    expect(w).toBeGreaterThan(0);
    expect(box().style.transform).toBe("translateX(0px)");
    expect(faces()).toEqual([is("color.track.1"), is("color.track.1"), is("color.track.1")]);
  });

  it("asked to draw again only when the set of sounding notes changes, not every frame", async () => {
    await play();
    runFrameCallbacks();
    const after = updates;
    for (let i = 0; i < 5; i++) runFrameCallbacks();
    at(3 * TPQ);
    flushSync();
    for (let i = 0; i < 5; i++) runFrameCallbacks();
    expect(updates).toBe(after);
    // C5 ends at 1.4.240, where G4 starts.
    at(3 * TPQ + TPQ / 2);
    flushSync();
    runFrameCallbacks();
    runFrameCallbacks();
    expect(updates).toBe(after + 1);
    expect(faces()).toHaveLength(3);
  });

  it("nothing where nothing sounds (after the clip's end)", async () => {
    await play();
    at(5 * 4 * TPQ);
    flushSync();
    runFrameCallbacks();
    expect(size()).toEqual([0, 0]);
  });

  it("goes out when the setting is turned off, and lights again when it is turned on", async () => {
    const s = await play();
    runFrameCallbacks();
    app.setState({ ...s, workspace: { ...s.workspace, noteGlow: false } });
    flushSync();
    runFrameCallbacks();
    expect(size()).toEqual([0, 0]);
    app.setState(s);
    flushSync();
    runFrameCallbacks();
    expect(size()[0]).toBeGreaterThan(0);
  });

  it("not on a muted track, nor with the setting off from the start", async () => {
    await play((s) => (s.song.tracks[0]!.muted = true));
    runFrameCallbacks();
    expect(size()).toEqual([0, 0]);
    unmount(mounted!);
    mounted = null;
    await play((s) => (s.workspace.noteGlow = false));
    runFrameCallbacks();
    expect(size()).toEqual([0, 0]);
  });

  it("goes out when the song stops", async () => {
    const s = await play();
    runFrameCallbacks();
    expect(size()[0]).toBeGreaterThan(0);
    app.setState({ ...s, transport: { ...s.transport, state: "stopped" } });
    at(AT, "stopped");
    flushSync();
    expect(size()).toEqual([0, 0]);
  });
});
