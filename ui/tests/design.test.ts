import { describe, expect, it } from "vitest";
import {
  SCALES,
  colors,
  cssVariables,
  fontFamily,
  line,
  part,
  meterFall,
  meterScale,
  plain,
  partAt,
  radius,
  size,
  space,
  text,
  tokensAt,
  writeCssVariables,
  type PartName,
} from "../src/design/tokens";
import { cellText, headers, rows, sectionTables, sectionText, specDocument, tableAfterH3 } from "./design-sources";

const doc = specDocument();
const triple = (s: string) => s.split("/").map((x) => Number(x.trim()));
const sorted = (xs: Iterable<string>) => [...xs].sort();

describe("仕様書 1 章の表と tokens.ts", () => {
  it("色: 名前と値が全部一致する（R=G=B の列も）", () => {
    const seen: string[] = [];
    for (const [, nameCell, value, rgb, use] of rows(tableAfterH3(doc, "色"))) {
      if (nameCell === "color.tip.bg / color.tip.fg") {
        // 値の列は地の色だけ。文字の色は「使う所」に gray.N で書いてある。
        expect(colors["color.tip.bg"]).toBe(value);
        const bg = use!.match(/地は (gray\.\d+)/)![1];
        const fg = use!.match(/文字は (gray\.\d+)/)![1];
        expect(colors["color.tip.bg"]).toBe(colors[`color.${bg}` as keyof typeof colors]);
        expect(colors["color.tip.fg"]).toBe(colors[`color.${fg}` as keyof typeof colors]);
        seen.push("color.tip.bg", "color.tip.fg");
        continue;
      }
      expect(colors[nameCell as keyof typeof colors], nameCell).toBe(value);
      if (rgb) {
        const n = parseInt(value!.slice(1, 3), 16);
        expect(value).toBe(`#${n.toString(16).padStart(2, "0").repeat(3)}`.toUpperCase());
        expect(n, nameCell).toBe(Number(rgb));
      }
      seen.push(nameCell!);
    }
    expect(sorted(seen)).toEqual(sorted(Object.keys(colors)));
  });

  it("文字: 大きさ・行の高さは率ごとに、太さは使ってよい組として一致する", () => {
    const seen: string[] = [];
    for (const [name, sz, lh, weight] of rows(tableAfterH3(doc, "文字"))) {
      const tok = text[name as keyof typeof text];
      expect(tok, name).toBeDefined();
      expect([...tok.size]).toEqual(triple(sz!));
      expect([...tok.lineHeight]).toEqual(triple(lh!));
      expect(tok.weights.join(" / ")).toBe(weight);
      seen.push(name!);
    }
    expect(sorted(seen)).toEqual(sorted(Object.keys(text)));
  });

  for (const [title, table] of [
    ["すきま", space],
    ["大きさ", size],
  ] as const) {
    it(`${title}: 100 / 125 / 150 % の値が全部一致する`, () => {
      const seen: string[] = [];
      for (const [name, values] of rows(tableAfterH3(doc, title))) {
        expect([...(table as Record<string, readonly number[]>)[name!]!], name).toEqual(triple(values!));
        seen.push(name!);
      }
      expect(sorted(seen)).toEqual(sorted(Object.keys(table)));
    });
  }

  it("線と角: 100 % の値と、決まりの文にある 125 % と 150 % の値が一致する", () => {
    const seen: string[] = [];
    for (const [name, value, rule] of rows(tableAfterH3(doc, "線と角"))) {
      seen.push(name!);
      if (name === "line.hair") {
        expect(line["line.hair"]).toBe(Number(value));
        expect(rule).toContain("物理 1px");
      } else if (name === "line.mark" || name === "line.track") {
        const [a, b, c] = line[name];
        expect(a).toBe(Number(value));
        expect(rule).toContain(`125 % で ${b}、150 % で ${c}`);
      } else if (name === "radius.round") {
        expect(radius["radius.round"]).toBe(value!.replace(" ", ""));
      } else {
        expect(radius[name as "radius.0" | "radius.1"], name).toBe(Number(value));
      }
    }
    expect(rows(tableAfterH3(doc, "線と角")).find((r) => r[0] === "radius.1")![2]).toContain("拡大しても 1");
    expect(sorted(seen)).toEqual(sorted([...Object.keys(line), ...Object.keys(radius)]));
  });

  it("書体の名前の並びが仕様書と同じ", () => {
    const css = doc.querySelector("style")!.textContent!;
    expect(css).toContain(`--sans:${fontFamily.sans};`);
    expect(css).toContain(`--mono:${fontFamily.mono};`);
  });
});

// 率をかけて整数にならないので、四捨五入した値（.5 は切り上げ）。部品の寸法は表の外なので、4 の倍数とはかぎらない。
const ROUNDED = [
  "part.button.icon@125=12.5→13",
  "part.choice.dot@125=7.5→8",
  "part.endpoint.friendly@125=187.5→188",
  "part.endpoint.link@125=7.5→8",
  "part.meter.black-key-slim-h@125=8.75→9",
  "part.meter.black-key-slim-h@150=10.5→11",
  "part.meter.black-key-slim-w@125=6.25→6",
  "part.meter.black-key-slim-w@150=7.5→8",
  "part.meter.clip@125=2.5→3",
  "part.meter.vertical@125=7.5→8",
  "part.note.edge@125=2.5→3",
  "part.note.height@125=13.75→14",
  "part.note.height@150=16.5→17",
  "part.param.knob-w@125=7.5→8",
  "part.param.step@125=7.5→8",
  "part.take.mark@125=7.5→8",
  "part.windowmark.size@125=12.5→13",
];

describe("part.*（仕様書の文の中にしかない寸法）", () => {
  const tables = { s1: sectionTables(doc, "s1"), s2: sectionTables(doc, "s2"), s3: sectionTables(doc, "s3") };

  const fromSpec = Object.entries(plain).flatMap(([k, v]) => ("spec" in v ? [[k, v] as const] : []));
  it.each([...Object.entries(part), ...fromSpec])("%s は仕様書の行にその言葉で書いてある", (_, tok) => {
    expect(tok.spec.quote).toMatch(new RegExp(`(^|\\D)${tok.value}(\\D|$)`));
    if (tok.spec.section === "sample") {
      const css = [...doc.querySelectorAll("style")].map((s) => s.textContent).join("\n");
      expect(css).toContain(tok.spec.quote);
      return;
    }
    if (tok.spec.row === null) {
      expect(sectionText(doc, tok.spec.section)).toContain(tok.spec.quote);
      return;
    }
    const row = tables[tok.spec.section]
      .flatMap((t) => Array.from(t.tBodies[0]?.rows ?? []))
      .find((tr) => cellText(tr.cells[0]!) === tok.spec.row);
    expect(row, tok.spec.row).toBeDefined();
    expect(cellText(row!)).toContain(tok.spec.quote);
    expect(tok.spec.quote).toMatch(new RegExp(`(^|\\D)${tok.value}(\\D|$)`));
  });

  it("率をかけて整数にならない値の一覧と、四捨五入した値（ふえたら、ここが落ちる）", () => {
    const odd = (Object.keys(part) as PartName[]).flatMap((k) =>
      SCALES.filter((s) => !Number.isInteger((part[k].value * s) / 100)).map(
        (s) => `${k}@${s}=${(part[k].value * s) / 100}→${partAt(k, s)}`,
      ),
    );
    expect(odd.sort()).toEqual(ROUNDED);
  });

  it("partAt はどの率でも整数", () => {
    for (const k of Object.keys(part) as PartName[]) for (const s of SCALES) expect(Number.isInteger(partAt(k, s)), k).toBe(true);
  });
});

describe("仕様書 2 章「メーターとフェーダーの目盛」", () => {
  const table = tableAfterH3(doc, "メーターとフェーダーの目盛");
  const num = (s: string) => Number(s.replace("−", "-").replace("+", "").replace(" %", ""));

  it("表の dB と長さが meterScale と同じ", () => {
    const dbs = headers(table).slice(1).map(num);
    const lengths = rows(table)[0]!.slice(1).map(num);
    expect(rows(table)[0]![0]).toBe("長さ");
    expect(meterScale).toEqual(dbs.map((db, i) => [db, lengths[i]]));
  });

  it("落ちる速さは見出しの文のとおり", () => {
    const h3 = Array.from(doc.querySelectorAll("h3")).find((h) => cellText(h) === "メーターとフェーダーの目盛")!;
    expect(cellText(h3.nextElementSibling!)).toContain(`落ちるときは ${meterFall.fallDb} dB を ${meterFall.fallSeconds} 秒かけて`);
  });

  it("meter.hot-from は目盛の −3 dB の長さ", () => {
    expect(meterScale.find(([db]) => db === -3)![1]).toBe(plain["meter.hot-from"].value);
  });
});

describe("率ごとの値と CSS の変数", () => {
  it("line.hair はどの率でも物理 1px（1 / devicePixelRatio の CSS px）", () => {
    for (const s of SCALES) {
      expect(tokensAt(s, 1)["line.hair"]).toBe(1);
      expect(tokensAt(s, 2)["line.hair"]).toBe(0.5);
      expect(tokensAt(s, 1.5)["line.hair"]).toBeCloseTo(2 / 3);
    }
  });

  it("line.mark は 2・2・3、line.track は 4・5・6、radius.1 は 1 のまま", () => {
    expect(SCALES.map((s) => tokensAt(s, 1)["line.mark"])).toEqual([2, 2, 3]);
    expect(SCALES.map((s) => tokensAt(s, 1)["line.track"])).toEqual([4, 5, 6]);
    expect(SCALES.map((s) => tokensAt(s, 1)["radius.1"])).toEqual([1, 1, 1]);
  });

  it("表の値は 100 % の値に率をかけたものと同じ", () => {
    for (const tbl of [space, size] as Record<string, readonly number[]>[]) {
      for (const [k, v] of Object.entries(tbl)) {
        expect([v[0]! * 1.25, v[0]! * 1.5], k).toEqual([v[1], v[2]]);
      }
    }
  });

  it("CSS の変数の名前と値", () => {
    const v = cssVariables(125, 2);
    expect(v["--color-gray-4"]).toBe("#222222");
    expect(v["--color-meter-ok"]).toBe("#4CD964");
    expect(v["--space-3"]).toBe("15px");
    expect(v["--size-meter-slim"]).toBe("30px");
    expect(v["--size-meter-name-slim"]).toBe("220px");
    expect(v["--text-s-size"]).toBe("15px");
    expect(v["--text-s-line-height"]).toBe("20px");
    expect(v["--part-button-icon"]).toBe("13px");
    expect(v["--line-hair"]).toBe("0.5px");
    expect(v["--radius-round"]).toBe("50%");
    expect(v["--part-tip-width"]).toBe("310px");
    expect(v["--font-sans"]).toBe(fontFamily.sans);
  });

  it("太さの変数は、その大きさで使ってよい組だけ（率によらない）", () => {
    for (const sc of SCALES) {
      const names = Object.keys(cssVariables(sc, 1)).filter((k) => k.includes("-weight"));
      expect(names.sort()).toEqual(
        [
          "--text-l-weight-600",
          "--text-m-weight-400",
          "--text-m-weight-500",
          "--text-s-weight-400",
          "--text-s-weight-500",
          "--text-s-weight-600",
        ],
      );
      for (const n of names) expect(cssVariables(sc, 1)[n]).toBe(n.slice(-3));
    }
  });

  it(":root に書ける", () => {
    writeCssVariables(150, 1);
    const st = document.documentElement.style;
    expect(st.getPropertyValue("--space-1")).toBe("6px");
    expect(st.getPropertyValue("--color-tip-bg")).toBe("#F2F2F2");
    expect(st.getPropertyValue("--line-hair")).toBe("1px");
  });
});
