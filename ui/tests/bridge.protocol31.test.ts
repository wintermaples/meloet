// Protocol 31 (round 12, wave 1): the shapes only. The behaviour comes with each feature; here the fake takes the new intents and changes nothing.
import { describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import {
  CLICK_SOUNDS,
  CLICK_VOLUME_MAX_DB,
  CLICK_VOLUME_MIN_DB,
  CLICK_WHENS,
  EXPORT_MATCH_LUFS,
  EXPORT_MATCH_QUIETEST_LUFS,
  EXPORT_MATCH_TRUE_PEAK_DB,
  EXPORT_WAVE_BARS,
  PROTOCOL_VERSION,
  RECENT_SHOWN,
  SCALE_MODES,
} from "../src/bridge";
import { createFakeTransport, fakeState } from "../src/bridge/fake";
import { SCALE_STEPS } from "../src/components/pianoroll/geom";

async function opened() {
  const t = createFakeTransport(fakeState());
  await t.subscribe(
    () => {},
    () => {},
  );
  const now = async () => (await t.snapshot()).state as AppState;
  return { now, send: (command: Command) => t.send(command) };
}

describe("shape of protocol 31", () => {
  it("version 31. A new state: the click as it always was, highlight on, no recent songs, the check for updates on", () => {
    expect(PROTOCOL_VERSION).toBe(31);
    const s = fakeState();
    expect([s.transport.clickVolumeDb, s.transport.clickSound, s.transport.clickWhen]).toEqual([0, "beep", "always"]);
    expect(s.pianoRoll.scale.highlight).toBe(true);
    expect(s.recent).toEqual([]);
    expect(s.updates).toEqual({ check: true, newVersion: null });
  });

  it("the numbers and lists of Rust", () => {
    expect([CLICK_VOLUME_MIN_DB, CLICK_VOLUME_MAX_DB]).toEqual([-24, 6]);
    expect(CLICK_SOUNDS).toEqual(["beep", "tick", "cowbell"]);
    expect(CLICK_WHENS).toEqual(["always", "recordingOnly"]);
    expect([EXPORT_WAVE_BARS, EXPORT_MATCH_LUFS, EXPORT_MATCH_TRUE_PEAK_DB, EXPORT_MATCH_QUIETEST_LUFS, RECENT_SHOWN]).toEqual([200, -14, -1, -70, 10]);
  });

  it("the scale steps of the screen are those of Rust, one row for each kind", () => {
    expect(Object.keys(SCALE_STEPS).sort()).toEqual([...SCALE_MODES].sort());
    expect(SCALE_STEPS.harmonicMinor).toEqual([0, 2, 3, 5, 7, 8, 11]);
    expect(SCALE_STEPS.majorPentatonic).toEqual([0, 2, 4, 7, 9]);
    expect(SCALE_STEPS.blues).toEqual([0, 3, 5, 6, 7, 10]);
  });
});

describe("fake intents", () => {
  it("setMusicalScale keeps highlight", async () => {
    const { now, send } = await opened();
    await send({ type: "setMusicalScale", root: 2, mode: "dorian", lock: false, highlight: false });
    expect((await now()).pianoRoll.scale).toEqual({ root: 2, mode: "dorian", lock: false, highlight: false });
  });

  it("the new intents are taken and change nothing yet", async () => {
    const { now, send } = await opened();
    const before = await now();
    const commands: Command[] = [
      { type: "setClick", volumeDb: -6, sound: "cowbell", when: "recordingOnly", commit: true },
      { type: "scaleVelocity", take: 1, notes: [0], percent: 120 },
      { type: "rampVelocity", take: 1, notes: [0, 1], from: 40, to: 110 },
      { type: "randomVelocity", take: 1, notes: [0], amount: 8, seed: 7 },
      { type: "legato", take: 1, notes: [0, 1] },
      { type: "addNotesAt", take: null, notes: [{ start: 0, length: 7680, key: 53, velocity: 100 }] },
      { type: "playExported" },
      { type: "duplicateTrack", track: before.song.tracks[0]!.id },
      { type: "openRecent", id: 1 },
      { type: "clearRecent" },
      { type: "normalizeClips", clips: [1] },
      { type: "reverseClips", clips: [1] },
      { type: "openUserGuide" },
      { type: "openReleasePage" },
      { type: "dismissNewVersion" },
    ];
    for (const command of commands) await send(command);
    const after = await now();
    expect(after.song).toEqual(before.song);
    expect(after.transport).toEqual(before.transport);
    expect(after.recent).toEqual(before.recent);
    expect(after.updates).toEqual(before.updates);
  });
});
