// Round 12 D4 「ことば / Language」: the screen's words in the language of the page, and the fake's setting.
import { afterEach, describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { createFakeTransport, fakeState } from "../src/bridge/fake";
import { languageFromUrl } from "../src/bridge/fakeWave3";
import { termParts } from "../src/components/format";
import { stepNameText as stepText } from "../src/components/history/stepText";
import { DRUM_KEYS, drumName, drumTerm } from "../src/components/pianoroll/drums";
import { rangeText } from "../src/components/format";
import { readBars } from "../src/components/valueClamp";
import { BRACKET_CLOSE, BRACKET_OPEN, findTerm, LIST_COMMA, rawText, STATUS_JOIN, stepNameText, t, TERM_SEPARATOR, textLanguage, useLanguage } from "../src/text";
import { tickRangeText } from "../src/time/bars";

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

  it("UI-TEXT #3 (later): two counts in one text each take their own form (semitones, cents)", () => {
    useLanguage("en");
    expect(t("msg.pitchDragging", { semitones: "+1", cents: "−1" })).toBe("Pitch +1 semitone −1 cent (dragging)");
    expect(t("msg.pitchDragging", { semitones: "+2", cents: "+1" })).toBe("Pitch +2 semitones +1 cent (dragging)");
    expect(t("clipParams.pitchValue", { semitones: "−1", cents: "+30" })).toBe("−1 semitone +30 cents");
    expect(t("clipParams.pitchValue", { semitones: "+0", cents: "0" })).toBe("+0 semitones 0 cents");
    useLanguage("ja");
    expect(t("clipParams.pitchValue", { semitones: "+1", cents: "+1" })).toBe("+1 半音 +1 セント");
  });

  it("English: a count of 1 takes the form for 1, also when signed with 「−」", () => {
    useLanguage("en");
    expect(t("msg.countingInRest", { beats: 1 })).toBe("Recording starts in 1 beat. Space to stop");
    expect(t("msg.countingIn", { beats: 1 })).toContain("in 1 beat.");
    expect(t("history.headLatest", { total: 1, current: 1 })).toBe("1 step · now at step 1 (the newest)");
    expect(t("history.headAllUndone", { total: 1, redoKey: "Ctrl Shift Z" })).toBe("1 step · all undone · Ctrl Shift Z to redo");
    expect(t("history.head", { total: 2, current: 1, redo: 1 })).toBe("2 steps · now at step 1. The 1 above is undone and can be redone");
    expect(t("history.head", { total: 3, current: 1, redo: 2 })).toContain("The 2 above are undone");
    expect(t("lane.now", { value: "50 %", count: 1 })).toBe("Now 50 % · 1 pt");
    expect(t("msg.scanDone", { added: 3, stopped: 1 })).toContain("1 stopped while being checked and was left out.");
    expect(t("msg.scanDone", { added: 3, stopped: 2 })).toContain("2 stopped while being checked and were left out.");
    expect(t("pr.readMove", { ticks: "+0.0000", keys: "−1" })).toBe("+0.0000 · −1 semitone");
    expect(t("pr.readMove", { ticks: "+0.0000", keys: "+1" })).toBe("+0.0000 · +1 semitone");
    expect(t("pr.readMove", { ticks: "+0.0000", keys: "−2" })).toBe("+0.0000 · −2 semitones");
    expect(t("arrange.readMove", { position: "3. 1. 0000", delta: "−1" })).toBe("3. 1. 0000 (−1 bar)");
    expect(t("arrange.readMove", { position: "3. 1. 0000", delta: "−1.5" })).toBe("3. 1. 0000 (−1.5 bars)");
    expect(t("msg.audioSkipped", { skipped: 1 })).toBe("1 file could not be read.");
    expect(t("msg.audioSkipped", { skipped: 2 })).toBe("2 files could not be read.");
  });

  it("the range mark and the brackets follow the language", () => {
    expect(rangeText("1", "127")).toBe("1〜127");
    expect(tickRangeText(1920)).toBe("0〜1919");
    expect([BRACKET_OPEN, BRACKET_CLOSE]).toEqual(["（", "）"]);
    useLanguage("en");
    expect(rangeText("1", "127")).toBe("1–127");
    expect(tickRangeText(1920)).toBe("0–1919");
    expect(readBars("99", 64)?.clamp?.range).toBe("1–64");
    expect([BRACKET_OPEN, BRACKET_CLOSE]).toEqual([" (", ")"]);
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

describe("drum row names of the piano roll keep their term tips in English (UI-TEXT-005)", () => {
  it("the English kit names find the same terms as the Japanese ones", () => {
    const terms = () => DRUM_KEYS.map((k) => drumTerm(drumName(k)!));
    const ja = terms();
    expect(ja.every((word) => word !== null)).toBe(true);
    useLanguage("en");
    expect(terms()).toEqual(ja);
    expect([drumTerm("Closed hi-hat"), drumTerm("Side stick"), drumTerm("Floor tom 2"), drumTerm("Tambourine")]).toEqual(["ハイハット", "サイドスティック", "タム", null]);
    expect(termParts("Closed hi-hat", "ハイハット")).toEqual([
      { text: "Closed ", term: false },
      { text: "hi-hat", term: true },
    ]);
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
