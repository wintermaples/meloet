#!/usr/bin/env python3
"""Checks make_song.py against the rules songfile.rs reads by. usage: python3 -I tools/measure/test_make_song.py"""
import importlib.util, os, sys

here = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("make_song", os.path.join(here, "make_song.py"))
make_song = importlib.util.module_from_spec(spec)
spec.loader.exec_module(make_song)


def check(notes, bars, cc):
    s = make_song.song(notes, bars, 96.0, cc, 1)
    assert s["format"] == "meloet-song" and s["version"] == 1
    take = s["takes"][0]
    clip = s["song"]["tracks"][0]["clips"][0]
    assert len(take["notes"]) == notes
    assert all(i < s["song"]["nextId"] for i in (clip["id"], take["id"]))
    starts = [n[0] for n in take["notes"]]
    assert starts == sorted(starts)
    for start, length, key, velocity in take["notes"]:
        assert length >= 1 and start + length <= clip["end"] and 0 <= key <= 127 and 1 <= velocity <= 127
    ticks = [c[0] for c in take["controls"]]
    assert len(ticks) == cc and ticks == sorted(ticks)
    assert make_song.song(notes, bars, 96.0, cc, 1) == s


for args in [(200, 16, 0), (10000, 100, 32), (0, 1, 0), (5, 1, 3)]:
    check(*args)
print("make_song: ok")
