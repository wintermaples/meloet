#!/usr/bin/env python3
"""Records a voice in the real app (release build) with no microphone: the monitor of the
output is the input (MELOET_INPUT_SINK_MONITOR), so the vocal track records the metronome.

    python3 -I tools/measure/vocal_app.py long OUT [--seconds 600] [--keys] [--tone] [--sink NAME]
    python3 -I tools/measure/vocal_app.py kill OUT [--after-ms 6000] [--settle-s 4] [--sink NAME]
    python3 -I tools/measure/vocal_app.py look [STATE_DIR] [--song FILE.meloet]

long: adds a vocal track, records for --seconds at 120 with the metronome on, stops, and looks
  at the file: where every click lies against the beat the take gives it (a matched filter, so
  other sound may be there too), how far the disk writer fell behind (the file's size, read
  every 5 ms, against a straight line), PipeWire's error counts (ERR of `pw-top`, read only)
  and the RSS of meloet and its WebKitWebProcess every 10 s. --keys: a virtual keyboard plays
  all the while, so keys and voice are recorded together. --tone: a very quiet tone (220 Hz,
  -50 dBFS) is played into the output all the while; a stretch of exact zeros in the file is
  then a hole that was filled with silence.
kill: records, ends meloet with SIGKILL --after-ms after R was sent (the count-in is 2 s),
  starts it again on the same folders and looks at what came back. --settle-s: the time
  between the last change to the song (the tempo, the vocal track) and R; the recovery file
  is written 2 s after a change, and not while recording.

look: starts nothing. Reads the vocal takes of the song not saved yet (STATE_DIR/recovery.json;
  STATE_DIR is ~/.local/state/meloet unless given) or of --song, finds every sharp sound in
  the recorded files (a clap, a click) and prints how far each lies from the nearest beat, in
  ms. For takes recorded at one tempo.

Writes OUT/summary.md and OUT/summary.json; the state folder (the wav file) stays in OUT/state.
The sink is the PC's default one unless --sink names it. Changes nothing on the PC.
"""
import argparse, json, math, os, shutil, signal, struct, subprocess, sys, threading, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
from remote import App, agent_env, mem

RATE = 48000
TICKS = 1920
BPM = 120
BEAT_FRAMES = RATE * 60 // BPM


def default_sink(env):
    out = subprocess.run(["pw-metadata", "-n", "default", "0", "default.audio.sink"], env=env,
                         capture_output=True, text=True).stdout
    for part in out.split("'"):
        if part.startswith("{"):
            return json.loads(part)["name"]
    raise SystemExit("vocal_app.py: no default sink found; name one with --sink")


def click(bar_head):
    """The metronome's click (crates/meloet-engine/src/transport.rs, `Click`)."""
    hz, level = (1760.0, 0.17) if bar_head else (1046.5, 0.13)
    total = int(0.035 * RATE)
    n = np.arange(total)
    fade = np.minimum((total - n) / 48.0, 1.0)
    return level * np.exp(-n / (0.008 * RATE)) * fade * np.cos(2 * math.pi * hz * n / RATE)


def read_wav(path):
    """(channels, bits, first channel as floats). The lengths in the head are not trusted."""
    with open(path, "rb") as f:
        data = f.read()
    channels, = struct.unpack_from("<H", data, 22)
    bits, = struct.unpack_from("<H", data, 34)
    width = bits // 8
    body = np.frombuffer(data, dtype=np.uint8, offset=44)
    body = body[:len(body) // (width * channels) * width * channels].reshape(-1, channels, width)[:, 0, :]
    if bits == 32:
        return channels, bits, body.copy().view("<f4")[:, 0].astype(np.float64), data[:44]
    if bits == 24:
        v = body[:, 0].astype(np.int32) | (body[:, 1].astype(np.int32) << 8) | (body[:, 2].astype(np.int32) << 16)
        v = np.where(v >= 1 << 23, v - (1 << 24), v)
        return channels, bits, v / float(1 << 23), data[:44]
    v = body.copy().view("<i2")[:, 0]
    return channels, bits, v / 32768.0, data[:44]


def vocal_takes(song):
    """[(the take's tick 0 in the song, firstFrame, frames, file)] of the vocal clips."""
    takes = {t["id"]: t for t in song.get("audioTakes", [])}
    files = {a["id"]: a["file"] for a in song["song"].get("audio", [])}
    out = []
    for track in song["song"]["tracks"]:
        if track.get("kind") != "vocal":
            continue
        for clip in track["clips"]:
            for in_clip in clip["takes"]:
                t = takes[in_clip["take"]]
                out.append({"origin": clip["start"] + in_clip["offset"], "clipStart": clip["start"],
                            "clipEnd": clip["end"], "firstFrame": t["firstFrame"], "frames": t["frames"],
                            "file": files[t["audio"]]})
    return out


def clicks_of(samples, take, window=1200):
    """Every click the take should hold: (beat, frames off, the louder kind was the right one)."""
    kinds = {True: click(True), False: click(False)}
    norm = {k: float(np.dot(v, v)) for k, v in kinds.items()}
    found = []
    first_beat = math.ceil(take["origin"] / TICKS)
    beat = first_beat
    while True:
        expected = take["firstFrame"] + (beat * TICKS - take["origin"]) * BEAT_FRAMES / TICKS
        e = int(round(expected))
        if e + 1680 + window > min(take["firstFrame"] + take["frames"], len(samples)):
            break
        lo = e - window
        if lo >= 0:
            seg = samples[lo:e + window + 1680]
            best = {}
            for k, shape in kinds.items():
                c = np.correlate(seg, shape, "valid") / norm[k]
                i = int(np.argmax(c))
                best[k] = (float(c[i]), i - window)
            kind = max(best, key=lambda k: best[k][0])
            found.append({"beat": beat, "off": best[kind][1] + (e - expected), "strength": round(best[kind][0], 3),
                          "accentRight": kind == (beat % 4 == 0)})
        beat += 1
    return found


def zero_runs(samples, least=32):
    """Stretches of at least `least` exact zeros: [(first frame, frames)]."""
    zero = np.concatenate(([False], samples == 0.0, [False]))
    edges = np.flatnonzero(zero[1:] != zero[:-1])
    starts, ends = edges[::2], edges[1::2]
    return [(int(s), int(e - s)) for s, e in zip(starts, ends) if e - s >= least]


class Watch:
    """The size of the wav file every 5 ms, and every 10 s `pw-top` and the RSS."""

    def __init__(self, app, audio_dir, env):
        self.app, self.audio_dir, self.env = app, audio_dir, env
        self.sizes, self.tops, self.memory = [], [], []
        self.stop = threading.Event()
        self.threads = [threading.Thread(target=f, daemon=True) for f in (self.size_loop, self.slow_loop)]
        for t in self.threads:
            t.start()

    def size_loop(self):
        path = None
        while not self.stop.is_set():
            if path is None:
                try:
                    names = [n for n in os.listdir(self.audio_dir) if n.endswith(".wav")]
                except OSError:
                    names = []
                if names:
                    path = os.path.join(self.audio_dir, names[0])
            if path:
                try:
                    self.sizes.append((time.monotonic(), os.stat(path).st_size))
                except OSError:
                    pass
            time.sleep(0.005)

    def top(self):
        out = subprocess.run(["pw-top", "-b", "-n", "2"], env=self.env, capture_output=True, text=True).stdout
        rows = {}
        for line in out.splitlines():
            p = line.split()
            if len(p) >= 10 and p[0] in ("R", "S", "I", "C") and p[1].isdigit() and p[8].isdigit():
                name = " ".join(p[9:]).lstrip("+ ").split(" ", 3)[-1] if p[9][0] in "SFU" and "LE" in p[9] else p[-1]
                rows[p[1]] = {"name": name, "state": p[0], "quantum": int(p[2]), "err": int(p[8])}
        return rows

    def slow_loop(self):
        while not self.stop.is_set():
            self.tops.append({"t": time.monotonic(), "rows": self.top()})
            self.memory.append({"t": time.monotonic(), **self.app.memory()})
            self.stop.wait(10)

    def end(self):
        self.stop.set()
        for t in self.threads:
            t.join(timeout=5)

    def writer_lag(self, from_t, to_t):
        """How far the file's growth fell behind a straight line, in ms: the size is frames
        on the disk, the line is the input's clock."""
        pts = [(t, s) for t, s in self.sizes if from_t <= t <= to_t]
        if len(pts) < 100:
            return None
        t = np.array([p[0] for p in pts]); s = np.array([p[1] for p in pts], dtype=np.float64)
        slope, at0 = np.polyfit(t, s, 1)
        behind = (slope * t + at0 - s) / slope * 1000.0
        behind -= behind.min()
        grew = np.flatnonzero(np.diff(s) > 0)
        stalls = np.diff(t[grew + 1]) * 1000.0 if len(grew) > 1 else np.array([0.0])
        return {"samples": len(pts), "bytesPerSecond": round(float(slope), 1),
                "behindMs": {"p50": round(float(np.percentile(behind, 50)), 2),
                             "p99": round(float(np.percentile(behind, 99)), 2),
                             "max": round(float(behind.max()), 2)},
                "longestWithoutGrowthMs": round(float(stalls.max()), 2)}

    def errs(self):
        """ERR of every node seen, first and last."""
        out = {}
        for sample in self.tops:
            for node, row in sample["rows"].items():
                key = f'{row["name"]} #{node}'
                out.setdefault(key, [row["err"], row["err"], row["quantum"]])
                out[key][1] = row["err"]
                out[key][2] = max(out[key][2], row["quantum"])
        return out


def vocal_track(state):
    return next((t for t in state["song"]["tracks"] if t["kind"]["kind"] == "vocal"), None)


def start_app(out, sink, env_more=None):
    env = {"MELOET_INPUT_SINK_MONITOR": sink, "MELOET_STATE_DIR": os.path.join(out, "state"),
           "MELOET_CONFIG_DIR": os.path.join(out, "config")}
    env.update(env_more or {})
    for d in ("state", "config"):
        os.makedirs(os.path.join(out, d), exist_ok=True)
    return App(out, env=env)


def arm(app, sink):
    state = app.snapshot()
    if not state["transport"]["metronome"]:
        app.send({"type": "toggleMetronome"})
    app.send({"type": "setTempo", "bpm": BPM})
    if vocal_track(state) is None:
        app.send({"type": "addVocalTrack"})
    app.wait(lambda s: s["inputs"]["open"] and vocal_track(s)["kind"]["armed"] and vocal_track(s)["kind"]["inputReady"],
             15, "the input to open")
    time.sleep(1.5)
    return app.snapshot()


def read_song(state_dir, wait_s=15):
    path = os.path.join(state_dir, "recovery.json")
    until = time.time() + wait_s
    while True:
        try:
            with open(path, encoding="utf-8") as f:
                song = json.load(f)["song"]
            if song.get("audioTakes"):
                return song
        except (OSError, ValueError, KeyError):
            pass
        if time.time() > until:
            return None
        time.sleep(0.1)


def judge(state_dir, song):
    """What the file and the takes say: one entry for every take."""
    out = []
    for take in vocal_takes(song):
        channels, bits, samples, head = read_wav(os.path.join(state_dir, "work", "音", take["file"]))
        found = clicks_of(samples, take)
        offs = np.array([c["off"] for c in found]) if found else np.array([0.0])
        tenth = max(1, len(found) // 10)
        out.append({**take, "channels": channels, "bits": bits, "fileFrames": len(samples),
                    "dataLengthInHead": struct.unpack_from("<I", head, 40)[0],
                    "clicks": len(found), "offMax": float(np.abs(offs).max()), "offMean": float(offs.mean()),
                    "offFirstTenth": float(offs[:tenth].mean()), "offLastTenth": float(offs[-tenth:].mean()),
                    "notOnTheFrame": [c for c in found if abs(c["off"]) >= 1.0][:20],
                    "notOnTheFrameCount": sum(abs(c["off"]) >= 1.0 for c in found),
                    "wrongAccents": sum(not c["accentRight"] for c in found),
                    "weakest": min((c["strength"] for c in found), default=None),
                    "zeroRuns": zero_runs(samples[take["firstFrame"]:take["firstFrame"] + take["frames"]])})
    return out


def write_tone(path, seconds):
    n = np.arange(int(seconds * RATE))
    wave = np.round(10 ** (-50 / 20) * 32767 * np.sin(2 * math.pi * 220 * n / RATE)).astype("<i2")
    with open(path, "wb") as f:
        f.write(b"RIFF" + struct.pack("<I", 36 + wave.nbytes) + b"WAVEfmt " +
                struct.pack("<IHHIIHH", 16, 1, 1, RATE, RATE * 2, 2, 16) + b"data" + struct.pack("<I", wave.nbytes))
        f.write(wave.tobytes())


def run_long(a):
    env = agent_env()
    sink = a.sink or default_sink(env)
    shutil.rmtree(os.path.join(a.out, "state"), ignore_errors=True)
    shutil.rmtree(os.path.join(a.out, "config"), ignore_errors=True)
    state_dir = os.path.join(a.out, "state")
    result = {"sink": sink, "seconds": a.seconds, "keys": a.keys, "tone": a.tone}
    tone = None
    with start_app(a.out, sink) as app:
        state = arm(app, sink)
        result["audio"] = state["audio"]
        result["input"] = state["inputs"]
        watch = Watch(app, os.path.join(state_dir, "work", "音"), env)
        if a.tone:
            write_tone(os.path.join(a.out, "tone.wav"), a.seconds + 30)
            tone = subprocess.Popen(["pw-play", "--target", sink, "-P", "node.name=meloet-measure-tone",
                                     os.path.join(a.out, "tone.wav")], env=env,
                                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            time.sleep(1.5)
        time.sleep(1)
        app.send({"type": "record"})
        app.wait(lambda s: s["transport"]["state"] == "recording" and s["transport"]["recordingVocals"], 20,
                 "the recording to roll")
        rolled = time.monotonic()
        keys = None
        if a.keys:
            count = int((a.seconds - 4) * 1000 / 250)
            keys = app.play_keys("--count", count, "--interval-ms", 250, "--hold-ms", 120, "--velocity", 20,
                                 "--notes", "60,62,64,65,67,69,71,72", wait=False,
                                 log=os.path.join(a.out, "raw", "vkeys.json"))
        while time.monotonic() - rolled < a.seconds:
            time.sleep(0.2)
            if not app.alive():
                raise SystemExit("vocal_app.py: meloet ended while recording")
        stopping = time.monotonic()
        result["statusWhileRecording"] = app.status()
        app.send({"type": "playStop"})
        after = app.wait(lambda s: not s["transport"]["recordingVocals"] and s["transport"]["state"] == "stopped"
                         and vocal_track(s)["clips"], 30, "the voice to be in the song")
        result["stopTookMs"] = round((time.monotonic() - stopping) * 1000)
        if keys:
            keys.wait()
        time.sleep(1)
        watch.end()
        if tone:
            tone.terminate()
            tone.wait()
        result["statusAfter"] = app.status()
        result["history"] = after.get("history")
        result["tracks"] = [{"name": t["name"], "kind": t["kind"]["kind"],
                             "clips": [{k: c[k] for k in ("start", "end", "takes", "noteCount", "vocal")}
                                       for c in t["clips"]]} for t in after["song"]["tracks"]]
        result["writer"] = watch.writer_lag(rolled + 1, stopping - 0.5)
        result["err"] = watch.errs()
        mems = [m for m in watch.memory if m["meloet"] and m["web"]]
        result["rssMb"] = {who: {"first": mems[0][who]["rssMb"], "last": mems[-1][who]["rssMb"],
                                 "most": max(m[who]["rssMb"] for m in mems)} for who in ("meloet", "web")}
        result["rssEvery10s"] = [[round(m["t"] - rolled), round(m["meloet"]["rssMb"], 1), round(m["web"]["rssMb"], 1)]
                                 for m in mems]
        song = read_song(state_dir)
        result["takes"] = judge(state_dir, song) if song else None
        result["workFolder"] = sorted(os.listdir(os.path.join(state_dir, "work")))
    finish(a.out, result, long_summary(result))


def long_summary(r):
    lines = [f'# vocal_app.py long: {r["seconds"]} s, keys {r["keys"]}, tone {r["tone"]}', "",
             f'- sink (its monitor is the input): {r["sink"]}; frames asked {r["audio"].get("requestedFrames")}, '
             f'running {r["audio"].get("actualFrames")}; the PC says {r["audio"]["latency"].get("reportedMs")} ms round trip',
             f'- status while recording: {r["statusWhileRecording"]}', f'- status after: {r["statusAfter"]}',
             f'- stopping took {r["stopTookMs"]} ms', f'- tracks: {json.dumps(r["tracks"], ensure_ascii=False)}',
             f'- disk writer: {json.dumps(r["writer"])}',
             f'- ERR (first, last, largest quantum): {json.dumps(r["err"], ensure_ascii=False)}',
             f'- RSS MB: {json.dumps(r["rssMb"])}', f'- work folder after: {r["workFolder"]}', ""]
    for t in r["takes"] or []:
        lines.append(f'- take in {t["file"]} ({t["bits"]} bits, {t["channels"]} ch, {t["fileFrames"]} frames, first frame '
                     f'{t["firstFrame"]}, {t["frames"]} frames): {t["clicks"]} clicks, off by at most {t["offMax"]:.2f} '
                     f'frames (mean {t["offMean"]:+.3f}; first tenth {t["offFirstTenth"]:+.3f}, last tenth '
                     f'{t["offLastTenth"]:+.3f}), {t["notOnTheFrameCount"]} a frame or more off, wrong accents '
                     f'{t["wrongAccents"]}, weakest match {t["weakest"]}, stretches of zeros {len(t["zeroRuns"])} '
                     f'({sum(n for _, n in t["zeroRuns"])} frames)')
    if not r["takes"]:
        lines.append("- no vocal take in the recovery file")
    return "\n".join(lines) + "\n"


def run_kill(a):
    env = agent_env()
    sink = a.sink or default_sink(env)
    for d in ("state", "config", "state-at-kill"):
        shutil.rmtree(os.path.join(a.out, d), ignore_errors=True)
    state_dir = os.path.join(a.out, "state")
    result = {"sink": sink, "afterMs": a.after_ms, "settleS": a.settle_s}
    app = start_app(a.out, sink).start()
    try:
        arm(app, sink)
        time.sleep(a.settle_s)
        app.send({"type": "record"})
        sent = time.monotonic()
        while (time.monotonic() - sent) * 1000 < a.after_ms:
            time.sleep(0.005)
        before = app.snapshot()["transport"]
        os.kill(app.pid, signal.SIGKILL)
        result["transportAtKill"] = {k: before.get(k) for k in ("state", "tick", "recordingVocals")}
    finally:
        time.sleep(1)
        app.close(save_measure=False)
    shutil.copytree(state_dir, os.path.join(a.out, "state-at-kill"))
    work = os.path.join(state_dir, "work")
    try:
        with open(os.path.join(state_dir, "recovery.json"), encoding="utf-8") as f:
            kept = json.load(f)["song"]["song"]
        result["recoveryFileAtKill"] = {"tempo": kept.get("tempo"), "tracks": [t["name"] for t in kept["tracks"]]}
    except (OSError, ValueError, KeyError):
        result["recoveryFileAtKill"] = None
    result["workAtKill"] = sorted(os.listdir(work)) if os.path.isdir(work) else None
    try:
        with open(os.path.join(work, "rec.json"), encoding="utf-8") as f:
            result["recNote"] = json.load(f)
    except (OSError, ValueError):
        result["recNote"] = None
    wavs = sorted(os.listdir(os.path.join(work, "音"))) if os.path.isdir(os.path.join(work, "音")) else []
    result["wavAtKill"] = []
    for name in wavs:
        with open(os.path.join(work, "音", name), "rb") as f:
            head = f.read(44)
        result["wavAtKill"].append({"file": name, "bytes": os.path.getsize(os.path.join(work, "音", name)),
                                    "dataLengthInHead": struct.unpack_from("<I", head, 40)[0] if len(head) == 44 else None})

    with start_app(a.out, sink) as app:
        state = app.snapshot()
        result["recoveryView"] = state["workspace"].get("recovery")
        result["tempoAfterStart"] = state["timeline"]["tempoBpm"]
        result["statusAfterStart"] = app.status()
        result["hintText"] = app.js(
            "return [...document.querySelectorAll('[role=dialog],[role=alert],[role=status]')]"
            ".map(e=>e.innerText).filter(t=>t&&t.includes('残しました'))")
        result["tracks"] = [{"name": t["name"], "kind": t["kind"]["kind"],
                             "clips": [{k: c[k] for k in ("start", "end", "takes", "vocal")} for c in t["clips"]]}
                            for t in state["song"]["tracks"]]
        app.shot(os.path.join(a.out, "after-start.png"))
        result["workAfterStart"] = sorted(os.listdir(work)) if os.path.isdir(work) else None
        song = read_song(state_dir, wait_s=5)
        result["takes"] = judge(state_dir, song) if song else None
    lines = [f'# vocal_app.py kill: SIGKILL {a.after_ms} ms after R', ""]
    for key in ("settleS", "transportAtKill", "recoveryFileAtKill", "tempoAfterStart", "workAtKill", "recNote", "wavAtKill", "recoveryView", "statusAfterStart",
                "hintText", "tracks", "workAfterStart"):
        lines.append(f"- {key}: {json.dumps(result[key], ensure_ascii=False)}")
    for t in result["takes"] or []:
        lines.append(f'- take in {t["file"]}: tick 0 of the take at {t["origin"]}, first frame {t["firstFrame"]}, '
                     f'{t["frames"]} frames of {t["fileFrames"]}; data length in the head {t["dataLengthInHead"]}; '
                     f'{t["clicks"]} clicks, off by at most {t["offMax"]:.2f} frames (mean {t["offMean"]:+.3f}), '
                     f'wrong accents {t["wrongAccents"]}, weakest match {t["weakest"]}')
    if not result["takes"]:
        lines.append("- no vocal take in the recovery file after the start")
    finish(a.out, result, "\n".join(lines) + "\n")


def run_look(a):
    state_dir = a.out
    if a.song:
        with open(a.song, encoding="utf-8") as f:
            song = json.load(f)
    else:
        song = read_song(state_dir, wait_s=0)
    if not song:
        raise SystemExit(f"vocal_app.py: no song with a vocal take in {state_dir}/recovery.json (name a saved one with --song)")
    by_id = {t["id"]: t for t in song["audioTakes"]}
    for take in vocal_takes(song):
        sync = next(t for t in by_id.values() if t["firstFrame"] == take["firstFrame"] and t["frames"] == take["frames"]).get("sync")
        if not sync or sync.get("points") or sync.get("headRamp"):
            print(f'{take["file"]}: the tempo moves in this take; not looked at')
            continue
        path = os.path.join(state_dir, "work", "音", take["file"])
        _, bits, samples, head = read_wav(path)
        rate, = struct.unpack_from("<I", head, 24)
        beat_frames = rate * 60 / (sync["tempo"] / 100)
        # From half a beat before the take: the file keeps that much, and a sound a little
        # early belongs to the first beat.
        early = int(min(take["firstFrame"], beat_frames / 2))
        part = np.abs(samples[take["firstFrame"] - early:take["firstFrame"] + take["frames"]])
        level = max(0.01, 0.25 * float(part.max()) if len(part) else 0.01)
        loud = np.flatnonzero(part > level)
        # A sharp sound: louder than a quarter of the loudest, after 150 ms below that.
        starts = loud[np.concatenate(([True], np.diff(loud) > 0.15 * rate))] if len(loud) else []
        print(f'{take["file"]} ({bits} bits, {rate} Hz), tempo {sync["tempo"] / 100}, the take begins at tick {take["origin"]}: '
              f'{len(starts)} sharp sounds (louder than {level:.3f})')
        offs = []
        for at in starts:
            beat = take["origin"] / TICKS + (at - early) / beat_frames
            off = (beat - round(beat)) * beat_frames / rate * 1000
            offs.append(off)
            print(f'  beat {round(beat) + 1:4d} (counted from 1): {off:+7.2f} ms')
        if offs:
            print(f'  from the beat: mean {np.mean(offs):+.2f} ms, middle {np.median(offs):+.2f} ms, '
                  f'least {min(offs):+.2f} ms, most {max(offs):+.2f} ms')


def finish(out, result, summary):
    with open(os.path.join(out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)
    with open(os.path.join(out, "summary.md"), "w", encoding="utf-8") as f:
        f.write(summary)
    print(summary)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("what", choices=["long", "kill", "look"])
    p.add_argument("out", nargs="?")
    p.add_argument("--song")
    p.add_argument("--seconds", type=float, default=600)
    p.add_argument("--after-ms", type=float, default=6000)
    p.add_argument("--settle-s", type=float, default=4)
    p.add_argument("--keys", action="store_true")
    p.add_argument("--tone", action="store_true")
    p.add_argument("--sink")
    a = p.parse_args()
    if a.what == "look":
        a.out = os.path.abspath(a.out or os.path.expanduser("~/.local/state/meloet"))
        return run_look(a)
    if not a.out:
        p.error("OUT is needed")
    a.out = os.path.abspath(a.out)
    os.makedirs(a.out, exist_ok=True)
    (run_long if a.what == "long" else run_kill)(a)


if __name__ == "__main__":
    main()
