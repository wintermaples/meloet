#!/usr/bin/env python3
"""Turns results/raw/*.json into results/summary.md.

Pass rule from the brief: at 1920x1080, 95 % of frames fit in one display refresh.
Timer resolution in WebKitGTK is 1 ms and rAF is vsync-quantized, so a frame "fits" when its
interval is <= 1.25 x the period; a frame that slips a whole vsync lands far above that.
"""
import json, pathlib, sys

root = pathlib.Path(__file__).parent / "results"
rows, stress = [], []
for f in sorted((root / "raw").glob("*.json")):
    d = json.loads(f.read_text())
    if f.name.startswith("stress__"):
        stress.append((f.stem.split("__", 1)[1], d))
        continue
    env, size, renderer, m = f.stem.split("__")
    rows.append((env, size, renderer, m == "m1", d))

def cell(d, k, s):
    return d.get(k, {}).get(s, "—") if isinstance(d.get(k), dict) else "—"

out = ["# s1-render summary", "", f"display period {rows[0][4].get('displayMs', '?') if rows else '?'} ms (144 Hz). 60 Hz cadence = 16.667 ms.", "",
       "| env | size | renderer | meters | rAF p50 | interval mean / p95 / max (ms) | work mean / p95 / max (ms) | ≤1 refresh @144Hz | p95 ≤ 1 frame @60Hz | note |",
       "|---|---|---|---|---|---|---|---|---|---|"]
for env, size, r, m, d in rows:
    if "intervals" not in d:
        note = d.get("error") or f"no result (exit {d.get('exit')})"
        out.append(f"| {env} | {size} | {r} | {'on' if m else 'off'} | — | — | — | — | — | {note} |")
        continue
    p95 = d["intervals"]["p95"]
    out.append(
        f"| {env} | {size} | {r} | {'on' if m else 'off'} | {d['rafMedianMs']} | "
        f"{cell(d,'intervals','mean')} / {p95} / {cell(d,'intervals','max')} | "
        f"{cell(d,'work','mean')} / {cell(d,'work','p95')} / {cell(d,'work','max')} | "
        f"{d['within1'] * 100:.0f} % | {'yes' if p95 <= 16.667 * 1.25 else 'NO'} | {d['host']['backend']} |")
out += ["", "## Resize stress (8 s of set_size every 120 ms, GL + meters, 1920x1080)", "", "| env | survived | note |", "|---|---|---|"]
for env, d in stress:
    ok = "intervals" in d
    out.append(f"| {env} | {'yes' if ok else 'NO'} | {'' if ok else (d.get('error') or 'exit ' + str(d.get('exit')))} |")
(root / "summary.md").write_text("\n".join(out) + "\n")
print("\n".join(out))
