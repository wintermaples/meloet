// Protocol 25 (first part): stretch and pitch. The shape, fake scenes, fake intents. The screen parts come in the second part.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { AppState, AudioClipView, Command, StatusMessage, StepName, StretchChange, TrackView } from "../src/bridge";
import { CLIP_CENTS_MAX, CLIP_SEMITONES_MAX, PROTOCOL_VERSION, SOURCE_BPM_MAX, SOURCE_BPM_MIN } from "../src/bridge";
import { SCENES, fakeScene, fakeState, reduceAudio } from "../src/bridge/fake";
import { STEP_NAME_KINDS } from "../src/bridge/generated/stepNames";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const scene = (name: string) => fakeScene(name)!;
const BAR = 4 * TPQ;
const CLIP = 70;
const vocalOf = (s: AppState): TrackView => s.song.tracks.find((tr) => tr.kind.kind === "vocal")!;
const audioOf = (s: AppState, clip = CLIP): AudioClipView => vocalOf(s).clips.find((c) => c.id === clip)!.audio!;

describe("protocol 25 shape", () => {
  it("ranges: semitones ±12, cents ±50, typed tempo 30 to 300", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(25);
    expect([CLIP_SEMITONES_MAX, CLIP_CENTS_MAX, SOURCE_BPM_MIN, SOURCE_BPM_MAX]).toEqual([12, 50, 30, 300]);
  });

  it("intents: one row of the stretch panel (setClipStretch) and 「さがす」 (findSourceTempo). On / off is bool", () => {
    const changes: StretchChange[] = [
      { kind: "follow", on: true },
      { kind: "sourceBpm", bpm: 96.5 },
      { kind: "semitones", semitones: 2 },
      { kind: "cents", cents: -30 },
      { kind: "keepFormant", on: false },
    ];
    const commands: Command[] = [...changes.map((change): Command => ({ type: "setClipStretch", clip: CLIP, change })), { type: "findSourceTempo", clip: CLIP }];
    expect(commands.map((c) => c.type)).toEqual(["setClipStretch", "setClipStretch", "setClipStretch", "setClipStretch", "setClipStretch", "findSourceTempo"]);
  });

  it("3 more step names. The status line says found / not found", () => {
    const names: StepName[] = [{ kind: "setClipFollow" }, { kind: "setSourceTempo" }, { kind: "setClipPitch" }];
    for (const name of names) expect(STEP_NAME_KINDS).toContain(name.kind);
    expect(STEP_NAME_KINDS.length).toBeGreaterThanOrEqual(74);
    const messages: StatusMessage[] = [
      { id: "sourceTempoFound", name: "ドラム", bpm: 137.14, bars: 4, same: false },
      { id: "sourceTempoNotFound", name: "ドラム" },
    ];
    expect(messages.map((m) => m.id)).toEqual(["sourceTempoFound", "sourceTempoNotFound"]);
  });
});

describe("fake scenes (mock F7-Stretch, spec 3 (28-A, Y22))", () => {
  it("stretch: a clip sung at 76.80 follows a 96.00 song (×1.25), +2 semitones. The sound is ready", () => {
    const s = scene("stretch").state;
    const audio = audioOf(s);
    expect(audio.stretch).toEqual({ follow: true, sourceBpm: 76.8, ratio: 1.25, semitones: 2, cents: 0, keepFormant: true, tune: null });
    expect(audio.sourceTempo).toEqual({ tempoBpm: 76.8, headRamp: false, tempoPoints: [] });
    expect(s.audioWork.jobs).toEqual([]);
    expect(s.params.selected).toEqual({ kind: "clip", track: vocalOf(s).id, clip: CLIP, missing: null, tuneUsable: true });
    expect(s.arrange.selectedClips).toEqual([CLIP]);
  });

  it("stretchRendering (F7-Stretch): the same clip's sound at 34 %. Progress is rendering with the clip", () => {
    const s = scene("stretchRendering").state;
    expect(audioOf(s).stretch).toMatchObject({ ratio: 1.25, semitones: 2 });
    expect(audioOf(s).state).toBe("ready");
    expect(s.audioWork.jobs).toEqual([{ kind: "rendering", audio: audioOf(s).audio, clip: CLIP, percent: 34 }]);
    expect(s.transport.position).toBe(4 * BAR);
    expect([s.song.undoCount, s.song.redoCount]).toEqual([14, 0]);
  });

  it("stretchPitch: pitch only (−1, 30). The recorded tempo stays, so the ratio is 1 (no mark)", () => {
    const audio = audioOf(scene("stretchPitch").state);
    expect(audio.stretch).toEqual({ follow: true, sourceBpm: 96, ratio: 1, semitones: -1, cents: 30, keepFormant: false, tune: null });
  });

  it("stretchNoTempo: an imported file. Does not follow, the source tempo is 「—」 (null), no ratio or point list", () => {
    const audio = audioOf(scene("stretchNoTempo").state);
    expect(audio.stretch).toEqual({ follow: false, sourceBpm: null, ratio: null, semitones: 0, cents: 0, keepFormant: true, tune: null });
    expect(audio.sourceTempo).toBeNull();
    expect(audio.file).toBe("ドラム.wav");
  });

  it("stretchRamp: 2 following clips on a ramp (one recorded with the map, one sung at 96.00)", () => {
    const s = scene("stretchRamp").state;
    expect(s.timeline.tempoPoints).toEqual([
      { tick: 4 * BAR, tempoBpm: 104, ramp: true },
      { tick: 8 * BAR, tempoBpm: 120, ramp: false },
    ]);
    const [recorded, sung] = vocalOf(s).clips;
    expect([recorded!.start, recorded!.end, sung!.start, sung!.end]).toEqual([2 * BAR, 5 * BAR, 5 * BAR, 9 * BAR]);
    // Points of a recorded song are ticks from tick 0 of the take (bar 3).
    expect(recorded!.audio!.sourceTempo).toEqual({
      tempoBpm: 96,
      headRamp: false,
      tempoPoints: [
        { tick: 2 * BAR, tempoBpm: 104, ramp: true },
        { tick: 6 * BAR, tempoBpm: 120, ramp: false },
      ],
    });
    expect(recorded!.audio!.stretch).toMatchObject({ follow: true, sourceBpm: 96, ratio: 1 });
    expect(sung!.audio!.stretch).toMatchObject({ follow: true, sourceBpm: 96, ratio: 1.125 });
    expect(s.params.selected).toMatchObject({ kind: "clip", clip: sung!.id });
  });

  it("older scenes: a recorded song is 96.00 with ratio 1. Clips that do not follow have no ratio or point list", () => {
    const edit = scene("audioEdit").state;
    for (const clip of vocalOf(edit).clips) {
      expect(clip.audio!.stretch).toEqual({ follow: true, sourceBpm: 96, ratio: 1, semitones: 0, cents: 0, keepFormant: true, tune: null });
      expect(clip.audio!.sourceTempo).toEqual({ tempoBpm: 96, headRamp: false, tempoPoints: [] });
    }
    const kinds = vocalOf(scene("audioKinds").state).clips;
    expect(kinds[2]!.audio!.stretch).toMatchObject({ sourceBpm: 76.8, ratio: 1.25, semitones: 2 });
    for (const name of Object.keys(SCENES)) {
      for (const clip of scene(name).state.song.tracks.flatMap((t) => t.clips)) {
        if (clip.audio === null) continue;
        const { stretch, sourceTempo } = clip.audio;
        const follows = stretch.follow && stretch.sourceBpm !== null;
        expect(stretch.ratio !== null, `${name} ${clip.id} ratio`).toBe(follows);
        expect(sourceTempo !== null, `${name} ${clip.id} sourceTempo`).toBe(follows);
        if (sourceTempo) expect(sourceTempo.tempoBpm).toBe(stretch.sourceBpm);
      }
    }
  });

  it("each scene name once", () => {
    const here = decodeURIComponent(new URL(import.meta.url).pathname);
    const source = new TextDecoder().decode(readFileSync(`${here.slice(0, here.lastIndexOf("/"))}/../src/bridge/fake.ts`));
    const names = [...source.matchAll(/^SCENES\.(\w+) = /gm)].map((m) => m[1]!);
    for (const name of ["stretch", "stretchRendering", "stretchPitch", "stretchNoTempo", "stretchRamp"]) {
      expect(names.filter((n) => n === name).length, name).toBe(1);
    }
  });
});

describe("fake intents (just enough for pictures and tests)", () => {
  type S = NonNullable<ReturnType<typeof fakeScene>>["state"];
  const stretch = (s: S, change: StretchChange, clip = CLIP) => reduceAudio(s, { type: "setClipStretch", clip, change })!;
  const songOf = (changes: ReturnType<typeof stretch>, s: S): S => {
    const song = changes.find((c) => c.section === "song");
    return song ? { ...s, song: song.value as S["song"] } : s;
  };

  it("semitones / cents: out of range is clamped, non-numbers do nothing, the same value is not a step. 1 step each", () => {
    const s = scene("stretch").state;
    const up = stretch(s, { kind: "semitones", semitones: 40 });
    expect(audioOf(songOf(up, s)).stretch.semitones).toBe(12);
    expect(songOf(up, s).song.undoCount).toBe(s.song.undoCount + 1);
    expect(audioOf(songOf(stretch(s, { kind: "semitones", semitones: -2.4 }), s)).stretch.semitones).toBe(-2);
    expect(audioOf(songOf(stretch(s, { kind: "cents", cents: -51 }), s)).stretch.cents).toBe(-50);
    expect(audioOf(songOf(stretch(s, { kind: "cents", cents: 29.6 }), s)).stretch.cents).toBe(30);
    expect(stretch(s, { kind: "semitones", semitones: Number.NaN })).toEqual([]);
    expect(stretch(s, { kind: "cents", cents: Number.NaN })).toEqual([]);
    expect(stretch(s, { kind: "semitones", semitones: 2 })).toEqual([]);
    // Not an audio clip: nothing happens.
    expect(stretch(s, { kind: "semitones", semitones: 1 }, 999)).toEqual([]);
  });

  it("source tempo: a new take (takeId changes). Following clips also change ratio and point list. The clip length stays", () => {
    const s = scene("stretch").state;
    const before = vocalOf(s).clips[0]!;
    const after = songOf(stretch(s, { kind: "sourceBpm", bpm: 120.004 }), s);
    const clip = vocalOf(after).clips[0]!;
    expect(clip.takeId).not.toBe(before.takeId);
    expect([clip.take, clip.takes, clip.start, clip.end, clip.takeOffset]).toEqual([before.take, before.takes, before.start, before.end, before.takeOffset]);
    expect(clip.audio!.stretch).toMatchObject({ follow: true, sourceBpm: 120, ratio: 0.8, semitones: 2 });
    expect(clip.audio!.sourceTempo).toEqual({ tempoBpm: 120, headRamp: false, tempoPoints: [] });
    expect(audioOf(songOf(stretch(s, { kind: "sourceBpm", bpm: 1000 }), s)).stretch.sourceBpm).toBe(300);
    expect(audioOf(songOf(stretch(s, { kind: "sourceBpm", bpm: 1 }), s)).stretch.sourceBpm).toBe(30);
    expect(stretch(s, { kind: "sourceBpm", bpm: Number.NaN })).toEqual([]);
    expect(stretch(s, { kind: "sourceBpm", bpm: 76.8 })).toEqual([]);
    // A clip that does not follow only gets the number (still no ratio or point list).
    const none = scene("stretchNoTempo").state;
    const typed = audioOf(songOf(stretch(none, { kind: "sourceBpm", bpm: 88 }), none));
    expect(typed.stretch).toMatchObject({ follow: false, sourceBpm: 88, ratio: null });
    expect(typed.sourceTempo).toBeNull();
  });

  it("follow / voice character: on / off. Even following with 「—」, there is no ratio (the file's speed)", () => {
    const s = scene("stretch").state;
    const off = audioOf(songOf(stretch(s, { kind: "follow", on: false }), s));
    expect(off.stretch).toMatchObject({ follow: false, sourceBpm: 76.8, ratio: null, semitones: 2 });
    expect(off.sourceTempo).toBeNull();
    expect(stretch(s, { kind: "follow", on: true })).toEqual([]);
    expect(audioOf(songOf(stretch(s, { kind: "keepFormant", on: false }), s)).stretch.keepFormant).toBe(false);
    expect(stretch(s, { kind: "keepFormant", on: true })).toEqual([]);
    const none = scene("stretchNoTempo").state;
    const on = audioOf(songOf(stretch(none, { kind: "follow", on: true }), none));
    expect(on.stretch).toMatchObject({ follow: true, sourceBpm: null, ratio: null });
    expect(on.sourceTempo).toBeNull();
  });

  it("search: treats the file length as 1, 2, 4, 8 bars, the number in 60 to 180 nearest the song tempo. Follow stays", () => {
    // 「ドラム.wav」 is 5 s: 96.00 with 2 bars (48 for 1 bar and 192 for 4 bars are outside).
    const s = scene("stretchNoTempo").state;
    const found = reduceAudio(s, { type: "findSourceTempo", clip: CLIP })!;
    const after = songOf(found, s);
    expect(audioOf(after).stretch).toMatchObject({ follow: false, sourceBpm: 96, ratio: null });
    expect(after.song.undoCount).toBe(s.song.undoCount + 1);
    expect(found.find((c) => c.section === "status")!.value).toEqual({ message: { id: "sourceTempoFound", name: "ドラム", bpm: 96, bars: 2, same: false } });
    // When not found: nothing changes, only the status line.
    const long = scene("audioLoading").state;
    const none = reduceAudio(long, { type: "findSourceTempo", clip: CLIP })!;
    expect(none).toEqual([{ section: "status", value: { message: { id: "sourceTempoNotFound", name: "ドラム" } } }]);
    expect(reduceAudio(s, { type: "findSourceTempo", clip: 999 })).toEqual([]);
    // In a new song nothing happens.
    expect(reduceAudio(fakeState(), { type: "setClipStretch", clip: 1, change: { kind: "follow", on: true } })).toEqual([]);
  });
});
