#!/usr/bin/env python3
"""What one app-watch.sh run saw.

usage: python3 -I tools/measure/app_watch_summary.py RAW_DIR LAUNCHED_MS [--label TEXT]
                                                      [--load-from S] [--load-to S]

RAW_DIR holds app-watch.json (ui/src/dev/watch.ts), frame-profile.json (ui/src/gl/profile.ts),
rss.json (rss_watch.py), key-to-sound.json when a stream ran, and meloet.stderr with the ms
since the launch in front of each line. LAUNCHED_MS is the wall clock of the launch.
--load-from, --load-to: the seconds of the watch (from the page's start) the load is read over;
without them, every second with telemetry. Writes summary.json next to RAW_DIR and prints
markdown.
"""
import argparse, json, os, re, sys


def load(path):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def main(argv):
    p = argparse.ArgumentParser()
    p.add_argument("raw")
    p.add_argument("launched_ms", type=int)
    p.add_argument("--label", default="")
    p.add_argument("--load-from", type=float)
    p.add_argument("--load-to", type=float)
    a = p.parse_args(argv)
    watch = load(os.path.join(a.raw, "app-watch.json")) or {}
    profile = load(os.path.join(a.raw, "frame-profile.json")) or {}
    rss = (load(os.path.join(a.raw, "rss.json")) or {}).get("samples", [])
    sound = load(os.path.join(a.raw, "key-to-sound.json")) or {}
    out = {"label": a.label}
    lines = ["# App watch", "", f"- {a.label}"]

    win = watch.get("window")
    if win:
        lines.append(f"- page: {win['width']}x{win['height']} CSS px, devicePixelRatio {win['dpr']}")
    if sound.get("actualFrames"):
        lines.append(f"- stream: {sound['actualFrames']} frames (asked {sound.get('requestedFrames')}), "
                     f"{sound.get('sampleRate')} Hz")

    origin = watch.get("timeOriginMs")
    frames = profile.get("kept") or []
    if origin is not None:
        page_ms = origin - a.launched_ms
        out["launchToPageMs"] = round(page_ms)
        line = f"- launch -> the page's time 0: {page_ms:.0f} ms"
        if frames:
            first = page_ms + frames[0]["atMs"]
            out["launchToFirstFrameMs"] = round(first)
            line += f"; -> the first frame of the WebGL engine: {first:.0f} ms"
        lines.append(line)

    sounds = []
    try:
        for text in open(os.path.join(a.raw, "meloet.stderr"), encoding="utf-8", errors="replace"):
            m = re.match(r"(\d+) meloet: read the sound of (\S+) in (\d+) ms, (\d+) MiB", text)
            if m:
                sounds.append({"atMs": int(m[1]), "name": m[2], "readMs": int(m[3]), "mib": int(m[4])})
    except OSError:
        pass
    out["sounds"] = sounds
    if sounds:
        lines.append(f"- sounds read: {len(sounds)}, the last one done {sounds[-1]['atMs']} ms after the launch, "
                     f"{sum(s['mib'] for s in sounds)} MiB of samples: "
                     + ", ".join(f"{s['name']} {s['readMs']} ms {s['mib']} MiB (at {s['atMs']})" for s in sounds))
    else:
        lines.append("- sounds read: none")

    made = [(f["atMs"], m) for f in frames for m in f.get("made", [])]
    ctx = watch.get("contexts")
    if ctx:
        out["contexts"] = ctx
        lines.append(f"- WebGL contexts at the end: made {ctx['made']}, canvases in the page {ctx['inPage']}, "
                     f"lost among those {ctx['lostInPage']}")
    if made:
        out["made"] = [{"atMs": round(t), **m} for t, m in made]
        lines.append("- contexts made (page ms, region, getContext ms): "
                     + ", ".join(f"{t:.0f} {m['region']} {m['contextMs']:.0f}" for t, m in made))

    secs = [s for s in watch.get("seconds", [])
            if (a.load_from is None or s["t"] >= a.load_from) and (a.load_to is None or s["t"] < a.load_to)]
    if secs:
        n = sum(s["n"] for s in secs)
        mean = sum(s["loadMean"] * s["n"] for s in secs) / n / 10
        top = max(s["loadMax"] for s in secs) / 10
        means = sorted(s["loadMean"] / 10 for s in secs)
        out["load"] = {"seconds": len(secs), "messages": n, "meanPct": round(mean, 2), "maxPct": top,
                       "from": secs[0]["t"], "to": secs[-1]["t"]}
        lines.append(f"- load (the last block's time / its length, as the screen got it; seconds {secs[0]['t']}.."
                     f"{secs[-1]['t']} of the page, {n} messages): mean {mean:.2f} %, max {top:.1f} %; "
                     f"per-second means {means[0]:.1f}..{means[-1]:.1f} %")

    mel = [s["meloetMb"] for s in rss if s.get("meloetMb") is not None]
    web = [s["webMb"] for s in rss if s.get("webMb") is not None]
    if mel:
        out["rss"] = {"meloetLastMb": mel[-1], "meloetMaxMb": max(mel),
                      "webLastMb": web[-1] if web else None, "webMaxMb": max(web) if web else None}
        lines.append(f"- RSS meloet: last {mel[-1]:.0f} MB, max {max(mel):.0f} MB; WebKitWebProcess: "
                     + (f"last {web[-1]:.0f} MB, max {max(web):.0f} MB" if web else "not seen"))
        every = max(1, len(rss) // 12)
        lines.append("- RSS each " + str(every) + " s (meloet / web MB): "
                     + ", ".join(f"{s.get('meloetMb') or 0:.0f}/{s.get('webMb') or 0:.0f}" for s in rss[::every]))

    with open(os.path.join(os.path.dirname(os.path.abspath(a.raw)), "summary.json"), "w") as f:
        json.dump(out, f, indent=1)
        f.write("\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main(sys.argv[1:])
