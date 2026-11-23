#!/usr/bin/env python3
"""Sums up webkit_damage.py's log, paint by paint (a paint: one display of the page that painted
something): how many paints and how many a second, how many were whole-window paints, and how many
rectangles each of the others painted.

WebKitGTK 2.52, DrawingAreaCoordinatedGraphics.cpp, shouldPaintBoundsRect: the changed region is cut
into rectangles (rows of y, then spans of x); with more than 10 of them, or when they fill 25 % or
more of the rectangle around them, that one rectangle is painted instead. A whole-window paint here
is a paint of one rectangle of at least WHOLE_PX pixels (default 500 000, about half of 1280 x 800
and a quarter of 1920 x 1080).

With a log of `--per-rect` (the rectangles the page reported as changed are in it, "need"), it also
shows the paints by kind with examples of what was changed and what was painted.

usage: webkit_damage_summary.py LOG [SHOWN] [WHOLE_PX]
"""
import collections
import sys


def area(rects):
    return sum(r[2] * r[3] for r in rects)


def text(r):
    return " ".join(map(str, r))


def paints(path):
    """(ms of the display or None, changed rectangles before it, painted rectangles) for each paint, and
    the lines gdb wrote when it stopped."""
    out, stopped = [], []
    need, paint, at = [], [], None

    def close():
        if paint:
            out.append((at, sorted(set(need)), list(paint)))

    for line in open(path):
        f = line.split()
        if not f:
            continue
        if f[0] == "display":
            close()
            need, paint = ([], []) if paint else (need, [])
            at = float(f[1]) if len(f) > 1 else None
        elif f[0] == "need":
            if paint:
                close()
                need, paint = [], []
            need.append(tuple(int(x) for x in f[1:5]))
        elif f[0] == "rect":
            paint.append(tuple(int(x) for x in f[1:5]))
        elif f[0] == "stopped:":
            stopped.append(line.strip())
    close()
    return out, stopped


def main():
    path = sys.argv[1]
    shown = int(sys.argv[2]) if len(sys.argv) > 2 else 5
    whole_px = int(sys.argv[3]) if len(sys.argv) > 3 else 500_000
    found, stopped = paints(path)
    for line in stopped:
        print(line)

    whole = [p for p in found if len(p[2]) == 1 and area(p[2]) >= whole_px]
    rest = [p for p in found if not (len(p[2]) == 1 and area(p[2]) >= whole_px)]
    per_paint = collections.Counter(len(p[2]) for p in rest)
    times = [p[0] for p in found if p[0] is not None]
    rate = f", {(len(times) - 1) * 1000 / (times[-1] - times[0]):.1f} a second" if len(times) > 1 and times[-1] > times[0] else ""
    print(f"paints: {len(found)}{rate}")
    print(f"whole-window paints (one rectangle of {whole_px} px or more): {len(whole)}")
    print("rectangles per paint (the others):", dict(sorted(per_paint.items())))
    print("largest count of a paint that was not whole-window:", max(per_paint, default=0))
    for _, _, painted in whole[:shown]:
        print(f"   whole: {text(painted[0])}")

    if not any(changed for _, changed, _ in found):
        return
    kinds = collections.Counter()
    examples = collections.defaultdict(list)
    for _, changed, painted in found:
        kind = "one rectangle around all" if len(painted) == 1 and len(changed) > 1 else f"{len(painted)} rectangles"
        kinds[kind] += 1
        examples[kind].append((changed, painted))
    print("paints by kind (per rect):", dict(kinds.most_common()))
    for kind, rows in examples.items():
        print(f"== {kind}")
        for changed, painted in rows[:shown]:
            print(f"   changed ({len(changed)}): " + " | ".join(map(text, changed)))
            print(f"   painted ({area(painted)} px): " + " | ".join(map(text, painted)))


if __name__ == "__main__":
    main()
