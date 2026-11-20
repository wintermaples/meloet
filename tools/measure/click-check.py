#!/usr/bin/env python3
"""Plays a song in the real app with the metronome on, records what reaches the output, and
compares the frame every click starts on with the tempo map's answer.

usage: python3 -I tools/measure/click-check.py OUT --song X.meloet [--seconds 60]
       python3 -I tools/measure/click-check.py OUT --song X.meloet --wav OUT/raw/output.wav

The song should make no sound of its own (make_song.py --notes 0): a click's first sample is
its loudest, and it is found as the first sample over a threshold after silence. The recording
(the output's monitor, 48 kHz) runs in the same PipeWire graph as the app's stream, so with no
xrun the distance between two clicks is the same number of frames as in the engine. The first
click (tick 0) lines the recording up with the map; every later one is compared with
`frame_of(tick) - frame_of(0)`, and its loudness tells a bar's first beat from the others.
--wav: only look at a recording made before.

On this PC the recording itself now and then loses three or four blocks about every 5 s (the
clicks after it come early by 768 or 1024 frames; docs/measurements/2026-11-20-round6-step1.md).
The summary lists where the distance between two clicks is not the map's, and compares where
the song stopped with the PC's clock, which tells a short recording from a song running fast.

Writes OUT/summary.md and OUT/clicks.json. Changes nothing on the PC; the clicks are heard.
"""
import argparse, json, os, sys, time

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import remote  # noqa: E402
from tempo_map import TempoMap  # noqa: E402

RATE = 48000
# meloet_engine::transport::Click: the first sample of a bar's click and of a beat's.
BAR_LEVEL, BEAT_LEVEL = 0.17, 0.13


def record(out, song, seconds):
    wav = os.path.join(out, "raw", "output.wav")
    with remote.App(out, song=song) as app:
        state = app.snapshot()
        if state["audio"]["sampleRate"] != RATE:
            sys.exit(f"click-check.py: the output runs at {state['audio']['sampleRate']} Hz, not {RATE}")
        if not state["transport"]["metronome"]:
            app.send({"type": "toggleMetronome"})
        app.send({"type": "goToStart"})
        state = app.wait(lambda s: s["transport"]["metronome"] and s["transport"]["position"] == 0, 5, "the metronome")
        rec = remote.Recorder(wav, env=app.env)
        time.sleep(1.5)
        started = time.monotonic()
        app.send({"type": "playStop"})
        time.sleep(seconds)
        stopped = time.monotonic()
        app.send({"type": "playStop"})
        time.sleep(0.5)
        rec.stop()
        state = app.wait(lambda s: s["transport"]["state"] == "stopped", 5, "the stop")
        audio = state["audio"]
        # Where the song stopped (the place heard when the stop came) against the PC's clock
        # between the two commands; the pipe and the output's delay are some ms each way.
        audio["playedS"] = stopped - started
        audio["stoppedAtTick"] = state["transport"]["position"]
        audio["status"] = app.status()
    return wav, audio


def onsets(x):
    """(frame, first sample) of every click: the first sample over the threshold after 10 ms
    under a quarter of it."""
    import numpy
    loud = numpy.abs(x)
    threshold = loud.max() * 0.4
    over = numpy.flatnonzero(loud > threshold)
    quiet = numpy.concatenate(([0], numpy.cumsum(loud > threshold / 4)))
    found = []
    for i in over:
        if found and i - found[-1][0] < RATE // 20:
            continue
        if i >= RATE // 100 and quiet[i] - quiet[i - RATE // 100] == 0:
            found.append((int(i), float(x[i])))
    return found


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out")
    p.add_argument("--song", required=True)
    p.add_argument("--seconds", type=float, default=60)
    p.add_argument("--wav")
    a = p.parse_args()
    os.makedirs(os.path.join(a.out, "raw"), exist_ok=True)
    audio = None
    wav = a.wav
    if not wav:
        wav, audio = record(a.out, a.song, a.seconds)
    with open(a.song, encoding="utf-8") as f:
        m = TempoMap.of_song(json.load(f))
    _, left, right = remote.read_wav(wav)
    found = onsets(left)
    if not found:
        sys.exit("click-check.py: no click in the recording")
    beats = m.beats(int(m.tick_at(len(left) / RATE + 5)) + 1)
    frame0 = found[0][0] - m.frame_of(beats[0][0])
    scale = max(v for _, v in found) / BAR_LEVEL
    rows, off, wrong_kind, steps, at, before = [], 0, 0, [], 0, 0
    for frame, first in found:
        # A click may be missing from the recording: the beat that fits best among the next
        # three, given how far off the click before was.
        near = range(at, min(at + 3, len(beats)))
        at = min(near, key=lambda k: abs(frame - frame0 - m.frame_of(beats[k][0]) - before))
        tick, downbeat = beats[at]
        d = frame - frame0 - m.frame_of(tick)
        heard_down = first / scale > (BAR_LEVEL + BEAT_LEVEL) / 2
        off += d != 0
        wrong_kind += heard_down != downbeat
        if d != before:
            steps.append((round(m.seconds_at(tick), 2), d - before))
        before = d
        at += 1
        rows.append({"tick": tick, "downbeat": downbeat, "frame": frame - frame0, "expected": m.frame_of(tick),
                     "off": d, "first": round(first, 5), "tempo": round(m.tempo_at(tick), 4)})
    worst = max(rows, key=lambda r: abs(r["off"]))
    last = rows[-1]
    missing = sum(1 for t, _ in beats if t <= last["tick"]) - len(rows)
    same = bool((left == right).all())
    lines = [
        "# Metronome clicks against the tempo map",
        "",
        f"- song {os.path.basename(a.song)}: {len(m.pieces)} stretches of tempo and meter",
        f"- recording: {len(left) / RATE:.1f} s, left and right {'the same' if same else 'DIFFER'}, "
        f"loudest first sample {scale * BAR_LEVEL:.4f} (the engine writes {BAR_LEVEL})",
        f"- clicks found: {len(found)}; the last one at tick {last['tick']}, "
        f"{last['expected'] / RATE:.3f} s after the first",
        f"- clicks not on their frame: {off} of {len(rows)}; worst {worst['off']:+d} frames at tick {worst['tick']}",
        f"- the last click: frame {last['frame']}, the map says {last['expected']} ({last['off']:+d})",
        f"- beats up to the last click with no click found: {missing}",
        "- places where the distance from the click before is not the map's (song seconds, frames): "
        + (", ".join(f"{t} s {d:+d}" for t, d in steps[:40]) or "none")
        + ("; every one a whole number of blocks of 256: frames went missing from the recording or from "
           "the stream, the song's own places are right" if steps and all(d % 256 == 0 for _, d in steps) else ""),
        f"- clicks whose loudness does not fit the bar's first beat: {wrong_kind}",
    ]
    if audio:
        lines.append(f"- output: {audio['actualFrames']} frames at {audio['sampleRate']} Hz, "
                     f"{audio['outputDevice']}")
        song_s = m.seconds_at(audio["stoppedAtTick"])
        lines.append(f"- by the PC's clock the song played {audio['playedS']:.3f} s and stopped at tick "
                     f"{audio['stoppedAtTick']}, {song_s:.3f} s of the song ({(song_s - audio['playedS']) * 1000:+.1f} ms)")
        lines.append(f"- status line at the end: {audio['status']}")
    with open(os.path.join(a.out, "clicks.json"), "w") as f:
        json.dump({"audio": audio, "clicks": rows}, f)
    text = "\n".join(lines) + "\n"
    with open(os.path.join(a.out, "summary.md"), "w") as f:
        f.write(text)
    print(text)


main()
