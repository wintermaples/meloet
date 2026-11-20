#!/usr/bin/env python3
"""The song's tempo map for the measuring tools: ticks to seconds and back, the heads of beats.

Written apart from crates/meloet-model/src/tempo_map.rs on purpose (the tools check the app
against it), and checked against the answers Rust writes into ui/src/time/tempoMap.table.json
(python3 -I tools/measure/test_tempo_map.py).

    from tempo_map import TempoMap
    m = TempoMap.of_song(json.load(open("x.meloet")))
    m.seconds_at(16.0 * TICKS_PER_QUARTER); m.tick_at(10.0); m.beats(until_tick)   # [(tick, downbeat), ...]

usage: python3 -I tools/measure/tempo_map.py cycle N
  prints the --tempo-points and --meter-changes of make_song.py for N rounds of 16 bars:
  4 bars of 4/4 at 96, 4 bars from 104 in a straight line to 120, 4 bars of 3/4 at 120, 4 bars
  of 6/8 (120, then 150 in a straight line down to 90 over two bars, then 90). One round is
  56 quarter notes (CYCLE_TICKS) and 37.72 s.
"""
import bisect, math, sys

TICKS_PER_QUARTER = 480
# 4 bars of 4/4, 4 of 4/4, 4 of 3/4, 4 of 6/8.
CYCLE_TICKS = (16 + 16 + 12 + 12) * TICKS_PER_QUARTER
CYCLE_BARS = 16


def _ln1p_over(x):
    return 1.0 if x == 0 else math.log1p(x) / x


def _expm1_over(y):
    return 1.0 if y == 0 else math.expm1(y) / y


class TempoMap:
    def __init__(self, tempo, meter, head_ramp=False, tempo_points=(), meter_changes=()):
        """tempo: bpm; meter: (num, den); tempo_points: (tick, bpm, ramp); meter_changes:
        (bar from 0, num, den)."""
        self.regions = [(0, 0, meter[0], meter[1])]  # tick0, bar0, num, den
        for bar, num, den in sorted(meter_changes):
            t0, b0, n0, d0 = self.regions[-1]
            self.regions.append((t0 + (bar - b0) * n0 * (4 * TICKS_PER_QUARTER // d0), bar, num, den))
        points = [(0, tempo, head_ramp)] + sorted(tempo_points)
        cuts = sorted({p[0] for p in points} | {r[0] for r in self.regions})
        self.pieces = []  # tick0, sec0, ticks per beat, tempo at tick0, tempo gained per tick
        sec0 = 0.0
        for i, cut in enumerate(cuts):
            pi = max(k for k, p in enumerate(points) if p[0] <= cut)
            region = max((r for r in self.regions if r[0] <= cut), key=lambda r: r[0])
            tick, bpm, ramp = points[pi]
            s = (points[pi + 1][1] - bpm) / (points[pi + 1][0] - tick) if ramp and pi + 1 < len(points) else 0.0
            piece = (float(cut), sec0, float(4 * TICKS_PER_QUARTER // region[3]), bpm + s * (cut - tick), s)
            if i + 1 < len(cuts):
                sec0 += self._seconds(piece, cuts[i + 1] - cut)
            self.pieces.append(piece)
        self._ticks0 = [p[0] for p in self.pieces]
        self._secs0 = [p[1] for p in self.pieces]

    @staticmethod
    def of_timeline(t, centi=False):
        """`t` as in a song file's "song" (centi: tempos in 0.01 steps) or in the answer table."""
        k = 100.0 if centi else 1.0
        return TempoMap(t["tempo"] / k, tuple(t["meter"]), t.get("headRamp", False),
                        [(p["tick"], p["tempo"] / k, p["ramp"]) for p in t.get("tempoPoints", [])],
                        [(c["bar"], c["meter"][0], c["meter"][1]) for c in t.get("meterChanges", [])])

    @staticmethod
    def of_song(file):
        return TempoMap.of_timeline(file["song"], centi=True)

    @staticmethod
    def _seconds(piece, u):
        _, _, tpb, a, s = piece
        return u * 60.0 / (tpb * a) * _ln1p_over(s * u / a)

    def seconds_at(self, tick):
        p = self.pieces[max(bisect.bisect_right(self._ticks0, tick), 1) - 1]
        return p[1] + self._seconds(p, tick - p[0])

    def tick_at(self, seconds):
        p = self.pieces[max(bisect.bisect_right(self._secs0, seconds), 1) - 1]
        minutes = (seconds - p[1]) / 60.0
        return p[0] + p[3] * p[2] * minutes * _expm1_over(p[4] * p[2] * minutes)

    def tempo_at(self, tick):
        p = self.pieces[max(bisect.bisect_right(self._ticks0, tick), 1) - 1]
        return p[3] + p[4] * (tick - p[0])

    def beats(self, until_tick):
        """The heads of beats from tick 0 to before `until_tick`: (tick, first of its bar)."""
        out = []
        for i, (t0, _, num, den) in enumerate(self.regions):
            end = self.regions[i + 1][0] if i + 1 < len(self.regions) else until_tick
            per = 4 * TICKS_PER_QUARTER // den
            out += [(t, (t - t0) // per % num == 0) for t in range(t0, min(end, until_tick), per)]
        return out

    def frame_of(self, tick, from_seconds=0.0, rate=48000.0):
        """As the engine counts: from `from_seconds`, the first frame at `tick` or past it."""
        return max(0, math.ceil((self.seconds_at(tick) - from_seconds) * rate - 1e-6))


def cycle(rounds):
    points, meters = [], []
    for r in range(rounds):
        t, b = r * CYCLE_TICKS, r * CYCLE_BARS
        if r:
            points.append(f"{t}:96")
            meters.append(f"{b}:4/4")
        q = TICKS_PER_QUARTER
        points += [f"{t + 16 * q}:104:ramp", f"{t + 32 * q}:120", f"{t + 47 * q}:150:ramp", f"{t + 53 * q}:90"]
        meters += [f"{b + 8}:3/4", f"{b + 12}:6/8"]
    return ",".join(points), ",".join(meters)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "cycle":
        points, meters = cycle(int(sys.argv[2]))
        print(f"--tempo-points {points} --meter-changes {meters}")
    else:
        sys.exit(__doc__)
