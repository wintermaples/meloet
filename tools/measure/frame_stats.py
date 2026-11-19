#!/usr/bin/env python3
"""The frames of one stretch of a run, from the page's frame profile (ui/src/gl/profile.ts).

usage: python3 -I tools/measure/frame_stats.py RAW_DIR FROM_S TO_S

RAW_DIR holds frame-profile.json (app-watch.sh, latency.sh and remote.py leave it there).
FROM_S..TO_S are seconds of the page's own clock. The profile keeps the first 4000 frames
(about 67 s). Prints one line of JSON: the frames in the stretch, the time between frames,
busyMs (from the start of the engine's frame to a message posted then), the parts of the
frame, and how many of the frames drew each region.
"""
import json, os, sys


def q(values, x):
    v = sorted(values)
    return round(v[min(len(v) - 1, int(x * len(v)))], 2) if v else None


def main(raw, a, b):
    with open(os.path.join(raw, "frame-profile.json")) as f:
        kept = json.load(f)["kept"]
    frames = [f for f in kept if a * 1000 <= f["atMs"] < b * 1000]
    gaps = [y["atMs"] - x["atMs"] for x, y in zip(frames, frames[1:])]
    busy = [f["busyMs"] for f in frames if f["busyMs"] is not None]
    drawn = {}
    for f in frames:
        for region in f["drawn"]:
            drawn[region] = drawn.get(region, 0) + 1
    out = {"frames": len(frames), "keptInAll": len(kept),
           "gapMs": {"p50": q(gaps, .5), "p95": q(gaps, .95), "p99": q(gaps, .99), "max": q(gaps, 1),
                     "over25": sum(g > 25 for g in gaps), "over41": sum(g > 41.7 for g in gaps)},
           "busyMs": {"mean": round(sum(busy) / len(busy), 2) if busy else None, "p50": q(busy, .5),
                      "p95": q(busy, .95), "p99": q(busy, .99), "max": q(busy, 1)},
           "partsMeanMs": {k: round(sum(f[k] for f in frames) / max(1, len(frames)), 3)
                           for k in ("callbacksMs", "measureMs", "paintMs", "glMs")},
           "framesThatDrew": drawn}
    print(json.dumps(out))


main(sys.argv[1], float(sys.argv[2]), float(sys.argv[3]))
