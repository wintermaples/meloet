// Protocol 23: vocal tracks, inputs, monitor, the deep part of the sound settings, measuring latency, the mic meter,
// the rest of 92 (19-A, 92-3, 92-19). The shapes, the fake scenes, and the (what) texts. The screen parts come in the second step.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { AppState, Command, SelectedParams, StatusMessage, StepName, StepWhat, TrackView } from "../src/bridge";
import { PROTOCOL_VERSION } from "../src/bridge";
import { SCENES, encodeTelemetry, fakeScene, fakeState } from "../src/bridge/fake";
import { LATENCY_CLICKS, RECORD_FORMATS, VOCAL_INPUTS } from "../src/bridge/types";
import { TELEMETRY_HEADER_LEN, TELEMETRY_MIC_LEN, TELEMETRY_VERSION, decodeTelemetry } from "../src/bridge/telemetry";
import { stepNameText, stepWhatText } from "../src/components/history/stepText";
import { t } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const scene = (name: string) => fakeScene(name)!;
const BAR = 4 * TPQ;
const VOCAL = 16;
const vocalOf = (s: AppState): TrackView => s.song.tracks.find((tr) => tr.kind.kind === "vocal")!;
const kindOf = (s: AppState) => {
  const kind = vocalOf(s).kind;
  if (kind.kind !== "vocal") throw new Error("a vocal track");
  return kind;
};

describe("shape of protocol 23", () => {
  it("version 24. A new song has one keyboard track, and keys play there. No input is open", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(26);
    const s = fakeState();
    expect(s.song.tracks.map((tr) => tr.kind)).toEqual([{ kind: "keys" }]);
    expect([s.song.playTarget, s.song.keysTarget]).toEqual([1, 1]);
    expect(s.transport.recordingVocals).toEqual([]);
    expect(s.inputs).toEqual({ list: [], chosen: null, open: false });
    expect(s.workspace.latencyDialog).toBeNull();
    expect(s.audio.recordFormat).toBe("bits24");
    expect(s.audio.latency).toEqual({ reportedMs: null, measuredMs: null, manualMs: null, used: "reported", usedMs: null });
    expect(s.audio.bufferChoices).toContain(s.audio.requestedFrames);
    expect(s.audio.sampleRateChoices).toContain(s.audio.requestedSampleRate);
  });

  it("lists: 3 inputs (in spec 3 order), recording formats 16, 24, 32, and 5 click sounds", () => {
    expect(VOCAL_INPUTS).toEqual(["mono1", "mono2", "stereo12"]);
    expect(VOCAL_INPUTS.map((i) => t(`vocal.input.${i}`))).toEqual(["入口 1 ・ モノ", "入口 2 ・ モノ", "入口 1 ・ 2 ・ ステレオ"]);
    expect(RECORD_FORMATS).toEqual([
      { format: "bits16", bits: 16 },
      { format: "bits24", bits: 24 },
      { format: "bits32", bits: 32 },
    ]);
    expect(LATENCY_CLICKS).toBe(5);
  });

  it("intents: vocal track, input, ●, monitor, deep part of the sound settings, measuring latency", () => {
    const commands: Command[] = [
      { type: "addVocalTrack" },
      { type: "setVocalInput", track: VOCAL, input: "stereo12" },
      { type: "toggleVocalArm", track: VOCAL },
      { type: "toggleMonitor", track: VOCAL },
      { type: "settingsSelectInput", input: "in-usb-mic" },
      { type: "settingsSelectInput", input: null },
      { type: "settingsSetBuffer", frames: 128 },
      { type: "settingsSetSampleRate", hz: 48000 },
      { type: "settingsSetRecordFormat", format: "bits32" },
      { type: "settingsSetManualLatency", ms: 12.5 },
      { type: "settingsSetManualLatency", ms: null },
      { type: "openLatencyMeasure" },
      { type: "startLatencyMeasure" },
      { type: "stopLatencyMeasure" },
      { type: "useMeasuredLatency" },
      { type: "closeLatencyMeasure" },
    ];
    expect(new Set(commands.map((c) => c.type)).size).toBe(14);
  });

  it("kinds of status line text, and the number of tracks removed with it", () => {
    const track = { number: 6, name: "歌" };
    const messages: StatusMessage[] = [
      { id: "noParams", sound: { kind: "builtin", instrument: "grandPiano" } },
      { id: "vocalTrackAdded", track, input: "mono1" },
      { id: "noInput" },
      { id: "monitorFirstOn", latencyMs: 13 },
      { id: "monitorFirstOn", latencyMs: null },
      { id: "micTooLoud" },
      { id: "vocalHasNoPianoRoll" },
      { id: "recordingVocals", keys: null, vocals: [{ track, input: "mono1" }], monitor: true },
      { id: "latencyMeasureOpen" },
      { id: "effectRemoved", insert: { strip: { kind: "track", id: 12 }, slot: 0 }, effect: { kind: "builtin", effect: "compressor" }, lanesRemoved: 2 },
      { id: "insertReplaced", insert: { strip: { kind: "track", id: 12 }, slot: 0 }, effect: { kind: "builtin", effect: "eq4" }, lanesRemoved: 1 },
      { id: "instrumentReplaced", track, instrument: "organ", lanesRemoved: 0 },
      { id: "instrumentReplacedSound", track, sound: { kind: "plugin", name: "Surge XT" }, lanesRemoved: 3 },
    ];
    expect(new Set(messages.map((m) => m.id)).size).toBe(12);
    // the text is already in ja.ts (spec 6).
    expect(t("msg.lanesAlsoGone", { count: 2 })).toBe("（そのパラメータの帯 2 本も消えました）");
    expect(t("msg.noSoundParams", { name: "グランドピアノ" })).toBe("グランドピアノには、動かせるパラメータがありません");
  });

  it("params panel: a sound with no parameters to move (19-A)", () => {
    const builtin: SelectedParams = { kind: "sound", track: 11, sound: { kind: "builtin", instrument: "grandPiano" } };
    const sf2: SelectedParams = { kind: "sound", track: 12, sound: { kind: "sf2", name: "Clean Guitar", file: "FluidR3_GM.sf2" } };
    expect([builtin.kind, sf2.kind]).toEqual(["sound", "sound"]);
    const s = scene("paramsSound").state;
    expect(s.params.selected).toEqual(builtin);
    expect(s.status.message).toEqual({ id: "noParams", sound: { kind: "builtin", instrument: "grandPiano" } });
    expect(s.workspace.layout.rightFront).toBe("params");
  });
});

describe("meter binary (version 4, and 5 from protocol 32): mic meter", () => {
  it("after the tracks, 16 bytes per vocal track. The fake writes it and the screen reads it", () => {
    expect(TELEMETRY_VERSION).toBe(5);
    expect([TELEMETRY_HEADER_LEN, TELEMETRY_MIC_LEN]).toEqual([124, 16]);
    const mics = [
      { track: VOCAL, peak: [0.25, 0.25] as [number, number], clipped: false, stereo: false },
      { track: 99, peak: [1, 0.5] as [number, number], clipped: true, stereo: true },
    ];
    const bytes = encodeTelemetry({ tracks: [[0.5, 0.125]], mics }, 7, 1000, 1);
    expect(bytes.byteLength).toBe(124 + 8 + 2 * 16);
    const read = decodeTelemetry(bytes);
    expect(read.mics).toEqual(mics);
    expect(Array.from(read.trackPeaks)).toEqual([0.5, 0.125]);
    // a song with no vocal tracks has only the head and the tracks.
    expect(decodeTelemetry(encodeTelemetry({}, 1, 0, 2)).mics).toEqual([]);
  });
});

describe("fake scenes (F7-VocalTrack, F7-VocalRec, F7-TwoRec, F7-Latency, F7-SettingsAdv, R11-Fit)", () => {
  // scenes added here. A scene with the same name as an earlier scene overwrites it, because the later definition wins.
  const ROUND_7 = [
    "vocalTrack",
    "vocalNoInput",
    "vocalMonitorOn",
    "vocalTooLoud",
    "vocalNoPianoRoll",
    "vocalRec",
    "twoRec",
    "settingsAdv7",
    "latency",
    "latencyDone",
    "latencyFailed",
    "vocalFit",
    "recoverVocal",
    "paramsSound",
  ];

  it("the names of these scenes each appear only once (they do not overwrite earlier scenes)", () => {
    const here = decodeURIComponent(new URL(import.meta.url).pathname);
    const source = new TextDecoder().decode(readFileSync(`${here.slice(0, here.lastIndexOf("/"))}/../src/bridge/fake.ts`));
    const start = source.indexOf("export const SCENES");
    const end = source.indexOf("\n};", start);
    const inside = [...source.slice(start, end).matchAll(/^ {2}([A-Za-z0-9_]+): (?:\{|[a-zA-Z])/gm)].map((m) => m[1]!);
    const assigned = [...source.matchAll(/^SCENES\.([A-Za-z0-9_]+) = /gm)].map((m) => m[1]!);
    const names = [...inside, ...assigned];
    expect(new Set(names)).toEqual(new Set(Object.keys(SCENES)));
    for (const name of ROUND_7) expect(names.filter((n) => n === name), name).toEqual([name]);
  });

  it("F7-VocalTrack: added 「6 歌」. Selected with ●, keys still play on 「1 ピアノ」 (B-94-1). The input is open and the meter comes", () => {
    const { state: s, meters } = scene("vocalTrack");
    const vocal = vocalOf(s);
    expect([vocal.id, vocal.number, vocal.name]).toEqual([VOCAL, 6, "歌"]);
    expect(vocal.kind).toEqual({ kind: "vocal", input: "mono1", inputReady: true, armed: true, monitor: false });
    expect(s.song.tracks.filter((tr) => tr.kind.kind === "keys").length).toBe(5);
    expect([s.song.playTarget, s.song.keysTarget]).toEqual([VOCAL, 11]);
    expect(s.inputs.open).toBe(true);
    expect(s.inputs.list.find((i) => i.id === s.inputs.chosen)).toMatchObject({ name: "USB マイク", channels: 2 });
    expect(s.status.message).toEqual({ id: "vocalTrackAdded", track: { number: 6, name: "歌" }, input: "mono1" });
    expect(meters.mics).toEqual([{ track: VOCAL, peak: [expect.closeTo(10 ** (-32 / 20)), expect.closeTo(10 ** (-32 / 20))], clipped: false, stereo: false }]);
  });

  it("no input, first monitor, reached 0 dB, no piano roll", () => {
    const none = scene("vocalNoInput").state;
    expect(kindOf(none).inputReady).toBe(false);
    expect([none.inputs.chosen, none.inputs.open]).toEqual([null, false]);
    expect(none.status.message).toEqual({ id: "noInput" });
    const monitor = scene("vocalMonitorOn").state;
    expect(kindOf(monitor).monitor).toBe(true);
    expect(monitor.status.message).toEqual({ id: "monitorFirstOn", latencyMs: 13 });
    const loud = scene("vocalTooLoud");
    expect(loud.meters.mics![0]).toMatchObject({ peak: [1, 1], clipped: true });
    expect(loud.state.status.message).toEqual({ id: "micTooLoud" });
    expect(scene("vocalNoPianoRoll").state.status.message).toEqual({ id: "vocalHasNoPianoRoll" });
  });

  it("F7-VocalRec: recording only vocals. No keyboard recording, one vocal track", () => {
    const { state: s, meters } = scene("vocalRec");
    expect(s.transport.state).toBe("recording");
    expect(s.transport.recording).toBeNull();
    expect(s.transport.recordingVocals).toEqual([{ track: VOCAL, from: 4 * BAR, take: 1, clip: null, audio: 59, sampleRate: 48000, firstFrame: 0 }]);
    expect(s.status.message).toEqual({ id: "recordingVocals", keys: null, vocals: [{ track: { number: 6, name: "歌" }, input: "mono1" }], monitor: true });
    expect(meters.position).toMatchObject({ tick: 6 * BAR + TPQ, state: "recording" });
  });

  it("F7-TwoRec: with 「1 ピアノ」 selected, also records to 「6 歌」 which has ●. One count-in and one recording number", () => {
    const { state: s, live } = scene("twoRec");
    expect([s.song.playTarget, s.song.keysTarget]).toEqual([11, 11]);
    expect(kindOf(s)).toMatchObject({ armed: true, monitor: true });
    expect(s.transport.recording).toMatchObject({ id: 1, track: 11, from: 4 * BAR });
    expect(s.transport.recordingVocals.map((v) => [v.track, v.from])).toEqual([[VOCAL, 4 * BAR]]);
    expect(live.recording).toBe(1);
    expect(s.status.message).toMatchObject({ id: "recordingVocals", keys: { number: 1, name: "ピアノ" } });
  });

  it("F7-SettingsAdv: running at 256, not the 128 that was asked for. The latency is the number the PC says", () => {
    const s = scene("settingsAdv7").state;
    expect(s.workspace.settings).toEqual({ draftOutput: "out-headphones", advancedOpen: true });
    expect([s.audio.requestedFrames, s.audio.actualFrames]).toEqual([128, 256]);
    expect([s.audio.requestedSampleRate, s.audio.sampleRate]).toEqual([48000, 48000]);
    expect(s.audio.latency).toEqual({ reportedMs: 8, measuredMs: null, manualMs: null, used: "reported", usedMs: 8 });
    expect(s.inputs.list[0]).toMatchObject({ id: s.inputs.chosen, channels: 2 });
    expect(t("settings.notAsRequested", { requested: 128, actual: 256 })).toContain("頼んだ 128 ではなく、256 で動いています");
  });

  it("F7-Latency: measured up to 3 / 5. When done 「この数を使う」, when too small 「もう一度」", () => {
    const s = scene("latency").state;
    expect(s.workspace.latencyDialog).toEqual({ measure: { state: "measuring", done: 3, of: LATENCY_CLICKS, meanMs: 12.4 } });
    expect(s.transport.canRecord).toBe(false);
    expect(s.status.message).toEqual({ id: "latencyMeasureOpen" });
    expect(t("latency.measuring", { done: 3, total: 5 })).toBe("測っています 3 / 5");
    expect(scene("latencyDone").state.workspace.latencyDialog).toEqual({ measure: { state: "done", measuredMs: 12.4 } });
    expect(scene("latencyFailed").state.workspace.latencyDialog).toEqual({ measure: { state: "failed", reason: "tooQuiet" } });
  });

  it("R11-Fit: in a narrow window, the keyboard and mic meters come at the same time. A vocal row left in the recovery file hint", () => {
    const fit = scene("vocalFit");
    expect(fit.meters.keys).toEqual([60, 64, 67]);
    expect(fit.meters.mics!.length).toBe(1);
    expect(fit.state.workspace.unfitScales).toEqual([125, 150]);
    const kept = scene("recoverVocal").state.workspace.recovery;
    expect(kept).toMatchObject({ kind: "recovered", vocals: { count: 1, until: 8 * BAR + 2 * TPQ } });
    expect(t("recovery.vocalKept", { count: 1, position: "9. 3. 0000" })).toBe("録っていた歌 1 つを、止まる直前（9. 3. 0000 ごろ）まで残しました。");
  });
});

describe("「（何を）」 in history rows (spec 3 (92-3))", () => {
  const ctx = { song: fakeState().song, library: fakeState().library };
  const pad = { number: 4, name: "パッド" };
  const says = (name: StepName["kind"], what: StepWhat) => stepWhatText({ kind: name } as StepName, what, ctx);

  it("1 note is the note name, quantize is the grid, velocity is before and after, CC is the name, color, instrument before and after", () => {
    expect(says("writeNote", { kind: "note", key: 60 })).toBe("C4");
    expect(says("quantize", { kind: "quantized", division: 16, triplet: false, count: 12 })).toBe("1/16 ・ 12 ノート");
    expect(says("changeVelocity", { kind: "velocity", count: 3, from: 96, to: 110 })).toBe("3 ノート ・ 96 → 110");
    expect(says("changeVelocity", { kind: "velocity", count: 3, from: null, to: null })).toBe("3 ノート");
    expect(says("moveControls", { kind: "controlPoints", control: { kind: "cc", number: 64 }, count: 4 })).toBe("サステイン ・ 4 点");
    expect(says("deleteControls", { kind: "controlPoints", control: null, count: 4 })).toBe("4 点");
    expect(says("setTrackColor", { kind: "color", track: pad, color: 3 })).toBe("4 パッド ・ 色 3");
    expect(
      says("setInstrument", {
        kind: "soundReplaced",
        track: { number: 1, name: "ピアノ" },
        from: { kind: "builtin", instrument: "grandPiano" },
        to: { kind: "builtin", instrument: "organ" },
      }),
    ).toBe("1 ピアノ ・ グランドピアノ → パイプオルガン");
  });

  it("step that changes the vocal input (name not final: builder:2026-11-21)", () => {
    expect(stepNameText({ kind: "setVocalInput" })).toBe("歌の入口を変える");
    expect(says("setVocalInput", { kind: "vocalInput", track: { number: 6, name: "歌" }, from: "mono1", to: "stereo12" })).toBe(
      "6 歌 ・ 入口 1 ・ モノ → 入口 1 ・ 2 ・ ステレオ",
    );
  });
});
