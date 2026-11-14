// 決まりの見はり: 部品に色の 16 進数、px、font-weight の数を直接書かない（数値は src/design/ の CSS の変数から）。
import { describe, expect, it } from "vitest";

const files = import.meta.glob(["../src/**/*.svelte", "../src/**/*.css", "!../src/design/**"], {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const RULES: [string, RegExp][] = [
  ["色の 16 進数", /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})\b/g],
  ["数字px", /(?<![\w-])\d+(?:\.\d+)?px\b/g],
  ["font-weight の数", /font-weight\s*[:=]\s*["'{]?\s*(?:\d+|bold(?:er)?|lighter)\b/g],
];

/** 見つかった物を「行: 決まり: 文字」の形で返す。 */
export function violations(source: string): string[] {
  const out: string[] = [];
  source.split("\n").forEach((lineText, i) => {
    for (const [name, re] of RULES) {
      for (const m of lineText.matchAll(re)) out.push(`${i + 1}: ${name}: ${m[0]}`);
    }
  });
  return out;
}

describe("決まりの見はり", () => {
  it("見はる物がある（.svelte を読めている）", () => {
    expect(Object.keys(files).some((f) => f.endsWith(".svelte"))).toBe(true);
    expect(Object.keys(files).some((f) => f.includes("/design/"))).toBe(false);
  });

  it.each(Object.entries(files))("%s に色の 16 進数、px、font-weight の数を直接書いていない", (_, source) => {
    expect(violations(source)).toEqual([]);
  });

  it("わざと書いた悪い例で落ちる", () => {
    const bad = `<div style="color:#F2F2F2"></div>
<style>
  .a { background: #222; width: 248px; padding: 0.5px; }
  .b { font-weight: 600; } .c { font-weight:bold; }
</style>
<i style:font-weight={500}></i>`;
    expect(violations(bad)).toEqual([
      "1: 色の 16 進数: #F2F2F2",
      "3: 色の 16 進数: #222",
      "3: 数字px: 248px",
      "3: 数字px: 0.5px",
      "4: font-weight の数: font-weight: 600",
      "4: font-weight の数: font-weight:bold",
      "6: font-weight の数: font-weight={500",
    ]);
  });

  it("CSS の変数や Svelte の書きかたは落ちない", () => {
    const ok = `{#if a}{#each xs as x}<i style:left="{x}px"></i>{/each}{/if}
<style>.a { width: var(--part-tip-width); margin: var(--space-2); font-weight: var(--text-s-weight-600); }</style>`;
    expect(violations(ok)).toEqual([]);
  });
});
