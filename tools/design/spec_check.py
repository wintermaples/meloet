#!/usr/bin/env python3
"""Check the spec in docs/spec/ (Markdown, one file per chapter).

Default: compare the text of every chapter with the old docs/spec/.
Tags and Markdown marks are taken out and spaces are joined, so only the words
are compared. Also checks that every table row has as many cells as its
header, and that docs/spec/samples.css is the page style of spec.html.

--strings: for every entry in ui/src/text/ja.ts whose source is spec:<n>
(and the terms of chapter 5), check that its text is in chapter <n>. A text
with holes ({name}) is cut at the holes; each piece must be in the chapter.

Exit code 1 if anything is wrong. Standard library only; run with python3 -I.
"""

import argparse
import difflib
import re
import sys
from html.parser import HTMLParser
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC_DIR = ROOT / "docs" / "spec"
SPEC_HTML = ROOT / "docs" / "design" / "spec.html"
JA_TS = ROOT / "ui" / "src" / "text" / "ja.ts"

BLOCK = {"h1", "h2", "h3", "p", "ul", "li", "table", "thead", "tbody", "tr", "th", "td", "div", "nav", "a", "main"}


def squeeze(s: str) -> str:
    return re.sub(r"\s+", " ", s).strip()


# --- the old spec.html -------------------------------------------------------


class SpecHtml(HTMLParser):
    """Text of spec.html per part: "preface" (h1 and lead) and "1".."7" (from each h2)."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: dict[str, list[str]] = {}
        self.part: str | None = None
        self.skip = 0
        self.style: list[str] = []
        self.in_style = False
        self.in_main = False

    def handle_starttag(self, tag, attrs):
        if tag == "style":
            self.in_style = True
        if tag == "main":
            self.in_main = True
            self.part = "preface"
        if tag == "nav":
            self.skip += 1
        if tag == "h2":
            self.part = dict(attrs)["id"].removeprefix("s")
        if self.part and tag in BLOCK:
            self.parts.setdefault(self.part, []).append(" ")

    def handle_endtag(self, tag):
        if tag == "style":
            self.in_style = False
        if tag == "nav":
            self.skip -= 1
        if tag == "main":
            self.in_main = False
        if self.part and tag in BLOCK:
            self.parts.setdefault(self.part, []).append(" ")

    def handle_data(self, data):
        if self.in_style:
            self.style.append(data)
        elif self.in_main and self.part and not self.skip:
            self.parts.setdefault(self.part, []).append(data)


def html_parts() -> tuple[dict[str, str], str]:
    p = SpecHtml()
    p.feed(SPEC_HTML.read_text(encoding="utf-8"))
    return {k: squeeze("".join(v)) for k, v in p.parts.items()}, "".join(p.style)


# --- the Markdown ------------------------------------------------------------

DELIM_ROW = re.compile(r"^\|(\s*:?-+:?\s*\|)+$")


def split_row(line: str) -> list[str]:
    """Cells of a table row; a pipe after a backslash is part of the text."""
    cells, cur, i = [], "", 0
    body = line.strip()[1:]
    while i < len(body):
        ch = body[i]
        if ch == "\\" and i + 1 < len(body):
            cur += body[i : i + 2]
            i += 2
            continue
        if ch == "|":
            cells.append(cur)
            cur = ""
        else:
            cur += ch
        i += 1
    if cur.strip():
        cells.append(cur)
    return cells


def plain_inline(s: str) -> str:
    s = re.sub(r"</?kbd>", "", s)
    s = s.replace("**", "").replace("`", "")
    return re.sub(r"\\(.)", r"\1", s)


def md_plain(md: str) -> str:
    out = []
    for line in md.splitlines():
        t = line.strip()
        if DELIM_ROW.match(t):
            continue
        if t.startswith("|"):
            out.extend(plain_inline(c) for c in split_row(t))
            continue
        t = re.sub(r"^#{1,6} ", "", t)
        t = re.sub(r"^- ", "", t)
        out.append(plain_inline(t))
    return squeeze(" ".join(out))


def table_shape_errors(name: str, md: str) -> list[str]:
    errs, width = [], None
    for n, line in enumerate(md.splitlines(), 1):
        t = line.strip()
        if not t.startswith("|"):
            width = None
            continue
        cells = t.count("|") - t.count("\\|") - 1
        if width is None:
            width = cells
        elif cells != width:
            errs.append(f"{name}:{n}: {cells} cells, the header has {width}")
    return errs


def chapter_files() -> dict[str, Path]:
    out = {}
    for f in sorted(SPEC_DIR.glob("[0-9]-*.md")):
        out[f.name.split("-", 1)[0]] = f
    return out


def md_parts() -> dict[str, str]:
    parts = {k: md_plain(f.read_text(encoding="utf-8")) for k, f in chapter_files().items()}
    readme = (SPEC_DIR / "README.md").read_text(encoding="utf-8")
    title = readme.splitlines()[0]
    preface = readme.split("\n## Preface\n", 1)[1]
    parts["preface"] = md_plain(title + "\n" + preface)
    return parts


def show_diff(a: str, b: str) -> list[str]:
    out = []
    sm = difflib.SequenceMatcher(None, a, b, autojunk=False)
    for op, i1, i2, j1, j2 in sm.get_opcodes():
        if op == "equal":
            continue
        ctx = a[max(0, i1 - 30) : i1]
        out.append(f"  {op} after …{ctx!r}: html {a[i1:i2]!r} / md {b[j1:j2]!r}")
    return out


def check_text() -> int:
    html, style = html_parts()
    md = md_parts()
    bad = 0
    for k in sorted(set(html) | set(md)):
        a, b = html.get(k), md.get(k)
        if a is None or b is None:
            print(f"part {k}: only in {'Markdown' if a is None else 'spec.html'}")
            bad += 1
            continue
        if a != b:
            lines = show_diff(a, b)
            print(f"part {k}: {len(lines)} differences")
            print("\n".join(lines[:40]))
            bad += len(lines)
    for k, f in chapter_files().items():
        for e in table_shape_errors(f.name, f.read_text(encoding="utf-8")):
            print(e)
            bad += 1
    if (SPEC_DIR / "samples.css").read_text(encoding="utf-8").strip() != style.strip():
        print("samples.css is not the style of spec.html")
        bad += 1
    print(f"text: {bad} differences in {len(md)} parts")
    return bad


# --- ja.ts -------------------------------------------------------------------

TOKEN = re.compile(
    r"""(?P<ws>\s+)|(?P<lc>//[^\n]*)|(?P<bc>/\*.*?\*/)"""
    r"""|(?P<str>"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`)"""
    r"""|(?P<id>[A-Za-z_$][\w$]*)|(?P<num>\d[\w.]*)|(?P<p>.)""",
    re.S,
)


def js_string(tok: str) -> str:
    named = {"n": "\n", "t": "\t", "r": "\r"}

    def unescape(m: re.Match) -> str:
        c = m.group(1)
        if c.startswith("u"):
            return chr(int(c[1:].strip("{}"), 16))
        return named.get(c, c)

    return re.sub(r"\\(u\{[0-9a-fA-F]+\}|u[0-9a-fA-F]{4}|.)", unescape, tok[1:-1], flags=re.S)


def tokens(src: str) -> list[tuple[str, str]]:
    out = []
    for m in TOKEN.finditer(src):
        kind = m.lastgroup
        if kind in ("ws", "lc", "bc"):
            continue
        out.append((kind, m.group()))
    return out


def ja_entries() -> list[tuple[str, str, str]]:
    """(key, chapter, text) for every spec: entry. Terms give two texts."""
    toks = tokens(JA_TS.read_text(encoding="utf-8"))
    out = []
    for i, (kind, val) in enumerate(toks):
        if kind != "id" or val not in ("e", "s6", "st") or toks[i + 1] != ("p", "("):
            continue
        if i > 0 and toks[i - 1][1] == "const":
            continue
        args, cur, depth, j = [], [], 0, i + 2
        while True:
            k, v = toks[j]
            if v in "([{" and k == "p":
                depth += 1
            elif v in ")]}" and k == "p":
                if depth == 0:
                    break
                depth -= 1
            if v == "," and k == "p" and depth == 0:
                args.append(cur)
                cur = []
            else:
                cur.append((k, v))
            j += 1
        if cur:
            args.append(cur)
        key = toks[i - 2][1].strip("\"'") if i >= 2 and toks[i - 1][1] == ":" else f"(line token {i})"

        def string(arg):
            if len(arg) != 1 or arg[0][0] != "str":
                raise SystemExit(f"ja.ts: {key}: an argument is not a plain string")
            return js_string(arg[0][1])

        if val == "e":
            source = string(args[1])
            if source.startswith("spec:"):
                out.append((key, source[5:], string(args[0])))
        elif val == "s6":
            out.append((key, "6", string(args[1])))
        else:
            term = string(args[0])
            out.append((f"term {term}", "5", term))
            out.append((f"term {term} (description)", "5", string(args[1])))
    return out


def check_strings() -> int:
    md = md_parts()
    entries = ja_entries()
    bad = []
    for key, chap, text in entries:
        chapter = md.get(chap)
        if chapter is None:
            bad.append((key, chap, text, "no such chapter"))
            continue
        pieces = [squeeze(p) for p in re.split(r"\{\w+\}", text)]
        missing = [p for p in pieces if p and p not in chapter]
        if missing:
            bad.append((key, chap, text, " / ".join(repr(p) for p in missing)))
    for key, chap, text, why in bad:
        print(f"{key} (spec:{chap}) {text!r}: not found {why}")
    print(f"strings: {len(bad)} of {len(entries)} not found")
    return len(bad)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--strings", action="store_true", help="check the spec: texts of ui/src/text/ja.ts instead")
    args = ap.parse_args()
    bad = check_strings() if args.strings else check_text()
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
