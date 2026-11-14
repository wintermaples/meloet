// テストが照らし合わせる元: docs/design/spec.html とモック。jsdom で読む。

// docs/ は Vite の root（ui/）の外なので import できない。Node の fs で読む（型は @types/node を入れずに済むよう最小限）。
interface NodeFs {
  readFileSync(path: URL, encoding: "utf8"): string;
}
const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): NodeFs } }).process.getBuiltinModule("node:fs");
const here = import.meta.url; // new URL(…, import.meta.url) と直接書くと Vite が import に書きかえる
const read = (rel: string) => fs.readFileSync(new URL("../../docs/design/" + rel, here), "utf8");

export function specDocument(): Document {
  return new DOMParser().parseFromString(read("spec.html"), "text/html");
}

const norm = (s: string) => s.replace(/\s+/g, " ").trim();

export const cellText = (el: Element) => norm(el.textContent ?? "");

/** 見出し h2#id から次の h2 までにある table を返す。 */
export function sectionTables(doc: Document, id: string): HTMLTableElement[] {
  const out: HTMLTableElement[] = [];
  let el = doc.getElementById(id)?.nextElementSibling ?? null;
  while (el && el.tagName !== "H2") {
    out.push(...(el.tagName === "TABLE" ? [el as HTMLTableElement] : Array.from(el.querySelectorAll("table"))));
    el = el.nextElementSibling;
  }
  return out;
}

/** 見出し h2#id から次の h2 までの文（空白はつめる）。 */
export function sectionText(doc: Document, id: string): string {
  const parts: string[] = [];
  let el = doc.getElementById(id)?.nextElementSibling ?? null;
  while (el && el.tagName !== "H2") {
    parts.push(el.textContent ?? "");
    el = el.nextElementSibling;
  }
  return norm(parts.join(" "));
}

/** 表の行（tbody の tr）ごとのセルの文。 */
export function rows(table: HTMLTableElement): string[][] {
  return Array.from(table.tBodies[0]?.rows ?? []).map((tr) => Array.from(tr.cells).map(cellText));
}

export function headers(table: HTMLTableElement): string[] {
  return Array.from(table.tHead?.rows[0]?.cells ?? []).map(cellText);
}

/** h3 の見出しの直後にある表。 */
export function tableAfterH3(doc: Document, title: string): HTMLTableElement {
  const h3 = Array.from(doc.querySelectorAll("h3")).find((h) => cellText(h) === title);
  let el = h3?.nextElementSibling ?? null;
  while (el && !el.querySelector("table") && el.tagName !== "TABLE") el = el.nextElementSibling;
  const table = el?.tagName === "TABLE" ? (el as HTMLTableElement) : el?.querySelector("table");
  if (!table) throw new Error(`表がない: ${title}`);
  return table;
}

/** モックのボードの、画面に出る文（文字と aria-label と placeholder）。title 属性は入れない。 */
export function mockScreenText(board: string): string {
  const doc = new DOMParser().parseFromString(read(`mock/${board}.dc.html`), "text/html");
  doc.querySelectorAll("script, style, helmet, title").forEach((n) => n.remove());
  const attrs = Array.from(doc.querySelectorAll("[aria-label], [placeholder]")).flatMap((n) => [
    n.getAttribute("aria-label") ?? "",
    n.getAttribute("placeholder") ?? "",
  ]);
  return norm([doc.body.textContent ?? "", ...attrs].join(" \n "));
}
