#!/usr/bin/env python3
"""Checks tools/measure/tempo_map.py against the answers of meloet-model
(ui/src/time/tempoMap.table.json). usage: python3 -I tools/measure/test_tempo_map.py"""
import json, os, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from tempo_map import CYCLE_BARS, CYCLE_TICKS, TempoMap, cycle  # noqa: E402


def close(a, b):
    return abs(a - b) <= 1e-9 * max(1.0, abs(a), abs(b))


table = json.load(open(os.path.join(here, "../../ui/src/time/tempoMap.table.json")))
checked = 0
for case in table["maps"]:
    m = TempoMap.of_timeline(case["timeline"])
    for row in case["atTicks"]:
        if row["tick"] < 0:
            continue
        assert close(m.seconds_at(row["tick"]), row["seconds"]), (case["name"], row, m.seconds_at(row["tick"]))
        assert close(m.tempo_at(row["tick"]), row["tempo"]), (case["name"], row)
        checked += 1
    for row in case["atSeconds"]:
        if row["seconds"] < 0:
            continue
        assert close(m.tick_at(row["seconds"]), row["tick"]), (case["name"], row, m.tick_at(row["seconds"]))
        checked += 1
    last = max(row["tick"] for row in case["positions"])
    heads = dict(m.beats(last + 1))
    for row in case["positions"]:
        bar, beat, tick = row["barBeatTick"]
        assert (tick == 0) == (row["tick"] in heads), (case["name"], row)
        if tick == 0:
            assert heads[row["tick"]] == (beat == 1), (case["name"], row)
        checked += 1
    for row in case["clicks"]:
        assert m.next_click(row["tick"]) == (row["next"], row["downbeat"]), (case["name"], row, m.next_click(row["tick"]))
        checked += 1

points, meters = cycle(3)
m = TempoMap(96.0, (4, 4), False,
             [(int(p.split(":")[0]), float(p.split(":")[1]), p.endswith(":ramp")) for p in points.split(",")],
             [(int(c.split(":")[0]), *map(int, c.split(":")[1].split("/"))) for c in meters.split(",")])
assert [r[0] for r in m.regions if r[1] % CYCLE_BARS == 0] == [0, CYCLE_TICKS, 2 * CYCLE_TICKS]
one = m.seconds_at(CYCLE_TICKS)
assert close(m.seconds_at(2 * CYCLE_TICKS), 2 * one) and abs(one - 31.1510) < 1e-3, one
print(f"ok: {checked} answers of the table, one round of the cycle is {one:.4f} s")
