#!/usr/bin/env python3
"""Bug reports of a sweep: one file per finding under docs/bugs/reports/.

  python3 -I tools/bugs/report.py new --area CORE-PLAY --kind bug --severity high --title "..."
      makes docs/bugs/reports/<AREA>-<NNN>-<slug>.md from docs/bugs/TEMPLATE.md and prints its path
  python3 -I tools/bugs/report.py index
      checks every report against the template and writes docs/bugs/INDEX.md (exit 1 when one is wrong)
"""
import argparse
import datetime
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
BUGS = ROOT / "docs" / "bugs"
REPORTS = BUGS / "reports"
KINDS = ("bug", "ui-ux", "parity")
SEVERITIES = ("high", "medium", "low")
STATUSES = ("reported", "confirmed", "needs-lead", "fixed")
FIELDS = ("id", "title", "area", "kind", "severity", "status", "found")
SECTIONS = (
    "Where",
    "What happens",
    "What should happen",
    "How to trigger",
    "Cause",
    "Suggested fix",
    "Verification",
    "Fix",
)
# Sections a finder must fill; the last two are filled later.
FILLED = SECTIONS[:6]


def new(args):
    area = args.area.upper()
    if not re.fullmatch(r"[A-Z][A-Z0-9]*(-[A-Z0-9]+)*", area):
        sys.exit("area: capital letters, digits and '-' only (for example CORE-PLAY)")
    title = " ".join(args.title.split())
    if not title or "\n" in args.title:
        sys.exit("title: one line")
    REPORTS.mkdir(parents=True, exist_ok=True)
    taken = [
        int(m.group(1))
        for p in REPORTS.glob(f"{area}-*.md")
        if (m := re.fullmatch(rf"{re.escape(area)}-(\d{{3}})-.*", p.stem))
    ]
    number = max(taken, default=0) + 1
    ident = f"{area}-{number:03d}"
    slug = re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")[:48].strip("-") or "report"
    text = (BUGS / "TEMPLATE.md").read_text(encoding="utf-8")
    for key, value in (
        ("id", ident),
        ("title", title),
        ("area", area),
        ("kind", args.kind),
        ("severity", args.severity),
        ("date", datetime.date.today().isoformat()),
    ):
        text = text.replace("{{" + key + "}}", value)
    path = REPORTS / f"{ident}-{slug}.md"
    # 'x': never write over a report that is already there.
    with open(path, "x", encoding="utf-8") as out:
        out.write(text)
    print(path.relative_to(ROOT))


def read(path):
    """The front matter, the sections with their text (comments taken out), and what is wrong."""
    text = path.read_text(encoding="utf-8")
    wrong = []
    head = {}
    m = re.match(r"---\n(.*?)\n---\n", text, re.S)
    if not m:
        return head, {}, ["no front matter"]
    for line in m.group(1).splitlines():
        key, _, value = line.partition(":")
        head[key.strip()] = value.strip()
    for key in FIELDS:
        if not head.get(key):
            wrong.append(f"front matter: no {key}")
    if list(head) != list(FIELDS):
        wrong.append("front matter: fields differ from the template")
    if head.get("kind") not in KINDS:
        wrong.append(f"kind: one of {', '.join(KINDS)}")
    if head.get("severity") not in SEVERITIES:
        wrong.append(f"severity: one of {', '.join(SEVERITIES)}")
    if head.get("status") not in STATUSES:
        wrong.append(f"status: one of {', '.join(STATUSES)}")
    if head.get("id") and not path.stem.startswith(head["id"] + "-"):
        wrong.append("file name does not start with the id")
    body = re.sub(r"<!--.*?-->", "", text[m.end():], flags=re.S)
    parts = re.split(r"^## (.+)$", body, flags=re.M)
    names = [n.strip() for n in parts[1::2]]
    sections = dict(zip(names, (p.strip() for p in parts[2::2])))
    if names != list(SECTIONS):
        wrong.append("sections differ from the template")
    for name in FILLED:
        if name in sections and not sections[name]:
            wrong.append(f"empty section: {name}")
    return head, sections, wrong


def index(_args):
    rows = []
    failed = False
    for path in sorted(REPORTS.glob("*.md")):
        head, _sections, wrong = read(path)
        for w in wrong:
            failed = True
            print(f"{path.relative_to(ROOT)}: {w}", file=sys.stderr)
        rows.append((head, path))
    order = {s: i for i, s in enumerate(SEVERITIES)}
    rows.sort(key=lambda r: (order.get(r[0].get("severity"), 9), r[0].get("id", "")))
    lines = [
        "# Bug reports: index",
        "",
        "Written by `python3 -I tools/bugs/report.py index`. Do not edit by hand.",
        "",
        f"{len(rows)} reports.",
        "",
        "| Id | Severity | Kind | Status | Title |",
        "|---|---|---|---|---|",
    ]
    for head, path in rows:
        title = head.get("title", "").replace("|", "\\|")
        lines.append(
            f"| [{head.get('id', '?')}](reports/{path.name}) | {head.get('severity', '?')} "
            f"| {head.get('kind', '?')} | {head.get('status', '?')} | {title} |"
        )
    (BUGS / "INDEX.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{len(rows)} reports, index written")
    if failed:
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    n = sub.add_parser("new")
    n.add_argument("--area", required=True)
    n.add_argument("--kind", required=True, choices=KINDS)
    n.add_argument("--severity", required=True, choices=SEVERITIES)
    n.add_argument("--title", required=True)
    n.set_defaults(run=new)
    i = sub.add_parser("index")
    i.set_defaults(run=index)
    args = parser.parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
