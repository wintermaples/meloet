// プラグインのパラメータの一覧（仕様書 3 章「パラメータ」）。DOM を知らない純粋な関数。
// 一覧はプラグインが言ってきた順とグループのまま。40 個をこえる子は、グループをたたんで始める（たたむのは画面だけ）。
// いちばん上に「オートメーションに書いている」。さがすと、合う物だけがグループごとに出て、1 グループ 6 個まで、残りは「… ほか N」。
// 鍵盤のつまみを受けるだけのパラメータ（ParamTable.keyboardKnobs）は、さがしているときだけ、いちばん下のグループ「鍵盤のつまみを受ける所」に出す（仕様書 3 章 91-3）。

import type { ParamGroup, ParamKind, ParamRow, ParamTable } from "../../bridge";
import { t } from "../../text/ja";

/** これより多いと、グループをたたんで始める。 */
export const FOLD_OVER = 40;
/** さがしているとき、1 グループに出す数。 */
export const SEARCH_SHOWN = 6;
export const AUTOMATED_GROUP = "auto";
export const KNOBS_GROUP = "knobs";

export type PluginEntry =
  | { kind: "heading"; id: string; group: string; name: string; count: number; folded: boolean }
  | { kind: "row"; id: string; row: ParamRow }
  | { kind: "more"; id: string; group: string; count: number };

export interface PluginLook {
  query: string;
  /** たたんだグループ（groupId）。 */
  folded: ReadonlySet<string>;
  /** さがしているとき「… ほか N」を押して広げたグループ。 */
  expanded: ReadonlySet<string>;
}

export const groupId = (index: number) => `g:${index}`;

export const tableCount = (table: ParamTable) => table.groups.reduce((n, g) => n + g.params.length, 0);

/** 表を受けとったときに、たたんでおくグループ。 */
export function foldedAtFirst(table: ParamTable): Set<string> {
  if (tableCount(table) <= FOLD_OVER) return new Set();
  return new Set(table.groups.map((_, i) => groupId(i)));
}

/** 2 つの表のグループの並び（名前と数）が同じ。同じなら、たたみ方（groupId は並びの番号）をそのまま使える。 */
export function sameGroups(a: ParamTable, b: ParamTable): boolean {
  return a.groups.length === b.groups.length && a.groups.every((g, i) => g.name === b.groups[i]!.name && g.params.length === b.groups[i]!.params.length);
}

/** グループの見出しの名前。名前のない物は、名前のあるグループがほかにあるときだけ「そのほか」（モック F-ParamMany）。見出しを出さないなら null。 */
function headingName(group: ParamGroup, table: ParamTable): string | null {
  if (group.name !== null) return group.name;
  return table.groups.some((g) => g.name !== null) ? t("params.groupOther") : null;
}

const matches = (row: ParamRow, q: string) => row.name.toLowerCase().includes(q);

/** 一覧に出す物（見出し ・ 行 ・「… ほか N」）を上から順に。 */
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
 * 一覧の下の「ほかに、鍵盤のつまみを受けるだけの 2080 個（MIDI CC 0|0 〜 15|129）は出していません。さがすと出ます。」の穴。
 * 最後の名前は、最初の名前と同じ頭（最後の空白まで）を外す。鍵盤のつまみを受けるだけの物がなければ null。
 */
export function hiddenKnobs(table: ParamTable): { count: number; first: string; last: string } | null {
  const knobs = table.keyboardKnobs;
  if (knobs.length === 0) return null;
  const first = knobs[0]!.name;
  const last = knobs.at(-1)!.name;
  const head = first.slice(0, first.lastIndexOf(" ") + 1);
  return { count: knobs.length, first, last: head !== "" && last.startsWith(head) ? last.slice(head.length) : last };
}

/** さがす枠に合う数（「24 / 2855」の 24）。空なら全部。 */
export function matchCount(table: ParamTable, query: string): number {
  const q = query.trim().toLowerCase();
  if (q === "") return tableCount(table);
  const knobs = table.keyboardKnobs.filter((r) => matches(r, q)).length;
  return table.groups.reduce((n, g) => n + g.params.filter((r) => matches(r, q)).length, knobs);
}

/** ↑ ↓ でえらぶ次の行（見えている行の中。はしで止まる）。 */
export function stepPluginRow(entries: readonly PluginEntry[], selected: number | null, delta: 1 | -1): number | null {
  const rows = entries.flatMap((e) => (e.kind === "row" ? [e.row.id] : []));
  if (rows.length === 0) return null;
  const at = selected === null ? -1 : rows.indexOf(selected);
  if (at < 0) return delta > 0 ? rows[0]! : rows.at(-1)!;
  return rows[Math.max(0, Math.min(rows.length - 1, at + delta))]!;
}

/** 段階の数（段階でなければ null）。 */
export const stepCount = (kind: ParamKind): number | null => (kind.kind === "steps" ? Math.max(2, kind.steps) : null);

/** 0..1 の値 → 段の番号（0 から）。 */
export function stepOf(kind: ParamKind, value: number): number {
  const n = stepCount(kind) ?? 2;
  return Math.max(0, Math.min(n - 1, Math.round(value * (n - 1))));
}

/** 段の番号 → 0..1 の値。 */
export function valueOfStep(kind: ParamKind, step: number): number {
  const n = stepCount(kind) ?? 2;
  return Math.max(0, Math.min(1, step / (n - 1)));
}

/** 値の文字。プラグインが言ってきた文字のまま。値がわからない行は「—」（user:2026-11-19）。文字をくれない子は 0..1 を % で。 */
export function pluginValueText(row: Pick<ParamRow, "value" | "text" | "kind">): string {
  if (row.value === null) return t("params.unknownValue");
  if (row.text !== null) return row.text;
  if (row.kind.kind === "toggle") return t(row.value >= 0.5 ? "param.on" : "param.off");
  if (row.kind.kind === "steps") return `${stepOf(row.kind, row.value) + 1} / ${stepCount(row.kind)}`;
  return row.value.toFixed(2);
}

/** Enter でひらく段の一覧の名前。名前を言わない子は「1」「2」…（user:2026-11-19）。 */
export function stepLabels(kind: ParamKind): string[] {
  if (kind.kind !== "steps") return [];
  const n = stepCount(kind)!;
  return Array.from({ length: n }, (_, i) => kind.labels[i] ?? String(i + 1));
}
