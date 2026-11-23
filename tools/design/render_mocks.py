#!/usr/bin/env python3
"""Render every mock board in docs/design/mock/ to PNG with headless Chrome.

usage: python3 -I tools/design/render_mocks.py [--dir BOARD_DIR] OUT_DIR [BOARD ...]
--dir reads the boards and canvas.json from another folder (for example docs/design/proposals).
Boards that use <sc-for> (A-Notes, Opt-Colors, Opt-Meter) render without their repeated rows.
"""
import json, os, subprocess, sys

root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
args = sys.argv[1:]
mock = os.path.join(root, "docs", "design", "mock")
if args[:1] == ["--dir"]:
    mock = os.path.abspath(args[1])
    args = args[2:]
out = os.path.abspath(args[0])
os.makedirs(out, exist_ok=True)
boards = json.load(open(os.path.join(mock, "canvas.json"), encoding="utf-8"))["boards"]
pick = set(args[1:])
for name, b in boards.items():
    stem = name.removesuffix(".dc.html")
    if pick and stem not in pick:
        continue
    png = os.path.join(out, stem + ".png")
    # --password-store=basic: without it Chrome can wait 11 to 25 s for the desktop keyring at start.
    subprocess.run(["google-chrome", "--headless=new", "--disable-gpu", "--hide-scrollbars", "--password-store=basic",
                    "--virtual-time-budget=4000", f"--window-size={b['w']},{b['h']}",
                    f"--screenshot={png}", "file://" + os.path.join(mock, name)],
                   check=True, capture_output=True, timeout=60)
    print(png)
