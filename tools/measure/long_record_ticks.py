#!/usr/bin/env python3
"""Where the keys of a long recording landed, against when they were sent.

usage: python3 -I tools/measure/long_record_ticks.py DIR --before SONG.meloet

Reads DIR/saved.meloet (the song long-record.sh saved after recording; the take that is not in
SONG.meloet is the recording) and DIR/vkeys.json (`noteOns`: CLOCK_MONOTONIC µs and note of
every key sent). For each key: the map's seconds of its recorded tick minus the time it was
sent. That difference is one constant for the whole recording when nothing drifts, plus the
rounding to whole ticks (half a tick each way) and the ALSA sequencer's delivery.

The song follows the output's clock and the keys are sent by CLOCK_MONOTONIC, so the two may
run apart by some parts in a million (a straight line over the whole recording: "clock
slope"). A fault in the tempo map would not be a straight line: it would show as steps where
the tempo changes, so the keys are also grouped by the kind of place they fell in.
Prints Markdown.
"""
import argparse, json, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tempo_map import TempoMap  # noqa: E402

FRAME_MS = 1000 / 48000


def recorded(saved, before):
    old = {t["id"] for t in before["takes"]}
    new = [t for t in saved["takes"] if t["id"] not in old]
    if len(new) != 1:
        sys.exit(f"long_record_ticks.py: {len(new)} new takes in the saved song, not 1")
    take = new[0]
    for track in saved["song"]["tracks"]:
        for clip in track["clips"]:
            for held in clip["takes"]:
                if held["take"] == take["id"]:
                    base = clip["start"] + held["offset"]
                    return [(base + n[0], n[2]) for n in take["notes"]], clip
    sys.exit("long_record_ticks.py: no clip holds the new take")


def line(xs, ys):
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs)
    return slope, my - slope * mx


def kind(m, tick):
    p = max((p for p in m.pieces if p[0] <= tick), key=lambda p: p[0])
    den = 4 * 480 // int(p[2])
    if p[4] == 0:
        return f"x/{den}, {p[3]:.2f}"
    return f"x/{den}, ramp {'up' if p[4] > 0 else 'down'}"


def main():
    p = argparse.ArgumentParser()
    p.add_argument("dir")
    p.add_argument("--before", required=True)
    a = p.parse_args()
    with open(os.path.join(a.dir, "saved.meloet"), encoding="utf-8") as f:
        saved = json.load(f)
    with open(a.before, encoding="utf-8") as f:
        before = json.load(f)
    with open(os.path.join(a.dir, "vkeys.json")) as f:
        sent = json.load(f)["noteOns"]
    notes, clip = recorded(saved, before)
    m = TempoMap.of_song(saved)
    lines = ["# Recorded ticks against the times the keys were sent", ""]
    lines.append(f"- keys sent {len(sent)}, notes in the take {len(notes)}; the clip is {clip['start']}..{clip['end']} "
                 f"ticks; the map has {len(m.pieces)} stretches of tempo and meter")
    if len(sent) != len(notes):
        # Notes are kept in the order of their starts; two keys are never sent at one time here.
        lines.append("- the counts differ: nothing compared")
        print("\n".join(lines))
        return
    notes.sort()
    wrong = sum(1 for (_, n), (_, k) in zip(notes, sent) if n != k)
    t0 = sent[0][0] / 1e6
    xs = [us / 1e6 - t0 for us, _ in sent]
    ys = [(m.seconds_at(tick) - us / 1e6) * 1000 for (tick, _), (us, _) in zip(notes, sent)]
    slope, at0 = line(xs, ys)
    rest = [y - (at0 + slope * x) for x, y in zip(xs, ys)]
    half_tick = [(m.seconds_at(tick + 0.5) - m.seconds_at(tick - 0.5)) * 500 for tick, _ in notes]
    over = [r for r, h in zip(rest, half_tick) if abs(r) > h + 0.25]
    first = [y for x, y in zip(xs, ys) if x < 30]
    last = [y for x, y in zip(xs, ys) if x > xs[-1] - 30]
    drift = sum(last) / len(last) - sum(first) / len(first)
    lines += [
        f"- keys whose note is not the one sent at that place: {wrong}",
        f"- first key at tick {notes[0][0]}, last key at tick {notes[-1][0]}: {xs[-1]:.3f} s apart as sent, "
        f"{m.seconds_at(notes[-1][0]) - m.seconds_at(notes[0][0]):.3f} s apart on the map "
        f"(the last key against the first: {ys[-1] - ys[0]:+.3f} ms; each is rounded to a whole tick)",
        f"- mean of the last 30 s minus mean of the first 30 s: {drift:+.3f} ms ({drift / FRAME_MS:+.1f} frames at 48 kHz)",
        f"- clock slope (straight line through all keys): {slope * 1000:+.2f} parts in a million "
        f"({slope * xs[-1]:+.3f} ms over the recording)",
        f"- around that line: mean abs {sum(abs(r) for r in rest) / len(rest):.3f} ms, "
        f"95 % {sorted(abs(r) for r in rest)[int(len(rest) * 0.95) - 1]:.3f} ms, max {max(abs(r) for r in rest):.3f} ms; "
        f"keys more than half a tick + 0.25 ms from it: {len(over)}",
        "",
        "| where the key fell | keys | half a tick (ms) | mean around the line (ms) | max abs (ms) |",
        "|---|---|---|---|---|",
    ]
    groups = {}
    for (tick, _), r, h in zip(notes, rest, half_tick):
        groups.setdefault(kind(m, tick), []).append((r, h))
    for name, g in groups.items():
        rs = [r for r, _ in g]
        hs = [h for _, h in g]
        lines.append(f"| {name} | {len(g)} | {min(hs):.3f}..{max(hs):.3f} | {sum(rs) / len(rs):+.3f} | "
                     f"{max(abs(r) for r in rs):.3f} |")
    lines += ["", "| minute | keys | mean around the line (ms) | mean, no line taken off (ms, against the first minute) |",
              "|---|---|---|---|"]
    base = None
    for minute in range(int(xs[-1] // 60) + 1):
        g = [(r, y) for x, r, y in zip(xs, rest, ys) if minute * 60 <= x < minute * 60 + 60]
        if not g:
            continue
        mean_y = sum(y for _, y in g) / len(g)
        base = mean_y if base is None else base
        lines.append(f"| {minute + 1} | {len(g)} | {sum(r for r, _ in g) / len(g):+.3f} | {mean_y - base:+.3f} |")
    print("\n".join(lines))


main()
