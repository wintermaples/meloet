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

  it("仕様書の外の用語は、ヤチヨさんが決めた「仮の音」「ベロシティ固定」「取り消しの履歴」だけ", () => {
    expect(terms.filter((x) => x.source !== "spec:5")).toEqual([
      { term: "仮の音", description: "本物のピアノの音が入るまでの、かわりの音。", source: "user:2026-11-14" },
      {
        term: "ベロシティ固定",
        description: "鍵盤をどの強さで押しても、決めた強さで鳴らして録る。書く道具のノートも同じ強さになる。",
        source: "user:2026-11-18",
      },
      { term: "取り消しの履歴", description: "これまでにした手の一覧。どこまで取り消したかが見える。", source: "user:2026-11-18" },
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
    if (e.fit === "quoted") expect(row![1]).toContain(`「${filled(e)}」`);
    else if (e.fit === "part") expect(row![1]).toContain(filled(e));
    else expect(filled(e)).toBe(row![1]);
  });

  // ja.ts にない場面: ほかの場面の文を出す決まりだけの場面と、殻（crates/meloet-app）が出す PC のファイルの窓の題。
  const NOT_IN_JA = ["meloet の音の流れだけが止まった", "PC のファイルの窓の題"];
  it("6 章の場面は全部 ja.ts に入っている（ja.ts にない 2 つのほか）", () => {
    for (const scene of NOT_IN_JA) expect(byScene.has(scene), scene).toBe(true);
    expect([...new Set(messages.map(([, e]) => e.scene))].sort()).toEqual(
      [...byScene.keys()].filter((k) => !NOT_IN_JA.includes(k)).sort(),
    );
  });

  it("PC のファイルの窓の題は、殻の定数が仕様書の文のまま", () => {
    const rust = readRepoFile("crates/meloet-app/src/pcwindow.rs");
    const cell = byScene.get("PC のファイルの窓の題")![1]!;
    const titles = [...rust.matchAll(/pub const (OPEN|FOLDER)_TITLE: &str = "([^"]+)";/g)].map((m) => [m[1], m[2]!]);
    expect(Object.fromEntries(titles)).toEqual({ OPEN: "ひらく", FOLDER: "場所をえらぶ" });
    expect(cell).toBe("ひらくときは「ひらく」、フォルダをえらぶときは「場所をえらぶ」");
  });

  it("決まりだけの場面は、使う文の場面とボタンを名前で言っている", () => {
    const row = byScene.get("meloet の音の流れだけが止まった")!;
    expect(row[1]).toContain("「音の出口が消えた」と同じ文");
    expect(row[1]).toContain("「録音中に出口が消えた」と同じ文");
    expect(row[3]).toBe(t("button.openAudioSettings"));
  });

  it("使えない値を打った: 4 つの欄と範囲で文になる。ベロシティは仕様書の例そのまま", () => {
    const cell = byScene.get("使えない値を打った")![1]!;
    const say = (field: Parameters<typeof t>[0], range: Parameters<typeof t>[0], kept: string) =>
      t("msg.valueRejected", { field: t(field), range: t(range), kept });
    expect(cell.startsWith(say("value.velocity", "value.velocityRange", "77"))).toBe(true);
    expect(say("value.tempo", "value.tempoRange", "96.00")).toBe("テンポは 30.00〜300.00 です。もとの 96.00 のままです。");
    expect(say("value.meterNum", "value.meterNumRange", "4")).toBe("拍子の上は 1〜12 です。もとの 4 のままです。");
    expect(say("value.meterDen", "value.meterDenRange", "4")).toBe("拍子の下は 2 ・ 4 ・ 8 ・ 16 です。もとの 4 のままです。");
    expect(cell).toContain(`（${t("value.tempo")}は ${t("value.tempoRange")}、${t("value.meterNum")}は ${t("value.meterNumRange")}、下は ${t("value.meterDenRange")}）`);
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
  const fromMock = entries.filter(([, e]) => e.source.startsWith("mock:"));
  it.each(fromMock)("%s", (_, e) => {
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
