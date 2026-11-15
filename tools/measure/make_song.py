#!/usr/bin/env python3
"""Writes a song file for measuring: one track, one clip with one take of N notes.

usage: python3 -I tools/measure/make_song.py OUT.meloet [--notes 200] [--bars 16] [--tempo 96]
                                             [--cc 0] [--seed 1]

The shape is crates/meloet-core/src/songfile.rs, version 1. The clip starts at the song's start
and is --bars bars of 4/4 long. Notes are spread over the clip in start order, keys 48..84,
lengths of a 16th to a half note (never past the clip's end), velocity 40..100. --cc puts that
many sustain (CC 64) points in the take, evenly. The same arguments always write the same file.
"""
import argparse, json, os, random, sys

TICKS_PER_QUARTER = 480
TICKS_PER_BAR = 4 * TICKS_PER_QUARTER
TRACK, CLIP, TAKE = 1, 2, 3
LENGTHS = [120, 240, 480, 960]


def song(notes, bars, tempo, cc, seed):
    rng = random.Random(seed)
    length = bars * TICKS_PER_BAR
    made = []
    for i in range(notes):
        start = (i * length) // notes + rng.randrange(0, 60)
        start = min(start, length - 1)
        note_len = min(rng.choice(LENGTHS), length - start)
        made.append([start, note_len, rng.randrange(48, 85), rng.randrange(40, 101)])
    made.sort(key=lambda n: (n[0], n[2]))
    controls = []
    for i in range(cc):
        controls.append([(i * length) // max(cc, 1), 64, 127 if i % 2 == 0 else 0])
    return {
        "format": "meloet-song",
        "version": 1,
        "app": "tools/measure/make_song.py",
        "song": {
            "name": f"測る曲 {notes} ノート",
            "tempo": round(tempo * 100),
            "meter": [4, 4],
            "tempoChanges": [],
            "nextId": TAKE + 1,
            "tracks": [{
                "id": TRACK,
                "name": "ピアノ",
                "color": 1,
                "instrument": {"kind": "placeholder"},
                "gainDb": 0.0,
                "clips": [{
                    "id": CLIP,
                    "name": "ピアノ",
                    "start": 0,
                    "end": length,
                    "active": 0,
                    "takes": [{"take": TAKE, "offset": 0}],
                }],
                "plugins": [],
                "automation": [],
            }],
            "audio": [],
        },
        "takes": [{"id": TAKE, "length": length, "notes": made, "controls": controls}],
        "session": {"playTarget": TRACK, "armed": TRACK, "loop": None, "loopOn": False},
    }


def main(argv):
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("out")
    p.add_argument("--notes", type=int, default=200)
    p.add_argument("--bars", type=int, default=16)
    p.add_argument("--tempo", type=float, default=96.0)
    p.add_argument("--cc", type=int, default=0)
    p.add_argument("--seed", type=int, default=1)
    a = p.parse_args(argv)
    if a.notes < 0 or a.bars < 1 or a.cc < 0 or not 30 <= a.tempo <= 300:
        p.error("notes >= 0, bars >= 1, cc >= 0, tempo 30..300")
    data = json.dumps(song(a.notes, a.bars, a.tempo, a.cc, a.seed), ensure_ascii=False, separators=(",", ":"))
    tmp = a.out + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(data + "\n")
    os.replace(tmp, a.out)
    print(a.out)


if __name__ == "__main__":
    main(sys.argv[1:])
