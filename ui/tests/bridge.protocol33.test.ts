// Protocol 33 (round 12, waves 3 and 4): the shapes only. The behaviour comes with each feature; here the fake takes the new intents and changes nothing.
import { describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import {
  ARP_KEYS_MAX,
  ARP_OCTAVES_MAX,
  ARP_RATES,
  ARP_SHAPES,
  COMP_ROWS_MAX,
  FADE_OUT_BARS_DEFAULT,
  FADE_OUT_BARS_MAX,
  INSERT_SPACE_BARS_DEFAULT,
  INSERT_SPACE_BARS_MAX,
  LANGUAGES,
  PROTOCOL_VERSION,
  SOUND_KNOB_DEFAULTS,
  SOUND_KNOBS,
  STEP_NAME_KINDS,
  TUNE_STRENGTH_MAX,
} from "../src/bridge";
import { createFakeTransport, fakeState } from "../src/bridge/fake";
import { laneNameText, stepPlaceText } from "../src/components/history/stepText";
import { stepNames } from "../src/text/ja";

async function opened() {
  const t = createFakeTransport(fakeState());
  await t.subscribe(
    () => {},
    () => {},
  );
  const now = async () => (await t.snapshot()).state as AppState;
  return { now, send: (command: Command) => t.send(command), hello: () => t.hello(PROTOCOL_VERSION) };
}

describe("shape of protocol 33", () => {
  it("version 33. A new state: the arpeggio off, no master lane, the spectrum shown, mono off, Japanese, nothing bounced", async () => {
    expect(PROTOCOL_VERSION).toBe(33);
    const s = fakeState();
    expect(s.song.tracks[0]!.arp).toEqual({ on: false, shape: "up", rate: "sixteenth", triplet: false, octaves: 1, hold: false });
    expect(s.song.master.lanes).toEqual([]);
    expect([s.workspace.spectrum, s.workspace.monoListen, s.workspace.language]).toEqual([true, false, "ja"]);
    expect(s.audioWork.bounce).toBeNull();
    await expect((await opened()).hello()).resolves.toMatchObject({ ok: true, language: "ja" });
  });

  it("the numbers and lists of Rust", () => {
    expect(ARP_SHAPES).toEqual(["up", "down", "upDown", "random"]);
    expect(ARP_RATES).toEqual(["quarter", "eighth", "sixteenth", "thirtySecond"]);
    expect([ARP_OCTAVES_MAX, ARP_KEYS_MAX]).toEqual([3, 16]);
    expect(SOUND_KNOBS).toEqual(["brightness", "attack", "release", "vibrato"]);
    expect(SOUND_KNOB_DEFAULTS).toEqual([50, 50, 50, 0]);
    expect([INSERT_SPACE_BARS_DEFAULT, INSERT_SPACE_BARS_MAX]).toEqual([4, 64]);
    expect([FADE_OUT_BARS_DEFAULT, FADE_OUT_BARS_MAX]).toEqual([4, 16]);
    expect([COMP_ROWS_MAX, TUNE_STRENGTH_MAX]).toEqual([8, 100]);
    expect(LANGUAGES).toEqual(["ja", "en"]);
  });

  it("ten more step names, each with its text; the knobs and the master have their names", () => {
    const kinds = ["setArp", "setSoundKnob", "duplicateSection", "deleteSection", "insertSpace", "bounce", "addFadeOut", "setComp", "clearComp", "setClipTune"] as const;
    for (const kind of kinds) {
      expect(STEP_NAME_KINDS).toContain(kind);
      expect(stepNames[kind].text.trim()).not.toBe("");
    }
    expect(stepNames.addFadeOut.text).toBe("フェードアウトをつける");
    expect(SOUND_KNOBS.map((knob) => laneNameText({ kind: "knob", knob }))).toEqual(["明るさ", "立ち上がり", "のび", "ゆれ"]);
    expect(stepPlaceText({ kind: "master" }, { song: fakeState().song, library: fakeState().library })).toBe("マスター");
  });
});

describe("fake intents", () => {
  it("the new intents are taken and change nothing yet", async () => {
    const { now, send } = await opened();
    const before = await now();
    const track = before.song.tracks[0]!.id;
    const master = { kind: "master" as const };
    const volume = { kind: "volume" as const };
    const commands: Command[] = [
      { type: "setArp", track, arp: { on: true, shape: "upDown", rate: "eighth", triplet: true, octaves: 2, hold: true } },
      { type: "setSoundKnob", track, instrument: "grandPiano", knob: "brightness", value: 80, commit: true },
      { type: "duplicateSection", marker: 99 },
      { type: "deleteSection", marker: 99 },
      { type: "insertSpace", marker: 99, bars: 4 },
      { type: "bounceToAudio", clips: [99] },
      { type: "bounceTrack", track },
      { type: "cancelBounce" },
      { type: "addLane", home: master, target: volume },
      { type: "foldLane", home: master, target: volume, folded: true },
      { type: "setLaneWrite", home: master, target: volume, on: true },
      { type: "restoreLane", home: master, target: volume },
      { type: "deleteLane", home: master, target: volume },
      { type: "addFadeOut", bars: 4 },
      { type: "setComp", clip: 99, span: { start: 0, end: 1920 }, pieces: [{ from: 0, take: 98 }] },
      { type: "clearComp", clip: 99 },
      { type: "setClipStretch", clip: 99, change: { kind: "tuneStrength", percent: 50 } },
      { type: "setClipStretch", clip: 99, change: { kind: "tuneScale" } },
      { type: "setLanguage", language: "en" },
    ];
    for (const command of commands) await send(command);
    const after = await now();
    expect(after.song).toEqual(before.song);
    expect(after.automation).toEqual(before.automation);
    expect(after.workspace).toEqual(before.workspace);
    expect(after.audioWork).toEqual(before.audioWork);
    expect(after.timeline).toEqual(before.timeline);
  });
});
