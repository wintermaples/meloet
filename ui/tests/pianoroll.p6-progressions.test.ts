// P6 (round 12): 「進行から並べる ▾」 of the chord row. The chord names of each progression, the bar heads (also over a
// meter change), the notes sent, ▶, and the fake's clip growing.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, Command, ScaleMode, TrackView } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import ChordRow from "../src/components/pianoroll/ChordRow.svelte";
import { parentScale, placedKeys, PROGRESSIONS, progressionText, type Progression } from "../src/components/pianoroll/chords";
import { ChordSequence, chordMs, firstBar, progressionNotes } from "../src/components/pianoroll/progression";
import { closeMenu } from "../src/lib/menu";
import { TempoMap } from "../src/time/tempoMap";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

const BAR = 4 * TPQ;
const C_MAJOR = { root: 0, mode: "major" as ScaleMode };
const A_MINOR = { root: 9, mode: "minor" as ScaleMode };

const names = (scale: { root: number; mode: ScaleMode }) => Object.fromEntries(PROGRESSIONS.map((p) => [p.id, progressionText(p.id, scale)]));

describe("the chords of each progression", () => {
  const C = {
    royal: "F – G – Em – Am",
    canon: "C – G – Am – Em – F – C – F – G",
    komuro: "Am – F – G – C",
    circle: "C – Am – F – G",
    pop: "C – G – Am – F",
    sad: "Am – G – F – G",
  };

  it("in C major and in A minor (read from its relative major) the same names", () => {
    expect(names(C_MAJOR)).toEqual(C);
    expect(names(A_MINOR)).toEqual(C);
  });

  it("other keys and scales", () => {
    expect(progressionText("royal", { root: 7, mode: "major" })).toBe("C – D – Bm – Em");
    expect(progressionText("komuro", { root: 4, mode: "minor" })).toBe("Em – C – D – G");
    // 5- and 6-note scales read as their parent scale.
    expect(names({ root: 0, mode: "majorPentatonic" })).toEqual(C);
    expect(names({ root: 9, mode: "minorPentatonic" })).toEqual(C);
    expect(names({ root: 9, mode: "blues" })).toEqual(C);
    expect(names({ root: 9, mode: "harmonicMinor" })).toEqual(C);
    // Dorian and Mixolydian read as the major scale with the same notes.
    expect(names({ root: 7, mode: "mixolydian" })).toEqual(C);
    expect(names({ root: 2, mode: "dorian" })).toEqual(C);
  });

  it("the parent scale", () => {
    expect(parentScale("majorPentatonic")).toBe("major");
    expect(parentScale("minorPentatonic")).toBe("minor");
    expect(parentScale("blues")).toBe("minor");
    for (const m of ["major", "minor", "harmonicMinor", "melodicMinor", "dorian", "mixolydian"] as const) expect(parentScale(m)).toBe(m);
  });

  it("the keys placed: root position, the root from C3 to B3", () => {
    expect(placedKeys(0, "major")).toEqual([48, 52, 55]);
    expect(placedKeys(11, "dim")).toEqual([59, 62, 65]);
    expect(placedKeys(9, "minor")).toEqual([57, 60, 64]);
  });
});

describe("where the chords go", () => {
  const flat = new TempoMap({ tempoBpm: 120, meterNum: 4, meterDen: 4, headRamp: false, tempoPoints: [], meterChanges: [] });
  // 4/4, then 3/4 from bar 3 (index 2), then 6/8 from bar 5 (index 4).
  const changing = new TempoMap({
    tempoBpm: 120,
    meterNum: 4,
    meterDen: 4,
    headRamp: false,
    tempoPoints: [],
    meterChanges: [
      { bar: 2, meterNum: 3, meterDen: 4 },
      { bar: 4, meterNum: 6, meterDen: 8 },
    ],
  });

  it("the bar of the play line inside the clip, else the clip's first bar", () => {
    const clip = { start: BAR, end: 5 * BAR };
    expect(firstBar(flat, 2 * BAR + 5, clip)).toBe(2);
    expect(firstBar(flat, 0, clip)).toBe(1);
    expect(firstBar(flat, 9 * BAR, clip)).toBe(1);
    // A clip not on a bar line: its first whole bar.
    expect(firstBar(flat, 0, { start: BAR + TPQ, end: 5 * BAR })).toBe(2);
    expect(firstBar(flat, BAR + 2 * TPQ, { start: BAR + TPQ, end: 5 * BAR })).toBe(2);
    // No clip: the bar of the play line.
    expect(firstBar(flat, 3 * BAR + 7, null)).toBe(3);
  });

  it("one chord a bar, a bar long, over a meter change", () => {
    const chords = [
      { root: 0, triad: "major" as const },
      { root: 9, triad: "minor" as const },
      { root: 5, triad: "major" as const },
      { root: 7, triad: "major" as const },
    ];
    const notes = progressionNotes(chords, 1, changing, 90);
    const three = 3 * TPQ;
    const starts = [BAR, 2 * BAR, 2 * BAR + three, 2 * BAR + 2 * three];
    const lengths = [BAR, three, three, 3 * TPQ];
    expect(notes.length).toBe(12);
    for (let i = 0; i < 4; i++) {
      for (const n of notes.slice(3 * i, 3 * i + 3)) {
        expect([n.start, n.length, n.velocity]).toEqual([starts[i], lengths[i], 90]);
      }
    }
    expect(notes.slice(3, 6).map((n) => n.key)).toEqual([57, 60, 64]);
  });
});

describe("▶", () => {
  it("plays the chords two quarter notes each, then stops; stop stops at once", () => {
    const sent: Command[] = [];
    const runs: Array<() => void> = [];
    const clock = { set: (run: () => void) => runs.push(run), clear: () => {} };
    const changes: Array<string | null> = [];
    const seq = new ChordSequence((c) => sent.push(c), (p) => changes.push(p), clock);
    seq.play("royal", [{ root: 5, triad: "major" }, { root: 7, triad: "major" }], 100, chordMs(120));
    expect(chordMs(120)).toBe(1000);
    expect(sent).toEqual([{ type: "audition", keys: [65, 69, 72], velocity: 100 }]);
    runs.shift()!();
    expect(sent.at(-1)).toEqual({ type: "audition", keys: [67, 71, 74], velocity: 100 });
    runs.shift()!();
    expect(sent.at(-1)).toEqual({ type: "audition", keys: [], velocity: 0 });
    expect(changes).toEqual(["royal", null]);
    seq.play("pop", [{ root: 0, triad: "major" }], 100, 1000);
    seq.stop();
    expect(sent.at(-1)).toEqual({ type: "audition", keys: [], velocity: 0 });
    expect(seq.playing).toBeNull();
  });
});

describe("the button and the list", () => {
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
  const q = (sel: string) => document.querySelector<HTMLElement>(sel);
  const qa = (sel: string) => [...document.querySelectorAll<HTMLElement>(sel)];
  const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

  async function show(lock: boolean, edit?: (s: AppState) => void): Promise<{ s: AppState; clip: ClipView }> {
    const s = fakeScene("pianoRoll")!.state;
    s.pianoRoll.scale = { root: 9, mode: "minor", lock, highlight: true };
    edit?.(s);
    const track = s.song.tracks[0]! as TrackView;
    const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
    const take = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
    app.setState(s);
    shown.setShown({ clip, track, take });
    mounted = mount(ChordRow as Component, { target });
    flushSync();
    return { s, clip };
  }
  const key = (el: Element, k: string) => el.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));

  it("only with the scale fixed", async () => {
    await show(false);
    expect(q("[data-test=pr-progression-open]")).toBeNull();
    unmount(mounted!);
    mounted = null;
    await show(true);
    expect(text(q("[data-test=pr-progression-open]"))).toBe("進行から並べる ▾");
  });

  it("the list: names and chords in the scale; ↓ and Enter put the chosen one from the clip's first bar", async () => {
    const { clip } = await show(true, (s) => (s.transport.position = 0));
    q("[data-test=pr-progression-open]")!.click();
    flushSync();
    const rows = qa("[data-test=pr-progressions] [data-row]");
    expect(rows.map(text)).toEqual([
      "王道 F – G – Em – Am",
      "カノン C – G – Am – Em – F – C – F – G",
      "小室 Am – F – G – C",
      "循環 C – Am – F – G",
      "ポップ C – G – Am – F",
      "切ない Am – G – F – G",
    ]);
    expect(text(q("[data-test=pr-progressions] .foot"))).toBe("▶ で聞く ・ Enter で 1 小節に 1 つずつ並べる 並べる");
    const list = q("[data-test=pr-progressions]")!;
    key(list, "ArrowDown");
    key(list, "ArrowDown");
    flushSync();
    expect(rows[2]!.getAttribute("aria-selected")).toBe("true");
    key(list, "Enter");
    flushSync();
    const sent = app.sent.at(-1) as Extract<Command, { type: "addNotesAt" }>;
    expect(sent.type).toBe("addNotesAt");
    expect(sent.take).toBe(clip.takeId);
    expect(sent.notes.length).toBe(12);
    const head = Math.ceil(clip.start / BAR) * BAR;
    expect(sent.notes.slice(0, 3)).toEqual([57, 60, 64].map((k) => ({ start: head, length: BAR, key: k, velocity: 100 })));
    expect(sent.notes[11]!.start).toBe(head + 3 * BAR);
    expect(q("[data-test=pr-progressions]")).toBeNull();
  });

  it("▶ plays and closing the list stops it; the fixed velocity is used", async () => {
    await show(true, (s) => (s.keyboards.velocityFixed = { on: true, value: 70 }));
    q("[data-test=pr-progression-open]")!.click();
    flushSync();
    qa("[data-test=pr-progression-listen]")[0]!.click();
    flushSync();
    expect(app.sent.at(-1)).toEqual({ type: "audition", keys: [65, 69, 72], velocity: 70 });
    expect(qa("[data-test=pr-progression-listen]")[0]!.classList.contains("playing")).toBe(true);
    closeMenu();
    flushSync();
    expect(q("[data-test=pr-progressions]")).toBeNull();
    expect(app.sent.at(-1)).toEqual({ type: "audition", keys: [], velocity: 0 });
  });

  it("「並べる」 from the bar of the play line", async () => {
    const { clip } = await show(true, (s) => (s.transport.position = 3 * BAR + 100));
    expect(clip.start <= 3 * BAR && 3 * BAR < clip.end).toBe(true);
    q("[data-test=pr-progression-open]")!.click();
    flushSync();
    q("[data-test=pr-progression-place]")!.click();
    const sent = app.sent.at(-1) as Extract<Command, { type: "addNotesAt" }>;
    expect(sent.notes[0]!.start).toBe(3 * BAR);
    expect(sent.notes.map((n) => n.key).slice(0, 3)).toEqual(placedKeys(5, "major"));
  });
});

describe("the fake", () => {
  it("puts the notes as one step, grows the clip and selects them", async () => {
    const s = fakeScene("pianoRoll")!.state;
    const transport = createFakeTransport(s);
    let state = (await transport.snapshot()).state;
    const clip = state.song.tracks[0]!.clips.find((c) => c.id === state.arrange.pianoRollClip)!;
    const undo = state.song.undoCount;
    const notes = PROGRESSIONS.find((p) => p.id === ("canon" as Progression))!.degrees.flatMap((_, i) =>
      [48, 52, 55].map((key) => ({ start: clip.end - BAR + i * BAR, length: BAR, key, velocity: 100 })),
    );
    await transport.send({ type: "addNotesAt", take: clip.takeId, notes });
    state = (await transport.snapshot()).state;
    const now = state.song.tracks[0]!.clips.find((c) => c.id === clip.id)!;
    expect(state.song.undoCount).toBe(undo + 1);
    expect(now.end).toBeGreaterThan(clip.end);
    expect(state.pianoRoll.selectedNotes.reduce((n, r) => n + r.end - r.start, 0)).toBeGreaterThan(0);
  });
});
