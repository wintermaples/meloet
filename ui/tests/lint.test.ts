// Rule watch: parts do not write hex colors, px, or font-weight numbers directly (values come from the CSS variables of src/design/).
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
  // Colors follow the theme, so they are not written as numbers either, nor is the scheme of the page's own parts.
  ["色の数", /(?<![\w-])(?:rgba?|hsla?)\(/g],
  ["color-scheme の値", /color-scheme\s*:\s*(?:dark|light|normal)\b/g],
];

/** Returns findings in the form 「行: 決まり: 文字」. */
export function violations(source: string): string[] {
  const out: string[] = [];
  source.split("\n").forEach((lineText, i) => {
    for (const [name, re] of RULES) {
      for (const m of lineText.matchAll(re)) out.push(`${i + 1}: ${name}: ${m[0]}`);
    }
  });
  return out;
}

describe("rule watch", () => {
  it("there is something to watch (.svelte files are read)", () => {
    expect(Object.keys(files).some((f) => f.endsWith(".svelte"))).toBe(true);
    expect(Object.keys(files).some((f) => f.includes("/design/"))).toBe(false);
  });

  it.each(Object.entries(files))("%s に色の 16 進数、px、font-weight の数を直接書いていない", (_, source) => {
    expect(violations(source)).toEqual([]);
  });

  it("fails on bad examples written on purpose", () => {
    const bad = `<div style="color:#F2F2F2"></div>
<style>
  .a { background: #222; width: 248px; padding: 0.5px; }
  .b { font-weight: 600; } .c { font-weight:bold; }
</style>
<i style:font-weight={500}></i>
<style>.d { color: rgba(0, 0, 0, 0.5); color-scheme: dark; }</style>`;
    expect(violations(bad)).toEqual([
      "1: 色の 16 進数: #F2F2F2",
      "3: 色の 16 進数: #222",
      "3: 数字px: 248px",
      "3: 数字px: 0.5px",
      "4: font-weight の数: font-weight: 600",
      "4: font-weight の数: font-weight:bold",
      "6: font-weight の数: font-weight={500",
      "7: 色の数: rgba(",
      "7: color-scheme の値: color-scheme: dark",
    ]);
  });

  it("CSS variables and Svelte syntax do not fail", () => {
    const ok = `{#if a}{#each xs as x}<i style:left="{x}px"></i>{/each}{/if}
<style>.a { width: var(--part-tip-width); margin: var(--space-2); font-weight: var(--text-s-weight-600); }</style>`;
    expect(violations(ok)).toEqual([]);
  });
});
