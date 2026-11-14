#!/usr/bin/env python3
"""Render every mock board in docs/design/mock/ to PNG with headless Chrome.

usage: python3 -I tools/design/render_mocks.py OUT_DIR [BOARD ...]
Boards that use <sc-for> (A-Notes, Opt-Colors, Opt-Meter) render without their repeated rows.
"""
import json, os, subprocess, sys

root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
mock = os.path.join(root, "docs", "design", "mock")
out = os.path.abspath(sys.argv[1])
os.makedirs(out, exist_ok=True)
boards = json.load(open(os.path.join(mock, "canvas.json"), encoding="utf-8"))["boards"]
pick = set(sys.argv[2:])
for name, b in boards.items():
    stem = name.removesuffix(".dc.html")
    if pick and stem not in pick:
        continue
    png = os.path.join(out, stem + ".png")
    subprocess.run(["google-chrome", "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    "--virtual-time-budget=4000", f"--window-size={b['w']},{b['h']}",
                    f"--screenshot={png}", "file://" + os.path.join(mock, name)],
                   check=True, capture_output=True, timeout=60)
    print(png)
