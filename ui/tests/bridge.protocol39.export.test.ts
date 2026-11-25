// Protocol 39 (the owner, 2026-11-25): the export dialog asks before it writes over files that are there already. The
// shape, its texts, the fake's scenes and its copy of the intents.
import { describe, expect, it } from "vitest";
import type { Command, ExportReplace } from "../src/bridge";
import { PROTOCOL_VERSION } from "../src/bridge";
import { fakeScene, reduceExport, type FakeState } from "../src/bridge/fake";
import { replaceText } from "../src/components/export/exportText";
import { t, useLanguage } from "../src/text";

const scene = (name: string) => fakeScene(name)!.state as FakeState;

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

describe("shape of protocol 39", () => {
  it("version 39; one file by its name, more by their count", () => {
    expect(PROTOCOL_VERSION).toBe(39);
    const one: ExportReplace = { count: 1, file: "はじめての曲.wav" };
    expect(replaceText(one)).toBe("「はじめての曲.wav」はもうあります。上書きします。");
    expect(replaceText({ count: 3, file: "ピアノ.wav" })).toBe("同じ名前のファイルが 3 個もうあります。上書きします。");
    expect(t("export.replaceOne", { file: "a.wav" })).toBe("「a.wav」はもうあります。上書きします。");
  });

  it("in English", () => {
    useLanguage("en");
    try {
      expect(replaceText({ count: 1, file: "First song.wav" })).toBe("“First song.wav” is already there. It will be replaced.");
      expect(replaceText({ count: 3, file: "Piano.wav" })).toBe("3 files with the same names are already there. They will be replaced.");
    } finally {
      useLanguage("ja");
    }
  });
});

describe("the fake", () => {
  it("has a scene for each form of the question; the dialogs without one have none", () => {
    expect(scene("exportReplace").export.dialog!.replace).toEqual({ count: 1, file: "はじめての曲.wav" });
    expect(scene("exportReplaceTracks").export.dialog!.replace).toEqual({ count: 3, file: "ピアノ.wav" });
    expect(scene("exportReplaceClips").export.dialog!.check).toMatchObject({ clips: true });
    expect(scene("exportReplaceMidi").export.dialog!.format).toBe("midi");
    expect(scene("exportDialog").export.dialog!.replace).toBeNull();
    expect(scene("exportMidi").export.dialog!.replace).toBeNull();
  });

  it("「やめる」 and Esc take the question away and leave the dialog; a choice takes it away too", () => {
    const back = after(scene("exportReplace"), { type: "closeExport" });
    expect(back.export.dialog).not.toBeNull();
    expect(back.export.dialog!.replace).toBeNull();
    expect(after(back, { type: "closeExport" }).export.dialog).toBeNull();
    const chose = after(scene("exportReplace"), { type: "setExport", change: { kind: "advancedOpen", open: true } });
    expect(chose.export.dialog!.replace).toBeNull();
  });

  it("「このまま書き出す」 starts the run", () => {
    const s = after(scene("exportReplace"), { type: "startExport" });
    expect(s.export.dialog).toBeNull();
    expect(s.export.run?.state).toBe("running");
  });
});
