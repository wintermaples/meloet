// 約束 25（第 7 回 区切り4 の 1 回目）: のび縮みと音の高さ。形と、偽物の場面と、偽物の意図。画面の部品は 2 回目。
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

describe("約束 25 の形", () => {
  it("範囲は半音 ±12、セント ±50、打ちこむテンポ 30〜300", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(25);
    expect([CLIP_SEMITONES_MAX, CLIP_CENTS_MAX, SOURCE_BPM_MIN, SOURCE_BPM_MAX]).toEqual([12, 50, 30, 300]);
  });

  it("意図: のび縮みの欄の 1 行（setClipStretch）と「さがす」（findSourceTempo）。オン ・ オフは bool", () => {
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

  it("手の名前が 3 つふえた。状態行は、さがした ・ 見つからなかった", () => {
    const names: StepName[] = [{ kind: "setClipFollow" }, { kind: "setSourceTempo" }, { kind: "setClipPitch" }];
    for (const name of names) expect(STEP_NAME_KINDS).toContain(name.kind);
    expect(STEP_NAME_KINDS.length).toBe(74);
    const messages: StatusMessage[] = [
      { id: "sourceTempoFound", name: "ドラム", bpm: 137.14, bars: 4, same: false },
      { id: "sourceTempoNotFound", name: "ドラム" },
    ];
    expect(messages.map((m) => m.id)).toEqual(["sourceTempoFound", "sourceTempoNotFound"]);
  });
});

describe("偽物の場面（モック F7-Stretch、仕様書 3 章 28-A ・ Y22）", () => {
  it("stretch: 76.80 で歌ったクリップが 96.00 の曲についていく（×1.25）、半音 +2。音はできている", () => {
    const s = scene("stretch").state;
    const audio = audioOf(s);
    expect(audio.stretch).toEqual({ follow: true, sourceBpm: 76.8, ratio: 1.25, semitones: 2, cents: 0, keepFormant: true });
    expect(audio.sourceTempo).toEqual({ tempoBpm: 76.8, headRamp: false, tempoPoints: [] });
    expect(s.audioWork.jobs).toEqual([]);
    expect(s.params.selected).toEqual({ kind: "clip", track: vocalOf(s).id, clip: CLIP, missing: null });
    expect(s.arrange.selectedClips).toEqual([CLIP]);
  });

  it("stretchRendering（F7-Stretch）: 同じクリップの音を作っている 34 %。進みはクリップつきの rendering", () => {
    const s = scene("stretchRendering").state;
    expect(audioOf(s).stretch).toMatchObject({ ratio: 1.25, semitones: 2 });
    expect(audioOf(s).state).toBe("ready");
    expect(s.audioWork.jobs).toEqual([{ kind: "rendering", audio: audioOf(s).audio, clip: CLIP, percent: 34 }]);
    expect(s.transport.position).toBe(4 * BAR);
    expect([s.song.undoCount, s.song.redoCount]).toEqual([14, 0]);
  });

  it("stretchPitch: 高さだけ（−1 ・ 30）。録ったままのテンポなので、比は 1（印は出さない）", () => {
    const audio = audioOf(scene("stretchPitch").state);
    expect(audio.stretch).toEqual({ follow: true, sourceBpm: 96, ratio: 1, semitones: -1, cents: 30, keepFormant: false });
  });

  it("stretchNoTempo: 入れたファイル。ついていかない、この音のテンポは「—」（null）、比も点の列もない", () => {
    const audio = audioOf(scene("stretchNoTempo").state);
    expect(audio.stretch).toEqual({ follow: false, sourceBpm: null, ratio: null, semitones: 0, cents: 0, keepFormant: true });
    expect(audio.sourceTempo).toBeNull();
    expect(audio.file).toBe("ドラム.wav");
  });

  it("stretchRamp: だんだんの所の、ついていくクリップ 2 つ（地図ごと録った物と、96.00 で歌った物）", () => {
    const s = scene("stretchRamp").state;
    expect(s.timeline.tempoPoints).toEqual([
      { tick: 4 * BAR, tempoBpm: 104, ramp: true },
      { tick: 8 * BAR, tempoBpm: 120, ramp: false },
    ]);
    const [recorded, sung] = vocalOf(s).clips;
    expect([recorded!.start, recorded!.end, sung!.start, sung!.end]).toEqual([2 * BAR, 5 * BAR, 5 * BAR, 9 * BAR]);
    // 録った歌の点は、テイクの 0 ティック（3 小節目）からのティック。
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

  it("前からの場面: 録った歌は 96.00 で、比は 1。ついていかない物に、比と点の列はない", () => {
    const edit = scene("audioEdit").state;
    for (const clip of vocalOf(edit).clips) {
      expect(clip.audio!.stretch).toEqual({ follow: true, sourceBpm: 96, ratio: 1, semitones: 0, cents: 0, keepFormant: true });
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

  it("場面の名前は 1 回ずつ", () => {
    const here = decodeURIComponent(new URL(import.meta.url).pathname);
    const source = new TextDecoder().decode(readFileSync(`${here.slice(0, here.lastIndexOf("/"))}/../src/bridge/fake.ts`));
    const names = [...source.matchAll(/^SCENES\.(\w+) = /gm)].map((m) => m[1]!);
    for (const name of ["stretch", "stretchRendering", "stretchPitch", "stretchNoTempo", "stretchRamp"]) {
      expect(names.filter((n) => n === name).length, name).toBe(1);
    }
  });
});

describe("偽物の意図（絵と試験に足りるだけ）", () => {
  type S = NonNullable<ReturnType<typeof fakeScene>>["state"];
  const stretch = (s: S, change: StretchChange, clip = CLIP) => reduceAudio(s, { type: "setClipStretch", clip, change })!;
  const songOf = (changes: ReturnType<typeof stretch>, s: S): S => {
    const song = changes.find((c) => c.section === "song");
    return song ? { ...s, song: song.value as S["song"] } : s;
  };

  it("半音 ・ セント: 範囲の外はまるめる、数でない値は何もしない、同じ値は手にならない。1 手ずつ", () => {
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
    // 歌のクリップでなければ何もしない。
    expect(stretch(s, { kind: "semitones", semitones: 1 }, 999)).toEqual([]);
  });

  it("この音のテンポ: 新しいテイク（takeId が変わる）。ついていく物は、比と点の列も変わる。クリップの長さはそのまま", () => {
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
    // ついていかないクリップは、数が入るだけ（比も点の列もないまま）。
    const none = scene("stretchNoTempo").state;
    const typed = audioOf(songOf(stretch(none, { kind: "sourceBpm", bpm: 88 }), none));
    expect(typed.stretch).toMatchObject({ follow: false, sourceBpm: 88, ratio: null });
    expect(typed.sourceTempo).toBeNull();
  });

  it("ついていく ・ 声の感じ: オン ・ オフ。「—」のままついていくにしても、比は出ない（ファイルの速さのまま）", () => {
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

  it("さがす: ファイルの長さを 1 ・ 2 ・ 4 ・ 8 小節と見て、60〜180 の中の、曲のテンポに近い数。ついていくはそのまま", () => {
    // ドラム.wav は 5 秒: 2 小節で 96.00（1 小節 48 と 4 小節 192 は外）。
    const s = scene("stretchNoTempo").state;
    const found = reduceAudio(s, { type: "findSourceTempo", clip: CLIP })!;
    const after = songOf(found, s);
    expect(audioOf(after).stretch).toMatchObject({ follow: false, sourceBpm: 96, ratio: null });
    expect(after.song.undoCount).toBe(s.song.undoCount + 1);
    expect(found.find((c) => c.section === "status")!.value).toEqual({ message: { id: "sourceTempoFound", name: "ドラム", bpm: 96, bars: 2, same: false } });
    // 見つからないとき: 何も変わらず、状態行だけ。
    const long = scene("audioLoading").state;
    const none = reduceAudio(long, { type: "findSourceTempo", clip: CLIP })!;
    expect(none).toEqual([{ section: "status", value: { message: { id: "sourceTempoNotFound", name: "ドラム" } } }]);
    expect(reduceAudio(s, { type: "findSourceTempo", clip: 999 })).toEqual([]);
    // 新しい曲では何もしない。
    expect(reduceAudio(fakeState(), { type: "setClipStretch", clip: 1, change: { kind: "follow", on: true } })).toEqual([]);
  });
});
