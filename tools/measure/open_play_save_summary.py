#!/usr/bin/env python3
"""What one open-play-save.sh run heard and saved.

usage: python3 -I tools/measure/open_play_save_summary.py RAW_DIR --keys-ms MS --song-ms MS [--label TEXT]

RAW_DIR holds output.wav (the output's monitor) and saved.meloet. --keys-ms: where in the WAV
(ms) the 5 s of single keys begin; --song-ms: where the 4 s of the playing song begin. "How
high" is the number of upward zero crossings per second of the left side while it sounds: not
a pitch, but a number that doubles when the same keys sound an octave up. Writes summary.json
next to RAW_DIR and prints markdown.
"""
import argparse, array, base64, hashlib, json, math, os, sys, wave


def sides(path, from_ms, length_ms):
    """(left, right) samples of that part of the WAV."""
    try:
        w = wave.open(path)
    except (OSError, wave.Error, EOFError):
        return None
    rate = w.getframerate()
    w.setpos(min(w.getnframes(), int(rate * from_ms / 1000)))
    a = array.array("h", w.readframes(int(rate * length_ms / 1000)))
    return a[0::2], a[1::2], rate


def rms(samples):
    return math.sqrt(sum(x * x for x in samples) / len(samples)) / 32768 if samples else 0.0


def db(x):
    return "-inf" if x <= 1e-6 else f"{20 * math.log10(x):.1f}"


def crossings_per_second(samples, rate):
    """Upward zero crossings per second, over the 50 ms steps that are louder than -50 dBFS."""
    step = rate // 20
    ups = sounding = 0
    for at in range(0, len(samples) - step, step):
        part = samples[at:at + step]
        if rms(part) < 0.003:
            continue
        sounding += 1
        ups += sum(1 for i in range(1, len(part)) if part[i - 1] < 0 <= part[i])
    return ups / (sounding * step / rate) if sounding else None


def plugin_line(where, plugin):
    try:
        state = base64.b64decode(plugin.get("state", ""))
    except ValueError:
        state = b""
    return {"where": where, "name": plugin.get("name"), "key": plugin.get("key"), "stateBytes": len(state),
            "stateSha256": hashlib.sha256(state).hexdigest()[:16] if state else None}


def main(argv):
    p = argparse.ArgumentParser()
    p.add_argument("raw")
    p.add_argument("--keys-ms", type=int, required=True)
    p.add_argument("--song-ms", type=int, required=True)
    p.add_argument("--label", default="")
    a = p.parse_args(argv)
    out = {"label": a.label}
    lines = ["# Open, play, save", "", f"- {a.label}"]

    wav = os.path.join(a.raw, "output.wav")
    keys = sides(wav, a.keys_ms, 5000)
    if keys and len(keys[0]):
        left, right, rate = keys
        hz = crossings_per_second(left, rate)
        out["keys"] = {"leftDb": db(rms(left)), "rightDb": db(rms(right)), "leftUpsPerS": hz and round(hz, 1)}
        lines.append(f"- the keys on the first track (5 s): left {db(rms(left))} dBFS, right {db(rms(right))} dBFS, "
                     f"left upward zero crossings {hz:.1f} per second" if hz else
                     f"- the keys on the first track (5 s): left {db(rms(left))} dBFS, right {db(rms(right))} dBFS, silent")
    else:
        lines.append("- no recording of the output")
    played = sides(wav, a.song_ms, 4000)
    if played and len(played[0]):
        left, right, _ = played
        out["song"] = {"leftDb": db(rms(left)), "rightDb": db(rms(right))}
        lines.append(f"- the song playing (4 s): left {db(rms(left))} dBFS, right {db(rms(right))} dBFS")

    try:
        with open(os.path.join(a.raw, "saved.meloet"), encoding="utf-8") as f:
            saved = json.load(f)
    except (OSError, ValueError):
        saved = None
    if saved:
        out["version"] = saved["version"]
        lines.append(f"- the saved song: version {saved['version']}")
        found = []
        for track in saved["song"]["tracks"]:
            inst = track["instrument"]
            if inst.get("kind") == "plugin":
                found.append(plugin_line(f"track {track['id']} instrument", inst))
            elif inst.get("kind") == "sf2":
                lines.append(f"  - track {track['id']} instrument: SF2 {inst.get('file')} bank {inst.get('bank')} "
                             f"program {inst.get('program')} ({inst.get('name')}), sha256 {str(inst.get('sha256'))[:16]}")
            else:
                lines.append(f"  - track {track['id']} instrument: {inst.get('kind')} {inst.get('name', '')}")
            for n, slot in enumerate(track.get("inserts") or []):
                if slot and "plugin" in slot:
                    found.append(plugin_line(f"track {track['id']} insert {n + 1}", slot["plugin"]))
        out["plugins"] = found
        for f in found:
            lines.append(f"  - {f['where']}: plugin {f['name']} ({f['key']}), state {f['stateBytes']} bytes, "
                         f"sha256 {f['stateSha256']}")
    else:
        lines.append("- no saved song")

    with open(os.path.join(os.path.dirname(os.path.abspath(a.raw)), "summary.json"), "w") as f:
        json.dump(out, f, indent=1)
        f.write("\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main(sys.argv[1:])
