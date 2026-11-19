#!/usr/bin/env python3
"""Splits the keys of a latency.sh run with --cache-home into those played while plugin bundles
were being scanned and those played after.

usage: python3 -I tools/measure/scan_split.py RAW_DIR

RAW_DIR holds key-to-sound.json, key-to-screen.json and scan-watch.json (scan_watch.py). A key
is "during the scan" when its time lies between the first and the last sight of a scanning
child. Prints one markdown table row for each part: key -> DAC and Rust -> drawn (rAF), each
p50 / p95 / max in ms, and how many keys were lit within 2 frames (drawn + 16.7 ms <= 33.3 ms).
"""
import json, os, sys, time


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(p * (len(xs) - 1))))] if xs else float("nan")


def row(name, dac, drawn):
    def three(xs):
        return f"{pct(xs, 0.5):.2f} / {pct(xs, 0.95):.2f} / {max(xs):.2f}" if xs else "— / — / —"
    lit = sum(1 for d in drawn if d + 1000 / 60 <= 2000 / 60)
    share = f"{100 * lit / len(drawn):.0f} %" if drawn else "—"
    print(f"| {name} | {len(dac)} | {three(dac)} | {len(drawn)} | {three(drawn)} | {share} |")


def main():
    raw = sys.argv[1]
    load = lambda n: json.load(open(os.path.join(raw, n)))
    sound, screen, watch = load("key-to-sound.json"), load("key-to-screen.json"), load("scan-watch.json")
    if watch.get("firstSeenMonoUs") is not None:
        first, last = watch["firstSeenMonoUs"], watch["lastSeenMonoUs"]
    elif watch.get("firstSeenWallMs") is not None:
        shift = (time.time() - time.clock_gettime(time.CLOCK_MONOTONIC)) * 1e6
        first, last = watch["firstSeenWallMs"] * 1000 - shift, watch["lastSeenWallMs"] * 1000 - shift
    else:
        first = last = 0
    inside = lambda us: first <= us <= last
    offset = screen.get("offsetUs")
    dac = [(r["keyUs"], (r["playbackUs"] - r["keyUs"]) / 1000) for r in sound.get("records", [])]
    drawn = [(k["tUs"], k["drawMs"] - (k["tUs"] - offset) / 1000)
             for k in screen.get("keys", []) if k.get("drawMs") is not None and offset is not None]
    print(f"scan seen for {watch.get('seenSeconds')} s")
    print("| keys | n | key -> DAC p50 / p95 / max | n | Rust -> drawn p50 / p95 / max | lit within 2 frames |")
    print("|---|---|---|---|---|---|")
    row("during the scan", [v for t, v in dac if inside(t)], [v for t, v in drawn if inside(t)])
    row("after the scan", [v for t, v in dac if not inside(t)], [v for t, v in drawn if not inside(t)])
    row("all", [v for _, v in dac], [v for _, v in drawn])


main()
