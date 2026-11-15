#!/usr/bin/env python3
"""Summary of one tools/measure/long-record.sh run.

usage: python3 -I tools/measure/long_record_summary.py DIR

Reads DIR/watch.json, DIR/vkeys.json and DIR/playback.wav and prints Markdown.
Bounds (homework 3 of round 2): WebKitWebProcess under 600 MB for the whole run, and no second
with a new xrun that is also a second with GC work.
"""
import array, json, math, os, sys, wave

WEB_LIMIT_MB = 600
GC_BUSY_MS = 20  # GC helper CPU time in one second that counts as "GC was running"


def load(path):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def rms_per_second(path):
    try:
        w = wave.open(path)
    except (OSError, wave.Error):
        return None
    rate, ch = w.getframerate(), w.getnchannels()
    out = []
    while True:
        frames = w.readframes(rate)
        if not frames:
            break
        a = array.array("h", frames)
        if not a:
            break
        peak = max(abs(x) for x in a) / 32768
        rms = math.sqrt(sum(x * x for x in a) / len(a)) / 32768
        out.append((rms, peak))
    return out


def db(x):
    return "-inf" if x <= 0 else f"{20 * math.log10(x):.1f}"


def main():
    d = sys.argv[1]
    watch = load(os.path.join(d, "watch.json")) or {}
    rows = watch.get("rows", [])
    vkeys = load(os.path.join(d, "vkeys.json")) or {}
    lines = ["# Long recording", ""]
    lines.append(f"- keys sent: {vkeys.get('notes', '?')} notes, {vkeys.get('pedalEvents', '?')} pedal "
                 f"events, {vkeys.get('bendEvents', '?')} bend events over {vkeys.get('seconds', '?')} s "
                 f"(latest send {vkeys.get('sendLateMaxUs', '?')} µs after its time)")

    errs = [(r["t"], r.get("errMeloet"), r.get("errDriver")) for r in rows]
    seen = [e for e in errs if e[1] is not None]
    if seen:
        first, last = seen[0], seen[-1]
        lines.append(f"- PipeWire ERR of the meloet stream: {first[1]} at {first[0]} s -> {last[1]} at "
                     f"{last[0]} s")
    else:
        names = watch.get("pwTopNames") or []
        lines.append("- PipeWire ERR: the meloet stream was never seen in pw-top "
                     f"(pw-top started again {watch.get('pwTopRestarts', '?')} times, last stderr "
                     f"`{watch.get('pwTopStderr')}`; nodes seen: {', '.join(names) or 'none'})")
    lines.append(f"- pw-top line of the meloet stream: `{watch.get('pwTopMeloetLine')}`")
    outs = [r.get("errOutputs") or {} for r in rows]
    outs = [o for o in outs if o]
    if outs:
        lines.append(f"- PipeWire ERR of the outputs: first {outs[0]}, last {outs[-1]}")
    new_err = set()
    for (t0, a0, b0), (t1, a1, b1) in zip(seen, seen[1:]):
        if (a1 or 0) > (a0 or 0) or (b1 or 0) > (b0 or 0):
            new_err.add(int(t1))
    for r0, r1 in zip(rows, rows[1:]):
        o0, o1 = r0.get("errOutputs") or {}, r1.get("errOutputs") or {}
        if any(o1[n] > o0.get(n, o1[n]) for n in o1):
            new_err.add(int(r1["t"]))
    gc_busy = {int(r["t"]) for r in rows
               if r.get("gcMs_web", 0) >= GC_BUSY_MS or r.get("gcMs_meloet", 0) >= GC_BUSY_MS}
    near = {t for t in new_err if any(abs(t - g) <= 1 for g in gc_busy)}
    lines.append(f"- seconds with a new xrun: {sorted(new_err) or 'none'}; seconds with GC work "
                 f"(>= {GC_BUSY_MS} ms of HeapHelper CPU): {len(gc_busy)}; xrun seconds within 1 s "
                 f"of GC work: {sorted(near) or 'none'}")

    rss = [(r["t"], r.get("rssMeloetMb"), r.get("rssWebMb")) for r in rows if "rssMeloetMb" in r]
    if rss:
        web = [w for _, _, w in rss if w is not None]
        mel = [m for _, m, _ in rss if m is not None]
        lines.append(f"- RSS meloet: {mel[0]:.0f} -> {mel[-1]:.0f} MB (max {max(mel):.0f}); "
                     f"WebKitWebProcess: {web[0]:.0f} -> {web[-1]:.0f} MB (max {max(web):.0f}, "
                     f"limit {WEB_LIMIT_MB}: {'ok' if max(web) <= WEB_LIMIT_MB else 'OVER'})"
                     if web and mel else "- RSS: missing")
        lines += ["", "| t (s) | meloet MB | WebKitWebProcess MB |", "|---|---|---|"]
        for t, m, w in rss[:: max(1, len(rss) // 20)] + [rss[-1]]:
            lines.append(f"| {t:.0f} | {m:.0f} | {w:.0f} |" if m and w else f"| {t:.0f} | ? | ? |")

    levels = rms_per_second(os.path.join(d, "playback.wav"))
    lines.append("")
    if levels:
        lines.append("playback recorded from the output's monitor, per second, RMS / peak dBFS: "
                     + ", ".join(f"{db(r)}/{db(p)}" for r, p in levels))
    else:
        lines.append("playback: no recording")
    print("\n".join(lines))


main()
