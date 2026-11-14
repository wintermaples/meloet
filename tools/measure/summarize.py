#!/usr/bin/env python3
"""Key -> screen and key -> sound from one measured run, next to the numbers of the first study.

usage: python3 -I tools/measure/summarize.py DIR [--vkeys FILE] [--out DIR] [--label TEXT]

DIR is what MELOET_MEASURE_DIR was: it holds key-to-sound.json (written by the core) and
key-to-screen.json (written by the screen). Writes summary.md and summary.json to --out
(default: DIR) and prints the table.

All times are CLOCK_MONOTONIC. The screen's times are performance.now() in ms; the file gives
offsetUs so that rustUs ~= ms * 1000 + offsetUs. The JS clock has 1 ms steps, so key -> screen
is good to about +-0.5 ms.
"""
import argparse, json, math, os, sys

# docs/report.md 5-2, row "本物の鍵 117 回", and 5-3, row "本物の鍵、quantum 256".
STUDY = {
    "keyToJsMs": {"p50": 0.02, "p95": 0.41, "max": None, "count": 117},
    "keyToDrawMs": {"p50": 0.28, "p95": 0.84, "max": 5.2, "count": 117},
    "keyToCallbackMs": {"mean": 2.8, "p95": 5.2, "max": None},
    "keyToDacMs": {"mean": 13.5, "p95": 15.8, "max": 16.0},
}
FRAME_MS = 1000 / 60


def quantile(sorted_xs, p):
    return sorted_xs[min(len(sorted_xs) - 1, int(p * len(sorted_xs)))]


def stats(xs):
    """None when there is nothing to describe."""
    if not xs:
        return None
    xs = sorted(xs)
    return {
        "count": len(xs),
        "mean": sum(xs) / len(xs),
        "p50": quantile(xs, 0.5),
        "p95": quantile(xs, 0.95),
        "max": xs[-1],
        "min": xs[0],
    }


def screen_times(screen):
    """(Rust -> JS handler, Rust -> rAF that drew it) in ms, and how many keys were left out."""
    offset = screen.get("offsetUs")
    keys = screen.get("keys", [])
    if offset is None:
        return [], [], len(keys)
    drawn = [k for k in keys if k.get("drawMs") is not None]
    rust_ms = lambda k: (k["tUs"] - offset) / 1000
    to_js = [k["recvMs"] - rust_ms(k) for k in drawn]
    to_draw = [k["drawMs"] - rust_ms(k) for k in drawn]
    return to_js, to_draw, len(keys) - len(drawn)


def sound_times(sound):
    records = sound.get("records", [])
    to_callback = [(r["callbackUs"] - r["keyUs"]) / 1000 for r in records]
    to_dac = [(r["playbackUs"] - r["keyUs"]) / 1000 for r in records]
    return to_callback, to_dac


def sent_to_received(vkeys, screen):
    """vkeys' send time -> Rust's receive time, when both saw the same keys in the same order."""
    sent = vkeys.get("notes", [])
    got = screen.get("keys", [])
    if not sent or len(sent) != len(got) or any(s["note"] != g["note"] for s, g in zip(sent, got)):
        return []
    return [(g["tUs"] - s["on_t_us"]) / 1000 for s, g in zip(sent, got)]


def summarize(sound, screen, vkeys, label=""):
    to_js, to_draw, undrawn = screen_times(screen or {})
    to_callback, to_dac = sound_times(sound or {})
    shown = [x + FRAME_MS for x in to_draw]
    return {
        "label": label,
        "sampleRate": (sound or {}).get("sampleRate"),
        "requestedFrames": (sound or {}).get("requestedFrames"),
        "actualFrames": (sound or {}).get("actualFrames"),
        "keysSent": len((vkeys or {}).get("notes", [])) or None,
        "keysNotDrawn": undrawn,
        "clockOffsetKnown": (screen or {}).get("offsetUs") is not None,
        "keyToJsMs": stats(to_js),
        "keyToDrawMs": stats(to_draw),
        "shownWithinTwoFramesPercent": (
            100 * sum(x <= 2 * FRAME_MS for x in shown) / len(shown) if shown else None
        ),
        "keyToCallbackMs": stats(to_callback),
        "keyToDacMs": stats(to_dac),
        "sentToRustMs": stats(sent_to_received(vkeys or {}, screen or {})),
        "study": STUDY,
    }


def num(x, digits=2):
    return "—" if x is None or (isinstance(x, float) and math.isnan(x)) else f"{x:.{digits}f}"


def cells(s, names):
    return " / ".join(num(s.get(n) if s else None) for n in names)


def markdown(r):
    count = lambda s: str(s["count"]) if s else "0"
    frames = r["actualFrames"] if r["actualFrames"] is not None else "?"
    rate = r["sampleRate"] or "?"
    out = [
        f"# Key latency{' — ' + r['label'] if r['label'] else ''}",
        "",
        f"- buffer: {frames} frames running (asked for {r['requestedFrames'] or '?'}), {rate} Hz",
        f"- keys sent by meloet-vkeys: {r['keysSent'] if r['keysSent'] is not None else '?'}",
        "",
        "## Key -> screen (ms)",
        "",
        "| | Rust -> JS p50 / p95 / max | Rust -> drawn (rAF) p50 / p95 / max | n |",
        "|---|---|---|---|",
        f"| this run | {cells(r['keyToJsMs'], ['p50', 'p95', 'max'])} "
        f"| {cells(r['keyToDrawMs'], ['p50', 'p95', 'max'])} | {count(r['keyToDrawMs'])} |",
        f"| study (real keys) | {cells(STUDY['keyToJsMs'], ['p50', 'p95', 'max'])} "
        f"| {cells(STUDY['keyToDrawMs'], ['p50', 'p95', 'max'])} | {STUDY['keyToDrawMs']['count']} |",
        "",
        "## Key -> sound (ms)",
        "",
        "| | Rust -> callback mean / p95 / max | Rust -> DAC (planned) mean / p95 / max | n |",
        "|---|---|---|---|",
        f"| this run | {cells(r['keyToCallbackMs'], ['mean', 'p95', 'max'])} "
        f"| {cells(r['keyToDacMs'], ['mean', 'p95', 'max'])} | {count(r['keyToDacMs'])} |",
        f"| study (real keys, quantum 256) | {cells(STUDY['keyToCallbackMs'], ['mean', 'p95', 'max'])} "
        f"| {cells(STUDY['keyToDacMs'], ['mean', 'p95', 'max'])} | — |",
        "",
    ]
    notes = []
    if r["shownWithinTwoFramesPercent"] is not None:
        notes.append(
            f"shown within 2 frames (drawn + one 60 Hz frame, an estimate): "
            f"{r['shownWithinTwoFramesPercent']:.0f} %"
        )
    if r["sentToRustMs"]:
        notes.append(
            "meloet-vkeys sent -> Rust received, p50 / p95 / max: "
            + cells(r["sentToRustMs"], ["p50", "p95", "max"])
            + " ms"
        )
    if not r["clockOffsetKnown"]:
        notes.append("key -> screen: no clock offset in key-to-screen.json, so nothing to compare")
    if r["keysNotDrawn"]:
        notes.append(f"keys that reached JS but were not drawn before the last report: {r['keysNotDrawn']}")
    if not r["keyToDacMs"]:
        notes.append("key -> sound: no record (no stream was running?)")
    return "\n".join(out + [f"- {n}" for n in notes] + [""])


def read_json(path, what):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError) as e:
        print(f"summarize: no {what}: {e}", file=sys.stderr)
        return None


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("dir", help="the MELOET_MEASURE_DIR of the run")
    p.add_argument("--vkeys", help="the --log file of meloet-vkeys play")
    p.add_argument("--out", help="where summary.md and summary.json go (default: DIR)")
    p.add_argument("--label", default="", help="a name for the run, shown in the title")
    a = p.parse_args()

    sound = read_json(os.path.join(a.dir, "key-to-sound.json"), "key -> sound")
    screen = read_json(os.path.join(a.dir, "key-to-screen.json"), "key -> screen")
    vkeys = read_json(a.vkeys, "vkeys log") if a.vkeys else None
    result = summarize(sound, screen, vkeys, a.label)

    out = a.out or a.dir
    os.makedirs(out, exist_ok=True)
    text = markdown(result)
    with open(os.path.join(out, "summary.md"), "w", encoding="utf-8") as f:
        f.write(text)
    with open(os.path.join(out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2)
        f.write("\n")
    print(text)
    return 0 if sound and screen else 1


if __name__ == "__main__":
    sys.exit(main())
