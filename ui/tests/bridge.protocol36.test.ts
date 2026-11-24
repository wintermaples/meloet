// Protocol 36 (the owner, 2026-11-24): MIDI export from the dialog 「書き出し」 (spec 3 "MIDI export", spec 7 A-31). The
// shapes, the fake's reading of 「1-3, 5」, its scenes and its copy of the two intents.
import { describe, expect, it } from "vitest";
import type { Command, ExportChange, ExportDialogView, ExportMidiProblem, ExportRunView } from "../src/bridge";
import { EXPORT_FORMATS, PROTOCOL_VERSION } from "../src/bridge";
import { fakeScene, reduceExport, type FakeState } from "../src/bridge/fake";
import { fakeMidiTaken, readTrackNumbers } from "../src/bridge/fakeMidi";

const scene = (name: string) => fakeScene(name)!.state as FakeState;
const set = (change: ExportChange): Command => ({ type: "setExport", change });

/** Sends the intents in order and returns the state with the changed regions put in. */
function after(state: FakeState, ...commands: Command[]): FakeState {
  let s = state;
  for (const command of commands) {
    const changes = reduceExport(s, command);
    expect(changes, command.type).not.toBeNull();
    s = structuredClone(s);
    for (const c of changes!) (s as unknown as Record<string, unknown>)[c.section] = c.value;
  }
  return s;
}

const taken = (d: ExportDialogView) => d.midiTaken.map((t) => `${t.number} ${t.name}`);
const vocal = (d: ExportDialogView) => d.midiVocal.map((t) => `${t.number} ${t.name}`);

describe("shape of protocol 36", () => {
  it("version 36; MIDI is the last format and has no bits; the intents and the problems", () => {
    expect(PROTOCOL_VERSION).toBe(36);
    expect(EXPORT_FORMATS.at(-1)).toEqual({ format: "midi", bits: [] });
    const changes: ExportChange[] = [
      { kind: "midiTracks", tracks: "selected" },
      { kind: "midiNumbers", text: "1-3, 5" },
    ];
    expect(changes.map((c) => c.kind)).toEqual(["midiTracks", "midiNumbers"]);
    const problems: ExportMidiProblem[] = [{ kind: "noSuchTrack", number: 8 }, { kind: "unreadable" }, { kind: "noTrack" }];
    expect(problems.map((p) => p.kind)).toEqual(["noSuchTrack", "unreadable", "noTrack"]);
    const done: ExportRunView = {
      state: "done",
      file: "はじめての曲.mid",
      files: 1,
      place: "~/音楽/はじめての曲.mid",
      lengthS: 182,
      sampleRate: 48000,
      bits: "bits24",
      wave: [],
      lufs: null,
      truePeakDb: null,
      mp3Kbps: null,
      midiTracks: 3,
    };
    expect(done.state === "done" && done.midiTracks).toBe(3);
  });
});

describe("the fake's reading of 「1-3, 5」", () => {
  it("numbers and ranges split by commas; spaces do not count; a range written backwards; empty parts passed by", () => {
    expect(readTrackNumbers("1-3, 5")).toEqual([
      [1, 3],
      [5, 5],
    ]);
    expect(readTrackNumbers(" 1 - 3 ,5 ")).toEqual([
      [1, 3],
      [5, 5],
    ]);
    expect(readTrackNumbers("5-3")).toEqual([[3, 5]]);
    expect(readTrackNumbers("1,,2,")).toEqual([
      [1, 1],
      [2, 2],
    ]);
    expect(readTrackNumbers("")).toEqual([]);
    expect(readTrackNumbers("007")).toEqual([[7, 7]]);
  });

  it("what the Japanese input types: full-width digits, 「、」 「，」 as commas, 「－」 「ー」 「‐」 「〜」 「~」 as hyphens", () => {
    expect(readTrackNumbers("１－３、５")).toEqual([
      [1, 3],
      [5, 5],
    ]);
    expect(readTrackNumbers("1ー2，3‐4,5〜6, 7~8")).toEqual([
      [1, 2],
      [3, 4],
      [5, 6],
      [7, 8],
    ]);
    expect(readTrackNumbers("１\u3000－\u3000３")).toEqual([[1, 3]]);
  });

  it("anything else cannot be read; a space between two digits", () => {
    for (const text of ["a", "1-", "-3", "1-2-3", "1.5", "1;2", "1 2", "1-2 3", "１ ２", "99999999999"]) expect(readTrackNumbers(text), text).toBeNull();
  });
});

describe("what a choice takes (the fake)", () => {
  // 1 ピアノ, 2 ベース, 3 ドラム, 4 歌 (vocal), 5 パッド; 1 ピアノ selected.
  const song = scene("exportMidi").song;

  it("「ぜんぶ」 passes the vocal track by; 「選んでいるトラック」 is the selected keys track", () => {
    const all = fakeMidiTaken(song, "all", "");
    expect(all.midiTaken.map((t) => t.number)).toEqual([1, 2, 3, 5]);
    expect([all.midiVocal, all.midiProblem, all.midiSelectedUsable]).toEqual([[], null, true]);
    expect(fakeMidiTaken(song, "selected", "").midiTaken).toEqual([{ number: 1, name: "ピアノ" }]);
    const onVocal = fakeMidiTaken({ ...song, playTarget: song.tracks[3]!.id }, "selected", "");
    expect([onVocal.midiSelectedUsable, onVocal.midiTaken, onVocal.midiProblem]).toEqual([false, [], { kind: "noTrack" }]);
  });

  it("「番号で」: vocal tracks named are told apart; the smallest number with no track; text that cannot be read; nothing taken", () => {
    const numbers = (text: string) => fakeMidiTaken(song, "numbers", text);
    expect(numbers("5, 1-2, 4")).toMatchObject({ midiTaken: [{ number: 1 }, { number: 2 }, { number: 5 }], midiVocal: [{ number: 4, name: "歌" }], midiProblem: null });
    expect(numbers("1-3, 9").midiProblem).toEqual({ kind: "noSuchTrack", number: 9 });
    expect(numbers("12-4, 8").midiProblem).toEqual({ kind: "noSuchTrack", number: 6 });
    expect(numbers("0-2").midiProblem).toEqual({ kind: "noSuchTrack", number: 0 });
    expect(numbers("1-3, 9").midiTaken.map((t) => t.number)).toEqual([1, 2, 3]);
    expect(numbers("1-x")).toMatchObject({ midiTaken: [], midiProblem: { kind: "unreadable" } });
    expect(numbers("4")).toMatchObject({ midiTaken: [], midiVocal: [{ number: 4 }], midiProblem: { kind: "noTrack" } });
    expect(numbers("").midiProblem).toEqual({ kind: "noTrack" });
    expect(numbers("1 2")).toMatchObject({ midiTaken: [], midiVocal: [], midiProblem: { kind: "unreadable" } });
    // Named twice, taken once.
    expect(numbers("1, 1-2, 2").midiTaken.map((t) => t.number)).toEqual([1, 2]);
  });
});

describe("fake scenes", () => {
  it("exportMidi: 「ぜんぶ」 takes four tracks, 「トラックごと」 would write four files", () => {
    const d = scene("exportMidi").export.dialog!;
    expect(d).toMatchObject({ format: "midi", place: "~/音楽/はじめての曲.mid", tracks: 4, canExport: true, midiTracks: "all", pictureUsable: false });
    expect(taken(d)).toEqual(["1 ピアノ", "2 ベース", "3 ドラム", "5 パッド"]);
    expect([vocal(d), d.midiProblem]).toEqual([[], null]);
  });

  it("exportMidiNumbers: 「1-2, 4」 takes two, 「4 歌」 in gray", () => {
    const d = scene("exportMidiNumbers").export.dialog!;
    expect([taken(d), vocal(d), d.midiProblem, d.canExport, d.tracks]).toEqual([["1 ピアノ", "2 ベース"], ["4 歌"], null, true, 2]);
  });

  it("exportMidiProblem: 「1-3, 9」 says there is no track 9, and 「書き出す」 is disabled", () => {
    const d = scene("exportMidiProblem").export.dialog!;
    expect([d.midiNumbers, d.midiProblem, d.canExport]).toEqual(["1-3, 9", { kind: "noSuchTrack", number: 9 }, false]);
  });

  it("exportDoneMidi and exportDoneMidiTracks: MIDI was written, no waveform", () => {
    expect(scene("exportDoneMidi").export.run).toMatchObject({ state: "done", file: "はじめての曲.mid", files: 1, midiTracks: 3, wave: [] });
    expect(scene("exportDoneMidiTracks").export.run).toMatchObject({ state: "done", files: 3, midiTracks: 3, wave: [] });
  });
});

describe("fake intents", () => {
  it("MIDI is chosen and left: the bits chosen stay, the ending follows, 「トラックごと」 counts the tracks taken", () => {
    const open = after(scene("exportDialog"), set({ kind: "bits", bits: "bits16" }));
    const midi = after(open, set({ kind: "format", format: "midi" }));
    expect(midi.export.dialog).toMatchObject({ format: "midi", bits: "bits16", bitsChoices: [], place: "~/音楽/はじめての曲.mid", tracks: 4, pictureUsable: false });
    const wav = after(midi, set({ kind: "midiNumbers", text: "9" }), set({ kind: "midiTracks", tracks: "numbers" }), set({ kind: "format", format: "wav" }));
    expect(wav.export.dialog).toMatchObject({ format: "wav", bits: "bits16", place: "~/音楽/はじめての曲.wav", tracks: 4, canExport: true, midiTracks: "numbers", midiNumbers: "9" });
  });

  it("the choices of 「入れるトラック」; 「選んでいるトラック」 does nothing while a vocal track is selected", () => {
    const s = scene("exportMidi");
    const numbers = after(s, set({ kind: "midiTracks", tracks: "numbers" }), set({ kind: "midiNumbers", text: "2-1" }));
    expect([taken(numbers.export.dialog!), numbers.export.dialog!.canExport]).toEqual([["1 ピアノ", "2 ベース"], true]);
    const unreadable = after(numbers, set({ kind: "midiNumbers", text: "1-" }));
    expect([unreadable.export.dialog!.midiProblem, unreadable.export.dialog!.canExport]).toEqual([{ kind: "unreadable" }, false]);
    expect(reduceExport(unreadable, { type: "startExport" })).toEqual([]);
    const onVocal = structuredClone(s);
    onVocal.song.playTarget = onVocal.song.tracks[3]!.id;
    onVocal.export.dialog = { ...onVocal.export.dialog!, midiSelectedUsable: false };
    expect(reduceExport(onVocal, set({ kind: "midiTracks", tracks: "selected" }))).toEqual([]);
    const selected = after(s, set({ kind: "midiTracks", tracks: "selected" }));
    expect(taken(selected.export.dialog!)).toEqual(["1 ピアノ"]);
  });

  it("「書き出す」 under MIDI is done at once: one file, or a file for each track", () => {
    const one = after(scene("exportMidi"), { type: "startExport" });
    expect(one.export.dialog).toBeNull();
    expect(one.export.run).toMatchObject({ state: "done", file: "はじめての曲.mid", files: 1, midiTracks: 4, wave: [], lufs: null, mp3Kbps: null });
    const tracks = after(scene("exportMidiNumbers"), set({ kind: "range", range: "tracks" }), { type: "startExport" });
    expect(tracks.export.run).toMatchObject({ state: "done", file: "ピアノ.mid", files: 2, midiTracks: 2, place: "~/音楽/" });
  });
});
