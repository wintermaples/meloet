import { describe, expect, it } from "vitest";
import { enStepNames, enTerms, enTexts, type EnText } from "../src/text/en";
import { t } from "../src/text";
import { TERM_SEPARATOR, fill, findTerm, stepNames, terms, texts, type Entry } from "../src/text/ja";
import { headers, mockScreenText, rows, sectionTables, sectionText, specDocument } from "./design-sources";

// crates/ is outside ui/, so read it with Node's fs as in design-sources.ts.
const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): { readFileSync(p: URL, e: "utf8"): string } } }).process.getBuiltinModule("node:fs");
const here = import.meta.url;
const readRepoFile = (path: string) => fs.readFileSync(new URL(`../../${path}`, here), "utf8");

const doc = specDocument();
const entries = Object.entries(texts) as [string, Entry][];
const filled = (e: Entry) => fill(e.text, e.example ?? {});

describe("spec 5 (Term tips)", () => {
  const table = sectionTables(doc, "s5")[0]!;
  const specTerms = rows(table);

  it("the table has the expected shape", () => {
    expect(headers(table)).toEqual(["Term", "Explanation wording"]);
    expect(specTerms.length).toBeGreaterThan(50);
  });

  it("every term and explanation is there without a single character changed", () => {
    for (const [term, desc] of specTerms) {
      expect(terms.filter((x) => x.term === term).map((x) => x.description), term).toContain(desc);
    }
    expect(findTerm(specTerms[0]![0]!)?.description).toBe(specTerms[0]![1]);
  });

  it("every spec:5 term in ja.ts is in the spec (in the same order)", () => {
    expect(terms.filter((x) => x.source === "spec:5").map((x) => [x.term, x.description])).toEqual(specTerms);
  });

  it("terms outside the spec are only the 5 from the round 10 proposal mocks (text as in the mocks). 「仮の音」 became 「サイン波」 (spec 3)", () => {
    const outside = terms.filter((x) => x.source !== "spec:5");
    expect(outside.map((x) => [x.term, x.source])).toEqual([
      ["重ねる", "user:2026-11-23"],
      ["和音のスタンプ", "user:2026-11-23"],
      ["根音", "user:2026-11-23"],
      ["調の和音", "user:2026-11-23"],
      ["AI とつなぐ", "user:2026-11-23"],
    ]);
    const mocks = ["P10-Overdub", "P10-ChordStamp", "P10-ChordTable", "P10-AiLink"]
      .map((b) => readRepoFile(`docs/design/proposals/${b}.dc.html`))
      .join("\n");
    for (const x of outside) expect(mocks).toContain(`${x.term}${TERM_SEPARATOR}${x.description}`);
    expect(findTerm("仮の音")).toBeUndefined();
    expect(findTerm(t("instrument.sine"))?.term).toBe("サイン波");
  });

  it("the form is 「用語 — 説明」", () => {
    expect(sectionText(doc, "s5")).toContain(`「用語${TERM_SEPARATOR}説明」`);
  });
});

describe("spec 6 (Error and hint wording)", () => {
  const table = sectionTables(doc, "s6")[0]!;
  const specRows = rows(table);
  const byScene = new Map(specRows.map((r) => [r[0]!, r]));
  const messages = entries.filter(([, e]) => e.scene !== undefined);

  it("the table has the expected shape", () => {
    expect(headers(table)).toEqual(["Scene", "Wording", "Where shown", "Button"]);
  });

  it.each(messages)("%s: filling the holes with the spec's example values gives the spec's text without a single character changed", (_, e) => {
    const row = byScene.get(e.scene!);
    expect(row, e.scene).toBeDefined();
    if (e.fit === "quoted") expect(row![1]).toContain(`「${filled(e)}」`);
    else if (e.fit === "part") expect(row![1]).toContain(filled(e));
    else expect(filled(e)).toBe(row![1]);
  });

  // Scenes not in ja.ts: scenes that are only rules for showing other scenes' text, and the titles of the PC file windows shown by the shell (crates/meloet-app).
  const NOT_IN_JA = ["Only the sound stream of meloet stopped", "Title of the PC file window", "Mic too loud (how long)"];
  it("every chapter 6 scene is in ja.ts (apart from those not in ja.ts)", () => {
    for (const scene of NOT_IN_JA) expect(byScene.has(scene), scene).toBe(true);
    expect([...new Set(messages.map(([, e]) => e.scene))].sort()).toEqual(
      [...byScene.keys()].filter((k) => !NOT_IN_JA.includes(k)).sort(),
    );
  });

  it("the titles of the PC file windows are the shell's constants, as in the spec text", () => {
    const rust = readRepoFile("crates/meloet-app/src/pcwindow.rs");
    const cell = byScene.get("Title of the PC file window")![1]!;
    const titles = [...rust.matchAll(/pub const (OPEN|FOLDER)_TITLE: &str = "([^"]+)";/g)].map((m) => [m[1], m[2]!]);
    expect(Object.fromEntries(titles)).toEqual({ OPEN: "ひらく", FOLDER: "場所をえらぶ" });
    expect(cell).toBe("「ひらく」 when opening, 「場所をえらぶ」 when choosing a folder");
  });

  it("the title of the PC save window for export is the shell's constant, as in the spec 3 (94-79) text", () => {
    const rust = readRepoFile("crates/meloet-app/src/pcwindow.rs");
    const titles = [...rust.matchAll(/pub const (EXPORT|EXPORT_FOLDER)_TITLE: &str = "([^"]+)";/g)].map((m) => [m[1], m[2]!]);
    expect(Object.fromEntries(titles)).toEqual({ EXPORT: "書き出す", EXPORT_FOLDER: "書き出すフォルダをえらぶ" });
    expect(sectionText(doc, "s3")).toContain(
      "for the export file, 「書き出す」; for the per-track folder, 「書き出すフォルダをえらぶ」",
    );
  });

  it("「マイクの音が大きすぎる」 (how long): the text is that of 「マイクの音が大きすぎる」. A scene that is only a rule for how long it is shown", () => {
    const row = byScene.get("Mic too loud (how long)")!;
    expect(row[1]!.endsWith("…")).toBe(true);
    expect(t("msg.micTooLoud").startsWith(row[1]!.slice(0, -1))).toBe(true);
    expect(row[2]).toContain("for 4 seconds after the last time it reached 0 dB, or until another notice comes");
  });

  it("rule-only scenes name the scene of the text they use and the button", () => {
    const row = byScene.get("Only the sound stream of meloet stopped")!;
    expect(row[1]).toContain('the same text as "Output gone"');
    expect(row[1]).toContain('the same text as "Output gone while recording"');
    expect(row[3]).toBe(t("button.openAudioSettings"));
  });

  it("cannot be read as a number: the field name makes the text. Velocity is exactly the spec's example", () => {
    const cell = byScene.get("Not readable as a number")![1]!;
    const say = (field: Parameters<typeof t>[0], kept: string) => t("msg.valueRejected", { field: t(field), kept });
    expect(cell).toBe(`${say("value.velocity", "77")} (the same form for the other fields)`);
    expect(say("value.tempo", "96.00")).toBe("テンポは数字で打ちます。もとの 96.00 のままです。");
    expect(say("value.meterNum", "4")).toBe("拍子の上は数字で打ちます。もとの 4 のままです。");
    expect(say("value.meterDen", "4")).toBe("拍子の下は数字で打ちます。もとの 4 のままです。");
  });

  it("out-of-range value typed: the 4 fields and the range make the text. Velocity and the meter's lower number are exactly the spec's examples", () => {
    const cell = byScene.get("Typed a value out of range")![1]!;
    const say = (field: Parameters<typeof t>[0], range: Parameters<typeof t>[0], value: string) =>
      t("msg.valueClamped", { field: t(field), range: t(range), value });
    expect(cell).toContain(`「${say("value.velocity", "value.velocityRange", "127")}」`);
    expect(cell).toContain(`「${say("value.meterDen", "value.meterDenRange", "4")}」`);
    expect(say("value.tempo", "value.tempoRange", "300.00")).toBe("テンポは 30.00〜300.00 なので、300.00 にしました。");
    expect(say("value.meterNum", "value.meterNumRange", "12")).toBe("拍子の上は 1〜12 なので、12 にしました。");
    expect(cell).toContain(`(the same form for 「${t("value.tempo")}」 ${t("value.tempoRange")} and 「${t("value.meterNum")}」 ${t("value.meterNumRange")})`);
  });

  it("every hole is filled by an example value", () => {
    for (const [k, e] of entries) expect(filled(e), k).not.toMatch(/\{\w+\}/);
  });

  const buttonLabels = new Set(
    specRows.flatMap((r) => r[3]!.split(/\s*\/\s*/).map((b) => b.replace(/\s*(（[^）]*）|\([^)]*\))$/, "").trim())),
  );
  const buttons = entries.filter(([k, e]) => k.startsWith("button.") && e.source === "spec:6");
  it.each(buttons)("%s: is in the chapter 6 button column", (_, e) => {
    expect(buttonLabels).toContain(filled(e));
  });
});

describe("text from spec is written as it is in that chapter", () => {
  it.each(entries.filter(([, e]) => e.source.startsWith("spec:")))("%s", (_, e) => {
    const section = `s${e.source.slice("spec:".length)}`;
    expect(sectionText(doc, section)).toContain(filled(e));
  });
});

describe("text from mock is written as it is on that board", () => {
  const cache = new Map<string, string>();
  const screen = (board: string) => {
    if (!cache.has(board)) cache.set(board, mockScreenText(board));
    return cache.get(board)!;
  };
  const fromMock = entries.filter(([, e]) => e.source.startsWith("mock:"));
  it.each(fromMock)("%s", (_, e) => {
    expect(screen(e.source.slice("mock:".length))).toContain(filled(e));
  });
});

describe("source form", () => {
  it("every text is one of spec:<chapter>, mock:<board>, user:<date>, draft:<date>, builder:<date>", () => {
    for (const [k, e] of entries) expect(e.source, k).toMatch(/^(spec:\d|mock:[\w-]+|(user|draft|builder):\d{4}-\d{2}-\d{2})$/);
  });

  it("draft: text is only the proposal for the sound pack's 「手を加えたこと」 (before the owner checks it)", () => {
    expect(entries.filter(([, e]) => e.source.startsWith("draft:")).every(([k]) => k.startsWith("about.changes."))).toBe(true);
  });
});

describe("the English table (round 12 D4: Japanese is the source, English follows it)", () => {
  const holes = (text: string) => [...new Set([...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]!))].sort();
  const forms = (e: EnText): string[] => (typeof e === "string" ? [e] : [...forms(e.one), ...forms(e.other)]);

  it("has every key of the Japanese table and no other", () => {
    expect(Object.keys(enTexts).sort()).toEqual(Object.keys(texts).sort());
    expect(Object.keys(enStepNames).sort()).toEqual(Object.keys(stepNames).sort());
  });

  it("each text has the same holes as the Japanese, in every form; a pair is chosen by one of them", () => {
    for (const [key, e] of entries) {
      const en = enTexts[key as keyof typeof enTexts];
      for (const form of forms(en)) expect(holes(form), key).toEqual(holes(e.text));
      if (typeof en !== "string") expect(holes(e.text), key).toContain(en.by);
    }
    for (const [kind, e] of Object.entries(stepNames)) expect(holes(enStepNames[kind as keyof typeof enStepNames]), kind).toEqual(holes(e.text));
  });

  it("a text that is not empty in Japanese is not empty in English, and no Japanese is left in it", () => {
    const japanese = /[぀-ヿ㐀-鿿]/;
    // 「ことば / Language」 and 「日本語」 read the same in both, so that the setting can be found in either.
    const both = ["menu.language", "language.ja", "language.en"];
    for (const key of both) expect(enTexts[key as keyof typeof enTexts], key).toBe(texts[key as keyof typeof texts].text);
    for (const [key, e] of entries) {
      for (const form of forms(enTexts[key as keyof typeof enTexts])) {
        if (e.text.trim() !== "") expect(form.trim(), key).not.toBe("");
        if (!both.includes(key)) expect(form, key).not.toMatch(japanese);
      }
    }
    for (const name of Object.values(enStepNames)) expect(name).not.toMatch(japanese);
  });

  it("every term has its English word and tip, by its Japanese word", () => {
    expect(Object.keys(enTerms).sort()).toEqual(terms.map((x) => x.term).sort());
    for (const { term, description } of Object.values(enTerms)) {
      expect(term.trim()).not.toBe("");
      expect(description.trim()).not.toBe("");
    }
    expect(new Set(Object.values(enTerms).map((x) => x.term)).size, "English words are unique").toBe(terms.length);
  });
});

describe("fill", () => {
  it("fills holes with values, and keeps holes without a value", () => {
    expect(fill("{a} と {b}", { a: 1 })).toBe("1 と {b}");
    expect(t("msg.keyboardFound", { keyboard: "Keystation 49 MK3" })).toBe(
      "Keystation 49 MK3 が見つかりました。鍵は届いています。音の出口をえらぶと鳴ります。",
    );
  });
});
