// 「オープンソースのライセンス」の中身。DOM を知らない。
// 一覧（generated/licenses.json）は tools/licenses/inventory.mjs が作り、licenses/check.mjs が古くなっていないか見はる。
// ライセンスの全文は大きいので、npm run build が dist/licenses/texts.json に書き、2 段目をひらいたときに読む。

import { t } from "../../text/ja";
import generated from "./generated/licenses.json";

export interface Part {
  /** `vendored`: ほかの人のソースを、そのまま写して meloet のクレートの中に置いた物（C++ のヘッダなど）。 */
  kind: "crate" | "npm" | "vendored";
  name: string;
  version: string;
  license: string;
}
export interface Font {
  name: string;
  package: string;
  version: string;
  license: string;
}
export interface SoundCredit {
  id: string;
  title: string;
  creator: string;
  license: string;
  source: string;
  licenseUri: string;
  also: string | null;
  changes: string;
}
/** クレートの中にある、ほかの人の物（licenses/upstream.json）。写したソースには名前と版がつく。 */
export interface Upstream {
  crate: string;
  license: string;
  name?: string;
  version?: string;
  vendored?: unknown;
}
export interface LicenseList {
  meloet: { version: string; license: string };
  sounds: SoundCredit[];
  fonts: Font[];
  upstream?: Upstream[];
  parts: Part[];
}

export const licenseList = generated as LicenseList;

/** 部品の行ぜんぶ: cargo と npm の部品と、写したソース（Signalsmith Stretch など）。 */
export function allParts(list: LicenseList): Part[] {
  const vendored: Part[] = (list.upstream ?? []).flatMap((u) =>
    u.vendored && u.name && u.version ? [{ kind: "vendored" as const, name: u.name, version: u.version, license: u.license }] : [],
  );
  return [...list.parts, ...vendored];
}

/** 行と全文をむすぶ名前。 */
export const partKey = (p: { name: string; version: string }): string => `${p.name}@${p.version}`;

/** ライセンスの見出し。ただの OR は名前の順にならべ、「MIT OR Apache-2.0」と「Apache-2.0 OR MIT」を 1 つにする（tools と同じ決まり）。 */
export function licenseGroup(expr: string): string {
  const e = expr.replace(/\s+/g, " ").trim();
  if (/[()]|\bAND\b|\bWITH\b/.test(e)) return e;
  return e
    .split(" OR ")
    .sort((a, b) => a.localeCompare(b, "en"))
    .join(" OR ");
}

/** 「A OR B」を「A または B」（モック R7-About、R7-AboutLicense の書きかた）。 */
export function licenseText(expr: string): string {
  return expr.split(" OR ").reduce((a, b) => t("about.licenseOr", { a, b }));
}

/** 音の素材のライセンスの名前: CC-BY-3.0 → CC BY 3.0（モック R7-AboutLicense）、CC0-1.0 → CC0 1.0。 */
export function soundLicenseText(id: string): string {
  const by = /^CC-BY-(\d\.\d)$/.exec(id);
  if (by) return `CC BY ${by[1]}`;
  if (id === "CC0-1.0") return "CC0 1.0";
  return id;
}

/** さがす枠: 名前かライセンスに、打った文字が（大文字 ・ 小文字をくらべずに）入っている。 */
export function matches(query: string, ...fields: string[]): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return fields.some((f) => f.toLowerCase().includes(q));
}

export interface LicenseGroup {
  group: string;
  label: string;
  parts: Part[];
}

/** 部品をライセンスごとに。数の多い順、同じ数なら名前の順。さがしているときは、合う物のない見出しを出さない。 */
export function partGroups(parts: readonly Part[], query: string): LicenseGroup[] {
  const by = new Map<string, Part[]>();
  for (const p of parts) {
    const g = licenseGroup(p.license);
    if (!matches(query, p.name, g, licenseText(g))) continue;
    by.set(g, [...(by.get(g) ?? []), p]);
  }
  return [...by.entries()]
    .map(([group, ps]) => ({ group, label: licenseText(group), parts: ps }))
    .sort((a, b) => b.parts.length - a.parts.length || a.group.localeCompare(b.group, "en"));
}

export function soundsMatching(sounds: readonly SoundCredit[], query: string): SoundCredit[] {
  return sounds.filter((s) => matches(query, s.title, s.creator, s.license, soundLicenseText(s.license)));
}

export function fontsMatching(fonts: readonly Font[], query: string): Font[] {
  return fonts.filter((f) => matches(query, f.name, f.license));
}

/** 全文の枠の中身。ファイルごとに、ファイルの名前の行と本文。 */
export interface TextsFile {
  parts: Record<string, { file: string; text: string }[]>;
  texts: Record<string, string>;
}

export function fullText(file: TextsFile, key: string): string | null {
  const entries = file.parts[key];
  if (!entries) return null;
  return entries.map((e) => `${e.file}\n\n${file.texts[e.text] ?? ""}`.trimEnd()).join("\n\n\n");
}

const TEXTS_URL = "licenses/texts.json";
let pending: Promise<TextsFile | null> | null = null;

/** 全文を読む（1 回だけ）。開発の vite（npm run dev）では作っていないので null。 */
export function loadTexts(fetcher: (url: string) => Promise<Response> = (u) => fetch(u)): Promise<TextsFile | null> {
  pending ??= fetcher(TEXTS_URL)
    .then((r) => (r.ok ? (r.json() as Promise<TextsFile>) : null))
    .catch(() => null);
  return pending;
}

/** テストのため。 */
export function forgetTexts(): void {
  pending = null;
}
