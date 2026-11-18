#!/usr/bin/env python3
"""Writes a song file for measuring: one track, one clip with one take of N notes.

usage: python3 -I tools/measure/make_song.py OUT.meloet [--notes 200] [--bars 16] [--tempo 96]
                                             [--cc 0] [--seed 1] [--tracks N]

--tracks N (1..64): a version 3 song with N tracks, each with one clip and one take of --notes
notes. The instruments go round BAND (the groups mixed), with the sends a new track of that
group gets, an EQ and a compressor on some tracks, a reverb on return A, a delay on return B
and the limiter on the master (every parameter at its default). The first track keeps the ids
1, 2, 3 (track, clip, take), so `selectClip 2` works for both shapes.

Without --tracks the shape is crates/meloet-core/src/songfile.rs, version 1. The clip starts at the song's start
and is --bars bars of 4/4 long. Notes are spread over the clip in start order, keys 48..84,
lengths of a 16th to a half note (never past the clip's end), velocity 40..100. --cc puts that
many sustain (CC 64) points in the take, evenly. The same arguments always write the same file.
"""
import argparse, json, os, random, sys

TICKS_PER_QUARTER = 480
TICKS_PER_BAR = 4 * TICKS_PER_QUARTER
TRACK, CLIP, TAKE = 1, 2, 3
LENGTHS = [120, 240, 480, 960]


# name in the file, name on the screen, group, keys, inserts.
BAND = [
    ("grandPiano", "グランドピアノ", "pianoKeys", (48, 85), ["eq4", "compressor"]),
    ("fingerBass", "フィンガーベース", "bass", (28, 53), ["compressor"]),
    ("studioKit", "スタジオキット", "drums", (36, 52), ["eq4", "compressor"]),
    ("strings", "ストリングス", "padStrings", (48, 85), ["eq4"]),
    ("electricPiano", "エレクトリックピアノ", "pianoKeys", (48, 85), []),
    ("synthLead", "シンセリード", "synth", (60, 85), ["compressor"]),
    ("warmPad", "あたたかいパッド", "padStrings", (48, 73), []),
    ("organ", "オルガン", "pianoKeys", (48, 85), ["eq4", "compressor"]),
]
# meloet_model::mixer::new_track_sends: [A, B] in dB, None is -inf.
NEW_TRACK_SENDS = {"pianoKeys": -15.0, "padStrings": -15.0, "synth": -15.0, "drums": -24.0, "bass": None}
INSERT_SLOTS = 4
TRACK_COLORS = 5
PANS = [0, -20, 0, 30, -40, 20, -10, 40]


def take_notes(rng, notes, length, keys):
    made = []
    for i in range(notes):
        start = (i * length) // notes + rng.randrange(0, 60)
        start = min(start, length - 1)
        note_len = min(rng.choice(LENGTHS), length - start)
        made.append([start, note_len, rng.randrange(*keys), rng.randrange(40, 101)])
    made.sort(key=lambda n: (n[0], n[2]))
    return made


def slots(effects):
    made = [{"effect": e, "params": {}} for e in effects]
    return made + [None] * (INSERT_SLOTS - len(made))


def band_song(tracks, notes, bars, tempo, cc, seed):
    """Version 3: `tracks` tracks, ids 1 + 3 i (track), 2 + 3 i (clip), 3 + 3 i (take)."""
    length = bars * TICKS_PER_BAR
    controls = [[(i * length) // max(cc, 1), 64, 127 if i % 2 == 0 else 0] for i in range(cc)]
    track_list, takes = [], []
    for i in range(tracks):
        file_name, name, group, keys, effects = BAND[i % len(BAND)]
        track, clip, take = 1 + 3 * i, 2 + 3 * i, 3 + 3 * i
        made = take_notes(random.Random(seed * 1000 + i), notes, length, keys)
        takes.append({"id": take, "length": length, "notes": made, "controls": controls if i == 0 else []})
        track_list.append({
            "id": track,
            "name": name if i < len(BAND) else f"{name} {i // len(BAND) + 1}",
            "color": i % TRACK_COLORS + 1,
            "instrument": {"kind": "builtin", "name": file_name},
            "gainDb": -9.0,
            "pan": PANS[i % len(PANS)],
            "sends": {"a": NEW_TRACK_SENDS[group], "b": None},
            "inserts": slots(effects),
            "clips": [{"id": clip, "name": name, "start": 0, "end": length, "active": 0,
                       "takes": [{"take": take, "offset": 0}]}],
            "plugins": [],
            "automation": [],
        })
    return {
        "format": "meloet-song",
        "version": 3,
        "app": "tools/measure/make_song.py",
        "song": {
            "name": f"測る曲 {tracks} トラック {notes} ノート",
            "tempo": round(tempo * 100),
            "meter": [4, 4],
            "tempoChanges": [],
            "nextId": 3 * tracks + 1,
            "tracks": track_list,
            "mixer": {
                "returns": {"a": {"gainDb": 0.0, "pan": 0, "inserts": slots(["reverb"])},
                            "b": {"gainDb": 0.0, "pan": 0, "inserts": slots(["delay"])}},
                "master": {"gainDb": 0.0, "pan": 0, "inserts": slots(["limiter"])},
            },
            "audio": [],
        },
        "takes": takes,
        "session": {"playTarget": TRACK, "loop": None, "loopOn": False},
    }


def song(notes, bars, tempo, cc, seed):
    rng = random.Random(seed)
    length = bars * TICKS_PER_BAR
    made = take_notes(rng, notes, length, (48, 85))
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
    p.add_argument("--tracks", type=int)
    a = p.parse_args(argv)
    if a.notes < 0 or a.bars < 1 or a.cc < 0 or not 30 <= a.tempo <= 300:
        p.error("notes >= 0, bars >= 1, cc >= 0, tempo 30..300")
    if a.tracks is not None and not 1 <= a.tracks <= 64:
        p.error("tracks 1..64")
    if a.tracks is None:
        made = song(a.notes, a.bars, a.tempo, a.cc, a.seed)
    else:
        made = band_song(a.tracks, a.notes, a.bars, a.tempo, a.cc, a.seed)
    data = json.dumps(made, ensure_ascii=False, separators=(",", ":"))
    tmp = a.out + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(data + "\n")
    os.replace(tmp, a.out)
    print(a.out)


if __name__ == "__main__":
    main(sys.argv[1:])
