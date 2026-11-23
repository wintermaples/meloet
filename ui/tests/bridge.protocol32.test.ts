// Protocol 32 (round 12, wave 2): the shapes only. The behaviour comes with each feature; here the fake takes the new intents and changes nothing.
import { describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import {
  EXPORT_MP3_DEFAULT_KBPS,
  EXPORT_MP3_KBPS,
  EXPORT_PICTURE_MAX_BYTES,
  FACTORY_PRESETS,
  PATTERN_GROUPS,
  PRESET_NAME_MAX,
  PRESETS_PER_EFFECT,
  PROTOCOL_VERSION,
  REPEAT_COUNT_DEFAULT,
  REPEAT_COUNT_MAX,
  SPECTRUM_BANDS,
  STEP_NAME_KINDS,
  TYPED_KEYBOARD_ID,
} from "../src/bridge";
import { createFakeTransport, encodeTelemetry, fakeState } from "../src/bridge/fake";
import { TELEMETRY_HEADER_LEN, TELEMETRY_VERSION, decodeTelemetry } from "../src/bridge/telemetry";
import { EFFECT_KINDS, EFFECT_PARAMS } from "../src/bridge/types";
import { stepNames } from "../src/text/ja";

async function opened() {
  const t = createFakeTransport(fakeState());
  await t.subscribe(
    () => {},
    () => {},
  );
  const now = async () => (await t.snapshot()).state as AppState;
  return { now, send: (command: Command) => t.send(command) };
}

describe("shape of protocol 32", () => {
  it("version 32. A new state: nothing to keep, the PC keyboard off, the glow on, the patterns of a new track's shelf, no own presets yet", () => {
    expect(PROTOCOL_VERSION).toBe(32);
    const s = fakeState();
    expect(s.transport.canKeepPlayed).toBe(false);
    expect(s.keyboards.typedKeys).toBe(false);
    expect(s.workspace.noteGlow).toBe(true);
    expect([s.shelf.patterns.length, s.shelf.selectedPattern, s.shelf.patternSample]).toEqual([12, null, null]);
    expect(s.presets).toEqual({ user: [] });
  });

  it("the numbers and lists of Rust", () => {
    expect(TYPED_KEYBOARD_ID).toBe("typedKeys");
    expect([REPEAT_COUNT_DEFAULT, REPEAT_COUNT_MAX]).toEqual([3, 99]);
    expect(PATTERN_GROUPS).toEqual(["basic", "fill"]);
    expect([PRESET_NAME_MAX, PRESETS_PER_EFFECT]).toEqual([40, 100]);
    expect(EXPORT_MP3_KBPS).toEqual([128, 192, 256, 320]);
    expect(EXPORT_MP3_DEFAULT_KBPS).toBe(192);
    expect(EXPORT_PICTURE_MAX_BYTES).toBe(5 * 1024 * 1024);
  });

  it("three more effects; the presets that come with meloet have every parameter of their effect", () => {
    expect(EFFECT_KINDS.slice(6)).toEqual(["gate", "deEsser", "saturation"]);
    expect(Object.keys(FACTORY_PRESETS).sort()).toEqual([...EFFECT_KINDS].sort());
    for (const kind of EFFECT_KINDS) {
      for (const preset of FACTORY_PRESETS[kind]) expect(preset.values.length, `${kind} ${preset.id}`).toBe(EFFECT_PARAMS[kind].length);
    }
  });

  it("five more step names, each with its text", () => {
    for (const kind of ["keepPlayed", "placePattern", "repeatClips", "applyPreset", "lowerAll"] as const) {
      expect(STEP_NAME_KINDS).toContain(kind);
      expect(stepNames[kind].source).toBe("user:2026-11-23");
    }
    expect(stepNames.lowerAll.text).toBe("全部のトラックを下げる");
  });
});

describe("telemetry version 5", () => {
  it("the fake writes the readings of the master's analysis and the screen reads them; none is null", () => {
    expect([TELEMETRY_VERSION, TELEMETRY_HEADER_LEN, SPECTRUM_BANDS]).toEqual([5, 124, 24]);
    const spectrum = Array.from({ length: 24 }, (_, i) => i * 10);
    const read = decodeTelemetry(encodeTelemetry({ masterReductionDb: 6.3, spectrum, lufsShort: -13.2, truePeakDb: -1.4 }, 1, 0, 1));
    expect([read.masterReductionDb, Array.from(read.spectrum), read.lufsShort, read.lufsIntegrated, read.truePeakDb]).toEqual([6.3, spectrum, -13.2, null, -1.4]);
    const quiet = decodeTelemetry(encodeTelemetry({}, 1, 0, 0));
    expect([quiet.masterReductionDb, quiet.spectrum.every((b) => b === 0), quiet.lufsShort, quiet.lufsIntegrated, quiet.truePeakDb]).toEqual([0, true, null, null, null]);
  });
});

describe("fake intents", () => {
  it("the new intents are taken and change nothing yet", async () => {
    const { now, send } = await opened();
    const before = await now();
    const commands: Command[] = [
      { type: "keepPlayed" },
      { type: "typedKeys", on: true },
      { type: "typedKey", note: 60, down: true, tUs: 1000 },
      { type: "repeatClips", clips: [1], count: 3 },
      { type: "lowerAllTracks", db: 6 },
      { type: "dismissTooLoud" },
    ];
    for (const command of commands) await send(command);
    const after = await now();
    expect(after.song).toEqual(before.song);
    expect(after.transport).toEqual(before.transport);
    expect(after.keyboards).toEqual(before.keyboards);
    expect(after.shelf).toEqual(before.shelf);
    expect(after.presets).toEqual(before.presets);
  });

  it("the export does not offer mp3 yet, and choosing it changes nothing", async () => {
    const { now, send } = await opened();
    await send({ type: "openExport" });
    const dialog = (await now()).export.dialog!;
    expect(dialog.formats).toEqual(["wav", "flac"]);
    expect([dialog.mp3Kbps, dialog.mp3KbpsChoices, dialog.picture, dialog.pictureUsable]).toEqual([192, [], null, false]);
    await send({ type: "setExport", change: { kind: "format", format: "mp3" } });
    await send({ type: "setExport", change: { kind: "title", title: "別の題名" } });
    expect((await now()).export.dialog).toEqual(dialog);
  });
});
