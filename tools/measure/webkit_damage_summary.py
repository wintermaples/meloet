#!/usr/bin/env python3
"""Sums up webkit_damage.py's log: what the page reported as changed before each paint, and whether
WebKit painted those rectangles one by one or one rectangle around them all.

WebKitGTK 2.52, DrawingAreaCoordinatedGraphics.cpp, shouldPaintBoundsRect: the changed region is cut
into rectangles (rows of y, then spans of x); with more than 10 of them, or when they fill 25 % or
more of the rectangle around them, that one rectangle is painted instead.
"""
import collections
import sys


def area(rects):
    return sum(int(r.split()[2]) * int(r.split()[3]) for r in rects)


def main():
    path = sys.argv[1]
    shown = int(sys.argv[2]) if len(sys.argv) > 2 else 5
    kinds = collections.Counter()
    examples = collections.defaultdict(list)
    need, paint = [], []

    def close():
        if not paint:
            return
        changed = sorted(set(need))
        kind = "one rectangle around all" if len(paint) == 1 and len(changed) > 1 else f"{len(paint)} rectangles"
        kinds[kind] += 1
        examples[kind].append((changed, list(paint)))

    for line in open(path):
        f = line.split()
        if not f:
            continue
        if f[0] == "need":
            if paint:
                close()
                need, paint = [], []
            need.append(" ".join(f[1:]))
        elif f[0] == "rect":
            paint.append(" ".join(f[1:]))
        elif f[0] == "stopped:":
            print(line.strip())
    close()

    print("paints:", dict(kinds.most_common()))
    for kind, rows in examples.items():
        print(f"== {kind}")
        for changed, painted in rows[:shown]:
            print(f"   changed ({len(changed)}): " + " | ".join(changed))
            print(f"   painted ({area(painted)} px): " + " | ".join(painted))


if __name__ == "__main__":
    main()
