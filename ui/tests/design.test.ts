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
import { THEME_IDS, palettes } from "../src/design/themes";
import { cellText, headers, rows, sectionTables, sectionText, specDocument, tableAfterH3 } from "./design-sources";

const doc = specDocument();
const pair = (s: string) => s.split("/").map((x) => Number(x.trim()));
const sorted = (xs: Iterable<string>) => [...xs].sort();

describe("spec 1 tables and tokens.ts", () => {
  it("colors: all names and values match (the R=G=B column too)", () => {
    const seen: string[] = [];
    for (const [, nameCell, value, rgb, use] of rows(tableAfterH3(doc, "Colors"))) {
      if (nameCell === "color.tip.bg / color.tip.fg") {
        // The value column is background colors only. Text colors are written as gray.N in the "where used" column.
        expect(colors["color.tip.bg"]).toBe(value);
        const bg = use!.match(/background is (gray\.\d+)/)![1];
        const fg = use!.match(/the text is (gray\.\d+)/)![1];
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

  it("themes: the table has every name, and every value of every theme (design/themes.ts)", () => {
    const table = tableAfterH3(doc, "Themes");
    expect(headers(table).slice(1)).toEqual(THEME_IDS.map((id) => id));
    const seen: string[] = [];
    for (const [name, ...values] of rows(table)) {
      THEME_IDS.forEach((id, i) => expect(values[i], `${name} ${id}`).toBe(palettes[id][name as keyof typeof colors]));
      seen.push(name!);
    }
    expect(sorted(seen)).toEqual(sorted(Object.keys(colors)));
  });

  it("text: size and line height match per scale; weight matches as the allowed sets", () => {
    const seen: string[] = [];
    for (const [name, sz, lh, weight] of rows(tableAfterH3(doc, "Text"))) {
      const tok = text[name as keyof typeof text];
      expect(tok, name).toBeDefined();
      expect([...tok.size]).toEqual(pair(sz!));
      expect([...tok.lineHeight]).toEqual(pair(lh!));
      expect(tok.weights.join(" / ")).toBe(weight);
      seen.push(name!);
    }
    expect(sorted(seen)).toEqual(sorted(Object.keys(text)));
  });

  for (const [title, table] of [
    ["Space", space],
    ["Sizes", size],
  ] as const) {
    it(`${title}: all values at 100 / 125 % match`, () => {
      const seen: string[] = [];
      for (const [name, values] of rows(tableAfterH3(doc, title))) {
        expect([...(table as Record<string, readonly number[]>)[name!]!], name).toEqual(pair(values!));
        seen.push(name!);
      }
      expect(sorted(seen)).toEqual(sorted(Object.keys(table)));
    });
  }

  it("lines and corners: the 100 % values, and the 125 % values in the rule text, match", () => {
    const seen: string[] = [];
    for (const [name, value, rule] of rows(tableAfterH3(doc, "Lines and corners"))) {
      seen.push(name!);
      if (name === "line.hair") {
        expect(line["line.hair"]).toBe(Number(value));
        expect(rule).toContain("physical 1px");
      } else if (name === "line.mark" || name === "line.track") {
        const [a, b] = line[name];
        expect(a).toBe(Number(value));
        expect(rule).toContain(`${b} at 125 %`);
      } else if (name === "radius.round") {
        expect(radius["radius.round"]).toBe(value!.replace(" ", ""));
      } else {
        expect(radius[name as "radius.0" | "radius.1"], name).toBe(Number(value));
      }
    }
    expect(rows(tableAfterH3(doc, "Lines and corners")).find((r) => r[0] === "radius.1")![2]).toContain("Stays 1 when scaled up");
    expect(sorted(seen)).toEqual(sorted([...Object.keys(line), ...Object.keys(radius)]));
  });

  it("the order of font names is the same as the spec", () => {
    const css = doc.querySelector("style")!.textContent!;
    expect(css).toContain(`--sans:${fontFamily.sans};`);
    expect(css).toContain(`--mono:${fontFamily.mono};`);
  });
});

// Scaling does not give an integer, so the value is rounded (.5 rounds up). Part sizes are outside the table, so they are not always multiples of 4.
const ROUNDED = [
  "part.audioclip.handle@125=7.5→8",
  "part.button.icon@125=12.5→13",
  "part.cc.point@125=7.5→8",
  "part.choice.dot@125=7.5→8",
  "part.clip.edge@125=7.5→8",
  "part.drum.hit-inset@125=2.5→3",
  "part.drum.hit-min@125=7.5→8",
  "part.endpoint.friendly@125=187.5→188",
  "part.endpoint.link@125=7.5→8",
  "part.meter.black-key-slim-h@125=8.75→9",
  "part.meter.black-key-slim-w@125=6.25→6",
  "part.meter.clip@125=2.5→3",
  "part.meter.vertical@125=7.5→8",
  "part.note.edge@125=2.5→3",
  "part.note.height@125=13.75→14",
  "part.note.resize@125=7.5→8",
  "part.param.knob-w@125=7.5→8",
  "part.param.step@125=7.5→8",
  "part.reading.offset@125=7.5→8",
  "part.reading.pad-x@125=7.5→8",
  "part.take.mark@125=7.5→8",
  "part.windowmark.size@125=12.5→13",
];

describe("part.* (sizes found only in the spec text)", () => {
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

  it("the list of values that do not become integers when scaled, and their rounded values (this fails if more appear)", () => {
    const odd = (Object.keys(part) as PartName[]).flatMap((k) =>
      SCALES.filter((s) => !Number.isInteger((part[k].value * s) / 100)).map(
        (s) => `${k}@${s}=${(part[k].value * s) / 100}→${partAt(k, s)}`,
      ),
    );
    expect(odd.sort()).toEqual(ROUNDED);
  });

  it("partAt is an integer at every scale", () => {
    for (const k of Object.keys(part) as PartName[]) for (const s of SCALES) expect(Number.isInteger(partAt(k, s)), k).toBe(true);
  });
});

describe("spec 2 (Scale marks of meters and faders)", () => {
  const table = tableAfterH3(doc, "Scale marks of meters and faders");
  const num = (s: string) => Number(s.replace("−", "-").replace("+", "").replace(" %", ""));

  it("the table's dB and lengths are the same as meterScale", () => {
    const dbs = headers(table).slice(1).map(num);
    const lengths = rows(table)[0]!.slice(1).map(num);
    expect(rows(table)[0]![0]).toBe("Length");
    expect(meterScale).toEqual(dbs.map((db, i) => [db, lengths[i]]));
  });

  it("the fall speed follows the heading text", () => {
    const h3 = Array.from(doc.querySelectorAll("h3")).find((h) => cellText(h) === "Scale marks of meters and faders")!;
    expect(cellText(h3.nextElementSibling!)).toContain(`fall ${meterFall.fallDb} dB in ${meterFall.fallSeconds} seconds`);
  });

  it("meter.hot-from is the length of −3 dB on the scale", () => {
    expect(meterScale.find(([db]) => db === -3)![1]).toBe(plain["meter.hot-from"].value);
  });
});

describe("values per scale and CSS variables", () => {
  it("line.hair is 1 physical px at every scale (1 / devicePixelRatio CSS px)", () => {
    for (const s of SCALES) {
      expect(tokensAt(s, 1)["line.hair"]).toBe(1);
      expect(tokensAt(s, 2)["line.hair"]).toBe(0.5);
      expect(tokensAt(s, 1.5)["line.hair"]).toBeCloseTo(2 / 3);
    }
  });

  it("line.mark is 2, 2, line.track is 4, 5, radius.1 stays 1", () => {
    expect(SCALES.map((s) => tokensAt(s, 1)["line.mark"])).toEqual([2, 2]);
    expect(SCALES.map((s) => tokensAt(s, 1)["line.track"])).toEqual([4, 5]);
    expect(SCALES.map((s) => tokensAt(s, 1)["radius.1"])).toEqual([1, 1]);
  });

  it("table values equal the 100 % values times the scale", () => {
    for (const tbl of [space, size] as Record<string, readonly number[]>[]) {
      for (const [k, v] of Object.entries(tbl)) {
        expect(v[0]! * 1.25, k).toBe(v[1]);
      }
    }
  });

  it("CSS variable names and values", () => {
    const v = cssVariables(125, 2);
    expect(v["--color-gray-4"]).toBe("#2F2F2F");
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

  it("weight variables are only the sets allowed at that size (not depending on the scale)", () => {
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

  it("can be written to :root", () => {
    writeCssVariables(125, 1);
    const st = document.documentElement.style;
    expect(st.getPropertyValue("--space-1")).toBe("5px");
    expect(st.getPropertyValue("--color-tip-bg")).toBe("#F2F2F2");
    expect(st.getPropertyValue("--line-hair")).toBe("1px");
  });
});
