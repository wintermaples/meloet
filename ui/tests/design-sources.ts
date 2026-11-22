// What the tests check against: the spec (docs/spec/*.md) and the mocks (docs/design/mock/). Read with jsdom.

// docs/ is outside the Vite root (ui/), so it cannot be imported. Read with Node's fs (a small type, so @types/node is not needed).
interface NodeFs {
  readFileSync(path: URL, encoding: "utf8"): string;
  readdirSync(path: URL): string[];
}
const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): NodeFs } }).process.getBuiltinModule("node:fs");
const here = import.meta.url; // Vite rewrites a direct new URL(…, import.meta.url) into an import
const docs = (rel: string) => new URL("../../docs/" + rel, here);
const read = (rel: string) => fs.readFileSync(docs("design/" + rel), "utf8");

const escapeHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** The Markdown inline marks the spec uses: `code`, **bold**, <kbd>, and backslash escapes. */
function inline(md: string): string {
  return md
    .split(/(`[^`]*`)/)
    .map((part) => {
      if (part.startsWith("`") && part.endsWith("`") && part.length >= 2) {
        return `<code>${escapeHtml(part.slice(1, -1).replace(/\\\|/g, "|"))}</code>`;
      }
      return part
        .split(/(<\/?kbd>|\*\*|\\.)/)
        .map((bit) => (/^<\/?kbd>$/.test(bit) ? bit : bit.startsWith("\\") ? escapeHtml(bit.slice(1)) : escapeHtml(bit)))
        .join("")
        .replace(/\*\*(.*?)\*\*/g, "<b>$1</b>");
    })
    .join("");
}

/** Cells of a table row. A pipe after a backslash is part of the text. */
function cells(line: string): string[] {
  const out: string[] = [];
  let cur = "";
  const body = line.trim().slice(1);
  for (let i = 0; i < body.length; i++) {
    if (body[i] === "\\" && i + 1 < body.length) {
      cur += body.slice(i, i + 2);
      i++;
    } else if (body[i] === "|") {
      out.push(cur.trim());
      cur = "";
    } else cur += body[i];
  }
  if (cur.trim()) out.push(cur.trim());
  return out;
}

const DELIM_ROW = /^\|(\s*:?-+:?\s*\|)+$/;

/** One chapter file to HTML in the shape of the old spec.html: h2#s<n>, h3, p, ul > li, div > table. */
function chapterHtml(md: string, id: string): string {
  const out: string[] = [];
  const blocks = md.split(/\n\s*\n/).map((b) => b.split("\n").filter((l) => l.trim() !== ""));
  for (const lines of blocks) {
    if (lines.length === 0) continue;
    const first = lines[0]!;
    if (first.startsWith("# ")) out.push(`<h2 id="${id}">${inline(first.slice(2).trim())}</h2>`);
    else if (first.startsWith("## ")) out.push(`<h3>${inline(first.slice(3).trim())}</h3>`);
    else if (first.startsWith("- ")) out.push(`<ul>${lines.map((l) => `<li>${inline(l.slice(2).trim())}</li>`).join("")}</ul>`);
    else if (first.startsWith("|")) {
      const [head, delim, ...body] = lines;
      if (!delim || !DELIM_ROW.test(delim.trim())) throw new Error(`not a table: ${first}`);
      const row = (l: string, tag: "th" | "td") => `<tr>${cells(l).map((c) => `<${tag}>${inline(c)}</${tag}>`).join("")}</tr>`;
      out.push(`<div class="tw"><table><thead>${row(head!, "th")}</thead><tbody>${body.map((l) => row(l, "td")).join("")}</tbody></table></div>`);
    } else out.push(`<p>${inline(lines.join(" ").trim())}</p>`);
  }
  return out.join("\n");
}

export function specDocument(): Document {
  const chapters = fs
    .readdirSync(docs("spec/"))
    .flatMap((name) => {
      const m = /^(\d+)-.*\.md$/.exec(name);
      return m ? [{ n: Number(m[1]), name }] : [];
    })
    .sort((a, b) => a.n - b.n)
    .map(({ n, name }) => chapterHtml(fs.readFileSync(docs("spec/" + name), "utf8"), `s${n}`));
  const style = fs.readFileSync(docs("spec/samples.css"), "utf8");
  const html = `<!doctype html><html><head><style>${style}</style></head><body><main>\n${chapters.join("\n")}\n</main></body></html>`;
  return new DOMParser().parseFromString(html, "text/html");
}

const norm = (s: string) => s.replace(/\s+/g, " ").trim();

export const cellText = (el: Element) => norm(el.textContent ?? "");

/** The tables from h2#id up to the next h2. */
export function sectionTables(doc: Document, id: string): HTMLTableElement[] {
  const out: HTMLTableElement[] = [];
  let el = doc.getElementById(id)?.nextElementSibling ?? null;
  while (el && el.tagName !== "H2") {
    out.push(...(el.tagName === "TABLE" ? [el as HTMLTableElement] : Array.from(el.querySelectorAll("table"))));
    el = el.nextElementSibling;
  }
  return out;
}

/** The text from h2#id up to the next h2 (spaces joined). */
export function sectionText(doc: Document, id: string): string {
  const parts: string[] = [];
  let el = doc.getElementById(id)?.nextElementSibling ?? null;
  while (el && el.tagName !== "H2") {
    parts.push(el.textContent ?? "");
    el = el.nextElementSibling;
  }
  return norm(parts.join(" "));
}

/** The text of each cell, row by row (the tr of tbody). */
export function rows(table: HTMLTableElement): string[][] {
  return Array.from(table.tBodies[0]?.rows ?? []).map((tr) => Array.from(tr.cells).map(cellText));
}

export function headers(table: HTMLTableElement): string[] {
  return Array.from(table.tHead?.rows[0]?.cells ?? []).map(cellText);
}

/** The table right after the h3 with this title. */
export function tableAfterH3(doc: Document, title: string): HTMLTableElement {
  const h3 = Array.from(doc.querySelectorAll("h3")).find((h) => cellText(h) === title);
  let el = h3?.nextElementSibling ?? null;
  while (el && !el.querySelector("table") && el.tagName !== "TABLE") el = el.nextElementSibling;
  const table = el?.tagName === "TABLE" ? (el as HTMLTableElement) : el?.querySelector("table");
  if (!table) throw new Error(`表がない: ${title}`);
  return table;
}

/** The text a mock board shows on the screen (the text, aria-label and placeholder). Not the title attribute. */
export function mockScreenText(board: string): string {
  const doc = new DOMParser().parseFromString(read(`mock/${board}.dc.html`), "text/html");
  doc.querySelectorAll("script, style, helmet, title").forEach((n) => n.remove());
  const attrs = Array.from(doc.querySelectorAll("[aria-label], [placeholder], [title]")).flatMap((n) => [
    n.getAttribute("aria-label") ?? "",
    n.getAttribute("placeholder") ?? "",
    n.getAttribute("title") ?? "",
  ]);
  return norm([doc.body.textContent ?? "", ...attrs].join(" \n "));
}
