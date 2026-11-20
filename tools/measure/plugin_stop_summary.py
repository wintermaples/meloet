#!/usr/bin/env python3
"""What one plugin-stop.sh run saw.

usage: python3 -I tools/measure/plugin_stop_summary.py RAW_DIR LAUNCHED_MS [--recorded-ms MS]
                                                        [--label TEXT] [--ended TEXT]

RAW_DIR holds meloet.stderr (the ms since the launch in front of each line), plug-watch.json
(plug_watch.py), output.wav (the output's monitor, started at --recorded-ms on the wall
clock), saved.meloet (the song as saved at the end), vkeys.json and rss.json. The song is one
of `make_song.py --hard-pan`: the plugin tracks are on the left, the others on the right; its
first track is ids 1, 2, 3 (track, clip, take), so every other take of track 1 was recorded
in this run. Writes summary.json next to RAW_DIR and prints markdown.
"""
import argparse, array, json, math, os, re, sys, wave

TICKS_PER_QUARTER = 480


def load(path):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def channel_rms(path, step_s=0.5):
    """[(left, right)] RMS of each `step_s` of the WAV, 0..1."""
    try:
        w = wave.open(path)
    except (OSError, wave.Error, EOFError):
        return []
    rate = w.getframerate()
    out = []
    while True:
        a = array.array("h", w.readframes(int(rate * step_s)))
        if len(a) < 2:
            break
        left, right = a[0::2], a[1::2]
        out.append(tuple(math.sqrt(sum(x * x for x in c) / len(c)) / 32768 for c in (left, right)))
    return out


def db(x):
    return "-inf" if x is None or x <= 1e-6 else f"{20 * math.log10(x):.1f}"


def mean(values):
    return sum(values) / len(values) if values else None


def main(argv):
    p = argparse.ArgumentParser()
    p.add_argument("raw")
    p.add_argument("launched_ms", type=int)
    p.add_argument("--recorded-ms", type=int)
    p.add_argument("--label", default="")
    p.add_argument("--ended", default="")
    a = p.parse_args(argv)
    out = {"label": a.label, "ended": a.ended}
    lines = ["# Plugin host stopped from outside", "", f"- {a.label}", f"- meloet ended: {a.ended}"]

    plug = load(os.path.join(a.raw, "plug-watch.json")) or {}
    sent = plug.get("signal")
    sent_ms = sent["wallMs"] - a.launched_ms if sent else None
    stderr = []
    try:
        for text in open(os.path.join(a.raw, "meloet.stderr"), encoding="utf-8", errors="replace"):
            m = re.match(r"(\d+) (meloet: .*)", text)
            if m:
                stderr.append((int(m[1]), m[2].strip()))
    except OSError:
        pass
    said = [(t, text) for t, text in stderr if "stopped" in text]
    if sent:
        out["signal"] = {"name": sent["name"], "pid": sent["pid"], "atMs": sent_ms}
        lines.append(f"- SIG{sent['name']} to the plugin host {sent['pid']}, {sent_ms} ms after the launch")
        after = [(t, text) for t, text in said if t >= sent_ms - 5]
        if after:
            out["noticedMs"] = after[0][0] - sent_ms
            lines.append(f"- meloet said so {after[0][0] - sent_ms} ms later (a line on stderr, read through "
                         f"a pipe): `{after[0][1]}`")
        else:
            lines.append("- meloet did not say that the process stopped")
    else:
        lines.append("- no signal was sent (no plugin host was there)")
    others = [f"{t} `{text}`" for t, text in stderr if "read the sound" not in text and (t, text) not in said[:1]]
    if others:
        lines.append("- other lines of meloet on stderr: " + "; ".join(others[:8]))

    hosts = plug.get("hosts", {})
    if hosts:
        rows = []
        for pid, h in sorted(hosts.items(), key=lambda kv: kv[1]["firstMs"]):
            rows.append(f"{pid} ({h['firstMs'] - a.launched_ms}..{h['lastMs'] - a.launched_ms} ms, "
                        f"RSS up to {h['peakRssMb']:.0f} MB)")
        out["hosts"] = len(hosts)
        lines.append("- plugin hosts seen (pid, from..to after the launch): " + ", ".join(rows))
        if sent:
            new = [pid for pid, h in hosts.items() if h["firstMs"] > sent["wallMs"]]
            out["hostsAfterSignal"] = len(new)
            lines.append(f"- hosts started after the signal: {len(new)}")

    step = 0.5
    rms = channel_rms(os.path.join(a.raw, "output.wav"), step)
    if rms and sent and a.recorded_ms is not None:
        def window(from_s, to_s):
            """Mean RMS (left, right) between those seconds after the signal."""
            base = (sent["wallMs"] - a.recorded_ms) / 1000
            part = [r for i, r in enumerate(rms) if base + from_s <= i * step and (i + 1) * step <= base + to_s]
            return (mean([r[0] for r in part]), mean([r[1] for r in part]))
        names = [("before", -4.0, -0.5), ("after", 1.0, 4.5), ("later", 6.5, 11.5)]
        out["rms"] = {}
        lines += ["", "| the output, seconds from the signal | left (plugin tracks) dBFS | right (the others) dBFS |",
                  "|---|---|---|"]
        for name, lo, hi in names:
            left, right = window(lo, hi)
            out["rms"][name] = {"leftDb": db(left), "rightDb": db(right)}
            lines.append(f"| {name}, {lo:+.1f}..{hi:+.1f} | {db(left)} | {db(right)} |")
        lines.append("")
        lines.append("- 'later' is after the reload when there was one (5 s after the signal)")
    elif not rms:
        lines.append("- no recording of the output")

    saved = load(os.path.join(a.raw, "saved.meloet"))
    vkeys = load(os.path.join(a.raw, "vkeys.json")) or {}
    if saved:
        tracks = saved["song"]["tracks"]
        first = next((t for t in tracks if t["id"] == 1), None)
        refs = {ref["take"] for clip in (first or {}).get("clips", []) for ref in clip["takes"]} - {3}
        notes = [(clip["start"] - ref["offset"] + n[0])
                 for clip in (first or {}).get("clips", []) for ref in clip["takes"] if ref["take"] in refs
                 for take in saved["takes"] if take["id"] == ref["take"] for n in take["notes"]]
        tempo = saved["song"]["tempo"] / 100
        out["saved"] = {"version": saved["version"], "recordedTakes": len(refs), "recordedNotes": len(notes)}
        line = (f"- the song saved at the end: version {saved['version']}, takes recorded on track 1: "
                f"{len(refs)}, notes in them: {len(notes)}")
        if notes:
            span = (max(notes) - min(notes)) / TICKS_PER_QUARTER * 60 / tempo
            out["saved"]["spanS"] = round(span, 1)
            line += f", first to last {span:.1f} s"
        sent_keys = vkeys.get("keys")
        if isinstance(sent_keys, list):
            line += f" (keys sent: {len(sent_keys)})"
        lines.append(line)
        kinds = [t["instrument"].get("kind") for t in tracks]
        lines.append("- instruments in the saved song: " + ", ".join(kinds))
    else:
        lines.append("- no saved song")

    rss = (load(os.path.join(a.raw, "rss.json")) or {}).get("samples", [])
    mel = [s["meloetMb"] for s in rss if s.get("meloetMb") is not None]
    web = [s["webMb"] for s in rss if s.get("webMb") is not None]
    if mel:
        lines.append(f"- RSS meloet: last {mel[-1]:.0f} MB, max {max(mel):.0f} MB"
                     + (f"; WebKitWebProcess: last {web[-1]:.0f} MB, max {max(web):.0f} MB" if web else ""))

    with open(os.path.join(os.path.dirname(os.path.abspath(a.raw)), "summary.json"), "w") as f:
        json.dump(out, f, indent=1)
        f.write("\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main(sys.argv[1:])
