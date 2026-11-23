// Round 12 D4 「ことば / Language」: the screen's words in the language of the page, and the fake's setting.
import { afterEach, describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { createFakeTransport, fakeState } from "../src/bridge/fake";
import { languageFromUrl } from "../src/bridge/fakeWave3";
import { stepNameText as stepText } from "../src/components/history/stepText";
import { findTerm, LIST_COMMA, rawText, STATUS_JOIN, stepNameText, t, TERM_SEPARATOR, textLanguage, useLanguage } from "../src/text";

afterEach(() => {
  useLanguage("ja");
  history.replaceState(null, "", "/");
});

describe("t() in the language of the page", () => {
  it("Japanese until told, as before", () => {
    expect(textLanguage()).toBe("ja");
    expect(t("menu.file")).toBe("ファイル");
    expect([STATUS_JOIN, LIST_COMMA, TERM_SEPARATOR]).toEqual([" ・ ", "、", " — "]);
  });

  it("English: the English table, its joints, and the form for one or for any other number", () => {
    useLanguage("en");
    expect(t("menu.file")).toBe("File");
    expect([STATUS_JOIN, LIST_COMMA, TERM_SEPARATOR]).toEqual([" · ", ", ", " — "]);
    expect(t("history.notes", { count: 1 })).toBe("1 note");
    expect(t("history.notes", { count: 3 })).toBe("3 notes");
    expect(rawText("history.notes", { count: 2 })).toBe("{count} notes");
    expect(stepNameText("moveNotes")).toBe("Move notes");
    useLanguage("ja");
    expect(t("history.notes", { count: 3 })).toBe("3 ノート");
  });

  it("a plugin's name in a step name: the space before 「の」 only in Japanese", () => {
    const name = { kind: "setPluginParam", plugin: "Surge XT", param: "Cutoff" } as const;
    expect(stepText(name)).toBe("Surge XT の「Cutoff」を変える");
    useLanguage("en");
    expect(stepText(name)).toBe("Change Surge XT “Cutoff”");
  });

  it("a term is found by its Japanese word, and in English also by its English word", () => {
    expect(findTerm("バッファ")?.term).toBe("バッファ");
    expect(findTerm("Buffer")).toBeUndefined();
    useLanguage("en");
    expect(findTerm("バッファ")).toEqual({ term: "Buffer", description: expect.stringContaining("latency") });
    expect(findTerm("Kick")?.term).toBe("Kick");
    expect(findTerm("まだない言葉")).toBeUndefined();
  });
});

describe("the fake's 「ことば / Language」", () => {
  async function opened(state = fakeState()) {
    const transport = createFakeTransport(state);
    await transport.subscribe(
      () => {},
      () => {},
    );
    const now = async () => (await transport.snapshot()).state as AppState;
    return { now, send: (command: Command) => transport.send(command) };
  }

  it("a setting: the workspace says the new language, and the address keeps it for the page that loads again", async () => {
    const { now, send } = await opened();
    await send({ type: "setLanguage", language: "en" });
    const after = await now();
    expect(after.workspace.language).toBe("en");
    expect(after.song.undoCount).toBe(0);
    expect(languageFromUrl()).toBe("en");
  });

  it("refused while counting in or recording", async () => {
    const state = fakeState();
    state.transport = { ...state.transport, state: "recording" };
    const { now, send } = await opened(state);
    await send({ type: "setLanguage", language: "en" });
    expect((await now()).workspace.language).toBe("ja");
  });

  it("?lang=en: the fake starts in English and greets in it", async () => {
    history.replaceState(null, "", "/?lang=en");
    const transport = createFakeTransport();
    expect((await transport.hello(33)).language).toBe("en");
    expect((await transport.snapshot()).state.workspace.language).toBe("en");
  });
});
