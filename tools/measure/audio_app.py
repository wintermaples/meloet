#!/usr/bin/env python3
"""Vocal clips in the real app (release build): playing many of them, making audio ready,
the same sound after a save, and where a played voice lies.

    python3 -I tools/measure/audio_app.py play OUT --song FILE [--phases head,loop,jumps,starts,steady]
                                                   [--steady-s 60] [--cold] [--keys]
    python3 -I tools/measure/audio_app.py load OUT --song FILE [--keys N]
    python3 -I tools/measure/audio_app.py same OUT --song FILE [--song FILE ...] [--seconds 15]
    python3 -I tools/measure/audio_app.py position OUT [--seconds 8] [--latency] [--sink NAME]
    python3 -I tools/measure/audio_app.py record-save OUT --song FILE [--seconds 15] [--sink NAME]
    python3 -I tools/measure/audio_app.py arrange OUT --song FILE [--splits N]

play: opens the song where it is, waits for its audio, and plays: from the head, around a
  loop (bars 3..5), through ten jumps while playing, five starts from other places, and
  --steady-s seconds on. All the while: the frames of vocal clips that were not read in time
  (the core's stderr line, `EngineMeters::stream_missing`), ERR and B/Q of `pw-top` (read
  only), the bytes meloet read (/proc/<pid>/io), the RSS and CPU of meloet and its
  WebKitWebProcess, the load the transport shows. --cold: first asks the kernel to forget
  the song's audio files (posix_fadvise DONTNEED), so that they are read from the disk.
  --keys: a virtual keyboard plays through the steady part; summarize.py reads the result.
load: empties the kept audio and peaks (XDG_CACHE_HOME of remote.py, meloet/audio and
  meloet/peaks only), opens the song and writes down how long each file was "loading", with
  the RSS. --keys N: N keys are played from the moment the song is open.
same: plays the first --seconds of each song from the head, records the output's monitor
  and says how far the recordings are from the first one, sample by sample.
position: with the output's monitor as the input (as vocal_app.py): records the metronome on
  a vocal track, turns the metronome off, and records the played clip on a second vocal
  track. Where the clicks of both takes lie against the beat. --latency: in a second start,
  「遅れを測る」 three times, 「この数を使う」, and one more take with that number in use.
record-save: opens the song where it is (make a copy first), records the metronome on a new
  vocal track while the song plays, plays --seconds from the head recording the output's
  monitor (OUT/before.wav), and saves. For `same` afterwards.

arrange: opens the song, cuts every clip --splits times (S, not saved), and sends wheel events
  to the arrange view, one a frame: sideways, zooming, then the same with a row 160 high, and
  playing. frame_stats.py reads the frames of each part (the time between frames, busyMs).

Writes OUT/summary.json. Changes nothing on the PC.
"""
import argparse, json, os, re, shutil, subprocess, sys, threading, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
from remote import App, Recorder, agent_env, mem, read_wav, scan_cache, root
import vocal_app

BAR = 4 * 1920
MISSING = re.compile(rb"meloet: (\d+) frames of vocal clips were not read in time \((\d+)\)")


def audio_clips(state):
    return [c["audio"] for t in state["song"]["tracks"] for c in t["clips"] if c.get("audio")]


def all_ready(state):
    clips = audio_clips(state)
    return bool(clips) and all(a["state"] == "ready" for a in clips)


def vocal_tracks(state):
    return [t for t in state["song"]["tracks"] if t["kind"]["kind"] == "vocal"]


def forget(folder):
    """Asks the kernel to drop its copy of every file in the folder."""
    count = 0
    for name in os.listdir(folder):
        fd = os.open(os.path.join(folder, name), os.O_RDONLY)
        try:
            os.fsync(fd)
            os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
            count += 1
        finally:
            os.close(fd)
    return count


def proc_io(pid):
    try:
        with open(f"/proc/{pid}/io") as f:
            return {k: int(v) for k, v in (line.strip().split(": ") for line in f)}
    except (OSError, ValueError):
        return None


def thread_cpu(pid):
    """CPU seconds of each thread of the process by name, the ones that worked at all."""
    out = {}
    tick = os.sysconf("SC_CLK_TCK")
    for task in os.listdir(f"/proc/{pid}/task"):
        try:
            with open(f"/proc/{pid}/task/{task}/stat") as f:
                stat = f.read()
        except OSError:
            continue
        name = stat[stat.index("(") + 1:stat.rindex(")")]
        rest = stat[stat.rindex(")") + 2:].split()
        seconds = (int(rest[11]) + int(rest[12])) / tick
        if seconds >= 0.05:
            out[name] = round(out.get(name, 0) + seconds, 2)
    return out


def missing_total(stderr_path):
    try:
        with open(stderr_path, "rb") as f:
            found = MISSING.findall(f.read())
    except OSError:
        return 0
    return int(found[-1][1]) if found else 0


class Watch:
    """Every 0.5 s: meloet's io, memory and CPU, the missing frames so far. Every 4 s: pw-top
    and the load the transport shows."""

    def __init__(self, app, env):
        self.app, self.env = app, env
        self.fast, self.slow = [], []
        self.stop = threading.Event()
        self.threads = [threading.Thread(target=f, daemon=True) for f in (self.fast_loop, self.slow_loop)]
        for t in self.threads:
            t.start()

    def fast_loop(self):
        while not self.stop.is_set():
            self.fast.append({"t": time.monotonic(), "io": proc_io(self.app.pid), "meloet": mem(self.app.pid),
                              "web": mem(self.app.web_pid()),
                              "missing": missing_total(self.app.stderr.name)})
            self.stop.wait(0.5)

    def top(self):
        out = subprocess.run(["pw-top", "-b", "-n", "2"], env=self.env, capture_output=True, text=True).stdout
        rows = {}
        # The second pass has the numbers of one second; the first is all zeros.
        for line in out.split("NAME")[-1].splitlines():
            p = line.split()
            if len(p) >= 10 and p[0] in ("R", "S", "I", "C") and p[1].isdigit() and p[8].isdigit():
                rows[p[1]] = {"name": p[-1], "state": p[0], "quantum": int(p[2]), "wait": p[4], "busy": p[5],
                              "busyOfQuantum": p[7], "err": int(p[8])}
        return rows

    def slow_loop(self):
        while not self.stop.is_set():
            rows = self.top()
            self.slow.append({"t": time.monotonic(), "rows": rows})
            self.stop.wait(4)

    def end(self):
        self.stop.set()
        for t in self.threads:
            t.join(timeout=8)

    def span(self, from_t, to_t):
        fast = [s for s in self.fast if from_t <= s["t"] <= to_t and s["io"] and s["meloet"] and s["web"]]
        slow = [s for s in self.slow if from_t <= s["t"] <= to_t]
        if len(fast) < 2:
            return None
        first, last = fast[0], fast[-1]
        dt = last["t"] - first["t"]
        out = {"seconds": round(dt, 1), "missingFrames": last["missing"] - first["missing"],
               "readBytesPerS": round((last["io"]["read_bytes"] - first["io"]["read_bytes"]) / dt),
               "rcharPerS": round((last["io"]["rchar"] - first["io"]["rchar"]) / dt),
               "cpuShare": {who: round((last[who]["cpuS"] - first[who]["cpuS"]) / dt, 3) for who in ("meloet", "web")},
               "rssMb": {who: [round(first[who]["rssMb"], 1), round(last[who]["rssMb"], 1),
                               round(max(s[who]["rssMb"] for s in fast), 1)] for who in ("meloet", "web")}}
        err, busy = {}, {}
        for s in slow:
            for node, row in s["rows"].items():
                key = f'{row["name"]} #{node}'
                err.setdefault(key, [row["err"], row["err"]])[1] = row["err"]
                if row["name"] == "meloet" and row["busyOfQuantum"] not in ("---", "???"):
                    busy.setdefault(key, []).append(float(row["busyOfQuantum"]))
        out["err"] = err
        out["meloetBusyOfQuantum"] = {k: {"mean": round(sum(v) / len(v), 4), "max": max(v)} for k, v in busy.items()}
        return out


def quiet_runs(wav, floor=3e-4, least_ms=1.5):
    """[(seconds into the recording, ms long)] of every stretch with next to no sound on
    both sides. The recording starts some tens of ms after pw-record was started."""
    _, left, right = read_wav(wav)
    low = (np.abs(left) < floor) & (np.abs(right) < floor)
    edges = np.flatnonzero(np.diff(np.concatenate(([0], low.astype(np.int8), [0]))))
    return [(round(s / 48000, 3), round((e - s) / 48, 1))
            for s, e in zip(edges[0::2], edges[1::2]) if (e - s) / 48 >= least_ms]


def load_shown(app):
    return app.js("const e = document.querySelector('[data-part=\"load\"]'); return e ? e.textContent.trim() : null")


def wait_ready(app, seconds=300):
    return app.wait(all_ready, seconds, "the audio to be ready", every=0.1)


def run_play(a):
    env = agent_env()
    song_dir = os.path.dirname(os.path.abspath(a.song[0]))
    result = {"song": a.song[0], "phases": {}, "cold": a.cold}
    if a.cold:
        result["forgot"] = forget(os.path.join(song_dir, "音"))
    phases = a.phases.split(",")
    with App(a.out, song=a.song[0], song_in_place=True) as app:
        opened = time.monotonic()
        state = wait_ready(app)
        result["readyAfterOpenS"] = round(time.monotonic() - opened, 2)
        result["audio"] = state["audio"]
        result["ioWhenReady"] = proc_io(app.pid)
        result["tracks"] = len(state["song"]["tracks"])
        result["clips"] = len(audio_clips(state))
        watch = Watch(app, env)
        rec_at = time.monotonic()
        rec = Recorder(os.path.join(a.out, "out.wav"), env)
        time.sleep(3)
        loads = []

        def phase(name, body):
            start = time.monotonic()
            body()
            end = time.monotonic()
            time.sleep(0.6)
            result["phases"][name] = {"from": start, "to": end}

        def stop():
            app.send({"type": "playStop"})
            app.wait(lambda s: s["transport"]["state"] == "stopped", 10, "the stop")

        def play():
            app.send({"type": "playStop"})

        def head():
            app.send({"type": "goToStart"})
            time.sleep(1.0)
            play()
            time.sleep(15)
            loads.append(load_shown(app))
            stop()

        def loop():
            app.send({"type": "setLoop", "fromBar": 3, "toBar": 5})
            app.send({"type": "locate", "tick": 2 * BAR})
            time.sleep(1.0)
            if not app.snapshot()["song"]["loopOn"]:
                app.send({"type": "toggleLoop"})
            time.sleep(0.5)
            play()
            time.sleep(21)      # two bars at 96 are 5 s: four turns
            stop()
            app.send({"type": "toggleLoop"})

        def jumps():
            app.send({"type": "goToStart"})
            time.sleep(1.0)
            play()
            for i in range(10):
                time.sleep(1.5)
                app.send({"type": "locate", "tick": ((i * 37) % 100 + 5) * BAR})
            time.sleep(1.5)
            stop()

        def starts():
            for i in range(5):
                app.send({"type": "locate", "tick": ((i * 23) % 90 + 10) * BAR})
                time.sleep(0.7)
                play()
                time.sleep(2.5)
                stop()

        def steady():
            app.send({"type": "goToStart"})
            time.sleep(1.0)
            play()
            keys = None
            if a.keys:
                count = int((a.steady_s - 6) * 1000 / 257)
                keys = app.play_keys("--count", count, "--interval-ms", 257, "--hold-ms", 120, "--velocity", 25,
                                     "--lead-ms", 500, wait=False, log=os.path.join(a.out, "raw", "vkeys.json"))
            until = time.monotonic() + a.steady_s
            while time.monotonic() < until:
                time.sleep(5)
                loads.append(load_shown(app))
            if keys:
                keys.wait()
            stop()

        bodies = {"head": head, "loop": loop, "jumps": jumps, "starts": starts, "steady": steady}
        for name in phases:
            phase(name, bodies[name])
        time.sleep(2.5)
        watch.end()
        rec.stop()
        quiet = quiet_runs(os.path.join(a.out, "out.wav"))
        for name, p in result["phases"].items():
            inside = [(round(t - (p["from"] - rec_at), 3), ms) for t, ms in quiet
                      if p["from"] - rec_at - 0.2 <= t <= p["to"] - rec_at + 0.2]
            result.setdefault("quietRuns", {})[name] = inside
        result["loadShown"] = loads
        result["statusAtTheEnd"] = app.status()
        for name, p in result["phases"].items():
            result["phases"][name] = watch.span(p["from"], p["to"] + 0.6)
        result["whole"] = watch.span(0, time.monotonic())
        result["ioAtTheEnd"] = proc_io(app.pid)
        with open(app.stderr.name, "rb") as f:
            result["missingLines"] = [m.decode() for m in re.findall(rb"meloet: \d+ frames of vocal[^\n]*", f.read())][:200]
    finish(a.out, result)
    if a.keys:
        subprocess.run([sys.executable, "-I", os.path.join(root, "tools", "measure", "summarize.py"),
                        os.path.join(a.out, "raw"), "--vkeys", os.path.join(a.out, "raw", "vkeys.json"),
                        "--out", os.path.join(a.out, "keys"), "--label", f"audio_app.py play, {a.song[0]}"])


def run_load(a):
    kept = os.path.join(scan_cache, "meloet")
    for name in ("audio", "peaks"):
        shutil.rmtree(os.path.join(kept, name), ignore_errors=True)
    song_dir = os.path.dirname(os.path.abspath(a.song[0]))
    result = {"song": a.song[0], "forgot": forget(os.path.join(song_dir, "音"))}
    samples = []
    with App(a.out, song=a.song[0], song_in_place=True, settle_s=0.0) as app:
        opened, opened_wall = time.monotonic(), time.time()
        keys = None
        if a.keys:
            keys = app.play_keys("--count", a.keys, "--interval-ms", 257, "--hold-ms", 120, "--velocity", 25,
                                 "--lead-ms", 300, wait=False, log=os.path.join(a.out, "raw", "vkeys.json"))
        until = opened + 600
        while time.monotonic() < until:
            state = app.snapshot()
            samples.append({"t": round(time.monotonic() - opened, 3),
                            "states": {x["file"]: x["state"] for x in audio_clips(state)},
                            "jobs": state.get("audioWork", {}).get("jobs", []),
                            "meloet": mem(app.pid), "web": mem(app.web_pid())})
            if all_ready(state):
                break
            time.sleep(0.03)
        result["allReadyS"] = samples[-1]["t"]
        result["audioFiles"] = audio_clips(app.snapshot())
        # The peaks are made after the audio: the kept folder says when.
        want = len({x["audio"] for x in result["audioFiles"]})
        peaks = os.path.join(kept, "peaks")
        while time.monotonic() < until:
            have = [n for n in (os.listdir(peaks) if os.path.isdir(peaks) else []) if n.endswith(".peaks")]
            if len(have) >= want:
                break
            samples.append({"t": round(time.monotonic() - opened, 3), "states": {}, "jobs": [],
                            "meloet": mem(app.pid), "web": mem(app.web_pid())})
            time.sleep(0.03)
        result["peaksThereS"] = round(time.monotonic() - opened, 3)
        if keys:
            keys.wait()
            time.sleep(3)
        result["rssMb"] = {who: {"first": samples[0][who]["rssMb"], "most": max(s[who]["rssMb"] for s in samples),
                                 "last": samples[-1][who]["rssMb"]} for who in ("meloet", "web")}
        result["cpuS"] = {who: round(samples[-1][who]["cpuS"] - samples[0][who]["cpuS"], 2) for who in ("meloet", "web")}
        spans = {}
        for s in samples:
            for name, st in s["states"].items():
                if st == "loading":
                    spans.setdefault(name, [s["t"], s["t"]])[1] = s["t"]
        result["loadingSpans"] = {name: {"from": v[0], "to": v[1], "seconds": round(v[1] - v[0], 2)}
                                  for name, v in spans.items()}
        # The files were made by one thread (meloet-audio), and it may have begun before the
        # first look here: its CPU time is the work, the files' times say when each was whole.
        result["keptAfterLaunchS"] = {
            name: sorted((round(os.stat(os.path.join(kept, name, n)).st_mtime - app.launched_ms / 1000, 2), n,
                          os.path.getsize(os.path.join(kept, name, n)))
                         for n in os.listdir(os.path.join(kept, name)))
            for name in ("audio", "peaks") if os.path.isdir(os.path.join(kept, name))}
        result["firstLookAfterLaunchS"] = round(opened_wall - app.launched_ms / 1000, 2)
        result["threadCpuS"] = thread_cpu(app.pid)
    with open(os.path.join(a.out, "raw", "samples.json"), "w") as f:
        json.dump(samples, f)
    finish(a.out, result)
    if a.keys:
        subprocess.run([sys.executable, "-I", os.path.join(root, "tools", "measure", "summarize.py"),
                        os.path.join(a.out, "raw"), "--vkeys", os.path.join(a.out, "raw", "vkeys.json"),
                        "--out", os.path.join(a.out, "keys"), "--label", f"audio_app.py load, {a.song[0]}"])


def play_and_record(app, wav, seconds, env):
    app.send({"type": "goToStart"})
    time.sleep(1.5)
    rec = Recorder(wav, env)
    time.sleep(1.5)
    app.send({"type": "playStop"})
    time.sleep(seconds)
    app.send({"type": "playStop"})
    app.wait(lambda s: s["transport"]["state"] == "stopped", 10, "the stop")
    time.sleep(1.0)
    out = subprocess.run(["pw-top", "-b", "-n", "1"], env=env, capture_output=True, text=True).stdout
    errs = {p[-1]: int(p[8]) for p in (line.split() for line in out.splitlines())
            if len(p) >= 10 and p[8].isdigit() and p[-1] in ("meloet", "pw-record")}
    rec.stop()
    with open(wav + ".err.json", "w") as f:
        json.dump(errs, f)
    return wav


def first_sound(x, floor=2e-4):
    loud = np.flatnonzero(np.abs(x) > floor)
    return int(loud[0]) if len(loud) else None


def compare(first, other, seconds):
    """How far `other` is from `first` once both start at their first sound, piece by piece
    (a quarter of a second each). pw-record itself loses blocks now and then (its ERR in
    pw-top): what it recorded after such a place is the same sound some blocks earlier in
    the file. So each piece is looked for at the place of the piece before, and when it is
    not the same there, at every place up to 4096 frames earlier or later."""
    _, l0, _ = read_wav(first)
    _, l1, _ = read_wav(other)
    a0, a1 = first_sound(l0), first_sound(l1)
    if a0 is None or a1 is None:
        return {"silent": True}
    piece, lag, lags, exact, pieces, worst = 12000, 0, [], 0, 0, 0.0
    for s in range(0, int((seconds - 1.5) * 48000) - piece, piece):
        x = l0[a0 + s:a0 + s + piece]
        if len(x) < piece:
            break
        pieces += 1
        found = None
        for d in [lag] + [d for d in range(-4096, 4097) if d != lag]:
            b = a1 + s + d
            if b >= 0 and np.array_equal(x, l1[b:b + piece]):
                found = d
                break
        if found is None:
            # A piece with the recorder's cut inside it is the same on neither side.
            y = l1[a1 + s + lag:a1 + s + lag + piece]
            if len(y) == piece:
                worst = max(worst, float(np.abs(x - y).max()))
            continue
        exact += 1
        if not lags or lags[-1][1] != found:
            lags.append((round(s / 48000, 2), found))
        lag = found
    return {"pieces": pieces, "sameToTheSample": exact, "lagsFromSecond": lags,
            "worstOfTheOthers": round(worst, 4)}


def run_same(a):
    env = agent_env()
    wavs, result = [], {"songs": a.song, "seconds": a.seconds, "against": [], "extra": a.wav or []}
    for i, song in enumerate(a.song):
        out = os.path.join(a.out, f"run{i}")
        with App(out, song=song, song_in_place=True) as app:
            state = wait_ready(app)
            result.setdefault("clips", []).append([(x["file"], x["state"]) for x in audio_clips(state)])
            time.sleep(1)
            wavs.append(play_and_record(app, os.path.join(a.out, f"song{i}.wav"), a.seconds, env))
    wavs = (a.wav or []) + wavs
    result["wavs"] = wavs
    for other in wavs[1:]:
        result["against"].append(compare(wavs[0], other, a.seconds))
    result["errOfPwTop"] = [json.load(open(w + ".err.json")) if os.path.exists(w + ".err.json") else None
                            for w in wavs]
    finish(a.out, result)


def run_record_save(a):
    env = agent_env()
    sink = a.sink or vocal_app.default_sink(env)
    result = {"song": a.song[0], "sink": sink}
    with App(a.out, song=a.song[0], song_in_place=True, env={"MELOET_INPUT_SINK_MONITOR": sink}) as app:
        wait_ready(app)
        app.send({"type": "goToStart"})
        app.send({"type": "toggleMetronome"})
        app.send({"type": "addVocalTrack"})
        app.wait(lambda s: s["inputs"]["open"] and vocal_tracks(s)[-1]["kind"]["armed"]
                 and vocal_tracks(s)[-1]["kind"]["inputReady"], 15, "the input to open")
        time.sleep(1.5)
        app.send({"type": "record"})
        app.wait(lambda s: s["transport"]["state"] == "recording" and s["transport"]["recordingVocals"], 20,
                 "the recording to roll")
        time.sleep(6)
        app.send({"type": "playStop"})
        app.wait(lambda s: not s["transport"]["recordingVocals"] and s["transport"]["state"] == "stopped", 30,
                 "the voice to be in the song")
        app.send({"type": "toggleMetronome"})
        time.sleep(1)
        state = wait_ready(app)
        result["clipsBefore"] = [(x["file"], x["state"], x["frames"]) for x in audio_clips(state)]
        play_and_record(app, os.path.join(a.out, "before.wav"), a.seconds, env)
        app.send({"type": "saveSong"})
        state = app.wait(lambda s: s["song"]["file"] == "saved", 30, "the save")
        result["statusAfterSave"] = app.status()
        time.sleep(1)
    folder = os.path.join(os.path.dirname(os.path.abspath(a.song[0])), "音")
    result["audioFolder"] = sorted((n, os.path.getsize(os.path.join(folder, n))) for n in os.listdir(folder))
    finish(a.out, result)


def one_take(app, seconds):
    app.send({"type": "goToStart"})
    time.sleep(0.5)
    app.send({"type": "record"})
    app.wait(lambda s: s["transport"]["state"] == "recording" and s["transport"]["recordingVocals"], 20,
             "the recording to roll")
    time.sleep(seconds)
    app.send({"type": "playStop"})
    app.wait(lambda s: not s["transport"]["recordingVocals"] and s["transport"]["state"] == "stopped", 30,
             "the voice to be in the song")
    time.sleep(0.5)


def takes_of(state_dir, count):
    """The takes of the song not saved yet, once it has `count` of them."""
    until = time.time() + 15
    while True:
        song = vocal_app.read_song(state_dir)
        if song and len(song.get("audioTakes", [])) >= count:
            break
        if time.time() > until:
            return None
        time.sleep(0.2)
    return [{k: t[k] for k in ("file", "origin", "firstFrame", "frames", "clicks", "offMean", "offMax",
                               "offFirstTenth", "offLastTenth", "wrongAccents", "weakest")}
            for t in vocal_app.judge(state_dir, song)]


def run_position(a):
    env = agent_env()
    sink = a.sink or vocal_app.default_sink(env)
    result = {"sink": sink, "seconds": a.seconds}
    for d in ("state", "config", "state2", "config2"):
        shutil.rmtree(os.path.join(a.out, d), ignore_errors=True)
    state_dir = os.path.join(a.out, "state")
    with vocal_app.start_app(a.out, sink) as app:
        state = vocal_app.arm(app, sink)
        result["audio"] = state["audio"]
        result["inputs"] = state["inputs"]
        one_take(app, a.seconds)
        # The second take hears only the clip: the metronome is off, the first track plays.
        app.send({"type": "toggleMetronome"})
        app.send({"type": "addVocalTrack"})
        app.wait(lambda s: len(vocal_tracks(s)) == 2 and vocal_tracks(s)[1]["kind"]["armed"]
                 and vocal_tracks(s)[1]["kind"]["inputReady"], 15, "the second vocal track")
        result["armed"] = [t["kind"]["armed"] for t in vocal_tracks(app.snapshot())]
        time.sleep(1.5)
        one_take(app, a.seconds)
        time.sleep(3)
        result["takes"] = takes_of(state_dir, 2)
        result["status"] = app.status()
    if a.latency:
        out2 = os.path.join(a.out, "latency")
        os.makedirs(out2, exist_ok=True)
        with vocal_app.start_app(out2, sink) as app:
            vocal_app.arm(app, sink)
            app.send({"type": "openLatencyMeasure"})
            runs = []
            for _ in range(3):
                app.send({"type": "startLatencyMeasure"})
                time.sleep(0.3)
                state = app.wait(lambda s: s["workspace"]["latencyDialog"]["measure"]["state"] in ("done", "failed"),
                                 30, "the measuring")
                runs.append({"dialog": state["workspace"]["latencyDialog"], "latency": state["audio"]["latency"]})
                time.sleep(0.5)
            app.send({"type": "useMeasuredLatency"})
            time.sleep(1.0)
            state = app.snapshot()
            result["latency"] = {"runs": runs, "after": state["audio"]["latency"]}
            try:
                with open(os.path.join(out2, "config", "settings.json"), encoding="utf-8") as f:
                    result["latency"]["settings"] = json.load(f).get("sound", {}).get("measured_latency")
            except (OSError, ValueError) as e:
                result["latency"]["settings"] = str(e)
            if state["workspace"].get("latencyDialog"):
                app.send({"type": "closeLatencyMeasure"})
            time.sleep(0.5)
            one_take(app, a.seconds)
            time.sleep(3)
            result["latency"]["takes"] = takes_of(os.path.join(out2, "state"), 1)
    finish(a.out, result)


WHEEL = """
const el = document.querySelector('.lanes-view'), r = el.getBoundingClientRect();
const w = (dy, more) => el.dispatchEvent(new WheelEvent('wheel', {deltaY: dy, deltaMode: 0,
  clientX: r.left + r.width / 2, clientY: r.top + 40, bubbles: true, cancelable: true, ...more}));
const t0 = performance.now();
for (const [dy, more, n] of %s) for (let i = 0; i < n; i++) { w(dy, more); await k.frame(); }
return [t0, performance.now()];
"""


def run_arrange(a):
    """Frames of the arrange view with many vocal clips: sideways, zooming, a 160 row, playing."""
    result = {"song": a.song[0], "splits": a.splits, "spans": {}}
    with App(a.out, song=a.song[0], song_in_place=True) as app:
        # A song with no vocal clip is measured too, to compare.
        state = wait_ready(app) if audio_clips(app.snapshot()) else app.snapshot()
        for i in range(a.splits):
            app.send({"type": "locate", "tick": (i + 1) * (110 // max(1, a.splits)) * BAR})
            app.send({"type": "selectAllClips"})
            app.send({"type": "splitClips"})
            time.sleep(0.15)
        app.send({"type": "selectClip", "clip": None})
        app.send({"type": "goToStart"})
        time.sleep(1.5)
        state = app.snapshot()
        result["clips"] = len(audio_clips(state))
        vocals = [t["id"] for t in vocal_tracks(state)]
        result["memoryBefore"] = app.memory()

        def page_now():
            return app.js("return performance.now()")

        def span(name, body):
            start = page_now()
            got = body()
            result["spans"][name] = got if got else [start, page_now()]
            time.sleep(0.4)

        def wheel(steps):
            return lambda: app.js(WHEEL % json.dumps(steps), timeout=60)

        def tall():
            for track in vocals[:8]:
                app.send({"type": "setTallTrack", "track": track})
                time.sleep(0.25)

        def play():
            app.send({"type": "playStop"})
            time.sleep(6)
            app.send({"type": "playStop"})

        span("still", lambda: time.sleep(3))
        span("sideways", wheel([[120, {"shiftKey": True}, 120], [-120, {"shiftKey": True}, 120]]))
        span("zoom", wheel([[-100, {"ctrlKey": True}, 40], [100, {"ctrlKey": True}, 40]]))
        span("tallRows", tall)
        span("sidewaysTall", wheel([[120, {"shiftKey": True}, 120], [-120, {"shiftKey": True}, 120]]))
        span("zoomTall", wheel([[-100, {"ctrlKey": True}, 40], [100, {"ctrlKey": True}, 40]]))
        span("playingTall", play)
        app.send({"type": "setTallTrack", "track": None})
        time.sleep(0.5)
        span("playing", play)
        time.sleep(1)
        result["memoryAfter"] = app.memory()
        result["tallTrack"] = app.snapshot().get("arrange", {}).get("tallTrack")
    result["frames"] = {}
    for name, (start, end) in result["spans"].items():
        out = subprocess.run([sys.executable, "-I", os.path.join(root, "tools", "measure", "frame_stats.py"),
                              os.path.join(a.out, "raw"), str(start / 1000), str(end / 1000)],
                             capture_output=True, text=True).stdout
        result["frames"][name] = json.loads(out) if out.strip() else None
    finish(a.out, result)


def finish(out, result):
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)
    print(json.dumps(result, ensure_ascii=False, indent=1)[:6000])


def main():
    p = argparse.ArgumentParser()
    p.add_argument("what", choices=["play", "load", "same", "position", "record-save", "arrange"])
    p.add_argument("out")
    p.add_argument("--song", action="append")
    p.add_argument("--wav", action="append")
    p.add_argument("--phases", default="head,loop,jumps,starts,steady")
    p.add_argument("--steady-s", type=float, default=60)
    p.add_argument("--seconds", type=float)
    p.add_argument("--cold", action="store_true")
    p.add_argument("--keys", nargs="?", const=1, type=int)
    p.add_argument("--latency", action="store_true")
    p.add_argument("--sink")
    p.add_argument("--splits", type=int, default=0)
    a = p.parse_args()
    if a.what != "position" and not a.song:
        p.error("--song is needed")
    if a.seconds is None:
        a.seconds = 8 if a.what == "position" else 15
    os.makedirs(a.out, exist_ok=True)
    {"play": run_play, "load": run_load, "same": run_same, "position": run_position,
     "record-save": run_record_save, "arrange": run_arrange}[a.what](a)


if __name__ == "__main__":
    main()
