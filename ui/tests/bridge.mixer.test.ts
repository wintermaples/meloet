import { describe, expect, it } from "vitest";
import { effectDefaults, encodeTelemetry, fakeScene, fakeState, reduceMixer, type FakeState as AppState } from "../src/bridge/fake";
import { decodeTelemetry } from "../src/bridge/telemetry";
import { EFFECT_KINDS, EFFECT_PARAMS, INSERT_SLOTS, type Command, type InsertRef } from "../src/bridge/types";
import { builtinOf } from "../src/components/params/selected";
import { texts } from "../src/text/ja";

const scene = (name: string) => fakeScene(name)!.state;

describe("protocol 14: fake scene (mixer)", () => {
  it("mixer is the song of mock F4-Mixer", () => {
    const s = scene("mixer");
    expect(s.song.tracks.map((t) => [t.number, t.name, t.instrument, t.gainDb, t.pan, t.sends, t.inserts[0]])).toEqual([
      [1, "ピアノ", "grandPiano", 0, 0, [-15, null], null],
      [2, "ベース", "fingerBass", 0, 0, [null, null], "compressor"],
      [3, "ドラム", "studioKit", 0, 0, [-24, null], "compressor"],
      [4, "パッド", "strings", -3, -20, [-15, null], "eq4"],
      [5, "シンセ", "synthLead", 0, 20, [-15, -18], null],
    ]);
    for (const t of s.song.tracks) {
      expect(t.inserts.length).toBe(INSERT_SLOTS);
      expect(t.inserts.slice(1)).toEqual([null, null, null]);
    }
    expect(s.song.returns).toMatchObject([
      { gainDb: 0, pan: 0, muted: false, inserts: ["reverb", null, null, null] },
      { gainDb: 0, pan: 0, muted: false, inserts: ["delay", null, null, null] },
    ]);
    expect(s.song.master).toMatchObject({ gainDb: 0, pan: 0, inserts: ["limiter", null, null, null] });
    const insert: InsertRef = { strip: { kind: "return", bus: "a" }, slot: 0 };
    expect(s.params.selected).toEqual({ kind: "builtin", insert, effect: "reverb", values: [2.4, 60, 20, 6000, 120, 100, 100], row: 0, preset: null });
    expect(s.status.message).toEqual({ id: "paramSelected", loopBars: { from: 5, to: 9 }, insert, effect: "reverb", param: 0 });
    expect(s.transport.state).toBe("playing");
    expect(s.workspace.layout.lower.map((b) => b.band)).toEqual(["mixer"]);
    expect(s.workspace.layout.rightFront).toBe("params");
  });

  it("paramsDelay is mock R6-Delay (the delay on return B), shelfEffects is the shelf opened from 「空き ＋」", () => {
    const delay = builtinOf(scene("paramsDelay").params)!;
    expect(delay.insert).toEqual({ strip: { kind: "return", bus: "b" }, slot: 0 });
    expect(delay.effect).toBe("delay");
    // Interval 1/8, sync to tempo on, repeat 35 %, 5000 Hz, 200 Hz, ping-pong off, mix 100 %.
    expect(delay.values).toEqual([2, 250, 1, 35, 5000, 200, 0, 100]);
    expect(EFFECT_PARAMS.delay[0]!.steps[delay.values[0]!]).toBe("eighth");

    const shelf = scene("shelfEffects");
    expect(shelf.shelf.mode).toEqual({ kind: "insertEffect", strip: { kind: "track", id: shelf.song.tracks[0]!.id }, slot: 1 });
    expect(shelf.shelf.selectedEffect).toBe("compressor");
    expect(shelf.workspace.layout.rightFront).toBe("shelf");
  });

  it("a new song has a reverb on return A, a delay on B and a limiter on the master. The piano sends −15 to A", () => {
    const s = fakeState();
    expect(s.song.returns.map((r) => r.inserts[0])).toEqual(["reverb", "delay"]);
    expect(s.song.master.inserts[0]).toBe("limiter");
    expect(s.song.tracks[0]!.sends).toEqual([-15, null]);
    expect(s.params).toEqual({ selected: null, clips: null });
    expect(effectDefaults("limiter")).toEqual([-0.3, 0, 100]);
  });
});

describe("protocol 14: the effect table and its text", () => {
  const has = (key: string) => expect(key in texts, key).toBe(true);

  it("9 effects, and every parameter has a name", () => {
    expect(EFFECT_KINDS).toEqual(["eq4", "compressor", "reverb", "delay", "chorus", "limiter", "gate", "deEsser", "saturation"]);
    for (const kind of EFFECT_KINDS) {
      has(`effect.${kind}`);
      for (const p of EFFECT_PARAMS[kind]) {
        has(`param.${kind}.${p.id}`);
        if (p.group !== null) has(`param.${kind}.group.${p.group}`);
        for (const step of p.steps) has(`param.${kind}.step.${step}`);
        if (p.unit !== "none") has(`param.value.${p.unit}`);
        expect(p.default).toBeGreaterThanOrEqual(p.min);
        expect(p.default).toBeLessThanOrEqual(p.max);
      }
    }
  });

  it("reverb and delay follow the order in spec 3", () => {
    const names = (kind: "reverb" | "delay") => EFFECT_PARAMS[kind].map((p) => (texts as Record<string, { text: string }>)[`param.${kind}.${p.id}`]!.text);
    expect(names("reverb")).toEqual(["響きの長さ", "部屋の大きさ", "プリディレイ", "高い音を落とす", "低い音を落とす", "広がり", "ミックス"]);
    expect(names("delay")).toEqual(["間隔", "間隔", "テンポに合わせる", "くり返し", "高い音を落とす", "低い音を落とす", "左右にゆれる", "ミックス"]);
  });
});

describe("protocol 14: returns A and B in the meters (version 3)", () => {
  it("what the fake writes, the reader reads the same", () => {
    const bytes = encodeTelemetry({ master: [0.5, 0.25], returns: [[0.125, 0.0625], [0, 2]], tracks: [[0.75, 1]] }, 3, 1000, 1);
    const t = decodeTelemetry(bytes);
    expect(Array.from(t.returnPeaks)).toEqual([0.125, 0.0625, 0, 2]);
    expect(Array.from(t.trackPeaks)).toEqual([0.75, 1]);
    expect(t.masterPeak).toEqual([0.5, 0.25]);
    expect(Array.from(decodeTelemetry(encodeTelemetry({}, 0, 0, 0)).returnPeaks)).toEqual([0, 0, 0, 0]);
  });
});

describe("protocol 14: the fake's commit and sound-only preview", () => {
  const run = (s: AppState, command: Command) => reduceMixer(s, s.shelf, s.song, command);

  it("commit: false does not change the state. commit: true changes the song and goes into the range", () => {
    const s = scene("mixer");
    const track = { kind: "track", id: s.song.tracks[0]!.id } as const;
    expect(run(s, { type: "setGain", strip: track, db: -9, commit: false })!.song).toEqual(s.song);
    expect(run(s, { type: "setGain", strip: track, db: -9, commit: true })!.song.tracks[0]!.gainDb).toBe(-9);
    expect(run(s, { type: "setGain", strip: track, db: -70, commit: true })!.song.tracks[0]!.gainDb).toBeNull();
    expect(run(s, { type: "setGain", strip: { kind: "master" }, db: 9, commit: true })!.song.master.gainDb).toBe(6);
    expect(run(s, { type: "setPan", strip: { kind: "return", bus: "b" }, pan: 150, commit: true })!.song.returns[1].pan).toBe(100);
    expect(run(s, { type: "setSend", track: track.id, bus: "b", db: -18, commit: true })!.song.tracks[0]!.sends).toEqual([-15, -18]);
    expect(run(s, { type: "toggleReturnMute", bus: "a" })!.song.returns[0].muted).toBe(true);
    const insert = builtinOf(s.params)!.insert;
    const set = (value: number, commit: boolean) => builtinOf(run(s, { type: "setParam", insert, effect: "reverb", param: 0, value, commit })!.params)!.values[0];
    expect(set(5, false)).toBe(2.4);
    expect(set(5, true)).toBe(5);
    expect(set(99, true)).toBe(10);
    expect(run(s, { type: "setParam", insert, effect: "delay", param: 0, value: 1, commit: true })!.params).toEqual(s.params);
  });

  it("choose: insert slot, parameter row, add an effect from the shelf", () => {
    const s = scene("shelfEffects");
    const master: InsertRef = { strip: { kind: "master" }, slot: 0 };
    const picked = run(s, { type: "selectInsert", insert: master })!;
    expect(picked.params.selected).toEqual({ kind: "builtin", insert: master, effect: "limiter", values: [-0.3, 0, 100], row: null, preset: null });
    expect(picked.showParams).toBe(true);
    expect(run(s, { type: "selectInsert", insert: { strip: { kind: "master" }, slot: 1 } })!.params.selected).toBeNull();
    expect(builtinOf(run(s, { type: "selectParam", param: 6 })!.params)!.row).toBe(6);
    expect(builtinOf(run(s, { type: "selectParam", param: 7 })!.params)!.row).toBeNull();
    expect(run(s, { type: "shelfSelectEffect", effect: "chorus" })!.shelf.selectedEffect).toBe("chorus");

    const added = run(s, { type: "shelfConfirm" })!;
    const track = { kind: "track", id: s.song.tracks[0]!.id } as const;
    expect(added.song.tracks[0]!.inserts).toEqual([null, "compressor", null, null]);
    expect(added.params.selected).toEqual({ kind: "builtin", insert: { strip: track, slot: 1 }, effect: "compressor", values: effectDefaults("compressor"), row: null, preset: null });
    expect(added.shelf.mode).toEqual({ kind: "newTrack" });
    expect(added.shelf.selectedEffect).toBeNull();
    const after = { ...s, song: added.song, params: added.params, shelf: added.shelf };
    const removed = run(after, { type: "removeEffect", insert: { strip: track, slot: 1 } })!;
    expect(removed.song.tracks[0]!.inserts).toEqual([null, null, null, null]);
    expect(removed.params.selected).toBeNull();
    // Mix of reverb and delay: 30 % when added to a track or the master; returns use 100 % as in the table.
    const mixOf = (strip: InsertRef["strip"], effect: "reverb" | "delay" | "chorus") => {
      const open = { ...s, shelf: { ...s.shelf, mode: { kind: "insertEffect", strip, slot: 3 } as const, selectedEffect: effect } };
      const values = builtinOf(run(open, { type: "shelfConfirm" })!.params)!.values;
      return values[EFFECT_PARAMS[effect].findIndex((p) => p.id === "mix")];
    };
    expect([mixOf(track, "reverb"), mixOf(track, "delay"), mixOf({ kind: "master" }, "reverb")]).toEqual([30, 30, 30]);
    expect([mixOf({ kind: "return", bus: "a" }, "reverb"), mixOf({ kind: "return", bus: "b" }, "delay")]).toEqual([100, 100]);
    expect(mixOf(track, "chorus")).toBe(EFFECT_PARAMS.chorus.find((p) => p.id === "mix")!.default);
    // Enter on the instrument shelf is not handled here.
    expect(run(scene("shelfAudition"), { type: "shelfConfirm" })).toBeNull();
  });
});
