// The parameter list of a plugin (spec 3, the 「パラメータ」 rule). Pure functions that do not know the DOM.
// The list keeps the order and groups the plugin gives. A plugin with over 40 starts with its groups folded (folding is only on the screen).
// 「オートメーションに書いている」 is at the very top. A search shows only matches, by group, up to 6 per group, the rest as 「… ほか N」.
// Parameters that only receive the keyboard's knobs (ParamTable.keyboardKnobs) are shown only while searching, in the last group 「鍵盤のつまみを受ける所」 (spec 3 (91-3)).

import type { ParamGroup, ParamKind, ParamRow, ParamTable } from "../../bridge";
import { t } from "../../text";

/** With more than this, start with the groups folded. */
export const FOLD_OVER = 40;
/** While searching, how many to show in one group. */
export const SEARCH_SHOWN = 6;
export const AUTOMATED_GROUP = "auto";
export const KNOBS_GROUP = "knobs";

export type PluginEntry =
  | { kind: "heading"; id: string; group: string; name: string; count: number; folded: boolean }
  | { kind: "row"; id: string; row: ParamRow }
  | { kind: "more"; id: string; group: string; count: number };

export interface PluginLook {
  query: string;
  /** The folded groups (groupId). */
  folded: ReadonlySet<string>;
  /** The groups opened with 「… ほか N」 while searching. */
  expanded: ReadonlySet<string>;
}

export const groupId = (index: number) => `g:${index}`;

export const tableCount = (table: ParamTable) => table.groups.reduce((n, g) => n + g.params.length, 0);

/** The groups to fold when a table comes. */
export function foldedAtFirst(table: ParamTable): Set<string> {
  if (tableCount(table) <= FOLD_OVER) return new Set();
  return new Set(table.groups.map((_, i) => groupId(i)));
}

/** The two tables have the same groups in order (names and counts). If so, the folding can be kept (groupId is the place in order). */
export function sameGroups(a: ParamTable, b: ParamTable): boolean {
  return a.groups.length === b.groups.length && a.groups.every((g, i) => g.name === b.groups[i]!.name && g.params.length === b.groups[i]!.params.length);
}

/** The group heading name. A group without a name is 「そのほか」 only when another group has a name (mock F-ParamMany). null for no heading. */
function headingName(group: ParamGroup, table: ParamTable): string | null {
  if (group.name !== null) return group.name;
  return table.groups.some((g) => g.name !== null) ? t("params.groupOther") : null;
}

const matches = (row: ParamRow, q: string) => row.name.toLowerCase().includes(q);

/** What the list shows (headings, rows, 「… ほか N」), from the top. */
export function pluginEntries(table: ParamTable, automated: readonly number[], look: PluginLook): PluginEntry[] {
  const q = look.query.trim().toLowerCase();
  const out: PluginEntry[] = [];
  const searching = q !== "";
  if (!searching && automated.length > 0) {
    const want = new Set(automated);
    const rows = table.groups.flatMap((g) => g.params.filter((r) => want.has(r.id)));
    if (rows.length > 0) {
      const folded = look.folded.has(AUTOMATED_GROUP);
      out.push({ kind: "heading", id: `h:${AUTOMATED_GROUP}`, group: AUTOMATED_GROUP, name: t("params.automated"), count: rows.length, folded });
      if (!folded) for (const row of rows) out.push({ kind: "row", id: `${AUTOMATED_GROUP}:${row.id}`, row });
    }
  }
  table.groups.forEach((g, i) => {
    const id = groupId(i);
    const name = headingName(g, table);
    const rows = searching ? g.params.filter((r) => matches(r, q)) : g.params;
    if (rows.length === 0) return;
    const folded = !searching && name !== null && look.folded.has(id);
    if (name !== null) out.push({ kind: "heading", id: `h:${id}`, group: id, name, count: rows.length, folded });
    if (folded) return;
    const cut = searching && !look.expanded.has(id) && rows.length > SEARCH_SHOWN ? SEARCH_SHOWN : rows.length;
    for (let k = 0; k < cut; k++) out.push({ kind: "row", id: `${id}:${rows[k]!.id}`, row: rows[k]! });
    if (cut < rows.length) out.push({ kind: "more", id: `m:${id}`, group: id, count: rows.length - cut });
  });
  if (searching) {
    const rows = table.keyboardKnobs.filter((r) => matches(r, q));
    if (rows.length > 0) {
      const id = KNOBS_GROUP;
      out.push({ kind: "heading", id: `h:${id}`, group: id, name: t("params.keyboardKnobs"), count: rows.length, folded: false });
      const cut = !look.expanded.has(id) && rows.length > SEARCH_SHOWN ? SEARCH_SHOWN : rows.length;
      for (let k = 0; k < cut; k++) out.push({ kind: "row", id: `${id}:${rows[k]!.id}`, row: rows[k]! });
      if (cut < rows.length) out.push({ kind: "more", id: `m:${id}`, group: id, count: rows.length - cut });
    }
  }
  return out;
}

/**
 * The blanks of 「ほかに、鍵盤のつまみを受けるだけの 2080 個（MIDI CC 0|0 〜 15|129）は出していません。さがすと出ます。」 under the list.
 * The last name drops the start it shares with the first name (up to the last space). null if no parameter only receives knobs.
 */
export function hiddenKnobs(table: ParamTable): { count: number; first: string; last: string } | null {
  const knobs = table.keyboardKnobs;
  if (knobs.length === 0) return null;
  const first = knobs[0]!.name;
  const last = knobs.at(-1)!.name;
  const head = first.slice(0, first.lastIndexOf(" ") + 1);
  return { count: knobs.length, first, last: head !== "" && last.startsWith(head) ? last.slice(head.length) : last };
}

/** How many match the search box (the 24 of 「24 / 2855」). All when empty. */
export function matchCount(table: ParamTable, query: string): number {
  const q = query.trim().toLowerCase();
  if (q === "") return tableCount(table);
  const knobs = table.keyboardKnobs.filter((r) => matches(r, q)).length;
  return table.groups.reduce((n, g) => n + g.params.filter((r) => matches(r, q)).length, knobs);
}

/** The next row chosen with ↑ ↓ (among the rows in view; stops at the ends). */
export function stepPluginRow(entries: readonly PluginEntry[], selected: number | null, delta: 1 | -1): number | null {
  const rows = entries.flatMap((e) => (e.kind === "row" ? [e.row.id] : []));
  if (rows.length === 0) return null;
  const at = selected === null ? -1 : rows.indexOf(selected);
  if (at < 0) return delta > 0 ? rows[0]! : rows.at(-1)!;
  return rows[Math.max(0, Math.min(rows.length - 1, at + delta))]!;
}

/** The number of steps (null when not stepped). */
export const stepCount = (kind: ParamKind): number | null => (kind.kind === "steps" ? Math.max(2, kind.steps) : null);

/** A 0..1 value → the step number (from 0). */
export function stepOf(kind: ParamKind, value: number): number {
  const n = stepCount(kind) ?? 2;
  return Math.max(0, Math.min(n - 1, Math.round(value * (n - 1))));
}

/** The step number → a 0..1 value. */
export function valueOfStep(kind: ParamKind, step: number): number {
  const n = stepCount(kind) ?? 2;
  return Math.max(0, Math.min(1, step / (n - 1)));
}

/** The value text, as the plugin gives it. 「—」 for a row whose value is unknown (user:2026-11-19). For a plugin that gives no text, 0..1 as %. */
export function pluginValueText(row: Pick<ParamRow, "value" | "text" | "kind">): string {
  if (row.value === null) return t("params.unknownValue");
  if (row.text !== null) return row.text;
  if (row.kind.kind === "toggle") return t(row.value >= 0.5 ? "param.on" : "param.off");
  if (row.kind.kind === "steps") return `${stepOf(row.kind, row.value) + 1} / ${stepCount(row.kind)}`;
  return row.value.toFixed(2);
}

/** The names in the list of steps that Enter opens. 「1」「2」… for a plugin that gives no names (user:2026-11-19). */
export function stepLabels(kind: ParamKind): string[] {
  if (kind.kind !== "steps") return [];
  const n = stepCount(kind)!;
  return Array.from({ length: n }, (_, i) => kind.labels[i] ?? String(i + 1));
}
