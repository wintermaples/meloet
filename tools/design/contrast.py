#!/usr/bin/env python3
"""Check the contrast of every theme (WCAG 2 contrast ratio).

usage: python3 -I tools/design/contrast.py [--all] [--theme ID ...]

Reads the color sets of ui/src/design/themes.ts (the dark set is `colors` of
ui/src/design/tokens.ts) and the pairs in tools/design/contrast-pairs.json
(which color is drawn on which, and how much contrast that kind needs).
Prints the pairs that are under their minimum; with --all, every pair.
Exit code 1 if any pair of any checked theme is under its minimum.
Standard library only.
"""

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOKENS_TS = ROOT / "ui" / "src" / "design" / "tokens.ts"
THEMES_TS = ROOT / "ui" / "src" / "design" / "themes.ts"
PAIRS_JSON = Path(__file__).resolve().with_name("contrast-pairs.json")

ENTRY = re.compile(r'"color\.([a-z0-9.]+)"\s*:\s*"(#[0-9A-Fa-f]{6})"')


def object_after(src: str, name: str) -> str:
    """The text of the object literal assigned to `const <name>`."""
    m = re.search(r"\bconst\s+" + re.escape(name) + r"\b[^=]*=\s*\{", src)
    if not m:
        raise SystemExit(f"contrast.py: no `const {name} = {{` found")
    depth, i = 1, m.end()
    while depth:
        depth += {"{": 1, "}": -1}.get(src[i], 0)
        i += 1
    return src[m.end() : i - 1]


def palette(src: str, name: str) -> dict[str, str]:
    return {k: v.upper() for k, v in ENTRY.findall(object_after(src, name))}


def load_themes() -> dict[str, dict[str, str]]:
    tokens = TOKENS_TS.read_text(encoding="utf-8")
    themes = THEMES_TS.read_text(encoding="utf-8")
    ids = re.findall(r'"([A-Za-z]+)"', re.search(r"THEME_IDS\s*=\s*\[([^\]]*)\]", themes).group(1))
    table = object_after(themes, "palettes")
    out = {}
    for tid in ids:
        m = re.search(r"\b" + tid + r"\b\s*(?::\s*([A-Za-z_]\w*))?\s*,", table + ",")
        if not m:
            raise SystemExit(f"contrast.py: theme {tid} is not in palettes")
        ref = m.group(1) or tid
        out[tid] = palette(tokens, ref) if ref == "colors" else palette(themes, ref)
    names = set(palette(tokens, "colors"))
    for tid, p in out.items():
        if set(p) != names:
            raise SystemExit(f"contrast.py: theme {tid} names differ from tokens.ts: {sorted(set(p) ^ names)}")
    return out


def luminance(hex_color: str) -> float:
    def channel(c: int) -> float:
        s = c / 255
        return s / 12.92 if s <= 0.04045 else ((s + 0.055) / 1.055) ** 2.4

    r, g, b = (int(hex_color[i : i + 2], 16) for i in (1, 3, 5))
    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)


def ratio(a: str, b: str) -> float:
    la, lb = sorted((luminance(a), luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def expand(pairs: list[dict]) -> list[tuple[str, str, str, str, dict]]:
    out, seen = [], set()
    for p in pairs:
        for bg in p["bg"]:
            key = (p["fg"], bg, p["kind"])
            if key not in seen:
                seen.add(key)
                out.append((p["fg"], bg, p["kind"], p["where"], p.get("except") or {}))
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--all", action="store_true", help="print every pair, not only the failing ones")
    ap.add_argument("--theme", action="append", help="check only this theme (may be given more than once)")
    args = ap.parse_args()

    spec = json.loads(PAIRS_JSON.read_text(encoding="utf-8"))
    kinds = spec["kinds"]
    pairs = expand(spec["pairs"])
    themes = load_themes()
    for fg, bg, kind, _, exc in pairs:
        if kind not in kinds:
            raise SystemExit(f"contrast.py: unknown kind {kind}")
        if exc and (not exc.get("why") or not set(exc.get("themes", [])) <= set(themes)):
            raise SystemExit(f"contrast.py: an exception needs known themes and a why: {fg} on {bg}")
    for tid in args.theme or []:
        if tid not in themes:
            raise SystemExit(f"contrast.py: unknown theme {tid}")
    checked = [t for t in themes if not args.theme or t in args.theme]

    bad_total = 0
    for tid in checked:
        p = themes[tid]
        bad = 0
        owned = 0
        lines = []
        for fg, bg, kind, where, exc in pairs:
            r = ratio(p[fg], p[bg])
            need = kinds[kind]["min"]
            under = need is not None and r < need
            # The owner's exceptions are always printed when they are under, so they stay in sight.
            excepted = under and tid in exc.get("themes", [])
            fail = under and not excepted
            bad += fail
            owned += excepted
            if under or args.all:
                mark = "FAIL" if fail else "OWN " if excepted else ("    " if need is not None else "note")
                need_s = f"{need:>4}" if need is not None else "   -"
                why = f"  [owner: {exc['why']}]" if excepted else ""
                lines.append(f"  {mark} {r:5.2f} (>= {need_s} {kind:8}) {fg:>9} {p[fg]} on {bg:<9} {p[bg]}  {where}{why}")
        print(f"{tid}: {len(pairs)} pairs, {bad} under the minimum" + (f", {owned} under it by the owner's decision" if owned else ""))
        print("\n".join(lines)) if lines else None
        bad_total += bad
    return 1 if bad_total else 0


if __name__ == "__main__":
    sys.exit(main())
