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


def check_band(tracks, notes, bars):
    s = make_song.band_song(tracks, notes, bars, 96.0, 4, 1)
    assert s["format"] == "meloet-song" and s["version"] == 3
    made = s["song"]["tracks"]
    takes = {t["id"]: t for t in s["takes"]}
    ids = [i for t in made for i in (t["id"], t["clips"][0]["id"], t["clips"][0]["takes"][0]["take"])]
    assert len(made) == tracks and len(set(ids)) == 3 * tracks and max(ids) < s["song"]["nextId"]
    assert (made[0]["id"], made[0]["clips"][0]["id"]) == (1, 2) and s["session"]["playTarget"] == 1
    for t in made:
        assert 1 <= t["color"] <= 5 and -100 <= t["pan"] <= 100 and len(t["inserts"]) == 4
        clip = t["clips"][0]
        take = takes[clip["takes"][0]["take"]]
        assert len(take["notes"]) == notes
        for start, length, key, velocity in take["notes"]:
            assert length >= 1 and start + length <= clip["end"] and 0 <= key <= 127 and 1 <= velocity <= 127
    if tracks >= 8:
        assert len({t["instrument"]["name"] for t in made}) == 8
    mixer = s["song"]["mixer"]
    assert [mixer["returns"][b]["inserts"][0]["effect"] for b in "ab"] == ["reverb", "delay"]
    assert mixer["master"]["inserts"][0]["effect"] == "limiter"
    assert make_song.band_song(tracks, notes, bars, 96.0, 4, 1) == s


for args in [(8, 200, 16), (1, 5, 1), (16, 50, 4)]:
    check_band(*args)
print("make_song: ok")
