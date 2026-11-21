#!/usr/bin/env python3
"""Stretched and pitched vocal clips in the real app (release build): where the made audio
sounds, what making it costs while the song plays, how the made audio comes in, what is kept.

    python3 -I tools/measure/stretch_app.py position OUT [--shapes base,t96,ramp,...] [--seconds 8] [--sink NAME]
    python3 -I tools/measure/stretch_app.py prepare OUT --song FILE [--source-bpm 96]
    python3 -I tools/measure/stretch_app.py load OUT --song FILE [--tempo 120] [--keys] [--after-s 20]
    python3 -I tools/measure/stretch_app.py kept OUT --song FILE [--tempo 120]
    python3 -I tools/measure/stretch_app.py splice OUT --song FILE [--cases pitch,tempo,loop,jump,undo]

position: as audio_app.py position (the output's monitor is the input): records the metronome
  at 120 on a vocal track, changes the song as the shape says, waits for the audio to be made,
  and records the played clip on a second vocal track. Where each click lies in the second
  take against the place the tempo map gives it (its start, its loudest place, the middle of
  its energy; its pitch), and the same in the made file itself. One start of the app for each
  shape, each with its own kept folder (OUT/<shape>/cache). Shapes: base (nothing changed),
  t96, t150 (the song's tempo), ramp (96 at the head in a straight line to 144 at beat 16),
  rampdown (150 down to 90), p3 (+3 semitones), c30 (+30 cents), t96p3c30, rampp3.
prepare: opens the song where it is (a copy: it is saved), turns 「曲のテンポについていく」 on
  for every vocal clip with 「この音のテンポ」 --source-bpm (the song's own tempo: nothing is
  made), and saves.
load: opens a prepared song, plays from the head and sets the song's tempo to --tempo, so
  that every clip is made while the song plays. Until all are made and --after-s seconds on:
  the jobs, `stream_missing`, pw-top (ERR, B/Q), the CPU of meloet's threads (meloet-render),
  the RSS, the load the transport shows; the output's monitor is recorded. --keys: a virtual
  keyboard plays for --keys-s seconds from the change (summarize.py reads key -> DAC and key -> screen). The song is
  not saved. The kept folder is OUT/cache, emptied first.
kept: after `load` with the same OUT (its kept folder is used as it is): opens the song, sets
  the tempo (nothing may be made: the kept audio is there), undo (the unstretched audio), redo
  (kept again), another tempo and undo; then saves at --tempo, closes, opens again. Each time:
  were there jobs, did files appear in stretch/. The sizes of the kept folders.
splice: a song with one long steady tone on a vocal track. While it plays, the pitch or the
  tempo is changed (and undone): the made audio comes in over 10 ms. The output's monitor is
  recorded and looked at around every such place: silence, a level above the tone's, a jump
  from one sample to the next. Also around a loop's turn and with jumps right after the audio
  was made.

Writes OUT/summary.json. Changes nothing on the PC.
"""
import argparse, json, math, os, shutil, struct, subprocess, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
from remote import App, Recorder, agent_env, mem, read_wav, scan_cache, root
from tempo_map import TempoMap
import vocal_app
from audio_app import (BAR, Watch, audio_clips, finish, load_shown, missing_total, one_take, proc_io,
                       thread_cpu, vocal_tracks, wait_ready)

RATE = 48000
TICKS = 1920


def own_cache(folder, empty=True):
    """A kept folder of this run alone, with the plugin scan of the shared one (so that no
    scan runs while measuring)."""
    if empty:
        shutil.rmtree(folder, ignore_errors=True)
    plugins = os.path.join(folder, "meloet", "plugins")
    if not os.path.isdir(plugins) and os.path.isdir(os.path.join(scan_cache, "meloet", "plugins")):
        os.makedirs(os.path.dirname(plugins), exist_ok=True)
        shutil.copytree(os.path.join(scan_cache, "meloet", "plugins"), plugins)
    return folder


def kept_files(cache, name="stretch"):
    folder = os.path.join(cache, "meloet", name)
    if not os.path.isdir(folder):
        return {}
    return {n: os.path.getsize(os.path.join(folder, n)) for n in sorted(os.listdir(folder))}


def kept_sizes(cache):
    return {name: {"files": len(kept_files(cache, name)), "mb": round(sum(kept_files(cache, name).values()) / 1e6, 1)}
            for name in ("audio", "stretch", "peaks")}


def jobs_of(state):
    return [j for j in state.get("audioWork", {}).get("jobs", []) if j["kind"] == "rendering"]


def wait_made(app, appear_s=2.0, done_s=600, every=0.05, on_sample=None):
    """Waits for the jobs to appear (at most appear_s) and then to be gone. Returns what was
    seen: when the first job showed, when none was left (seconds from the call), the most jobs."""
    start = time.monotonic()
    first = last = None
    most = 0
    samples = []
    while True:
        state = app.snapshot()
        now = time.monotonic() - start
        jobs = jobs_of(state)
        samples.append([round(now, 3), len(jobs), [j["percent"] for j in jobs][:4]])
        if on_sample:
            on_sample(now, state, jobs)
        most = max(most, len(jobs))
        if jobs and first is None:
            first = now
        if first is None and now > appear_s:
            break
        if first is not None and not jobs:
            last = now
            break
        if now > done_s:
            break
        time.sleep(every)
    return {"firstJobS": None if first is None else round(first, 3), "noJobS": None if last is None else round(last, 3),
            "mostJobs": most, "samples": samples[:40] + samples[-5:] if len(samples) > 45 else samples}


def read_f32(path):
    """(channels, rate, frames x channels) of a wav file of floats or 16/24 bit, by its chunks."""
    with open(path, "rb") as f:
        data = f.read()
    at, fmt, body = 12, None, None
    while at + 8 <= len(data):
        name, size = data[at:at + 4], struct.unpack_from("<I", data, at + 4)[0]
        if name == b"fmt ":
            fmt = struct.unpack_from("<HHIIHH", data, at + 8)
        if name == b"data":
            body = data[at + 8:at + 8 + size] if size and at + 8 + size <= len(data) else data[at + 8:]
            break
        at += 8 + size + (size & 1)
    kind, channels, rate, _, _, bits = fmt
    if bits == 32 and kind in (3, 0xFFFE):
        x = np.frombuffer(body[:len(body) // 4 * 4], dtype="<f4").astype(np.float64)
    elif bits == 16:
        x = np.frombuffer(body[:len(body) // 2 * 2], dtype="<i2").astype(np.float64) / 32768
    elif bits == 24:
        b = np.frombuffer(body[:len(body) // 3 * 3], dtype=np.uint8).reshape(-1, 3).astype(np.int32)
        v = b[:, 0] | (b[:, 1] << 8) | (b[:, 2] << 16)
        x = np.where(v >= 1 << 23, v - (1 << 24), v) / float(1 << 23)
    else:
        raise SystemExit(f"stretch_app.py: {path}: {bits} bits of kind {kind}")
    return channels, rate, x[:len(x) // channels * channels].reshape(-1, channels)


def envelope(x):
    """The size of the sound at every frame (the analytic signal)."""
    n = len(x)
    spec = np.fft.fft(x)
    h = np.zeros(n)
    h[0] = 1
    h[1:(n + 1) // 2] = 2
    if n % 2 == 0:
        h[n // 2] = 1
    return np.abs(np.fft.ifft(spec * h))


def look_at_click(samples, expected, before=1500, after=3600):
    """One click near the frame `expected` (a float): frames from there to its start (the
    first place above a quarter of its loudest), its loudest place and the middle of its
    energy, its level and its pitch."""
    e = int(round(expected))
    lo, hi = e - before, e + after
    if lo < 0 or hi > len(samples):
        return None
    seg = samples[lo:hi]
    env = envelope(seg)
    peak = int(np.argmax(env))
    top = float(env[peak])
    if top < 0.01:
        return {"level": round(top, 4)}
    # Only the click itself: from 600 frames before the loudest place to 2400 after it.
    a, b = max(0, peak - 600), min(len(seg), peak + 2400)
    onset = a + int(np.flatnonzero(env[a:b] > 0.25 * top)[0])
    early = a + int(np.flatnonzero(env[a:b] > 0.05 * top)[0])
    power = env[a:b] ** 2
    middle = a + float(np.sum(np.arange(b - a) * power) / np.sum(power))
    # The pitch: the strongest line of the click's spectrum.
    n = 1 << 16
    piece = seg[onset:min(len(seg), onset + 1400)]
    spec = np.abs(np.fft.rfft(piece * np.hanning(len(piece)), n))
    k = int(np.argmax(spec[20:])) + 20
    y0, y1, y2 = np.log(spec[k - 1:k + 2] + 1e-12)
    hz = (k + 0.5 * (y0 - y2) / (y0 - 2 * y1 + y2)) * RATE / n
    off = lo - expected
    return {"start": round(onset + off, 1), "start5": round(early + off, 1), "loudest": round(peak + off, 1),
            "middle": round(middle + off, 1), "level": round(top, 4), "hz": round(hz, 1)}


SHAPES = {
    "base": [],
    "t96": [{"type": "setTempo", "bpm": 96}],
    "t150": [{"type": "setTempo", "bpm": 150}],
    "ramp": [{"type": "setTempo", "bpm": 96}, {"type": "addTempoPoint", "tick": 16 * TICKS, "bpm": 144},
             {"type": "setTempoRamp", "tick": 0, "on": True}],
    "rampdown": [{"type": "setTempo", "bpm": 150}, {"type": "addTempoPoint", "tick": 16 * TICKS, "bpm": 90},
                 {"type": "setTempoRamp", "tick": 0, "on": True}],
    "p3": [("semitones", 3)],
    "c30": [("cents", 30)],
    "t96p3c30": [{"type": "setTempo", "bpm": 96}, ("semitones", 3), ("cents", 30)],
    "rampp3": [{"type": "setTempo", "bpm": 96}, {"type": "addTempoPoint", "tick": 16 * TICKS, "bpm": 144},
               {"type": "setTempoRamp", "tick": 0, "on": True}, ("semitones", 3)],
}


def stat(values):
    v = np.array(values, dtype=np.float64)
    return {"n": len(v), "mean": round(float(v.mean()), 1), "least": round(float(v.min()), 1),
            "most": round(float(v.max()), 1)} if len(v) else {"n": 0}


def one_shape(a, shape, sink):
    out = os.path.join(a.out, shape)
    for d in ("state", "config"):
        shutil.rmtree(os.path.join(out, d), ignore_errors=True)
    cache = own_cache(os.path.join(out, "cache"))
    state_dir = os.path.join(out, "state")
    r = {"shape": shape, "commands": [c if isinstance(c, dict) else list(c) for c in SHAPES[shape]]}
    with vocal_app.start_app(out, sink, {"XDG_CACHE_HOME": cache}) as app:
        vocal_app.arm(app, sink)
        one_take(app, a.seconds)
        app.send({"type": "toggleMetronome"})
        app.send({"type": "addVocalTrack"})
        app.wait(lambda s: len(vocal_tracks(s)) == 2 and vocal_tracks(s)[1]["kind"]["armed"]
                 and vocal_tracks(s)[1]["kind"]["inputReady"], 15, "the second vocal track")
        app.send({"type": "toggleVocalArm", "track": vocal_tracks(app.snapshot())[0]["id"]})
        app.wait(lambda s: not vocal_tracks(s)[0]["kind"]["armed"], 15, "the first vocal track to be let go")
        clip = vocal_tracks(app.snapshot())[0]["clips"][0]
        r["jobsBeforeTheChange"] = len(jobs_of(app.snapshot()))
        for c in SHAPES[shape]:
            app.send(c if isinstance(c, dict) else
                     {"type": "setClipStretch", "clip": clip["id"], "change": {"kind": c[0], c[0]: c[1]}})
            time.sleep(0.05)
        r["made"] = wait_made(app)
        state = app.snapshot()
        clip = vocal_tracks(state)[0]["clips"][0]
        r["clip"] = {"start": clip["start"], "end": clip["end"], "stretch": clip["audio"]["stretch"],
                     "state": clip["audio"]["state"]}
        r["timeline"] = {k: state["timeline"][k] for k in ("tempoBpm", "headRamp", "tempoPoints")}
        r["kept"] = kept_files(cache)
        now = TempoMap(state["timeline"]["tempoBpm"], (4, 4), state["timeline"]["headRamp"],
                       [(p["tick"], p["tempoBpm"], p["ramp"]) for p in state["timeline"]["tempoPoints"]])
        # Stopped, the ring holds two seconds from here: time for it to be filled again.
        time.sleep(2.5)
        one_take(app, now.seconds_at(clip["end"]) + 0.6)
        time.sleep(3)
        until = time.time() + 15
        song = None
        while time.time() < until:
            song = vocal_app.read_song(state_dir)
            if song and len(song.get("audioTakes", [])) >= 2:
                break
            time.sleep(0.2)
        r["status"] = app.status()
    takes = vocal_app.vocal_takes(song)
    first = next(t for t in takes if t["clipStart"] == clip["start"] and t["clipEnd"] == clip["end"]
                 and t is takes[0])
    second = takes[-1]
    r["takes"] = [{k: t[k] for k in ("origin", "clipStart", "clipEnd", "firstFrame", "frames", "file")} for t in takes]
    song_map = TempoMap.of_song(song)
    folder = os.path.join(state_dir, "work", "音")
    _, _, a_samples, _ = vocal_app.read_wav(os.path.join(folder, first["file"]))
    _, _, b_samples, _ = vocal_app.read_wav(os.path.join(folder, second["file"]))
    in_first = {c["beat"]: c for c in vocal_app.clicks_of(a_samples, first)}
    r["firstTake"] = stat([c["off"] for c in in_first.values()])
    made = None
    if len(r["kept"]) == 1:
        name = next(iter(r["kept"]))
        channels, rate, frames = read_f32(os.path.join(cache, "meloet", "stretch", name))
        made = frames[:, 0]
        want = song_map.frame_of(clip["end"]) - song_map.frame_of(clip["start"])
        r["madeFile"] = {"name": name, "channels": channels, "rate": rate, "frames": len(frames),
                         "framesByTheMap": want,
                         "framesByTheMapNotRounded": round((song_map.seconds_at(clip["end"]) - song_map.seconds_at(clip["start"])) * RATE, 2)}
    clicks = []
    for beat, c in sorted(in_first.items()):
        tick = beat * TICKS
        # The second take's beat 0 holds a click of the metronome that was just turned off.
        if beat == 0 or not (clip["start"] <= tick < clip["end"] - TICKS // 2):
            continue
        # The click lies `off` frames from the beat in the first file; the clip follows the
        # song by its ticks, at 120 a frame of that file is this many ticks.
        click_tick = tick + c["off"] / RATE * (vocal_app.BPM / 60) * TICKS
        row = {"beat": beat, "inFirstFile": round(c["off"], 2), "tempoThere": round(song_map.tempo_at(tick), 2)}
        sec = song_map.seconds_at(click_tick)
        expected = second["firstFrame"] + (sec - song_map.seconds_at(second["origin"])) * RATE
        row["inSecondFile"] = look_at_click(b_samples, expected)
        if made is not None:
            row["inMadeFile"] = look_at_click(made, (sec - song_map.seconds_at(clip["start"])) * RATE)
        clicks.append(row)
    r["clicks"] = clicks
    for where in ("inSecondFile", "inMadeFile"):
        rows = [c[where] for c in clicks if c.get(where) and "start" in c[where]]
        r[where] = {k: stat([x[k] for x in rows]) for k in ("start", "start5", "loudest", "middle", "level")}
        for strong, name in ((True, "hzOfBarHeads"), (False, "hzOfTheOthers")):
            r[where][name] = stat([c[where]["hz"] for c in clicks if c.get(where) and "hz" in c[where]
                                   and (c["beat"] % 4 == 0) == strong])
    return r


def run_position(a):
    env = agent_env()
    sink = a.sink or vocal_app.default_sink(env)
    result = {"sink": sink, "seconds": a.seconds, "shapes": {}}
    for shape in a.shapes.split(","):
        result["shapes"][shape] = one_shape(a, shape, sink)
        short = {k: result["shapes"][shape].get(k) for k in ("made", "madeFile", "inSecondFile", "inMadeFile", "firstTake")}
        short["made"] = {k: v for k, v in short["made"].items() if k != "samples"}
        print(shape, json.dumps(short, ensure_ascii=False), flush=True)
    os.makedirs(a.out, exist_ok=True)
    with open(os.path.join(a.out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)


def vocal_clips(state):
    return [c for t in vocal_tracks(state) for c in t["clips"] if c.get("audio")]


def run_prepare(a):
    cache = own_cache(os.path.join(a.out, "cache"))
    result = {"song": a.song}
    with App(a.out, song=a.song, song_in_place=True, env={"XDG_CACHE_HOME": cache}) as app:
        state = wait_ready(app)
        for c in vocal_clips(state):
            app.send({"type": "setClipStretch", "clip": c["id"], "change": {"kind": "sourceBpm", "bpm": a.source_bpm}})
            app.send({"type": "setClipStretch", "clip": c["id"], "change": {"kind": "follow", "on": True}})
        state = app.wait(lambda s: all(c["audio"]["stretch"]["follow"] and c["audio"]["stretch"]["sourceBpm"] == a.source_bpm
                                       for c in vocal_clips(s)), 20, "every clip to follow")
        result["made"] = wait_made(app, appear_s=3.0)
        result["stretch"] = [c["audio"]["stretch"] for c in vocal_clips(state)]
        result["tempo"] = state["timeline"]["tempoBpm"]
        app.send({"type": "saveSong"})
        app.wait(lambda s: s["song"]["file"] == "saved", 30, "the save")
        result["status"] = app.status()
        result["kept"] = kept_sizes(cache)
    finish(a.out, result)


def wait_peaks(cache, count, seconds=120):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if len([n for n in kept_files(cache, "peaks") if n.endswith(".peaks")]) >= count:
            return True
        time.sleep(0.2)
    return False


def run_load(a):
    env = agent_env()
    cache = own_cache(os.path.join(a.out, "cache"))
    result = {"song": a.song, "tempo": a.tempo, "phases": {}}
    with App(a.out, song=a.song, song_in_place=True, env={"XDG_CACHE_HOME": cache}) as app:
        state = wait_ready(app)
        clips = vocal_clips(state)
        result["clips"] = len(clips)
        result["tempoBefore"] = state["timeline"]["tempoBpm"]
        result["peaksThere"] = wait_peaks(cache, len({c["audio"]["audio"] for c in clips}))
        result["jobsBefore"] = wait_made(app, appear_s=1.5)["mostJobs"]
        time.sleep(2)
        watch = Watch(app, env)
        rec_at = time.monotonic()
        rec = Recorder(os.path.join(a.out, "out.wav"), env)
        time.sleep(3)
        marks, cpu, loads = {}, {}, []

        def mark(name):
            marks[name] = time.monotonic()
            cpu[name] = thread_cpu(app.pid)

        app.send({"type": "goToStart"})
        time.sleep(1.0)
        app.send({"type": "playStop"})
        mark("play")
        time.sleep(10)
        loads.append(["before", load_shown(app)])
        # The keys begin half a second after the change and go on for --keys-s seconds.
        keys = None
        if a.keys:
            count = int(a.keys_s * 1000 / 257)
            keys = app.play_keys("--count", count, "--interval-ms", 257, "--hold-ms", 120, "--velocity", 25,
                                 "--lead-ms", 500, wait=False, log=os.path.join(a.out, "raw", "vkeys.json"))
        mark("change")
        app.send({"type": "setTempo", "bpm": a.tempo})

        def look(now, state, jobs):
            if int(now) % 4 == 0 and (not loads or loads[-1][0] != int(now)):
                loads.append([int(now), load_shown(app)])

        result["made"] = wait_made(app, appear_s=5.0, every=0.5, on_sample=look)
        mark("made")
        time.sleep(a.after_s)
        loads.append(["after", load_shown(app)])
        mark("end")
        if keys:
            keys.wait()
        result["stretchAfter"] = vocal_clips(app.snapshot())[0]["audio"]["stretch"]
        app.send({"type": "playStop"})
        app.wait(lambda s: s["transport"]["state"] == "stopped", 10, "the stop")
        time.sleep(2.5)
        watch.end()
        rec.stop()
        result["madeS"] = round(marks["made"] - marks["change"], 2)
        for name, (start, end) in (("before", ("play", "change")), ("making", ("change", "made")), ("after", ("made", "end"))):
            span = watch.span(marks[start], marks[end]) or {}
            span["threadCpuS"] = {k: round(v - cpu[start].get(k, 0), 2) for k, v in cpu[end].items()
                                  if v - cpu[start].get(k, 0) >= 0.05}
            span["wallS"] = round(marks[end] - marks[start], 2)
            result["phases"][name] = span
        quiet = quiet_runs_of(os.path.join(a.out, "out.wav"))
        result["quietRuns"] = [(round(t - (marks["play"] - rec_at), 3), ms) for t, ms in quiet
                               if marks["play"] - rec_at + 0.3 <= t <= marks["end"] - rec_at]
        result["loadShown"] = loads
        result["kept"] = kept_sizes(cache)
        result["statusAtTheEnd"] = app.status()
        result["missingTotal"] = missing_total(app.stderr.name)
        result["memoryAtTheEnd"] = app.memory()
    finish(a.out, result)
    if a.keys:
        subprocess.run([sys.executable, "-I", os.path.join(root, "tools", "measure", "summarize.py"),
                        os.path.join(a.out, "raw"), "--vkeys", os.path.join(a.out, "raw", "vkeys.json"),
                        "--out", os.path.join(a.out, "keys"), "--label", f"stretch_app.py load, {a.song}"])


def quiet_runs_of(wav):
    from audio_app import quiet_runs
    return quiet_runs(wav)


def run_kept(a):
    cache = own_cache(os.path.join(a.out, "cache"), empty=False)
    result = {"song": a.song, "steps": [], "keptBefore": kept_sizes(cache)}

    def step(app, name, command, appear_s=2.5):
        before = set(kept_files(cache))
        at = time.monotonic()
        if command:
            app.send(command)
        made = wait_made(app, appear_s=appear_s, every=0.2)
        state = app.snapshot()
        result["steps"].append({"step": name, "tempo": state["timeline"]["tempoBpm"],
                                "ratio": vocal_clips(state)[0]["audio"]["stretch"]["ratio"],
                                "mostJobs": made["mostJobs"], "firstJobS": made["firstJobS"], "noJobS": made["noJobS"],
                                "tookS": round(time.monotonic() - at, 2),
                                "newFiles": len(set(kept_files(cache)) - before), "files": len(kept_files(cache)),
                                "status": app.status()})
        print(json.dumps(result["steps"][-1], ensure_ascii=False), flush=True)

    with App(os.path.join(a.out, "kept1"), song=a.song, song_in_place=True, env={"XDG_CACHE_HOME": cache}) as app:
        wait_ready(app)
        step(app, "opened", None)
        step(app, "tempo", {"type": "setTempo", "bpm": a.tempo})
        step(app, "undo", {"type": "undo"})
        step(app, "redo", {"type": "redo"})
        step(app, "anotherTempo", {"type": "setTempo", "bpm": a.tempo - 10})
        step(app, "undoToTheTempo", {"type": "undo"})
        app.send({"type": "saveSong"})
        app.wait(lambda s: s["song"]["file"] == "saved", 30, "the save")
        result["keptAtTheSave"] = kept_sizes(cache)
    names = set(kept_files(cache))
    with App(os.path.join(a.out, "kept2"), song=a.song, song_in_place=True, env={"XDG_CACHE_HOME": cache}) as app:
        wait_ready(app)
        step(app, "openedAgain", None, appear_s=4.0)
        result["partFiles"] = [n for n in kept_files(cache) if not n.endswith(".wav")]
    result["sameFilesAfterOpeningAgain"] = names == set(kept_files(cache))
    result["keptAfter"] = kept_sizes(cache)
    sizes = sorted(kept_files(cache).values())
    result["stretchFileMb"] = {"least": round(sizes[0] / 1e6, 1), "most": round(sizes[-1] / 1e6, 1)} if sizes else None
    # OUT is the folder of a `load`: its summary.json stays.
    with open(os.path.join(a.out, "kept.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)
    print(json.dumps({k: v for k, v in result.items() if k != "steps"}, ensure_ascii=False, indent=1))


def look_at_tone(wav, hz_about):
    """What the recording of one steady tone holds besides the tone: stretches where it is
    weak, where it is louder than it was, and jumps from one sample to the next. Also every
    place where its pitch moves from one steady value to another."""
    _, left, _ = read_wav(wav)
    n = len(left)
    size = 1 << 16
    env = np.zeros(n)
    freq = np.zeros(n)
    for at in range(0, n, size // 2):
        piece = left[at:at + size]
        if len(piece) < 64:
            break
        spec = np.fft.fft(piece)
        h = np.zeros(len(piece))
        h[0] = 1
        h[1:(len(piece) + 1) // 2] = 2
        z = np.fft.ifft(spec * h)
        e = np.abs(z)
        f = np.concatenate(([0], np.diff(np.unwrap(np.angle(z))))) * RATE / (2 * math.pi)
        # The middle half of each piece is kept (the ends of a piece ring).
        a, b = (0 if at == 0 else size // 4), (len(piece) if at + size >= n else 3 * size // 4)
        env[at + a:at + b] = e[a:b]
        freq[at + a:at + b] = f[a:b]
    sounding = np.flatnonzero(env > 0.02)
    if len(sounding) == 0:
        return {"silent": True}
    level = float(np.median(env[sounding]))

    def runs(mask, least):
        edges = np.flatnonzero(np.diff(np.concatenate(([0], mask.astype(np.int8), [0]))))
        return [(int(s), int(e)) for s, e in zip(edges[0::2], edges[1::2]) if e - s >= least]

    first, last = int(sounding[0]), int(sounding[-1])
    inside = np.zeros(n, dtype=bool)
    inside[first:last] = True
    weak = [{"atS": round(s / RATE, 4), "ms": round((e - s) / 48, 2), "least": round(float(env[s:e].min()) / level, 3)}
            for s, e in runs((env < 0.7 * level) & inside, 12)]
    loud = [{"atS": round(s / RATE, 4), "ms": round((e - s) / 48, 2), "most": round(float(env[s:e].max()) / level, 3)}
            for s, e in runs(env > 1.15 * level, 12)]
    # A steady tone bends by at most level * (2 pi f / rate)^2 from one sample to the next.
    bend = np.abs(np.diff(left, 2))
    bound = level * (2 * math.pi * hz_about * 1.3 / RATE) ** 2
    sharp = np.flatnonzero(bend > 3 * bound + 3 / 32768)
    jumps = []
    for i in sharp:
        if jumps and i - jumps[-1]["frame"] < 480:
            jumps[-1]["size"] = max(jumps[-1]["size"], round(float(bend[i]) / bound, 1))
            jumps[-1]["frames"] = int(i - jumps[-1]["frame"]) + 1
        else:
            jumps.append({"frame": int(i), "atS": round(i / RATE, 4), "size": round(float(bend[i]) / bound, 1), "frames": 1})
    # What a jump is: how far the tone is from where it would have gone on (frames, within
    # half a period), and how many lost blocks of 256 frames would give just that. The
    # recorder (pw-record) loses blocks by itself; the output going wrong may give any step.
    for j in jumps:
        f = j["frame"]
        if f - 5000 < 0 or f + 5000 > n:
            continue
        hz = float(np.median(freq[f - 4800:f - 200]))
        hz_after = float(np.median(freq[f + 200:f + 4800]))
        j["hz"] = [round(hz, 1), round(hz_after, 1)]
        if hz <= 0 or abs(hz_after - hz) > 0.01 * hz:
            continue
        before_n, after_n = np.arange(-4800, -8), np.arange(8, 4800)
        turn = lambda k: np.angle(np.dot(left[f + k], np.exp(-2j * math.pi * hz * k / RATE)))
        period = RATE / hz
        step = (turn(after_n) - turn(before_n)) / (2 * math.pi) * period
        step = (step + period / 2) % period - period / 2
        j["stepFrames"] = round(float(step), 2)
        j["asLostBlocks"] = [k for k in range(1, 9)
                             if abs((256 * k - step + period / 2) % period - period / 2) < 0.7]
    # The pitch every 2 ms (the middle value), and where it changes.
    step = 96
    pitch = np.array([np.median(freq[i:i + step]) for i in range(first, last - step, step)])
    changes = []
    i = 250
    while i < len(pitch) - 250:
        before, after = float(np.median(pitch[i - 250:i - 25])), float(np.median(pitch[i + 25:i + 250]))
        if abs(after - before) > 0.004 * before:
            j = i
            while j < len(pitch) - 250 and abs(pitch[j] - before) <= 0.25 * abs(after - before):
                j += 1
            k = j
            while k < len(pitch) - 250 and abs(pitch[k] - after) > 0.25 * abs(after - before):
                k += 1
            after = float(np.median(pitch[k + 25:k + 250]))
            changes.append({"atS": round((first + j * step) / RATE, 3), "fromHz": round(before, 2), "toHz": round(after, 2),
                            "overMs": (k - j) * 2})
            i = k + 275
        else:
            i += 1
    # The level between the changes of the pitch (the middle value of each stretch).
    edges = [first] + [round(c["atS"] * RATE) for c in changes] + [last]
    levels = [{"fromS": round(a / RATE, 2), "toS": round(b / RATE, 2), "hz": round(float(np.median(freq[a + 2400:b - 2400])), 2),
               "level": round(float(np.median(env[a + 2400:b - 2400])), 4)}
              for a, b in zip(edges, edges[1:]) if b - a > 9600]
    return {"seconds": round(n / RATE, 2), "level": round(level, 4), "soundFromS": round(first / RATE, 3),
            "soundToS": round(last / RATE, 3), "weak": weak, "loud": loud, "jumps": jumps, "pitchChanges": changes,
            "levels": levels}


def run_splice(a):
    env = agent_env()
    result = {"song": a.song, "cases": {}}
    for case in a.cases.split(","):
        out = os.path.join(a.out, case)
        cache = own_cache(os.path.join(out, "cache"))
        r = {"events": []}
        with App(out, song=a.song, song_in_place=True, env={"XDG_CACHE_HOME": cache}) as app:
            state = wait_ready(app)
            clip = vocal_clips(state)[0]
            tempo = state["timeline"]["tempoBpm"]
            if case in ("tempo",):
                app.send({"type": "setClipStretch", "clip": clip["id"], "change": {"kind": "sourceBpm", "bpm": tempo}})
                app.send({"type": "setClipStretch", "clip": clip["id"], "change": {"kind": "follow", "on": True}})
                time.sleep(1.0)
            if case == "loop":
                app.send({"type": "setLoop", "fromBar": 3, "toBar": 5})
                app.send({"type": "locate", "tick": 2 * BAR})
                time.sleep(0.5)
                if not app.snapshot()["song"]["loopOn"]:
                    app.send({"type": "toggleLoop"})
            else:
                app.send({"type": "goToStart"})
            time.sleep(2.5)
            # Two recorders: pw-record itself loses blocks now and then (three blocks are then
            # missing in its file). What went wrong in the output is in both files.
            # They write into memory (the runtime folder) and the files are moved afterwards:
            # with the file on the disk pw-record lost blocks every 5 s (the kernel's writeback).
            memory = os.path.join(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}"), f"k7m4-rec-{os.getpid()}")
            os.makedirs(memory, exist_ok=True)
            rec = Recorder(os.path.join(memory, "out.wav"), env)
            rec_at = time.monotonic()
            rec2 = Recorder(os.path.join(memory, "out2.wav"), env)
            time.sleep(2.0)

            def do(name, command, wait=True, then_s=4.0):
                at = time.monotonic() - rec_at
                if command:
                    app.send(command)
                made = wait_made(app, appear_s=1.0, every=0.02) if wait else None
                r["events"].append({"what": name, "atS": round(at, 3),
                                    "jobsGoneS": None if not made or made["noJobS"] is None else round(at + made["noJobS"], 3),
                                    "mostJobs": made["mostJobs"] if made else None})
                if then_s:
                    time.sleep(then_s)

            def pitch(semitones):
                return {"type": "setClipStretch", "clip": clip["id"], "change": {"kind": "semitones", "semitones": semitones}}

            do("play", {"type": "playStop"}, wait=False, then_s=3.0)
            if case == "pitch":
                do("+2", pitch(2))
                do("undo", {"type": "undo"})
                do("redo", {"type": "redo"})
                do("+5", pitch(5))
                do("0", pitch(0))
            elif case == "tempo":
                do("tempo x1.25", {"type": "setTempo", "bpm": tempo * 1.25})
                do("undo", {"type": "undo"})
                do("redo", {"type": "redo"})
            elif case == "loop":
                # Two bars are a turn: the changes come at other places of the turn each time.
                do("+2", pitch(2), then_s=7.3)
                do("undo", {"type": "undo"}, then_s=6.1)
                do("redo", {"type": "redo"}, then_s=6.7)
                do("+5", pitch(5), then_s=7.9)
            elif case == "jump":
                for n, semitones in enumerate((2, 4, 5, 7)):
                    do(f"+{semitones}", pitch(semitones), then_s=0.15 * n)
                    do("jump", {"type": "locate", "tick": (10 + 7 * n) * BAR}, wait=False, then_s=4.0)
            elif case == "undo":
                do("+2", pitch(2))
                for n in range(6):
                    do("undo", {"type": "undo"}, then_s=0.4 + 0.5 * n)
                    do("redo", {"type": "redo"}, then_s=0.4 + 0.5 * n)
                time.sleep(3)
            do("stop", {"type": "playStop"}, wait=False, then_s=1.5)
            out_top = subprocess.run(["pw-top", "-b", "-n", "2"], env=env, capture_output=True, text=True).stdout
            r["errOfPwTop"] = [[p[-1], int(p[8])] for p in (line.split() for line in out_top.split("NAME")[-1].splitlines())
                               if len(p) >= 10 and p[8].isdigit() and p[-1] in ("meloet", "pw-record")]
            rec.stop()
            rec2.stop()
            for name in ("out.wav", "out2.wav"):
                shutil.move(os.path.join(memory, name), os.path.join(out, name))
            os.rmdir(memory)
            r["missingTotal"] = missing_total(app.stderr.name)
            r["kept"] = kept_sizes(cache)
        r["sound"] = look_at_tone(os.path.join(out, "out.wav"), a.hz)
        other = look_at_tone(os.path.join(out, "out2.wav"), a.hz)
        if not r["sound"].get("silent") and not other.get("silent"):
            # From the first sound on both files hold the same; after a lost block one is
            # three blocks ahead of the other.
            def since(sound):
                return [j["frame"] - round(sound["soundFromS"] * RATE) for j in sound["jumps"]]
            mine, theirs = since(r["sound"]), since(other)
            for j, at in zip(r["sound"]["jumps"], mine):
                j["inBothFiles"] = any(abs(at - t) <= 4096 for t in theirs)
            r["sound"]["jumpsOfTheOtherFile"] = len(theirs)
            r["sound"]["weakOfTheOtherFile"] = other["weak"]
            r["sound"]["pitchChangesOfTheOtherFile"] = other["pitchChanges"]
        result["cases"][case] = r
        print(case, json.dumps(r, ensure_ascii=False)[:3000], flush=True)
    with open(os.path.join(a.out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("what", choices=["position", "prepare", "load", "kept", "splice"])
    p.add_argument("out")
    p.add_argument("--song")
    p.add_argument("--shapes", default="base,t96,t150,ramp,rampdown,p3,c30,t96p3c30,rampp3")
    p.add_argument("--seconds", type=float, default=8)
    p.add_argument("--sink")
    p.add_argument("--source-bpm", type=float, default=96)
    p.add_argument("--tempo", type=float, default=120)
    p.add_argument("--after-s", type=float, default=20)
    p.add_argument("--keys", action="store_true")
    p.add_argument("--keys-s", type=float, default=36)
    p.add_argument("--hz", type=float, default=440)
    p.add_argument("--cases", default="pitch,tempo,loop,jump,undo")
    a = p.parse_args()
    if a.what != "position" and not a.song:
        p.error("--song is needed")
    a.out = os.path.abspath(a.out)
    os.makedirs(a.out, exist_ok=True)
    {"position": run_position, "prepare": run_prepare, "load": run_load, "kept": run_kept,
     "splice": run_splice}[a.what](a)


if __name__ == "__main__":
    main()
