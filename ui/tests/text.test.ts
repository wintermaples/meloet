import { describe, expect, it } from "vitest";
import { TERM_SEPARATOR, fill, findTerm, t, terms, texts, type Entry } from "../src/text/ja";
import { headers, mockScreenText, rows, sectionTables, sectionText, specDocument } from "./design-sources";

const doc = specDocument();
const entries = Object.entries(texts) as [string, Entry][];
const filled = (e: Entry) => fill(e.text, e.example ?? {});

describe("仕様書 5 章「用語の説明」", () => {
  const table = sectionTables(doc, "s5")[0]!;
  const specTerms = rows(table);

  it("表の形が想定どおり", () => {
    expect(headers(table)).toEqual(["用語", "説明の文面"]);
    expect(specTerms.length).toBeGreaterThan(50);
  });

  it("用語と説明が全部、一字も変わらずに入っている", () => {
    for (const [term, desc] of specTerms) {
      expect(findTerm(term!)?.description, term).toBe(desc);
    }
  });

  it("ja.ts の spec:5 の用語は、全部仕様書にある（順番も同じ）", () => {
    expect(terms.filter((x) => x.source === "spec:5").map((x) => [x.term, x.description])).toEqual(specTerms);
  });

  it("仕様書の外の用語は、ヤチヨさんが決めた「仮の音」だけ", () => {
    expect(terms.filter((x) => x.source !== "spec:5")).toEqual([
      { term: "仮の音", description: "本物のピアノの音が入るまでの、かわりの音。", source: "user:2026-11-14" },
    ]);
    expect(t("instrument.placeholder")).toBe("仮の音");
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
    expect(headers(table)).toEqual(["場面", "文面", "出る場所", "ボタン"]);
  });

  it.each(messages)("%s: 穴に仕様書の例の値を入れると、仕様書の文と一字も変わらない", (_, e) => {
    const row = byScene.get(e.scene!);
    expect(row, e.scene).toBeDefined();
    if (e.quoted) expect(row![1]).toContain(`「${filled(e)}」`);
    else expect(filled(e)).toBe(row![1]);
  });

  it("6 章の場面は全部 ja.ts に入っている", () => {
    expect([...new Set(messages.map(([, e]) => e.scene))].sort()).toEqual([...byScene.keys()].sort());
  });

  it("穴は全部、例の値で埋まる", () => {
    for (const [k, e] of entries) expect(filled(e), k).not.toMatch(/\{\w+\}/);
  });

  const buttonLabels = new Set(
    specRows.flatMap((r) => r[3]!.split(/\s*\/\s*/).map((b) => b.replace(/（[^）]*）$/, "").trim())),
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
  it.each(entries.filter(([, e]) => e.source.startsWith("mock:")))("%s", (_, e) => {
    expect(screen(e.source.slice("mock:".length))).toContain(filled(e));
  });
});

describe("出どころの形", () => {
  it("どの文も spec:<章>・mock:<ボード>・user:<日付> のどれか", () => {
    for (const [k, e] of entries) expect(e.source, k).toMatch(/^(spec:\d|mock:[\w-]+|user:\d{4}-\d{2}-\d{2})$/);
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
