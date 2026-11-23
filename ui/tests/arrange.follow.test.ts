// Checks that the screen's "song tick -> file frame" (src/components/arrange/follow.ts) gives the same answers as the Rust formula
// (FileClock in crates/meloet-model/src/audio/follow.rs).
// The answer table is written by `cargo test -p meloet-model --test follow`.
import { describe, expect, it } from "vitest";
import type { ClipView } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { fileFrameAt } from "../src/components/arrange/audioClipGeom";
import { clipFrameAt, followClipOf, sourceMapOf, stretchRatioAt } from "../src/components/arrange/follow";
import table from "../src/components/arrange/follow.table.json";
import { TempoMap } from "../src/time/tempoMap";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const BAR = 4 * TPQ;

/** Frames to 1/1000 of a frame (JS log1p can differ from Rust in the last digit; 1 px of the waveform is hundreds of frames). */
const nearFrame = (got: number, want: number, what: string) => expect(Math.abs(got - want), `${what}: ${got} / ${want}`).toBeLessThanOrEqual(1e-3 + 1e-9 * Math.abs(want));

describe("formula for clips that follow the tempo (protocol 25, docs/architecture.md 22-19)", () => {
  it("same as the Rust table: not following, following, and with a tempo ramp on both", () => {
    expect(table.cases.length).toBe(6);
    expect(table.cases.filter((c) => c.clip.sourceTempo !== null).length).toBe(4);
    for (const c of table.cases) {
      const song = new TempoMap(c.song);
      const source = c.clip.sourceTempo === null ? null : sourceMapOf(c.clip.sourceTempo);
      expect(c.rows.length).toBeGreaterThan(40);
      for (const [tick, frame, ratio] of c.rows) {
        nearFrame(clipFrameAt(song, source, c.clip, tick!), frame!, `${c.name} @${tick}`);
        const got = stretchRatioAt(song, source, c.clip, tick!);
        if (ratio === null) expect(got, c.name).toBeNull();
        else expect(Math.abs(got! - ratio!), `${c.name} ×@${tick}`).toBeLessThanOrEqual(1e-12);
      }
    }
  });

  it("a clip that does not follow gives the same answer as the current fileFrameAt", () => {
    const s = fakeScene("tempo")!.state;
    const map = new TempoMap(s.timeline);
    const clip = { id: 1, name: "ドラム", start: 3 * BAR, end: 9 * BAR, take: 1, takes: 1, takeId: 2, takeOffset: -TPQ, noteCount: 0, audio: null } as ClipView;
    const audio = fakeScene("stretchNoTempo")!.state.song.tracks.flatMap((t) => t.clips).find((c) => c.audio !== null)!.audio!;
    const withAudio: ClipView = { ...clip, audio: { ...audio, firstFrame: 777 } };
    const shape = followClipOf(withAudio)!;
    expect(followClipOf(clip)).toBeNull();
    for (let tick = 2 * BAR; tick <= 10 * BAR; tick += 333) expect(clipFrameAt(map, null, shape, tick)).toBe(fileFrameAt(map, withAudio, tick));
  });

  it("when following, the same tick gets the same frame even if the song tempo changes (the clip length stays in ticks)", () => {
    const source = sourceMapOf({ tempoBpm: 96, headRamp: false, tempoPoints: [] });
    const clip = { start: 4 * BAR, takeOffset: 0, firstFrame: 0, sampleRate: 48000 };
    for (const tempoBpm of [60, 96, 120, 181.5]) {
      const song = new TempoMap({ tempoBpm, meterNum: 4, meterDen: 4, headRamp: false, tempoPoints: [], meterChanges: [] });
      // 2 bars sung at 96 are 5 seconds of the file.
      expect(clipFrameAt(song, source, clip, 6 * BAR)).toBeCloseTo(240000, 6);
      expect(stretchRatioAt(song, source, clip, 4 * BAR)).toBeCloseTo(tempoBpm / 96, 12);
      // When not following, the file plays at its own speed (the frame changes with the song tempo).
      expect(clipFrameAt(song, null, clip, 6 * BAR)).toBeCloseTo((240000 * 96) / tempoBpm, 6);
    }
  });

  it("the fake scene's numbers match the formula (×1.25, 1.125 inside the ramp, 1 for as recorded)", () => {
    for (const name of ["stretch", "stretchRendering", "stretchPitch", "stretchRamp", "audioEdit", "audioKinds"]) {
      const s = fakeScene(name)!.state;
      const song = new TempoMap(s.timeline);
      for (const clip of s.song.tracks.flatMap((t) => t.clips)) {
        if (clip.audio === null) continue;
        const source = clip.audio.sourceTempo === null ? null : sourceMapOf(clip.audio.sourceTempo);
        const ratio = stretchRatioAt(song, source, followClipOf(clip)!, clip.start);
        if (ratio === null) expect(clip.audio.stretch.ratio, `${name} ${clip.id}`).toBeNull();
        else expect(clip.audio.stretch.ratio, `${name} ${clip.id}`).toBeCloseTo(ratio, 9);
      }
    }
    const ramp = fakeScene("stretchRamp")!.state;
    const [recorded, sung] = ramp.song.tracks.find((t) => t.kind.kind === "vocal")!.clips;
    expect([recorded!.audio!.stretch.ratio, sung!.audio!.stretch.ratio]).toEqual([1, 1.125]);
    // A clip recorded in this tempo map lands at the same place as the file's own speed, even when following (even inside the ramp).
    const song = new TempoMap(ramp.timeline);
    const source = sourceMapOf(recorded!.audio!.sourceTempo!);
    for (let tick = recorded!.start; tick <= recorded!.end; tick += 111) {
      nearFrame(clipFrameAt(song, source, followClipOf(recorded!)!, tick), fileFrameAt(song, recorded!, tick), `録ったまま @${tick}`);
    }
    // After the ramp, the ratio gets larger.
    const shape = followClipOf(sung!)!;
    const own = sourceMapOf(sung!.audio!.sourceTempo!);
    expect(stretchRatioAt(song, own, shape, 8 * BAR)).toBeCloseTo(120 / 96, 12);
  });
});
