#!/usr/bin/env python3
"""Check the spec in docs/spec/ (Markdown, one file per chapter).

Two checks; both run when no option is given.

--tables: every table row in a chapter has as many cells as its header.

--strings: for every entry in ui/src/text/ja.ts whose source is spec:<n>
(and the terms of chapter 5), check that its text is in chapter <n>. A text
with holes ({name}) is cut at the holes; each piece must be in the chapter.
Tags and Markdown marks are taken out and spaces are joined first.

Exit code 1 if anything is wrong. Standard library only; run with python3 -I.
"""

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC_DIR = ROOT / "docs" / "spec"
JA_TS = ROOT / "ui" / "src" / "text" / "ja.ts"


def squeeze(s: str) -> str:
    return re.sub(r"\s+", " ", s).strip()


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
    return {k: md_plain(f.read_text(encoding="utf-8")) for k, f in chapter_files().items()}


def check_tables() -> int:
    bad = 0
    for f in chapter_files().values():
        for e in table_shape_errors(f.name, f.read_text(encoding="utf-8")):
            print(e)
            bad += 1
    print(f"tables: {bad} rows with a wrong number of cells")
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
    ap.add_argument("--tables", action="store_true", help="check the cells of the tables only")
    ap.add_argument("--strings", action="store_true", help="check the spec: texts of ui/src/text/ja.ts only")
    args = ap.parse_args()
    both = not (args.tables or args.strings)
    bad = 0
    if args.tables or both:
        bad += check_tables()
    if args.strings or both:
        bad += check_strings()
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
