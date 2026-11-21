#!/usr/bin/env python3
"""The export (「書き出し」) in the real app (release build).

    python3 -I tools/measure/export_app.py same OUT --song FILE [--in-place] [--plays 2] [--cache DIR]
    python3 -I tools/measure/export_app.py speed OUT --song FILE [--in-place] [--times 3] [--cache DIR]
    python3 -I tools/measure/export_app.py flow OUT --song FILE [--cache DIR]

same: opens the song, writes it out (wav, 32 bit floats, the engine's sample rate, the tail
  until silence), then plays it from the head --plays times, each time recording the output's
  monitor as floats (pw-record). Says how far each recording is from the written file, piece by
  piece (a tenth of a second), with the place of each piece found anew when pw-record lost
  blocks, and how far the recordings are from each other.
speed: opens the dialog --times times and writes down when the check showed its first number
  and when it was done (the audio of the range is written while the dialog is open), the
  memory of meloet, its WebKitWebProcess and the plugin hosts (those started with --offline
  are the export's) every --memory-every s (0.2), ERR of meloet in pw-top before and after, and how long the
  export's hosts stay after the dialog closes. At the end presses 「書き出す」 once.
flow: the dialog's rules against the real core: a change of format, bits or sample rate keeps
  the audio already written (the job's folder under <state>/export stays), a change of the
  tail or the range writes anew; every format and number of bits, 44100 Hz, the loop, a file
  for each track; the written files are read with ffprobe and ffmpeg and held against the
  status line's text and against the 32 bit file; written as fast as it plays against
  written fast; 「やめる」 while writing leaves no file.

--in-place: a song with audio is its folder; it is opened where it is and the files are
  written into that folder (make a copy first). Without it the song file is copied.
--cache: XDG_CACHE_HOME for the app, an absolute path (default: OUT/cache). The plugin scan
  of remote.py's cache is copied into it.

Writes OUT/summary.json. Changes nothing on the PC.
"""
import argparse, json, os, re, shutil, subprocess, sys, threading, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
from remote import App, Recorder, RemoteError, agent_env, mem, plughosts, SF2_SOURCE
from stretch_app import jobs_of, own_cache, read_f32

PIECE = 4800
BITS = {16: "bits16", 24: "bits24", 32: "bits32"}


# ---- the dialog --------------------------------------------------------------------------

def export_of(app):
    return app.snapshot()["export"]


def set_export(app, kind, **value):
    app.send({"type": "setExport", "change": {"kind": kind, **value}})


def open_dialog(app, timeout=10):
    """Sends 「書き出し…」 and returns (the time it was sent, the first dialog seen)."""
    sent = time.monotonic()
    app.send({"type": "openExport"})
    state = app.wait(lambda s: s["export"]["dialog"], timeout, "the export dialog", every=0.005)
    return sent, state["export"]["dialog"]


def wait_check(app, sent, timeout=900, every=0.01):
    """Polls the check until it is done or failed. Seconds from `sent`: the dialog was there
    (the caller's), the first number, done; and every state seen in between."""
    first = None
    seen = []
    until = time.monotonic() + timeout
    while True:
        dialog = export_of(app)["dialog"]
        now = time.monotonic() - sent
        check = dialog["check"] if dialog else {"state": "noDialog"}
        if not seen or seen[-1][1] != check:
            seen.append([round(now, 3), check])
        if check["state"] in ("counting", "done") and first is None:
            first = now
        if check["state"] in ("done", "failed", "noDialog"):
            return {"firstNumberS": None if first is None else round(first, 3), "doneS": round(now, 3),
                    "check": check, "canExport": bool(dialog and dialog["canExport"]),
                    "seen": seen[:6] + seen[-3:] if len(seen) > 9 else seen, "states": len(seen)}
        if time.monotonic() > until:
            raise RemoteError(f"the check was not done in {timeout} s: {check}")
        time.sleep(every)


def start_export(app, timeout=900):
    """Presses 「書き出す」 and waits for the end. Returns the run and the seconds it took."""
    sent = time.monotonic()
    app.send({"type": "startExport"})
    state = app.wait(lambda s: s["export"]["run"] and s["export"]["run"]["state"] != "running", timeout,
                     "the export to end", every=0.01)
    return state["export"]["run"], round(time.monotonic() - sent, 3)


def song_folder(app):
    return os.path.dirname(app.song)


def job_folders(app):
    folder = os.path.join(app.work, "state", "export")
    found = {}
    if os.path.isdir(folder):
        for name in sorted(os.listdir(folder)):
            inside = os.path.join(folder, name)
            found[name] = {n: os.path.getsize(os.path.join(inside, n)) for n in sorted(os.listdir(inside))} \
                if os.path.isdir(inside) else os.path.getsize(inside)
    return found


def meloet_err(env):
    out = subprocess.run(["pw-top", "-b", "-n", "1"], env=env, capture_output=True, text=True).stdout
    return {p[-1]: int(p[8]) for p in (line.split() for line in out.splitlines())
            if len(p) >= 10 and p[8].isdigit() and p[-1] in ("meloet", "pw-record")}


def wait_song(app, seconds=300):
    """The vocal clips are ready and nothing is being made (a song without them: at once)."""
    def ready(s):
        clips = [c["audio"] for t in s["song"]["tracks"] for c in t["clips"] if c.get("audio")]
        return all(a["state"] == "ready" for a in clips) and not jobs_of(s)
    time.sleep(1.0)
    return app.wait(ready, seconds, "the song's audio", every=0.1)


def start_app(a, out, **more):
    cache = own_cache(os.path.abspath(a.cache or os.path.join(a.out, "cache")), empty=False)
    return App(out, song=a.song, song_in_place=a.in_place, sf2=True, env={"XDG_CACHE_HOME": cache}, **more)


class Memory:
    """Every `every` s (reading the PSS of many big processes is not free): meloet, its WebKitWebProcess, and each plugin host with whether it is the
    export's (--offline among its arguments)."""

    def __init__(self, app, every=0.2):
        self.app, self.rows, self.stop, self.every = app, [], threading.Event(), every
        self.thread = threading.Thread(target=self.loop, daemon=True)
        self.thread.start()

    def loop(self):
        while not self.stop.is_set():
            hosts = {}
            for pid, args in self.app.hosts().items():
                m = mem(pid)
                if m:
                    hosts[str(pid)] = {"offline": "--offline" in args, **m}
            self.rows.append({"t": time.monotonic(), "meloet": mem(self.app.pid), "web": mem(self.app.web_pid()),
                              "hosts": hosts})
            self.stop.wait(self.every)

    def end(self):
        self.stop.set()
        self.thread.join(timeout=5)

    def span(self, from_t, to_t):
        rows = [r for r in self.rows if from_t <= r["t"] <= to_t and r["meloet"] and r["web"]]
        if not rows:
            return None

        def of(who, key):
            v = [r[who][key] for r in rows]
            return [round(v[0], 1), round(max(v), 1), round(v[-1], 1)]

        def hosts(offline, key):
            v = [sum((h[key] or 0) for h in r["hosts"].values() if h["offline"] == offline) for r in rows]
            return [round(v[0], 1), round(max(v), 1), round(v[-1], 1)]

        dt = max(rows[-1]["t"] - rows[0]["t"], 1e-9)
        cpu = {who: round((rows[-1][who]["cpuS"] - rows[0][who]["cpuS"]) / dt, 2) for who in ("meloet", "web")}
        return {"seconds": round(dt, 1), "firstMaxLast": {
            "meloetRssMb": of("meloet", "rssMb"), "meloetPssMb": of("meloet", "pssMb"),
            "webRssMb": of("web", "rssMb"), "webPssMb": of("web", "pssMb"),
            "playHostsPssMb": hosts(False, "pssMb"), "exportHostsRssMb": hosts(True, "rssMb"),
            "exportHostsPssMb": hosts(True, "pssMb")},
            "mostExportHosts": max(sum(1 for h in r["hosts"].values() if h["offline"]) for r in rows),
            "cpuShare": cpu}


# ---- comparing ---------------------------------------------------------------------------

def first_sound(x, floor=1e-4):
    loud = np.flatnonzero(np.abs(x).max(axis=1) > floor)
    return int(loud[0]) if len(loud) else None


def best_lag(piece, y, at, reach):
    """Where `piece` (frames x channels) lies in y near `at`, by the correlation of the left
    side, -reach..reach frames. None when y ends there."""
    a, b = at - reach, at + reach + len(piece)
    if a < 0 or b > len(y):
        a, b = max(a, 0), min(b, len(y))
    if b - a < len(piece):
        return None
    window = y[a:b, 0]
    size = 1 << int(np.ceil(np.log2(len(window) + len(piece))))
    corr = np.fft.irfft(np.fft.rfft(window, size) * np.conj(np.fft.rfft(piece[:, 0], size)), size)
    return a + int(np.argmax(corr[:len(window) - len(piece) + 1])) - at


def compare(x, y, reach=8192):
    """x: the written file, y: a recording (frames x 2 each). Each piece of x from its first
    sound is held against y at the place of the piece before; when it is not the same there,
    its place is looked for anew (pw-record loses blocks now and then: after such a place the
    same sound is some blocks earlier in the file)."""
    a0, a1 = first_sound(x), first_sound(y)
    if a0 is None or a1 is None:
        return {"silent": [a0 is None, a1 is None]}
    # The first sound may be below the floor on one side only: settle the start by correlation.
    head = x[a0:a0 + 48000]
    d = best_lag(head, y, a1, 4800)
    lag = (a1 + d - a0) if d is not None else a1 - a0
    pieces = exact = close = 0
    lags, worst, far, near = [[0.0, lag]], 0.0, [], []
    num = den = 0.0
    for s in range(0, len(x) - PIECE + 1, PIECE):
        p = x[s:s + PIECE]
        if s + lag < 0:
            continue
        q = y[s + lag:s + lag + PIECE]
        if len(q) < PIECE:
            break
        pieces += 1
        diff = float(np.abs(p - q).max())
        if diff > 1e-4 and float(np.abs(p).max()) > 1e-3:
            d = best_lag(p, y, s + lag, reach)
            if d:
                q2 = y[s + lag + d:s + lag + d + PIECE]
                diff2 = float(np.abs(p - q2).max()) if len(q2) == PIECE else diff
                if diff2 < diff:
                    lag, q, diff = lag + d, q2, diff2
                    lags.append([round(s / 48000, 2), lag])
        if diff == 0.0:
            exact += 1
        if 0.0 < diff <= 1e-4:
            near.append([round(s / 48000, 2), diff])
        if diff <= 1e-4:
            close += 1
            num += float((p * q).sum())
            den += float((p * p).sum())
        else:
            far.append([round(s / 48000, 2), round(diff, 5), round(float(np.abs(p).max()), 4)])
        worst = max(worst, diff if diff <= 1e-4 else 0.0)
    return {"pieces": pieces, "sameToTheBit": exact, "within1e-4": close, "worstOfTheClose": worst,
            "gainOfTheClose": round(num / den, 7) if den else None,
            "firstSoundFrames": [a0, a1], "lagsFromSecond": lags[:20], "lagChanges": len(lags) - 1,
            "farPieces": far[:20], "farCount": len(far), "closeButNotTheSame": near[:30],
            "framesFile": len(x), "framesRecording": len(y)}


def play_and_record(app, wav, seconds, env):
    app.send({"type": "goToStart"})
    time.sleep(1.5)
    before = meloet_err(env)
    rec = Recorder(wav, env, fmt="f32")
    time.sleep(1.5)
    app.send({"type": "playStop"})
    time.sleep(seconds)
    app.send({"type": "playStop"})
    app.wait(lambda s: s["transport"]["state"] == "stopped", 10, "the stop")
    time.sleep(12.0)   # the tail: the export waits at most 10 s for silence
    after = meloet_err(env)
    rec.stop()
    return {"errBefore": before, "errAfter": after}


def run_same(a):
    env = agent_env()
    result = {"song": a.song}
    with start_app(a, a.out) as app:
        wait_song(app)
        result["tracks"] = [[t["name"], t.get("kind")] for t in app.snapshot()["song"]["tracks"]]
        sent, _ = open_dialog(app)
        set_export_all(app, "wav", 32, None, "untilSilent", False)
        dialog = export_of(app)["dialog"]
        result["dialog"] = {k: dialog[k] for k in ("range", "span", "format", "place", "sampleRate", "bits", "tail",
                                                   "realtime", "realtimeForced", "stoppedPlugins")}
        result["check"] = wait_check(app, sent)
        result["exportHostsWhileOpen"] = sum(1 for args in app.hosts().values() if "--offline" in args)
        run, took = start_export(app)
        result["run"], result["finishS"] = run, took
        result["statusLine"] = app.status()
        if run["state"] != "done":
            finish(a.out, result)
            return
        written = os.path.join(song_folder(app), run["file"])
        kept = os.path.join(a.out, "export.wav")
        shutil.copy(written, kept)
        os.remove(written)
        _, rate, x = read_f32(kept)
        result["file"] = {"rate": rate, "frames": len(x), "seconds": round(len(x) / rate, 3),
                          "peak": float(np.abs(x).max())}
        result["plays"] = []
        recs = []
        for i in range(a.plays):
            wav = os.path.join(a.out, f"play{i}.wav")
            # Played past the file's end: the stop then is after everything that is compared.
            errs = play_and_record(app, wav, a.play_s or len(x) / rate + 0.3, env)
            _, _, y = read_f32(wav)
            recs.append(y)
            result["plays"].append({**errs, "against": compare(x, y)})
            time.sleep(1.0)
        for i in range(1, len(recs)):
            result["plays"][i]["againstPlay0"] = compare(recs[0][first_sound(recs[0]) or 0:], recs[i])
        result["stderrTail"] = tail_of(app.stderr.name)
    finish(a.out, result)


def tail_of(path, lines=12):
    try:
        with open(path, "rb") as f:
            return f.read().decode(errors="replace").splitlines()[-lines:]
    except OSError:
        return []


def set_export_all(app, fmt, bits, rate, tail, realtime, range_="song"):
    """With the dialog open (the core takes no choice without it). The snapshot at the end
    comes after the choices: the check a caller reads next is the new job's."""
    set_export(app, "range", range=range_)
    set_export(app, "format", format=fmt)
    set_export(app, "bits", bits=BITS[bits])
    if rate:
        set_export(app, "sampleRate", hz=rate)
    set_export(app, "tail", tail=tail)
    set_export(app, "realtime", on=realtime)
    export_of(app)


# ---- speed -------------------------------------------------------------------------------

def run_speed(a):
    env = agent_env()
    result = {"song": a.song, "opens": []}
    with start_app(a, a.out) as app:
        wait_song(app)
        time.sleep(a.settle)
        watch = Memory(app, a.memory_every)
        time.sleep(2.0)
        t_before = time.monotonic()
        result["errBefore"] = meloet_err(env)
        result["hostsBefore"] = len(app.hosts())
        for i in range(a.times):
            t0 = time.monotonic()
            sent, dialog = open_dialog(app)
            if i == 0:
                set_export_all(app, "wav", 32, None, "untilSilent", False)
                result["dialog"] = {k: dialog[k] for k in ("range", "span", "place", "sampleRate", "realtimeForced")}
            one = wait_check(app, sent, every=0.02)
            one["jobFolders"] = job_folders(app)
            t1 = time.monotonic()
            one["memory"] = watch.span(t0, t1)
            last = i == a.times - 1
            if last:
                run, took = start_export(app)
                one["run"], one["finishS"] = run, took
                one["statusLine"] = app.status()
                if run["state"] == "done":
                    written = os.path.join(song_folder(app), run["file"])
                    one["fileBytes"] = os.path.getsize(written)
                    one["rangeS"] = None
                    channels, rate, x = read_f32(written)
                    one["fileSeconds"] = round(len(x) / rate, 3)
                    one["filePeak"] = float(np.abs(x).max())
                    os.remove(written)
            else:
                app.send({"type": "closeExport"})
            # How long the export's hosts and the job's folder stay.
            gone = time.monotonic()
            while time.monotonic() - gone < 10:
                if not any("--offline" in args for args in app.hosts().values()) and not job_folders(app):
                    break
                time.sleep(0.02)
            one["exportHostsAndFolderGoneS"] = round(time.monotonic() - gone, 3)
            one["exportHostsLeft"] = sum(1 for args in app.hosts().values() if "--offline" in args)
            one["jobFoldersLeft"] = job_folders(app)
            result["opens"].append(one)
            time.sleep(3.0)
        time.sleep(5.0)
        result["errAfter"] = meloet_err(env)
        result["hostsAfter"] = len(app.hosts())
        result["memoryAll"] = watch.span(t_before - 2.0, time.monotonic())
        watch.end()
        with open(os.path.join(a.out, "memory.json"), "w") as f:
            json.dump([{**r, "t": round(r["t"] - t_before, 2)} for r in watch.rows], f)
        result["stderrTail"] = tail_of(app.stderr.name)
    finish(a.out, result)


# ---- flow --------------------------------------------------------------------------------

def probe(path):
    out = subprocess.run(["ffprobe", "-v", "error", "-show_entries",
                          "stream=codec_name,sample_rate,channels,bits_per_sample,bits_per_raw_sample,sample_fmt,duration,duration_ts",
                          "-of", "json", path], capture_output=True, text=True).stdout
    try:
        return json.loads(out)["streams"][0]
    except (ValueError, KeyError, IndexError):
        return {"error": out[:200]}


def decoded(path):
    """The file's samples as floats (frames x 2), by ffmpeg."""
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-f", "f64le", "-acodec", "pcm_f64le", "-"],
                         capture_output=True).stdout
    return np.frombuffer(raw, dtype="<f8").reshape(-1, 2)


def states_for(app, seconds):
    """The check's states and the job folders' names seen over `seconds`."""
    states, folders, times = [], [], []
    start = time.monotonic()
    until = start + seconds
    while time.monotonic() < until:
        dialog = export_of(app)["dialog"]
        state = dialog["check"]["state"] if dialog else "noDialog"
        if not states or states[-1] != state:
            states.append(state)
            times.append(round(time.monotonic() - start, 3))
        names = sorted(job_folders(app))
        if not folders or folders[-1] != names:
            folders.append(names)
        time.sleep(0.01)
    return {"states": states, "stateAtS": times, "jobFolders": folders}


def run_flow(a):
    result = {"song": a.song, "steps": [], "files": []}
    files = os.path.join(a.out, "files")
    os.makedirs(files, exist_ok=True)
    with start_app(a, a.out) as app:
        wait_song(app)
        app.send({"type": "setLoop", "fromBar": 3, "toBar": 5})
        time.sleep(0.3)

        def step(name, **more):
            result["steps"].append({"step": name, **more})

        def change(name, kind, **value):
            before = sorted(job_folders(app))
            sent = time.monotonic()
            set_export(app, kind, **value)
            seen = states_for(app, 0.3 if kind in ("format", "bits", "sampleRate", "advancedOpen") else 3.0)
            anew = seen["jobFolders"][-1] != before
            if seen["states"][-1] != "done":
                wait_check(app, sent)
            dialog = export_of(app)["dialog"]
            step(name, anew=anew, **seen, check=dialog["check"],
                 span=dialog["span"], place=dialog["place"], bitsChoices=dialog["bitsChoices"], bits=dialog["bits"])

        sent, dialog = open_dialog(app)
        set_export_all(app, "wav", 24, 48000, "untilSilent", False)
        first = wait_check(app, sent)
        step("open", **first, jobFolders=job_folders(app), sampleRateChoices=dialog["sampleRateChoices"],
             tracks=dialog["tracks"], loopRange=dialog["loopRange"], span=dialog["span"])
        change("format flac", "format", format="flac")
        change("bits 16", "bits", bits="bits16")
        change("sample rate 44100", "sampleRate", hz=44100)
        change("format wav", "format", format="wav")
        change("bits 32", "bits", bits="bits32")
        change("format flac (from 32 bits)", "format", format="flac")
        change("advanced open", "advancedOpen", open=True)
        change("tail off", "tail", tail="off")
        change("tail until silent", "tail", tail="untilSilent")
        change("range loop", "range", range="loop")
        change("range tracks", "range", range="tracks")
        change("range song", "range", range="song")
        # Closing while it counts: the job's folder goes.
        set_export(app, "realtime", on=True)
        time.sleep(1.0)
        during = job_folders(app)
        gone = time.monotonic()
        app.send({"type": "closeExport"})
        while job_folders(app) and time.monotonic() - gone < 10:
            time.sleep(0.005)
        step("close while counting (as fast as it plays)", jobFoldersDuring=during,
             goneS=round(time.monotonic() - gone, 3), left=job_folders(app))

        reference = {}

        def write(name, fmt, bits, rate, range_="song", tail="untilSilent", realtime=False):
            before = set(os.listdir(song_folder(app)))
            sent, _ = open_dialog(app)
            set_export_all(app, fmt, bits, rate, tail, realtime, range_)
            dialog = export_of(app)["dialog"]
            check = wait_check(app, sent) if not realtime else None
            if realtime:
                app.wait(lambda s: s["export"]["dialog"]["canExport"], 30, "the first number", every=0.01)
            run, took = start_export(app)
            line = app.status()
            one = {"name": name, "asked": [fmt, bits, rate, range_, tail, realtime], "check": check and check["check"],
                   "checkDoneS": check and check["doneS"], "run": run, "finishS": took, "statusLine": line,
                   "span": dialog["span"], "files": []}
            for new in sorted(set(os.listdir(song_folder(app))) - before):
                src = os.path.join(song_folder(app), new)
                if os.path.isdir(src):
                    continue
                dst = os.path.join(files, f"{name}--{new}")
                shutil.move(src, dst)
                p = probe(dst)
                x = decoded(dst)
                info = {"file": new, "bytes": os.path.getsize(dst), "ffprobe": p, "frames": len(x),
                        "seconds": round(len(x) / int(p.get("sample_rate", rate)), 4),
                        "peakDb": round(20 * np.log10(max(float(np.abs(x).max()), 1e-12)), 2)}
                key = (rate, range_, tail)
                if bits == 32 and fmt == "wav" and not realtime:
                    reference[key] = x
                elif key in reference and len(reference[key]) == len(x):
                    info["farthestFromThe32BitFile"] = float(np.abs(reference[key] - x).max())
                    info["sameAsThe32BitFile"] = bool(np.array_equal(reference[key], x))
                elif key in reference:
                    info["framesOfThe32BitFile"] = len(reference[key])
                one["files"].append(info)
            one["jobFoldersLeft"] = job_folders(app)
            result["files"].append(one)
            app.send({"type": "back"})
            time.sleep(0.5)
            return one

        write("wav32", "wav", 32, 48000)
        write("wav24", "wav", 24, 48000)
        write("wav16", "wav", 16, 48000)
        write("wav16-again", "wav", 16, 48000)
        write("flac24", "flac", 24, 48000)
        write("flac16", "flac", 16, 48000)
        write("wav32-realtime", "wav", 32, 48000, realtime=True)
        write("wav32-44100", "wav", 32, 44100)
        write("wav24-44100", "wav", 24, 44100)
        write("flac16-44100", "flac", 16, 44100)
        write("wav32-notail", "wav", 32, 48000, tail="off")
        write("wav32-loop", "wav", 32, 48000, range_="loop")
        write("tracks", "wav", 32, 48000, range_="tracks")

        # 「やめる」 while it writes (as fast as it plays, so that there is time).
        before = set(os.listdir(song_folder(app)))
        sent, _ = open_dialog(app)
        set_export_all(app, "wav", 24, 48000, "untilSilent", True)
        export_of(app)
        app.wait(lambda s: s["export"]["dialog"]["canExport"], 30, "the first number", every=0.01)
        app.send({"type": "startExport"})
        time.sleep(3.0)
        running = export_of(app)["run"]
        line = app.status()
        folders = job_folders(app)
        during = sorted(set(os.listdir(song_folder(app))) - before)
        gone = time.monotonic()
        app.send({"type": "cancelExport"})
        state = app.wait(lambda s: s["export"]["run"] and s["export"]["run"]["state"] != "running", 10, "the cancel",
                         every=0.005)
        cancelled_s = round(time.monotonic() - gone, 3)
        while job_folders(app) and time.monotonic() - gone < 10:
            time.sleep(0.005)
        result["cancel"] = {"running": running, "statusLineRunning": line, "jobFoldersRunning": folders,
                            "newInTheSongFolderRunning": during, "run": state["export"]["run"],
                            "cancelledS": cancelled_s, "foldersGoneS": round(time.monotonic() - gone, 3),
                            "jobFoldersLeft": job_folders(app), "statusLine": app.status(),
                            "newInTheSongFolder": sorted(set(os.listdir(song_folder(app))) - before),
                            "exportHostsLeft": sum(1 for args in app.hosts().values() if "--offline" in args)}
        result["stderrTail"] = tail_of(app.stderr.name)
    finish(a.out, result)


def finish(out, result):
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)
    print(json.dumps(result, ensure_ascii=False, indent=1))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("what", choices=["same", "speed", "flow"])
    p.add_argument("out")
    p.add_argument("--song", required=True)
    p.add_argument("--in-place", action="store_true")
    p.add_argument("--cache")
    p.add_argument("--plays", type=int, default=2)
    p.add_argument("--play-s", type=float)
    p.add_argument("--times", type=int, default=3)
    p.add_argument("--settle", type=float, default=3.0)
    p.add_argument("--memory-every", type=float, default=0.2)
    a = p.parse_args()
    a.out = os.path.abspath(a.out)
    a.song = os.path.abspath(a.song)
    os.makedirs(a.out, exist_ok=True)
    {"same": run_same, "speed": run_speed, "flow": run_flow}[a.what](a)


if __name__ == "__main__":
    main()
