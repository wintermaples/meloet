#!/usr/bin/env python3
"""One HTML page of everything under docs/bugs/, for reading in a browser.

  python3 -I tools/bugs/html.py [--out docs/bugs/report.html]
      reads docs/bugs/reports/*.md and docs/bugs/later/*.md and writes one page with no outside files
  python3 -I tools/bugs/html.py --lang ja
      the same from the Japanese translations under docs/bugs/ja/, to docs/bugs/report.ja.html

The page is made from the files as they are; nothing is added or judged here.
"""
import argparse
import html
import os
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import report  # noqa: E402

ROOT = report.ROOT
BUGS = report.BUGS

# The page's own words. Report text comes from the files; these are only the labels around it.
WORDS = {
    "en": {},
    "ja": {
        "meloet bug reports": "meloet バグ報告",
        "Made by": "作成:",
        "from": "元:",
        "Do not edit by hand.": "手で書き換えないでください。",
        "reports": "報告",
        "to look at later": "後で見たい点",
        "About these reports (README.md)": "この報告について (README.md)",
        "Reports": "報告",
        "Search in every report (words, file names, ids)": "すべての報告から探す(言葉、ファイル名、ID)",
        "severity": "重さ",
        "kind": "種類",
        "area": "領域",
        "all": "すべて",
        "Open / close all": "全部開く / 閉じる",
        "To look at later (not bugs, not fixed)": "後で見たい点(バグではない、直していない)",
        "found": "見つけた日",
        "Where": "場所",
        "What happens": "起きること",
        "What should happen": "あるべき動き",
        "How to trigger": "起こし方",
        "Cause": "原因",
        "Suggested fix": "直し方の案",
        "Verification": "検証",
        "Fix": "直した内容",
        "high": "高",
        "medium": "中",
        "low": "低",
        "bug": "バグ",
        "ui-ux": "UI/UX",
        "parity": "片方にだけある",
        "reported": "報告済み",
        "confirmed": "確認済み",
        "needs-lead": "要判断",
        "fixed": "修正済み",
    },
}
LANG = "en"


def w(text):
    return html.escape(WORDS[LANG].get(text, text))


def inline(text):
    """Links, code spans and bold of one piece of text."""
    out = []
    for i, piece in enumerate(re.split(r"(\[(?:`[^`\n]+`|[^\]`\n])+\]\([^)\s]+\))", text)):
        if i % 2:
            label, href = re.fullmatch(r"\[(.+)\]\(([^)\s]+)\)", piece).groups()
            out.append(f'<a href="{html.escape(href)}">{spans(label)}</a>')
        else:
            out.append(spans(piece))
    return "".join(out)


def spans(text):
    out = []
    for i, piece in enumerate(re.split(r"(`[^`\n]+`)", text)):
        if i % 2:
            out.append(f"<code>{html.escape(piece[1:-1])}</code>")
        else:
            out.append(re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", html.escape(piece)))
    return "".join(out)


def cells(line):
    return [c.strip() for c in re.split(r"(?<!\\)\|", line.strip().strip("|"))]


def blocks(text, keep_lines=False):
    """The small part of Markdown the reports use: paragraphs, lists, fences, tables."""
    out = []
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
        elif line.startswith("```"):
            i += 1
            code = []
            while i < len(lines) and not lines[i].startswith("```"):
                code.append(lines[i])
                i += 1
            i += 1
            out.append(f"<pre>{html.escape(chr(10).join(code))}</pre>")
        elif line.startswith("|") and i + 1 < len(lines) and re.match(r"\|[\s:|-]+\|?\s*$", lines[i + 1]):
            head = cells(line)
            i += 2
            rows = []
            while i < len(lines) and lines[i].startswith("|"):
                rows.append(cells(lines[i]))
                i += 1
            out.append(
                "<table><thead><tr>"
                + "".join(f"<th>{inline(c)}</th>" for c in head)
                + "</tr></thead><tbody>"
                + "".join("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>" for r in rows)
                + "</tbody></table>"
            )
        elif m := re.match(r"(\d+\.|[-*]) ", line):
            numbered = m.group(1)[0].isdigit()
            mark = r"\d+\. " if numbered else r"[-*] "
            items = []
            while i < len(lines) and lines[i].strip():
                if re.match(mark, lines[i]):
                    items.append(re.sub("^" + mark, "", lines[i]))
                elif lines[i].startswith(" "):
                    items[-1] += " " + lines[i].strip()
                else:
                    break
                i += 1
            tag = "ol" if numbered else "ul"
            out.append(f"<{tag}>" + "".join(f"<li>{inline(t)}</li>" for t in items) + f"</{tag}>")
        else:
            para = []
            while (
                i < len(lines)
                and lines[i].strip()
                and not re.match(r"(\d+\.|[-*]) |```|\|", lines[i])
            ):
                para.append(lines[i])
                i += 1
            if not para:
                para.append(lines[i])
                i += 1
            out.append("<p>" + ("<br>" if keep_lines else " ").join(inline(p) for p in para) + "</p>")
    return "\n".join(out)


def card(head, sections, path):
    ident = html.escape(head.get("id", "?"))
    sev, kind, status = (html.escape(head.get(k, "?")) for k in ("severity", "kind", "status"))
    parts = []
    for name in report.SECTIONS:
        body = sections.get(name, "")
        if body:
            parts.append(f"<h4>{w(name)}</h4>\n{blocks(body, keep_lines=name == 'Where')}")
    return (
        f'<details class="card" id="{ident}" data-sev="{sev}" data-kind="{kind}" '
        f'data-area="{html.escape(head.get("area", "?"))}">\n'
        f'<summary><span class="id">{ident}</span><span class="tag sev-{sev}">{w(sev)}</span>'
        f'<span class="tag">{w(kind)}</span><span class="tag">{w(status)}</span>'
        f'<span class="title">{inline(head.get("title", ""))}</span></summary>\n'
        f'<div class="body">\n<p class="src">{html.escape(str(path.relative_to(ROOT)))}'
        f' · {w("found")} {html.escape(head.get("found", "?"))}</p>\n' + "\n".join(parts) + "\n</div>\n</details>"
    )


def later(path):
    text = path.read_text(encoding="utf-8")
    title = path.stem
    if m := re.match(r"# (.+)\n", text):
        title, text = m.group(1), text[m.end():]
    count = len(re.findall(r"^[-*] ", text, flags=re.M))
    return count, (
        f'<details class="card later">\n<summary><span class="id">{html.escape(path.stem)}</span>'
        f'<span class="tag">{count}</span><span class="title">{inline(title)}</span></summary>\n'
        f'<div class="body">\n{blocks(text)}\n</div>\n</details>'
    )


STYLE = """
:root { --bg:#fbfbfa; --fg:#1d1f21; --dim:#6b7075; --line:#dcdfe2; --card:#fff; --code:#f0f1f2;
  --high:#b3261e; --medium:#9a6700; --low:#3d6b8c; --link:#0b57a4; }
@media (prefers-color-scheme: dark) { :root { --bg:#16181a; --fg:#e4e6e8; --dim:#9aa0a6; --line:#33373b;
  --card:#1d2023; --code:#272b2f; --high:#ff8a80; --medium:#e3b341; --low:#8cbfe0; --link:#8ab4f8; } }
* { box-sizing: border-box; }
body { margin:0; background:var(--bg); color:var(--fg);
  font:15px/1.6 system-ui, -apple-system, "Segoe UI", "Noto Sans", sans-serif; }
main { max-width:1000px; margin:0 auto; padding:16px; }
h1 { font-size:24px; margin:16px 0 4px; } h2 { font-size:18px; margin:32px 0 8px; }
h4 { font-size:13px; margin:16px 0 4px; color:var(--dim); text-transform:uppercase; letter-spacing:.04em; }
p { margin:6px 0; } ul, ol { margin:6px 0; padding-left:22px; } li { margin:3px 0; }
a { color:var(--link); }
code, pre { font:13px/1.5 ui-monospace, "IBM Plex Mono", Menlo, Consolas, monospace; background:var(--code);
  border-radius:3px; }
code { padding:1px 4px; overflow-wrap:anywhere; } pre { padding:10px; overflow-x:auto; }
table { border-collapse:collapse; display:block; overflow-x:auto; margin:8px 0; }
th, td { border:1px solid var(--line); padding:4px 8px; text-align:left; vertical-align:top; }
.dim, .src { color:var(--dim); font-size:13px; }
.bar { position:sticky; top:0; z-index:1; background:var(--bg); border-bottom:1px solid var(--line);
  padding:8px 0; display:flex; flex-wrap:wrap; gap:8px; align-items:center; }
.bar input, .bar select, .bar button { font:inherit; color:inherit; background:var(--card);
  border:1px solid var(--line); border-radius:4px; padding:4px 8px; }
.bar input { flex:1 1 200px; min-width:0; }
.card { scroll-margin-top:60px; background:var(--card); border:1px solid var(--line); border-radius:6px; margin:6px 0; }
.card > summary { cursor:pointer; padding:8px 12px; display:flex; flex-wrap:wrap; gap:6px; align-items:baseline; }
.card[open] > summary { border-bottom:1px solid var(--line); }
.id { font-weight:600; font-family:ui-monospace, Menlo, Consolas, monospace; font-size:13px; }
.title { flex:1 1 100%; }
.tag { font-size:12px; border:1px solid var(--line); border-radius:10px; padding:0 8px; color:var(--dim); }
.sev-high { color:var(--high); border-color:var(--high); } .sev-medium { color:var(--medium);
  border-color:var(--medium); } .sev-low { color:var(--low); border-color:var(--low); }
.body { padding:4px 12px 12px; overflow-wrap:break-word; }
.counts { display:flex; flex-wrap:wrap; gap:16px; margin:8px 0; }
.counts b { font-size:20px; display:block; }
[hidden] { display:none !important; }
"""

SCRIPT = """
const cards = [...document.querySelectorAll('.card:not(.later)')];
const q = document.getElementById('q'), sev = document.getElementById('sev'),
  kind = document.getElementById('kind'), area = document.getElementById('area'),
  shown = document.getElementById('shown');
function apply() {
  const words = q.value.toLowerCase().split(/\\s+/).filter(Boolean);
  let n = 0;
  for (const c of cards) {
    const ok = (!sev.value || c.dataset.sev === sev.value) && (!kind.value || c.dataset.kind === kind.value)
      && (!area.value || c.dataset.area === area.value)
      && words.every(w => c.textContent.toLowerCase().includes(w));
    c.hidden = !ok; if (ok) n++;
  }
  shown.textContent = n + ' / ' + cards.length;
}
for (const e of [q, sev, kind, area]) e.addEventListener('input', apply);
document.getElementById('open').addEventListener('click', () => {
  const all = cards.filter(c => !c.hidden), to = !all.every(c => c.open);
  for (const c of all) c.open = to;
});
function jump() {
  const c = document.getElementById(decodeURIComponent(location.hash.slice(1)));
  if (c && c.tagName === 'DETAILS') { c.hidden = false; c.open = true; c.scrollIntoView(); }
}
addEventListener('hashchange', jump); apply(); jump();
"""


def options(values):
    return f'<option value="">{w("all")}</option>' + "".join(
        f'<option value="{html.escape(v)}">{w(v)}</option>' for v in values
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--lang", choices=sorted(WORDS), default="en")
    parser.add_argument("--out")
    args = parser.parse_args()
    global LANG
    LANG = args.lang
    source = BUGS if LANG == "en" else BUGS / LANG
    out = pathlib.Path(args.out or BUGS / ("report.html" if LANG == "en" else f"report.{LANG}.html"))

    rows = []
    failed = False
    for path in sorted((source / "reports").glob("*.md")):
        head, sections, wrong = report.read(path)
        for fault in wrong:
            failed = True
            print(f"{path.relative_to(ROOT)}: {fault}", file=sys.stderr)
        rows.append((head, sections, path))
    order = {s: i for i, s in enumerate(report.SEVERITIES)}
    rows.sort(key=lambda r: (order.get(r[0].get("severity"), 9), r[0].get("id", "")))

    def count(key, values):
        return "".join(
            f"<div><b>{sum(1 for h, _, _ in rows if h.get(key) == v)}</b>{w(v)}</div>" for v in values
        )

    laters = [later(p) for p in sorted((source / "later").glob("*.md"))]
    areas = sorted({h.get("area", "?") for h, _, _ in rows})
    intro = ""
    readme = source / "README.md"
    if readme.exists():
        text = re.sub(r"^# .+\n", "", readme.read_text(encoding="utf-8"))
        text = re.sub(r"^## (.+)$", r"**\1**", text, flags=re.M)
        # Links are written from the README's own folder; the page may sit elsewhere.
        text = re.sub(
            r"\]\((?![a-z]+:|#)([^)\s]+)\)",
            lambda m: f"]({os.path.relpath(source / m.group(1), out.parent)}{'/' if m.group(1).endswith('/') else ''})",
            text,
        )
        intro = f'<details class="card later"><summary><span class="title">{w("About these reports (README.md)")}'
        intro += f'</span></summary><div class="body">{blocks(text)}</div></details>'

    page = f"""<!doctype html>
<html lang="{LANG}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{w("meloet bug reports")}</title>
<style>{STYLE}</style>
</head>
<body>
<main>
<h1>{w("meloet bug reports")}</h1>
<p class="dim">{w("Made by")} <code>python3 -I tools/bugs/html.py{"" if LANG == "en" else " --lang " + LANG}</code>
{w("from")} <code>{html.escape(str(source.relative_to(ROOT)))}/</code>. {w("Do not edit by hand.")}</p>
<div class="counts"><div><b>{len(rows)}</b>{w("reports")}</div>{count("severity", report.SEVERITIES)}
{count("kind", report.KINDS)}{count("status", [s for s in report.STATUSES if any(h.get("status") == s for h, _, _ in rows)])}
<div><b>{sum(n for n, _ in laters)}</b>{w("to look at later")}</div></div>
{intro}
<h2>{w("Reports")}</h2>
<div class="bar">
<input id="q" type="search" placeholder="{w("Search in every report (words, file names, ids)")}">
<label>{w("severity")} <select id="sev">{options(report.SEVERITIES)}</select></label>
<label>{w("kind")} <select id="kind">{options(report.KINDS)}</select></label>
<label>{w("area")} <select id="area">{options(areas)}</select></label>
<button id="open" type="button">{w("Open / close all")}</button>
<span class="dim" id="shown"></span>
</div>
{chr(10).join(card(*r) for r in rows)}
<h2>{w("To look at later (not bugs, not fixed)")}</h2>
{chr(10).join(text for _, text in laters)}
</main>
<script>{SCRIPT}</script>
</body>
</html>
"""
    out.write_text(page, encoding="utf-8")
    print(f"{len(rows)} reports, {sum(n for n, _ in laters)} later lines, {out} ({len(page) // 1024} KB)")
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
