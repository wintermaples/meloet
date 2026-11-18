"""Summarize the audio files under a folder with ffprobe.

python3 -I tools/sounds/probe.py <dir> [<glob> ...]
python3 -I tools/sounds/probe.py <dir> --list <file with one path under <dir> per line>

Groups the files by (codec, sample rate, bits, channels) and prints the count,
the shortest and longest length and the total size of each group. Reads only;
nothing in <dir> is run.
"""

import json
import os
import pathlib
import subprocess
import sys

AUDIO = {".wav", ".flac", ".ogg", ".aif", ".aiff"}


def probe(path):
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", "--", str(path)],
        capture_output=True, text=True, check=False,
    )
    if out.returncode != 0:
        return None
    data = json.loads(out.stdout)
    audio = [s for s in data.get("streams", []) if s.get("codec_type") == "audio"]
    if not audio:
        return None
    s = audio[0]
    bits = s.get("bits_per_raw_sample") or s.get("bits_per_sample") or "?"
    dur = float(s.get("duration") or data.get("format", {}).get("duration") or 0)
    return (s.get("codec_name"), s.get("sample_rate"), str(bits), s.get("channels")), dur


def main():
    root = pathlib.Path(sys.argv[1])
    if sys.argv[2:3] == ["--list"]:
        with open(sys.argv[3], encoding="utf-8") as f:
            names = [line.strip().lstrip("/") for line in f if line.strip()]
        files = sorted({root / n for n in names if (root / n).is_file()})
    else:
        globs = sys.argv[2:] or ["**/*"]
        files = sorted({p for g in globs for p in root.glob(g) if p.is_file() and p.suffix.lower() in AUDIO})
    groups = {}
    bad = []
    for p in files:
        r = probe(p)
        if r is None:
            bad.append(p)
            continue
        key, dur = r
        g = groups.setdefault(key, {"n": 0, "min": None, "max": None, "bytes": 0, "minf": "", "maxf": ""})
        g["n"] += 1
        g["bytes"] += p.stat().st_size
        if g["min"] is None or dur < g["min"]:
            g["min"], g["minf"] = dur, p.name
        if g["max"] is None or dur > g["max"]:
            g["max"], g["maxf"] = dur, p.name
    print(f"files {len(files)}  unreadable {len(bad)}")
    for (codec, rate, bits, ch), g in sorted(groups.items(), key=lambda kv: -kv[1]["n"]):
        print(
            f"{codec} {rate} Hz {bits} bit {ch} ch: {g['n']} files, "
            f"{g['min']:.2f}-{g['max']:.2f} s ({g['minf']} / {g['maxf']}), {g['bytes'] / 1e6:.1f} MB"
        )
    for p in bad:
        print("unreadable:", os.fspath(p))


if __name__ == "__main__":
    main()
