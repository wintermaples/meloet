// Protocol 24: audio clips, putting in and dragging in sound files, missing sounds, the 160 row, peaks.
// The shapes, the fake scenes, and the status line and history texts. The screen parts come in the second step.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { AppState, AudioClipView, Command, Event, SelectedParams, StatusMessage, StepName, StepWhat, TrackView } from "../src/bridge";
import { AUDIO_EXTENSIONS, CLIP_GAIN_MAX_DB, CLIP_GAIN_MIN_DB, PROTOCOL_VERSION } from "../src/bridge";
import { FAKE_AUDIO, SCENES, createFakeTransport, fakeScene, fakeState, reduceAudio } from "../src/bridge/fake";
import { decodePeaks } from "../src/bridge/peaks";
import { stepNameText, stepWhatText } from "../src/components/history/stepText";
import { t } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const scene = (name: string) => fakeScene(name)!;
const BAR = 4 * TPQ;
const VOCAL = 16;
const vocalOf = (s: AppState): TrackView => s.song.tracks.find((tr) => tr.kind.kind === "vocal")!;
const audioOf = (s: AppState, clip: number): AudioClipView => vocalOf(s).clips.find((c) => c.id === clip)!.audio!;

describe("shape of protocol 24", () => {
  it("a new song has no audio clips, no sounds being made, and no tall rows", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(24);
    const s = fakeState();
    expect(s.audioWork).toEqual({ jobs: [] });
    expect(s.arrange.tallTrack).toBeNull();
    expect(s.transport.recordingFaults).toBeNull();
    expect(s.workspace.fileDrop).toBeNull();
    expect([CLIP_GAIN_MIN_DB, CLIP_GAIN_MAX_DB]).toEqual([-60, 12]);
    expect(AUDIO_EXTENSIONS).toEqual(["wav", "mp3", "flac", "ogg"]);
    expect(t("msg.audioUnreadable", { file: "声.aac" })).toContain(AUDIO_EXTENSIONS.join(" ・ "));
  });

  it("intents: split, clip values, the 160 row, put in a sound file, where it was dropped, search, remove from the song", () => {
    const commands: Command[] = [
      { type: "splitClips" },
      { type: "setClipField", clip: 72, field: "start", value: 6 * BAR },
      { type: "setClipField", clip: 72, field: "length", value: 2 * BAR },
      { type: "setClipField", clip: 72, field: "gainDb", value: -3 },
      { type: "setClipField", clip: 72, field: "fadeIn", value: 480 },
      { type: "setClipField", clip: 72, field: "fadeOut", value: 0 },
      { type: "setTallTrack", track: VOCAL },
      { type: "setTallTrack", track: null },
      { type: "importAudio", track: null },
      { type: "importAudio", track: VOCAL },
      { type: "dropAudio", drop: 1, track: VOCAL, tick: 4 * BAR, mode: "separate" },
      { type: "dropAudio", drop: 1, track: null, tick: 4 * BAR, mode: "inRow" },
      { type: "findAudio", audio: 64 },
      { type: "removeMissing", missing: { kind: "audio", audio: 64 } },
    ];
    expect(new Set(commands.map((c) => c.type)).size).toBe(7);
  });

  it("things sent at once: where it is being dragged in, and where it was dropped (page px)", () => {
    const events: Event[] = [
      { type: "fileDrag", x: 520, y: 385 },
      { type: "fileDrop", drop: 1, x: 520, y: 385 },
    ];
    expect(events.map((e) => e.type)).toEqual(["fileDrag", "fileDrop"]);
  });

  it("status line texts: missing, split, put in, cannot read, dragging in", () => {
    const messages: StatusMessage[] = [
      { id: "audioMissing", count: 2 },
      { id: "clipSplit", name: "歌 テイク 1", tick: 6 * BAR, count: 1 },
      { id: "audioImported", file: "ドラム.wav", track: { number: 6, name: "歌" }, tick: 4 * BAR, count: 1, skipped: 0 },
      { id: "audioUnreadable", file: "声.aac" },
      { id: "audioDragging", audio: 8, sf2: 0 },
    ];
    expect(new Set(messages.map((m) => m.id)).size).toBe(5);
    // spec 6 text (ja.ts already has it).
    expect(t("msg.audioFilesMissing", { count: 2 })).toBe("曲で使っている音のファイルが 2 つ見つかりません。そのクリップは鳴りません。");
    expect(t("msg.clipSplit", { clip: "歌 テイク 1", position: "7. 1. 0000" })).toBe("歌 テイク 1 を 7. 1. 0000 で切りました。");
    expect(t("msg.audioImported", { file: "ドラム.wav", track: "6 歌", position: "5. 1. 0000" })).toBe("ドラム.wav を入れました（6 歌 ・ 5. 1. 0000）。");
    expect(t("msg.audioDragging", { count: 8 })).toBe("音のファイルを 8 つ持ってきています ・ 左に落とすと別々のトラック、右に落とすと 1 本にならべる");
    expect(t("drop.mixed", { sf2: 1, audio: 3 })).toBe("SF2 は 1 つ棚へ ・ 音のファイル 3 つはアレンジへ");
  });

  it("step names and (what): split (S), cut (Mod X), put in a sound file, clip gain, fade", () => {
    const names: StepName[] = [{ kind: "splitClips" }, { kind: "cutClips" }, { kind: "importAudio" }, { kind: "setClipGain" }, { kind: "setClipFade" }];
    expect(names.map(stepNameText)).toEqual(["クリップを切る", "クリップを切り取る", "音のファイルを入れる", "クリップの音量を変える", "フェードを変える"]);
    const ctx = { song: fakeState().song, library: fakeState().library };
    const says = (name: StepName, what: StepWhat) => stepWhatText(name, what, ctx);
    expect(says({ kind: "setClipGain" }, { kind: "clipGain", name: "歌 テイク 1", fromDb: -3, toDb: 0 })).toBe("歌 テイク 1 ・ −3.0 → 0.0 dB");
    expect(says({ kind: "splitClips" }, { kind: "name", name: "歌 テイク 1" })).toBe("歌 テイク 1");
    expect(says({ kind: "importAudio" }, { kind: "clips", count: 3 })).toBe("3 クリップ");
  });
});

describe("fake scenes (F7-AudioEdit, F7-AudioDrop, F7-AudioMissing, R11-Audio)", () => {
  it("audioEdit: two parts split with S. Left has a fade at the end, right has a fade at the start and −3.0 dB, and is selected. The panel is 「選んだクリップ」", () => {
    const s = scene("audioEdit").state;
    const [left, right] = vocalOf(s).clips;
    expect([left!.name, left!.start, left!.end, right!.start, right!.end]).toEqual(["歌 テイク 1", 4 * BAR, 6 * BAR, 6 * BAR, 8 * BAR]);
    // the same take (splitting does not make takes). On the right, the take's tick 0 is 2 bars before.
    expect([left!.takeId, right!.takeId, left!.takeOffset, right!.takeOffset]).toEqual([71, 71, 0, -2 * BAR]);
    expect(left!.audio).toMatchObject({ audio: 60, file: "歌 テイク 1.wav", sampleRate: 48000, bits: 24, channels: 1, resampled: false, fadeIn: 0, fadeOut: TPQ, gainDb: 0, state: "ready" });
    expect(right!.audio).toMatchObject({ audio: 60, firstFrame: 0, frames: 480000, fadeIn: TPQ / 4, fadeOut: 0, gainDb: -3 });
    expect(right!.audio!.stretch).toEqual({ follow: true, sourceBpm: 96, ratio: 1, semitones: 0, cents: 0, keepFormant: true });
    expect([left!.noteCount, right!.noteCount]).toEqual([0, 0]);
    expect(s.arrange).toMatchObject({ selectedClips: [72], selectedClip: 72, pianoRollClip: null, tallTrack: null });
    const selected: SelectedParams = { kind: "clip", track: VOCAL, clip: 72, missing: null };
    expect(s.params.selected).toEqual(selected);
    expect(s.workspace.layout.rightOpen).toContain("params");
    expect(s.transport.position).toBe(6 * BAR);
    expect(s.status.message).toEqual({ id: "clipSplit", name: "歌 テイク 1", tick: 6 * BAR, count: 1 });
    // keyboard clips have no audio.
    expect(s.song.tracks[0]!.clips.every((c) => c.audio === null)).toBe(true);
  });

  it("audioTall: after pressing the audio clip twice. The 「6 歌」 row is 160", () => {
    expect(scene("audioTall").state.arrange.tallTrack).toBe(VOCAL);
  });

  it("audioDrop*: one, eight (left and right), mixed with SF2. The place comes as a thing sent at once", () => {
    const one = scene("audioDropOne");
    expect(one.state.workspace.fileDrop).toEqual({ name: "ドラム.wav", addable: true, files: 1, sf2: 0, audio: 1 });
    expect(one.state.status.message).toEqual({ id: "audioDragging", audio: 1, sf2: 0 });
    expect(one.events).toEqual([{ type: "fileDrag", x: 600, y: 540 }]);
    const many = scene("audioDrop");
    expect(many.state.workspace.fileDrop).toEqual({ name: "ドラム キック.wav", addable: true, files: 8, sf2: 0, audio: 8 });
    expect(many.state.status.message).toEqual({ id: "audioDragging", audio: 8, sf2: 0 });
    expect([many.events[0], scene("audioDropRight").events[0]]).toEqual([
      { type: "fileDrag", x: 520, y: 385 },
      { type: "fileDrag", x: 760, y: 385 },
    ]);
    const mixed = scene("audioDropMixed").state;
    expect(mixed.workspace.fileDrop).toMatchObject({ files: 4, sf2: 1, audio: 3 });
    expect(mixed.status.message).toEqual({ id: "audioDragging", audio: 3, sf2: 1 });
    // the older SF2 scenes have no sound files.
    expect(scene("sf2DropMany").state.workspace.fileDrop).toMatchObject({ sf2: 2, audio: 0 });
  });

  it("audioMissing: two missing sounds. The clips are missing, the panel shows the notice of missing things, and the status line says so", () => {
    const s = scene("audioMissing").state;
    expect(vocalOf(s).clips.map((c) => [c.name, c.audio!.state, c.audio!.file])).toEqual([
      ["歌 テイク 1", "missing", "歌 テイク 1.wav"],
      ["ハモり", "missing", "ハモり.wav"],
    ]);
    expect(s.params.selected).toEqual({
      kind: "clip",
      track: VOCAL,
      clip: 70,
      missing: { audio: 64, file: "歌 テイク 1.wav", wasAt: "はじめての曲/音/歌 テイク 1.wav" },
    });
    expect(s.status.message).toEqual({ id: "audioMissing", count: 2 });
    expect(s.song.file).toBe("saved");
    expect(t("params.audioFileMissing", { file: "歌 テイク 1.wav", path: "はじめての曲/音/歌 テイク 1.wav" })).toBe(
      "歌 テイク 1.wav が見つかりません。前はここにありました: はじめての曲/音/歌 テイク 1.wav",
    );
  });

  it("audioLoading: just after putting in a long mp3. The clip is placed first, and the progress is in another region (audioWork)", () => {
    const s = scene("audioLoading").state;
    const clip = vocalOf(s).clips[0]!;
    expect(clip.audio).toMatchObject({ file: "ドラム.mp3", sampleRate: 44100, bits: null, channels: 2, resampled: true, state: "loading" });
    expect(clip.audio!.stretch).toMatchObject({ follow: false, sourceBpm: null });
    expect(s.audioWork).toEqual({ jobs: [{ kind: "loading", audio: 62, clip: null, percent: 40 }] });
    expect(t("clip.loading", { percent: s.audioWork.jobs[0]!.percent })).toBe("読みこんでいます 40 %");
    expect(s.status.message).toMatchObject({ id: "audioImported", file: "ドラム.mp3", tick: 4 * BAR });
  });

  it("audioKinds (R11-Audio): normal, fade and gain, stretch and pitch marks with the making progress, missing", () => {
    const s = scene("audioKinds").state;
    const clips = vocalOf(s).clips;
    expect(clips.map((c) => c.audio!.state)).toEqual(["ready", "ready", "ready", "missing"]);
    expect(clips[1]!.audio).toMatchObject({ fadeIn: TPQ, fadeOut: 2 * TPQ, gainDb: -3 });
    // 「×1.25」 is the song tempo ÷ this sound's tempo (96 ÷ 76.8), 「+2」 is semitones.
    expect(clips[2]!.audio!.stretch).toMatchObject({ follow: true, sourceBpm: 76.8, semitones: 2 });
    expect(s.timeline.tempoBpm / clips[2]!.audio!.stretch.sourceBpm!).toBeCloseTo(1.25, 9);
    expect(s.audioWork.jobs).toEqual([{ kind: "rendering", audio: 63, clip: 74, percent: 34 }]);
  });

  it("vocalRec: the number of the sound being recorded and its peaks (growing)", async () => {
    const rec = scene("vocalRec").state.transport.recordingVocals[0]!;
    expect([rec.audio, rec.sampleRate, rec.firstFrame]).toEqual([59, 48000, 0]);
    const peaks = decodePeaks(await createFakeTransport().peaks(rec.audio, 0, 0, 100));
    expect([peaks.growing, peaks.count, peaks.total]).toEqual([true, 100, 1055]);
  });

  it("the fake peaks know the sound of every audio clip in every scene", () => {
    for (const name of Object.keys(SCENES)) {
      const s = scene(name).state;
      for (const c of s.song.tracks.flatMap((tr) => tr.clips)) if (c.audio) expect(FAKE_AUDIO.has(c.audio.audio), `${name}: ${c.name}`).toBe(true);
    }
  });

  it("each scene name appears once (apart from the older paramMany6, no name is defined twice)", () => {
    const here = decodeURIComponent(new URL(import.meta.url).pathname);
    const source = new TextDecoder().decode(readFileSync(`${here.slice(0, here.lastIndexOf("/"))}/../src/bridge/fake.ts`));
    const names = [...source.matchAll(/^SCENES\.(\w+) = /gm)].map((m) => m[1]!);
    const twice = [...new Set(names.filter((n, i) => names.indexOf(n) !== i))].sort();
    // the first mixer step is in the first table of SCENES, so it does not show up here.
    expect(twice).toEqual(["paramMany6"]);
    for (const name of ["audioEdit", "audioTall", "audioDropOne", "audioDrop", "audioDropRight", "audioDropMixed", "audioMissing", "audioLoading", "audioKinds"]) {
      expect(names.filter((n) => n === name).length, name).toBe(1);
    }
  });
});

describe("fake intents (only enough for the pictures and the tests)", () => {
  it("setTallTrack: only vocal tracks. The same track again or null goes back. Not a step", async () => {
    const f = createFakeTransport(scene("audioEdit").state);
    const send = async (c: Command) => {
      await f.send(c);
      return (await f.snapshot()).state;
    };
    const before = (await f.snapshot()).state.song.undoCount;
    expect((await send({ type: "setTallTrack", track: 11 })).arrange.tallTrack).toBeNull();
    expect((await send({ type: "setTallTrack", track: VOCAL })).arrange.tallTrack).toBe(VOCAL);
    expect((await send({ type: "setTallTrack", track: VOCAL })).arrange.tallTrack).toBeNull();
    await send({ type: "setTallTrack", track: VOCAL });
    const s = await send({ type: "setTallTrack", track: null });
    expect([s.arrange.tallTrack, s.song.undoCount]).toEqual([null, before]);
  });

  it("setClipField: gain is −60 to +12, fades go up to 「length − the other one」. Each is one step", async () => {
    const f = createFakeTransport(scene("audioEdit").state);
    const send = async (c: Command) => {
      await f.send(c);
      return (await f.snapshot()).state;
    };
    const before = (await f.snapshot()).state.song.undoCount;
    let s = await send({ type: "setClipField", clip: 72, field: "gainDb", value: 0 });
    expect([audioOf(s, 72).gainDb, s.song.undoCount, s.song.file]).toEqual([0, before + 1, "changed"]);
    s = await send({ type: "setClipField", clip: 72, field: "gainDb", value: 40 });
    expect(audioOf(s, 72).gainDb).toBe(12);
    s = await send({ type: "setClipField", clip: 72, field: "fadeOut", value: 9 * BAR });
    expect([audioOf(s, 72).fadeIn, audioOf(s, 72).fadeOut]).toEqual([TPQ / 4, 2 * BAR - TPQ / 4]);
    // the same value, a value that is not a number, a keyboard clip: nothing happens.
    const count = s.song.undoCount;
    s = await send({ type: "setClipField", clip: 72, field: "gainDb", value: 12 });
    s = await send({ type: "setClipField", clip: 72, field: "gainDb", value: Number.NaN });
    s = await send({ type: "setClipField", clip: s.song.tracks[0]!.clips[0]!.id, field: "gainDb", value: -6 });
    expect(s.song.undoCount).toBe(count);
  });

  it("remove from the song (missing sound): the clips of that sound go away and the panel is empty. One step", async () => {
    const f = createFakeTransport(scene("audioMissing").state);
    await f.send({ type: "removeMissing", missing: { kind: "audio", audio: 64 } });
    const s = (await f.snapshot()).state;
    expect(vocalOf(s).clips.map((c) => c.name)).toEqual(["ハモり"]);
    expect([s.params.selected, s.arrange.selectedClips, s.song.undoCount]).toEqual([null, [], 15]);
  });

  it("intents that do nothing yet: split, put in, where it was dropped, search", () => {
    const s = scene("audioEdit").state;
    for (const c of [
      { type: "splitClips" },
      { type: "importAudio", track: null },
      { type: "dropAudio", drop: 1, track: null, tick: 0, mode: "separate" },
      { type: "findAudio", audio: 64 },
    ] satisfies Command[]) {
      expect(reduceAudio(s, c)).toEqual([]);
    }
    expect(reduceAudio(s, { type: "undo" })).toBeNull();
    expect(reduceAudio(s, { type: "removeMissing", missing: { kind: "sf2", file: "x.sf2" } })).toBeNull();
  });
});
