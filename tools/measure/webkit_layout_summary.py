#!/usr/bin/env python3
"""Sums up webkit_layout.py's log: how many rectangles each layout (or style update) gathered, and
whether WebKit handed them on one by one or as one rectangle around them all.

WebKitGTK 2.52, RenderView.cpp, accumulateRepaintRect: the rectangles asked for during one layout
are united in a Region; when its gridSize (Region.h) goes over 256, the Region becomes the one
rectangle around everything gathered so far, and when that covers 75 % of the view, the whole view.

usage: python3 -I tools/measure/webkit_layout_summary.py LOG [EXAMPLES]
"""
import collections
import math
import sys


def box(rects):
    """The rectangle around all of `rects` (x, y, w, h)."""
    x0 = min(r[0] for r in rects)
    y0 = min(r[1] for r in rects)
    x1 = max(r[0] + r[2] for r in rects)
    y1 = max(r[1] + r[3] for r in rects)
    return (x0, y0, x1 - x0, y1 - y0)


def enclosing(r):
    """The whole pixels a rectangle asked for touches, as it reaches the drawing area."""
    x0, y0 = math.floor(r[0]), math.floor(r[1])
    return (x0, y0, math.ceil(r[0] + r[2]) - x0, math.ceil(r[1] + r[3]) - y0)


def text(r):
    return " ".join(f"{v:g}" for v in r)


def main():
    path = sys.argv[1]
    shown = int(sys.argv[2]) if len(sys.argv) > 2 else 5
    kinds = collections.Counter()
    examples = collections.defaultdict(list)
    direct = collections.Counter()
    empty = 0
    # asked: gathered since the last flush. group: (gathered, handed on) of the flush being read.
    asked, group, in_flush = [], None, False

    def close():
        nonlocal empty
        if group is None:
            return
        gathered, handed = group
        if not gathered and not handed:
            empty += 1
            return
        distinct = sorted(set(gathered))
        if len(handed) == 1 and len(distinct) > 1:
            kind = "one rectangle around all"
        else:
            kind = f"{len(handed)} rectangles"
        kinds[kind] += 1
        examples[kind].append((gathered, distinct, handed))

    for line in open(path):
        f = line.split()
        if not f:
            continue
        if f[0] == "ask":
            in_flush = False
            asked.append(tuple(float(v) for v in f[1:5]))
        elif f[0] == "flush":
            close()
            group = (asked, [])
            asked, in_flush = [], True
        elif f[0] == "need":
            rect = tuple(int(v) for v in f[1:5])
            if in_flush and group is not None:
                group[1].append(rect)
            else:
                # Asked outside a layout (a canvas shown): it goes straight on.
                for i in range(len(asked) - 1, -1, -1):
                    if enclosing(asked[i]) == rect:
                        del asked[i]
                        break
                direct[rect] += 1
        elif f[0] == "stopped:":
            print(line.strip())
    close()

    print("layouts that asked for something:", dict(kinds.most_common()), f"(and {empty} that asked for nothing)")
    for kind, rows in examples.items():
        print(f"== {kind}")
        rows = sorted(rows, key=lambda row: -len(row[0]))
        for gathered, distinct, handed in rows[:shown]:
            area = sum(r[2] * r[3] for r in handed)
            around = text(box(distinct)) if distinct else "-"
            print(f"   asked {len(gathered)} ({len(distinct)} different, around them: {around})")
            print("      first: " + " | ".join(text(r) for r in gathered[:12]))
            print(f"      handed on ({area} px): " + " | ".join(text(r) for r in handed[:12]))
    print(f"asked outside a layout: {sum(direct.values())}")
    for rect, n in direct.most_common(shown * 2):
        print(f"   {n} x {text(rect)}")


if __name__ == "__main__":
    main()
