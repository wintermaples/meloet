import { describe, expect, it } from "vitest";
import { TERM_SEPARATOR, fill, findTerm, t, terms, texts, type Entry } from "../src/text/ja";
import { headers, mockScreenText, rows, sectionTables, sectionText, specDocument } from "./design-sources";

// crates/ は ui/ の外なので、design-sources.ts と同じく Node の fs で読む。
const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): { readFileSync(p: URL, e: "utf8"): string } } }).process.getBuiltinModule("node:fs");
const here = import.meta.url;
const readRepoFile = (path: string) => fs.readFileSync(new URL(`../../${path}`, here), "utf8");

const doc = specDocument();
const entries = Object.entries(texts) as [string, Entry][];
const filled = (e: Entry) => fill(e.text, e.example ?? {});

describe("仕様書 5 章「用語の説明」", () => {
  const table = sectionTables(doc, "s5")[0]!;
  const specTerms = rows(table);

  it("表の形が想定どおり", () => {
    expect(headers(table)).toEqual(["Term", "Explanation wording"]);
    expect(specTerms.length).toBeGreaterThan(50);
  });

  it("用語と説明が全部、一字も変わらずに入っている", () => {
    for (const [term, desc] of specTerms) {
      expect(terms.filter((x) => x.term === term).map((x) => x.description), term).toContain(desc);
    }
    expect(findTerm(specTerms[0]![0]!)?.description).toBe(specTerms[0]![1]);
  });

  it("ja.ts の spec:5 の用語は、全部仕様書にある（順番も同じ）", () => {
    expect(terms.filter((x) => x.source === "spec:5").map((x) => [x.term, x.description])).toEqual(specTerms);
  });

  it("仕様書の外の用語は、オーナーさんが決めた和音の 3 つだけ。「仮の音」は「サイン波」になった（仕様書 3 章）", () => {
    expect(terms.filter((x) => x.source !== "spec:5").map((x) => [x.term, x.source])).toEqual([
      ["和音のスタンプ", "user:2026-11-23"],
      ["根音", "user:2026-11-23"],
      ["調の和音", "user:2026-11-23"],
    ]);
    expect(findTerm("仮の音")).toBeUndefined();
    expect(findTerm(t("instrument.sine"))?.term).toBe("サイン波");
  });

  it("形は「用語 — 説明」", () => {
    expect(sectionText(doc, "s5")).toContain(`「用語${TERM_SEPARATOR}説明」`);
  });
});

describe("仕様書 6 章「エラーと案内の文面」", () => {
  const table = sectionTables(doc, "s6")[0]!;
  const specRows = rows(table);
  const byScene = new Map(specRows.map((r) => [r[0]!, r]));
  const messages = entries.filter(([, e]) => e.scene !== undefined);

  it("表の形が想定どおり", () => {
    expect(headers(table)).toEqual(["Scene", "Wording", "Where shown", "Button"]);
  });

  it.each(messages)("%s: 穴に仕様書の例の値を入れると、仕様書の文と一字も変わらない", (_, e) => {
    const row = byScene.get(e.scene!);
    expect(row, e.scene).toBeDefined();
    if (e.fit === "quoted") expect(row![1]).toContain(`「${filled(e)}」`);
    else if (e.fit === "part") expect(row![1]).toContain(filled(e));
    else expect(filled(e)).toBe(row![1]);
  });

  // ja.ts にない場面: ほかの場面の文を出す決まりだけの場面と、殻（crates/meloet-app）が出す PC のファイルの窓の題。
  const NOT_IN_JA = ["Only the sound stream of meloet stopped", "Title of the PC file window", "Mic too loud (how long)"];
  it("6 章の場面は全部 ja.ts に入っている（ja.ts にない物のほか）", () => {
    for (const scene of NOT_IN_JA) expect(byScene.has(scene), scene).toBe(true);
    expect([...new Set(messages.map(([, e]) => e.scene))].sort()).toEqual(
      [...byScene.keys()].filter((k) => !NOT_IN_JA.includes(k)).sort(),
    );
  });

  it("PC のファイルの窓の題は、殻の定数が仕様書の文のまま", () => {
    const rust = readRepoFile("crates/meloet-app/src/pcwindow.rs");
    const cell = byScene.get("Title of the PC file window")![1]!;
    const titles = [...rust.matchAll(/pub const (OPEN|FOLDER)_TITLE: &str = "([^"]+)";/g)].map((m) => [m[1], m[2]!]);
    expect(Object.fromEntries(titles)).toEqual({ OPEN: "ひらく", FOLDER: "場所をえらぶ" });
    expect(cell).toBe("「ひらく」 when opening, 「場所をえらぶ」 when choosing a folder");
  });

  it("書き出しの PC の保存の窓の題は、殻の定数が仕様書 3 章（94-79）の文のまま", () => {
    const rust = readRepoFile("crates/meloet-app/src/pcwindow.rs");
    const titles = [...rust.matchAll(/pub const (EXPORT|EXPORT_FOLDER)_TITLE: &str = "([^"]+)";/g)].map((m) => [m[1], m[2]!]);
    expect(Object.fromEntries(titles)).toEqual({ EXPORT: "書き出す", EXPORT_FOLDER: "書き出すフォルダをえらぶ" });
    expect(sectionText(doc, "s3")).toContain(
      "for the export file, 「書き出す」; for the per-track folder, 「書き出すフォルダをえらぶ」",
    );
  });

  it("マイクの音が大きすぎる（いつまで）: 文は「マイクの音が大きすぎる」の文。出す長さの決まりだけの場面", () => {
    const row = byScene.get("Mic too loud (how long)")!;
    expect(row[1]!.endsWith("…")).toBe(true);
    expect(t("msg.micTooLoud").startsWith(row[1]!.slice(0, -1))).toBe(true);
    expect(row[2]).toContain("for 4 seconds after the last time it reached 0 dB, or until another notice comes");
  });

  it("決まりだけの場面は、使う文の場面とボタンを名前で言っている", () => {
    const row = byScene.get("Only the sound stream of meloet stopped")!;
    expect(row[1]).toContain('the same text as "Output gone"');
    expect(row[1]).toContain('the same text as "Output gone while recording"');
    expect(row[3]).toBe(t("button.openAudioSettings"));
  });

  it("数字として読めない: 欄の名前で文になる。ベロシティは仕様書の例そのまま", () => {
    const cell = byScene.get("Not readable as a number")![1]!;
    const say = (field: Parameters<typeof t>[0], kept: string) => t("msg.valueRejected", { field: t(field), kept });
    expect(cell).toBe(`${say("value.velocity", "77")} (the same form for the other fields)`);
    expect(say("value.tempo", "96.00")).toBe("テンポは数字で打ちます。もとの 96.00 のままです。");
    expect(say("value.meterNum", "4")).toBe("拍子の上は数字で打ちます。もとの 4 のままです。");
    expect(say("value.meterDen", "4")).toBe("拍子の下は数字で打ちます。もとの 4 のままです。");
  });

  it("範囲の外の値を打った: 4 つの欄と範囲で文になる。ベロシティと拍子の下は仕様書の例そのまま", () => {
    const cell = byScene.get("Typed a value out of range")![1]!;
    const say = (field: Parameters<typeof t>[0], range: Parameters<typeof t>[0], value: string) =>
      t("msg.valueClamped", { field: t(field), range: t(range), value });
    expect(cell).toContain(`「${say("value.velocity", "value.velocityRange", "127")}」`);
    expect(cell).toContain(`「${say("value.meterDen", "value.meterDenRange", "4")}」`);
    expect(say("value.tempo", "value.tempoRange", "300.00")).toBe("テンポは 30.00〜300.00 なので、300.00 にしました。");
    expect(say("value.meterNum", "value.meterNumRange", "12")).toBe("拍子の上は 1〜12 なので、12 にしました。");
    expect(cell).toContain(`(the same form for 「${t("value.tempo")}」 ${t("value.tempoRange")} and 「${t("value.meterNum")}」 ${t("value.meterNumRange")})`);
  });

  it("穴は全部、例の値で埋まる", () => {
    for (const [k, e] of entries) expect(filled(e), k).not.toMatch(/\{\w+\}/);
  });

  const buttonLabels = new Set(
    specRows.flatMap((r) => r[3]!.split(/\s*\/\s*/).map((b) => b.replace(/\s*(（[^）]*）|\([^)]*\))$/, "").trim())),
  );
  const buttons = entries.filter(([k, e]) => k.startsWith("button.") && e.source === "spec:6");
  it.each(buttons)("%s: 6 章のボタンの列にある", (_, e) => {
    expect(buttonLabels).toContain(filled(e));
  });
});

describe("出どころが spec の文は、その章にそのまま書いてある", () => {
  it.each(entries.filter(([, e]) => e.source.startsWith("spec:")))("%s", (_, e) => {
    const section = `s${e.source.slice("spec:".length)}`;
    expect(sectionText(doc, section)).toContain(filled(e));
  });
});

describe("出どころが mock の文は、そのボードにそのまま書いてある", () => {
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

describe("出どころの形", () => {
  it("どの文も spec:<章>・mock:<ボード>・user:<日付>・draft:<日付>・builder:<日付> のどれか", () => {
    for (const [k, e] of entries) expect(e.source, k).toMatch(/^(spec:\d|mock:[\w-]+|(user|draft|builder):\d{4}-\d{2}-\d{2})$/);
  });

  it("draft: の文は、音の素材の「手を加えたこと」の案だけ（ヤチヨさんに確かめる前）", () => {
    expect(entries.filter(([, e]) => e.source.startsWith("draft:")).every(([k]) => k.startsWith("about.changes."))).toBe(true);
  });
});

describe("fill", () => {
  it("穴に値を入れ、値のない穴はのこす", () => {
    expect(fill("{a} と {b}", { a: 1 })).toBe("1 と {b}");
    expect(t("msg.keyboardFound", { keyboard: "Keystation 49 MK3" })).toBe(
      "Keystation 49 MK3 が見つかりました。鍵は届いています。音の出口をえらぶと鳴ります。",
    );
  });
});
