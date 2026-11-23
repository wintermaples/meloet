// Content of 「オープンソースのライセンス」. Does not know the DOM.
// The list (generated/licenses.json) is made by tools/licenses/inventory.mjs, and licenses/check.mjs checks that it is not out of date.
// The full license texts are large, so npm run build writes them to dist/licenses/texts.json, read when the second page opens.

import { t } from "../../text";
import generated from "./generated/licenses.json";

export interface Part {
  /** `vendored`: other people's source copied as it is into a meloet crate (C++ headers and so on). */
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
/** Other people's things inside the crates (licenses/upstream.json). Copied sources have a name and version. */
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

/** All part rows: cargo and npm parts, and copied sources (Signalsmith Stretch and so on). */
export function allParts(list: LicenseList): Part[] {
  const vendored: Part[] = (list.upstream ?? []).flatMap((u) =>
    u.vendored && u.name && u.version ? [{ kind: "vendored" as const, name: u.name, version: u.version, license: u.license }] : [],
  );
  return [...list.parts, ...vendored];
}

/** The name that links a row and its full text. */
export const partKey = (p: { name: string; version: string }): string => `${p.name}@${p.version}`;

/** License heading. A plain OR is sorted by name, so 「MIT OR Apache-2.0」 and 「Apache-2.0 OR MIT」 become one (the same rule as in tools). */
export function licenseGroup(expr: string): string {
  const e = expr.replace(/\s+/g, " ").trim();
  if (/[()]|\bAND\b|\bWITH\b/.test(e)) return e;
  return e
    .split(" OR ")
    .sort((a, b) => a.localeCompare(b, "en"))
    .join(" OR ");
}

/** 「A OR B」 to 「A または B」 (as written in mock R7-About, R7-AboutLicense). */
export function licenseText(expr: string): string {
  return expr.split(" OR ").reduce((a, b) => t("about.licenseOr", { a, b }));
}

/** License names of the sound pack: CC-BY-3.0 → CC BY 3.0 (mock R7-AboutLicense), CC0-1.0 → CC0 1.0. */
export function soundLicenseText(id: string): string {
  const by = /^CC-BY-(\d\.\d)$/.exec(id);
  if (by) return `CC BY ${by[1]}`;
  if (id === "CC0-1.0") return "CC0 1.0";
  return id;
}

/** Search box: the typed text is in the name or license (case is ignored). */
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

/** Parts grouped by license. Most first; with the same count, by name. While searching, headings with no match are hidden. */
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

/** Content of the full text box. For each file, a file name line and the body. */
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

/** Reads the full texts (only once). null in the dev vite (npm run dev), which does not make them. */
export function loadTexts(fetcher: (url: string) => Promise<Response> = (u) => fetch(u)): Promise<TextsFile | null> {
  pending ??= fetcher(TEXTS_URL)
    .then((r) => (r.ok ? (r.json() as Promise<TextsFile>) : null))
    .catch(() => null);
  return pending;
}

/** For tests. */
export function forgetTexts(): void {
  pending = null;
}
