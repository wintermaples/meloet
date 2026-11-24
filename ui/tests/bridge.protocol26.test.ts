// Protocol 26: export. The shapes, the fake scenes and the fake intents. The screen parts come in the second step.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Command, ExportChange, ExportCheck, ExportRunView, ExportView, StatusMessage } from "../src/bridge";
import { EXPORT_FORMATS, EXPORT_TAIL_MAX_S, PROTOCOL_VERSION } from "../src/bridge";
import { fakeExportWave, fakeScene, fakeState, reduceExport, type FakeState } from "../src/bridge/fake";
import App from "../src/App.svelte";
import { t } from "../src/text";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { mockScreenText } from "./design-sources";
import { current } from "../src/state/app.svelte";

// jsdom has no WebGL2.
vi.mock("../src/gl/Surface.svelte", () => ({ default: () => {} }));

const scene = (name: string) => fakeScene(name)!.state as FakeState;
const BAR = 4 * TPQ;
const squash = (s: string) => s.replace(/\s+/g, "");

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
const set = (change: ExportChange): Command => ({ type: "setExport", change });

describe("shape of protocol 26", () => {
  it("version 26. Formats are wav, flac, mp3 and midi (mp3 and midi have no bits). 32 bit is wav only. The tail is at most 10 seconds", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(26);
    expect(EXPORT_FORMATS).toEqual([
      { format: "wav", bits: ["bits16", "bits24", "bits32"] },
      { format: "flac", bits: ["bits16", "bits24"] },
      { format: "mp3", bits: [] },
      { format: "midi", bits: [] },
    ]);
    expect(EXPORT_TAIL_MAX_S).toBe(10);
  });

  it("a new song has no dialog and no progress", () => {
    expect(fakeState().export).toEqual({ dialog: null, run: null } satisfies ExportView);
  });

  it("7 intents. None of them is a step (they do not change the song)", () => {
    const changes: ExportChange[] = [
      { kind: "range", range: "loop" },
      { kind: "format", format: "flac" },
      { kind: "advancedOpen", open: true },
      { kind: "sampleRate", hz: 44100 },
      { kind: "bits", bits: "bits16" },
      { kind: "tail", tail: "off" },
      { kind: "realtime", on: true },
    ];
    const commands: Command[] = [
      { type: "openExport" },
      { type: "closeExport" },
      ...changes.map(set),
      { type: "chooseExportPlace" },
      { type: "startExport" },
      { type: "cancelExport" },
      { type: "showExported" },
    ];
    expect(new Set(commands.map((c) => c.type)).size).toBe(7);
    const s = scene("exportDialog");
    for (const command of commands) {
      const changes = reduceExport(s, command);
      expect(changes?.some((c) => c.section === "song" || c.section === "history") ?? false, command.type).toBe(false);
    }
  });

  it("the check is not yet, counting, done, failed. The progress is exporting, finished, cancelled, failed", () => {
    const checks: ExportCheck[] = [
      { state: "waiting" },
      { state: "counting", percent: 38, peakDb: -3.5, clips: false, over: 0, lufs: null, truePeakDb: null },
      { state: "done", peakDb: null, clips: false, over: 0, lufs: null, truePeakDb: null },
      { state: "failed", reason: { kind: "plugin", name: "Surge XT" } },
    ];
    expect(checks.map((c) => c.state)).toEqual(["waiting", "counting", "done", "failed"]);
    const runs: ExportRunView[] = [
      { state: "running", percent: 46, file: "はじめての曲.wav", remainingS: 40, realtime: false, realtimeForced: false, lengthS: 182 },
      { state: "done", file: "はじめての曲.wav", files: 1, place: "~/音楽/はじめての曲.wav", lengthS: 182, sampleRate: 48000, bits: "bits24", wave: [], lufs: null, truePeakDb: null, mp3Kbps: null, midiTracks: null },
      { state: "cancelled" },
      { state: "failed", reason: { kind: "cannotWrite" } },
    ];
    expect(runs.map((r) => r.state)).toEqual(["running", "done", "cancelled", "failed"]);
    const message: StatusMessage = { id: "exportDialogOpen" };
    expect(message.id).toBe("exportDialogOpen");
  });
});

describe("fake scenes (mock F7-Export, F7-Exporting, R11-Fit)", () => {
  it("exportDialog (F7-Export): the whole song as wav. The check is done, −0.6 dB, no clipping", () => {
    const s = scene("exportDialog");
    expect(s.export.run).toBeNull();
    expect(s.export.dialog).toEqual({
      range: "song",
      loopRange: { start: 4 * BAR, end: 8 * BAR },
      tracks: 4,
      span: { start: 0, end: 16 * BAR },
      format: "wav",
      formats: ["wav", "flac", "mp3", "midi"],
      place: "~/音楽/はじめての曲.wav",
      check: { state: "done", peakDb: -0.6, clips: false, over: 0, lufs: -12.5, truePeakDb: -0.5 },
      solo: null,
      stoppedPlugins: [],
      advancedOpen: false,
      sampleRate: 48000,
      sampleRateChoices: [44100, 48000, 88200, 96000],
      bits: "bits24",
      bitsChoices: ["bits16", "bits24", "bits32"],
      mp3Kbps: 192,
      mp3KbpsChoices: [128, 192, 256, 320],
      title: "はじめての曲",
      artist: "",
      picture: null,
      pictureUsable: false,
      tail: "untilSilent",
      realtime: false,
      realtimeForced: false,
      canExport: true,
      loudness: "asIs",
      canMatch: true,
      loudnessResult: { gainDb: -1.5, limited: false, lufs: -14 },
      midiTracks: "all",
      midiNumbers: "",
      midiSelectedUsable: true,
      midiTaken: [
        { number: 1, name: "ピアノ" },
        { number: 2, name: "ベース" },
        { number: 3, name: "ドラム" },
        { number: 4, name: "パッド" },
      ],
      midiVocal: [],
      midiProblem: null,
    });
    expect(s.status.message).toEqual({ id: "exportDialogOpen" });
    expect([s.song.name, s.song.file, s.song.undoCount]).toEqual(["はじめての曲", "saved", 14]);
  });

  it("check in progress, clipping, stopped plugin, advanced, failed", () => {
    expect(scene("exportWaiting").export.dialog).toMatchObject({ check: { state: "waiting" }, canExport: false });
    expect(scene("exportCounting").export.dialog).toMatchObject({ check: { state: "counting", percent: 38, peakDb: -3.5, clips: false }, canExport: true });
    expect(scene("exportClips").export.dialog).toMatchObject({ check: { state: "done", peakDb: 1.4, clips: true }, canExport: true });
    const stopped = scene("exportStopped");
    const drums = stopped.song.tracks[2]!;
    expect([drums.number, drums.name]).toEqual([3, "ドラム"]);
    expect(stopped.export.dialog!.stoppedPlugins).toEqual([{ name: "Brick EQ", slot: { kind: "insert", insert: { strip: { kind: "track", id: drums.id }, slot: 0 } } }]);
    expect(scene("exportAdvanced").export.dialog).toMatchObject({
      range: "loop",
      span: { start: 4 * BAR, end: 8 * BAR },
      format: "flac",
      place: "~/音楽/はじめての曲.flac",
      bitsChoices: ["bits16", "bits24"],
      advancedOpen: true,
    });
    expect(scene("exportCheckFailed").export.dialog).toMatchObject({ check: { state: "failed", reason: { kind: "plugin", name: "Surge XT" } }, canExport: false });
  });

  it("exporting (F7-Exporting): exporting a 6-track song at 46 %, 40 seconds left. The dialog is closed", () => {
    const s = scene("exporting");
    expect(s.export).toEqual({ dialog: null, run: { state: "running", percent: 46, file: "はじめての曲.wav", remainingS: 40, realtime: false, realtimeForced: false, lengthS: 182 } });
    expect(s.song.tracks.map((tr) => `${tr.number} ${tr.name}`)).toEqual(["1 ピアノ", "2 ベース", "3 ドラム", "4 パッド", "5 Surge XT", "6 歌"]);
    expect(s.song.tracks[5]!.clips.map((c) => [c.name, c.start, c.end])).toEqual([["歌 テイク 1", 4 * BAR, 6 * BAR]]);
    expect([s.song.file, s.song.keysTarget, s.song.playTarget]).toEqual(["saved", s.song.tracks[0]!.id, s.song.tracks[0]!.id]);
    expect(s.transport.state).toBe("stopped");
  });

  it("exporting in real time, finished, cancelled, failed", () => {
    expect(scene("exportingRealtime").export.run).toMatchObject({ state: "running", realtime: true, realtimeForced: true, lengthS: 182 });
    expect(scene("exportDone").export.run).toEqual({
      state: "done",
      file: "はじめての曲.wav",
      files: 1,
      place: "~/ミュージック/はじめての曲/はじめての曲.wav",
      lengthS: 182,
      sampleRate: 48000,
      mp3Kbps: null,
      bits: "bits24",
      wave: fakeExportWave(),
      lufs: -14.0,
      truePeakDb: -1.0,
      midiTracks: null,
    });
    expect(scene("exportCancelled").export.run).toEqual({ state: "cancelled" });
    expect(scene("exportFailed").export.run).toEqual({ state: "failed", reason: { kind: "cannotWrite" } });
  });
});

describe("fake intents", () => {
  it("open → choose → export → cancel", () => {
    const start = scene("tracks6");
    expect(reduceExport(start, { type: "closeExport" })).toEqual([]);
    expect(reduceExport(start, { type: "startExport" })).toEqual([]);
    expect(reduceExport(start, { type: "goArrange" })).toBeNull();

    const open = after(start, { type: "openExport" });
    expect(open.export.dialog).toMatchObject({ range: "song", format: "wav", tracks: 6, canExport: true });
    expect(open.status.message).toEqual({ id: "exportDialogOpen" });
    // opening it again gives the same.
    expect(reduceExport(open, { type: "openExport" })).toEqual([]);

    const flac = after(open, set({ kind: "bits", bits: "bits32" }), set({ kind: "format", format: "flac" }));
    expect(flac.export.dialog).toMatchObject({ format: "flac", bits: "bits24", bitsChoices: ["bits16", "bits24"] });
    expect(flac.export.dialog!.place.endsWith(".flac")).toBe(true);
    // flac has no 32 bit. A sample rate not in the list does not go in either.
    const same = after(flac, set({ kind: "bits", bits: "bits32" }), set({ kind: "sampleRate", hz: 12345 }));
    expect(same.export.dialog).toMatchObject({ bits: "bits24", sampleRate: 48000 });

    const looped = after(flac, set({ kind: "range", range: "loop" }), set({ kind: "tail", tail: "off" }), set({ kind: "advancedOpen", open: true }), set({ kind: "realtime", on: true }));
    expect(looped.export.dialog).toMatchObject({ range: "loop", span: open.song.loopRange, tail: "off", advancedOpen: true, realtime: true });
    const tracks = after(looped, set({ kind: "range", range: "tracks" }));
    expect(tracks.export.dialog!.place.endsWith("/")).toBe(true);

    const running = after(looped, { type: "startExport" });
    expect(running.export.dialog).toBeNull();
    expect(running.export.run).toMatchObject({ state: "running", realtime: true, realtimeForced: false });
    expect(running.status.message).toBeNull();
    // while exporting, the dialog cannot open.
    expect(reduceExport(running, { type: "openExport" })).toEqual([]);

    const cancelled = after(running, { type: "cancelExport" });
    expect(cancelled.export).toEqual({ dialog: null, run: { state: "cancelled" } });
    expect(reduceExport(cancelled, { type: "cancelExport" })).toEqual([]);
    // after it finishes, it can open again.
    expect(after(cancelled, { type: "openExport" }).export).toMatchObject({ run: null, dialog: { range: "song" } });
  });

  it("in a song without a loop, the loop cannot be chosen. Closing removes the dialog and the status line text", () => {
    const open = after(fakeState(), { type: "openExport" });
    expect(open.export.dialog).toMatchObject({ loopRange: null, canExport: false });
    expect(reduceExport(open, set({ kind: "range", range: "loop" }))).toEqual([]);
    const closed = after(open, { type: "closeExport" });
    expect(closed.export).toEqual({ dialog: null, run: null });
    expect(closed.status.message).toBeNull();
  });
});

describe("wording (as the mock writes it)", () => {
  it("text of the F7-Export dialog", () => {
    const mock = squash(mockScreenText("F7-Export"));
    const lines = [
      t("export.title"),
      t("export.range"),
      t("export.rangeSong"),
      t("export.rangeLoop", { from: 5, to: 9 }),
      t("export.rangeTracks"),
      t("export.spanSong", { start: "1. 1. 0000", end: "17. 1. 0000" }),
      t("export.format"),
      t("export.place"),
      t("export.choosePlace"),
      t("export.check"),
      `${t("export.peak")}${t("export.peakDb", { peak: "−0.6" })}${t("export.noClipping")}`,
      t("export.noStoppedPlugins"),
      `${t("button.details")}${t("export.detailsSummary", { rate: 48000, bits: 24, seconds: EXPORT_TAIL_MAX_S })}`,
      `${t("button.cancel")}Esc${t("export.start")}Enter`,
      t("status.songDialogOpen", { dialog: t("export.title") }),
    ];
    for (const line of lines) expect(mock, line).toContain(squash(line));
    expect(mock).toContain(squash(`${t("export.formatWav")} ${t("export.formatFlac")}`));
  });

  it("the F7-Exporting status line text is the same as spec 6 「書き出している」", () => {
    const mock = squash(mockScreenText("F7-Exporting"));
    const parts = [t("export.running"), t("export.runningPercent", { percent: 46 }), t("export.runningRest", { file: "はじめての曲.wav", left: "40 秒" })];
    expect(mock).toContain(squash(parts.join("")));
    expect(squash(`${parts[0]} ${parts[1]} ・ ${parts[2]}`)).toBe(squash(t("msg.exporting", { percent: 46, file: "はじめての曲.wav", left: "40 秒" })));
  });

  it("the stopped plugin row is as in F-Export", () => {
    const mock = squash(mockScreenText("F-Export"));
    for (const line of [t("export.stoppedPlugins"), t("export.stoppedPlugin", { plugin: "Brick EQ", strip: "3 ドラム" }), t("export.stoppedSkipped")]) expect(mock, line).toContain(squash(line));
  });
});

describe("the current screen does not break in the export scenes either", () => {
  let mounted: Record<string, unknown> | null = null;
  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = null;
    history.replaceState(null, "", "/");
  });

  it.each(["exportDialog", "exportCounting", "exportStopped", "exporting", "exportDone", "exportFailed"])("%s", async (name) => {
    history.replaceState(null, "", `/?scene=${name}&scale=100`);
    mounted = mount(App, { target: document.body });
    for (let i = 0; i < 5; i++) await new Promise((r) => setTimeout(r, 0));
    flushSync();
    expect(current.failure).toBeNull();
    expect(current.state?.export).toEqual(scene(name).export);
    expect(document.body.textContent).toContain("はじめての曲");
  });
});
