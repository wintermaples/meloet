#!/usr/bin/env python3
"""Frame work, main-thread time and RSS of one ui-script.sh run.

usage: python3 -I tools/measure/ui_script_summary.py RAW_DIR [--label TEXT]

RAW_DIR holds ui-script.json (ui/src/dev/script.ts), frame-profile.json (ui/src/gl/profile.ts)
and rss.json (rss_watch.py). For each part of the script, over the frames of the engine that
began while the part ran:
- work: the engine's own time in the frame (callbacks + measuring boxes + building rectangles + WebGL),
- busy: from the start of the engine's frame until a message posted then ran (the rest of the
  frame: Svelte, style, layout, paint),
- input: from dispatching an event until the microtasks after it ran (the handler and Svelte),
- frame gaps: time between animation frames (a gap well over 16.7 ms is a frame that was late),
- reply (drag): from letting go until the new take is shown,
and the RSS at every 100 steps. Writes summary.json next to RAW_DIR and prints markdown.
"""
import argparse, json, os, sys


def quantile(xs, p):
    return xs[min(len(xs) - 1, int(p * len(xs)))]


def stats(xs):
    xs = sorted(x for x in xs if x is not None)
    if not xs:
        return None
    return {"count": len(xs), "p50": quantile(xs, 0.5), "p95": quantile(xs, 0.95), "max": xs[-1]}


def rss_at(samples, wall_ms):
    """The last sample at or before wall_ms (or the first one)."""
    best = None
    for s in samples:
        if s["wallMs"] <= wall_ms:
            best = s
    return best or (samples[0] if samples else None)


def part_summary(part, frames, samples):
    inside = [f for f in frames if part["startMs"] <= f["atMs"] <= part["endMs"]]
    drawn = {}
    for f in inside:
        for name, rects in f["drawn"].items():
            d = drawn.setdefault(name, {"frames": 0, "rectsMax": 0})
            d["frames"] += 1
            d["rectsMax"] = max(d["rectsMax"], rects)
    gaps = part.get("frameGapsMs", [])
    steps = part["steps"]
    return {
        "kind": part["kind"],
        "count": part["count"],
        "changed": sum(1 for s in steps if s["changed"]),
        "seconds": (part["endMs"] - part["startMs"]) / 1000,
        "frames": len(inside),
        "workMs": stats([f["callbacksMs"] + f["measureMs"] + f["paintMs"] + f["glMs"] for f in inside]),
        "paintMs": stats([f["paintMs"] for f in inside]),
        "glMs": stats([f["glMs"] for f in inside]),
        "busyMs": stats([f["busyMs"] for f in inside]),
        "inputMs": stats([s["inputMs"] for s in steps]),
        "replyMs": stats([s.get("replyMs") for s in steps]) if part["kind"] == "drag" else None,
        "frameGapMs": stats(gaps),
        "gapsOver33": sum(1 for g in gaps if g > 33.4),
        "drawn": drawn,
        "rss": [dict(done=m["done"], **{k: v for k, v in (rss_at(samples, m["wallMs"]) or {}).items() if k != "wallMs"})
                for m in part["marks"]],
    }


def num(x):
    return "—" if x is None else f"{x:.2f}"


def trio(s):
    return "—" if s is None else f"{num(s['p50'])} / {num(s['p95'])} / {num(s['max'])}"


def markdown(label, script, parts, samples, frames_total):
    lines = [f"# ui script: {script.get('script')}", "", f"- {label}" if label else "",
             f"- done: {script['done']}, error: {script['error']}",
             f"- engine frames in the whole run: {frames_total}"]
    if samples:
        first, last = samples[0], samples[-1]
        lines.append(f"- RSS meloet {first['meloetMb']} → {last['meloetMb']} MB, WebKitWebProcess "
                     f"{first['webMb']} → {last['webMb']} MB (first and last sample)")
    lines += ["", "ms, p50 / p95 / max.", "",
              "| part | steps (moved) | s | frames | work | busy | input | frame gap | gaps > 33 ms | reply |",
              "|---|---|---|---|---|---|---|---|---|---|"]
    for p in parts:
        lines.append(f"| {p['kind']} | {p['count']} ({p['changed']}) | {p['seconds']:.1f} | {p['frames']} | "
                     f"{trio(p['workMs'])} | {trio(p['busyMs'])} | {trio(p['inputMs'])} | {trio(p['frameGapMs'])} | "
                     f"{p['gapsOver33']} | {trio(p['replyMs'])} |")
    for p in parts:
        lines += ["", f"{p['kind']}: paint {trio(p['paintMs'])}, gl {trio(p['glMs'])}; regions drawn (frames, most rectangles): "
                  + ", ".join(f"{k} {v['frames']} ({v['rectsMax']})" for k, v in sorted(p["drawn"].items()))]
        if p["rss"]:
            lines += ["", "| steps | meloet MB | WebKitWebProcess MB |", "|---|---|---|"]
            lines += [f"| {r['done']} | {r.get('meloetMb')} | {r.get('webMb')} |" for r in p["rss"]]
    return "\n".join(l for l in lines if l is not None) + "\n"


def read(path, default=None):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        if default is None:
            sys.exit(f"cannot read {path}")
        return default


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("raw")
    ap.add_argument("--label", default="")
    a = ap.parse_args(argv)
    script = read(os.path.join(a.raw, "ui-script.json"))
    profile = read(os.path.join(a.raw, "frame-profile.json"), {"frames": 0, "kept": []})
    samples = read(os.path.join(a.raw, "rss.json"), {"samples": []})["samples"]
    parts = [part_summary(p, profile["kept"], samples) for p in script["parts"]]
    with open(os.path.join(os.path.dirname(os.path.abspath(a.raw)), "summary.json"), "w") as f:
        json.dump({"label": a.label, "script": script["script"], "done": script["done"], "error": script["error"],
                   "framesTotal": profile["frames"], "framesKept": len(profile["kept"]), "parts": parts}, f, indent=1)
    sys.stdout.write(markdown(a.label, script, parts, samples, profile["frames"]))


if __name__ == "__main__":
    main(sys.argv[1:])
