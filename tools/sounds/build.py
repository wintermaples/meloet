"""Build the meloet sound pack (docs/architecture.md 19-8) from the fetched sources.

python3 -I tools/sounds/build.py --out <dir> [--only grandPiano,strings] [--organ loud|quiet]
                                 [--src <dir>] [--src-dir <id>=<dir> ...] [--pack <version>] [--jobs N]
python3 -I tools/sounds/build.py --check <dir>

--src is the folder that holds the fetched sources (default ~/.cache/meloet/sounds-src); each source
sits in the sub folder SRC_DIRS names, or where --src-dir points. The sources are data only: nothing
in them is run. Audio goes through ffmpeg/ffprobe; numpy does the rest.

Writes <out>/pack.json, credits.json, licenses/<id>/ and one folder per instrument with
instrument.json and samples/*.flac. Building a few instruments into a folder that already holds a
pack keeps the other instruments (pack.json and credits.json are merged). Every instrument.json is
checked against the 19-8 shape at the end (--check runs only that).

Loudness: the instrument gainDb puts the peak of C4 (the kick for drums) at velocity 100 near
TARGET_DB, using the velocity gain of vel_gain_db() and a mono file played at unity on both sides.
"""

import argparse
import concurrent.futures
import json
import math
import os
import re
import shutil
import subprocess
import sys
import time

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import sf2  # noqa: E402
import sfz_flat  # noqa: E402

REPO = os.path.dirname(os.path.dirname(HERE))
TARGET_DB = -12.0
HEAD_DB = -50.0
HEAD_PRE_MS = 2.0
FADE_IN_MS = 1.0
NORM_DB = -1.0

SRC_DIRS = {
    "salamander-grand-piano": "salamander",
    "gregsullivan-wurlitzer-ep200": "gregsullivan-epianos",
    "naked-drums": "nakeddrums/repo",
    "vcsl-claps": "vcsl/repo",
    "vcsl-pipe-organ": "vcsl/repo",
    "black-and-blue-basses": "blackandblue/repo",
    "vsco-2-ce-strings": "vsco2ce/repo",
    # the SFZ files of the SFZ branch, saved as plain files (sources.json, vsco-2-ce-strings fetch.how)
    "vsco-2-ce-strings-sfz": "vsco2ce-sfz",
    "fluidr3-gm": "fluidr3",
}

FOLDERS = {
    "grandPiano": "grand-piano",
    "electricPiano": "electric-piano",
    "fingerBass": "finger-bass",
    "studioKit": "studio-kit",
    "strings": "strings",
    "organ": "organ",
    "organDrawbar": "organ-drawbar",
    "organPercussive": "organ-percussive",
    "organRock": "organ-rock",
    "organChurch": "organ-church",
    "organReed": "organ-reed",
}
ORDER = ["grandPiano", "electricPiano", "organ", "organDrawbar", "organPercussive", "organRock",
         "organChurch", "organReed", "fingerBass", "studioKit", "strings"]

CHANGES = {
    "salamander-grand-piano": "Kept 4 of the 16 velocity layers with the release, resonance and pedal samples, cut the silence before each sound, and converted the files to 16-bit FLAC raised to -1 dBFS (the level is restored at playback).",
    "gregsullivan-wurlitzer-ep200": "Cut the silence before each sound, made a crossfade loop at the end of each sample (the original loops were lost), and converted the files to 16-bit FLAC raised to -1 dBFS (the level is restored at playback).",
    "naked-drums": "Mixed the direct and overhead microphones to stereo with the levels and pans of the author's GM preset, kept 11 sounds and 4 of the 10 takes, cut the silence before each sound, and converted the files to 16-bit FLAC.",
    "vcsl-claps": "Used the six Clap takes, cut the silence before each sound, and converted the files to 16-bit FLAC.",
    "vcsl-pipe-organ": "Made a crossfade loop in each manual sample, shortened the files to the loop end, cut the silence before each sound, and converted the files to 16-bit FLAC.",
    "black-and-blue-basses": "Kept 2 of the 4 takes of the darkblack finger plucks, moved the key map down one octave so each key plays its own pitch, faded out the last 100 ms, cut the silence before each sound, and converted the files to 16-bit FLAC.",
    "fluidr3-gm": "Took the five organ presets (General MIDI 17 to 21) out of the SoundFont: joined each left and right sample pair into one stereo file, cut the samples at the loop end and the silence before each sound, converted the files to 16-bit FLAC raised to -1 dBFS (the level is restored at playback), and left out the filter, the chorus and reverb sends, the envelope delay and the velocity-to-filter modulator; a short decay to a lower sustain level became a lower level.",
    "vsco-2-ce-strings": "Joined the violin, viola and cello section and solo double bass sustains into one key range, made a crossfade loop in each sample, cut the silence before each sound, and converted the files to 16-bit FLAC.",
}


# ---------------------------------------------------------------- audio in and out

def probe(path):
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries",
         "stream=sample_rate,channels,codec_name,sample_fmt,bits_per_raw_sample,duration_ts",
         "-of", "json", os.path.abspath(path)],
        capture_output=True, text=True, check=True)
    return json.loads(out.stdout)["streams"][0]


def read_audio(path):
    """(frames x channels float32, sample rate)."""
    info = probe(path)
    ch = int(info["channels"])
    sr = int(info["sample_rate"])
    raw = subprocess.run(
        ["ffmpeg", "-v", "error", "-nostdin", "-i", os.path.abspath(path),
         "-f", "f32le", "-c:a", "pcm_f32le", "pipe:1"],
        capture_output=True, check=True).stdout
    data = np.frombuffer(raw, dtype="<f4").astype(np.float64)
    return data.reshape(-1, ch), sr


def write_flac(path, pcm16, sr):
    ch = pcm16.shape[1]
    subprocess.run(
        ["ffmpeg", "-v", "error", "-nostdin", "-y", "-f", "s16le", "-ar", str(sr), "-ac", str(ch),
         "-i", "pipe:0", "-map_metadata", "-1", "-fflags", "+bitexact", "-flags:a", "+bitexact",
         "-c:a", "flac", "-compression_level", "8", os.path.abspath(path)],
        input=pcm16.astype("<i2").tobytes(), check=True)


def db(x):
    return 20.0 * math.log10(max(x, 1e-12))


def rms(x):
    return float(np.sqrt(np.mean(np.square(x)))) if len(x) else 0.0


# ---------------------------------------------------------------- per file processing

def trim_head(x, sr):
    peak = np.max(np.abs(x))
    level = np.max(np.abs(x), axis=1)
    over = np.nonzero(level > peak * 10 ** (HEAD_DB / 20))[0]
    start = max(0, int(over[0]) - int(sr * HEAD_PRE_MS / 1000)) if len(over) else 0
    y = x[start:].copy()
    n = min(len(y), max(1, int(sr * FADE_IN_MS / 1000)))
    y[:n] *= (np.arange(n) / n)[:, None]
    return y, start


def env_db(mono, sr, win_s=0.05):
    w = max(1, int(sr * win_s))
    n = len(mono) // w
    if n == 0:
        return np.array([db(rms(mono))]), w
    frames = mono[: n * w].reshape(n, w)
    return 20 * np.log10(np.maximum(np.sqrt(np.mean(frames ** 2, axis=1)), 1e-9)), w


def best_loop_start(mono, le, lmin, lmax, xf):
    """Loop start in [le-lmax, le-lmin] whose xf frames before it look most like the xf frames
    before le (those are the frames the crossfade blends). Returns (ls, normalized correlation)."""
    a = max(0, le - lmax - xf)
    b = le - lmin
    seg = mono[a:b]
    tmpl = mono[le - xf: le]
    if len(seg) < xf + 1:
        raise ValueError("no room for a loop")
    n = 1 << int(math.ceil(math.log2(len(seg) + xf)))
    corr = np.fft.irfft(np.fft.rfft(seg, n) * np.conj(np.fft.rfft(tmpl, n)), n)[: len(seg) - xf + 1]
    c = np.concatenate([[0.0], np.cumsum(seg ** 2)])
    energy = c[xf:] - c[:-xf]
    ncc = corr / np.sqrt(np.maximum(energy * np.sum(tmpl ** 2), 1e-20))
    k = int(np.argmax(ncc))
    return a + k + xf, float(ncc[k])


def make_loop(x, sr, mode):
    """Crossfade loop baked into x. Returns (y cut at the loop end, ls, le, decay dB/s, info)."""
    mono = x.mean(axis=1)
    n = len(mono)
    env, w = env_db(mono, sr)
    if mode == "decay":
        # Wurlitzer: the files stop while the sound is still dying away. Loop the last part.
        att = int(0.08 * sr)
        le = n - int(0.002 * sr)
        span = le - att
        lmax = int(min(0.5 * sr, 0.6 * span))
        lmin = int(min(0.05 * sr, 0.3 * lmax))
        xf = max(32, int(min(0.04 * sr, 0.4 * lmin)))
        if le - lmax - xf < att:
            lmax = le - xf - att
        # decay rate from the second half (least squares on the 50 ms RMS steps)
        t = (np.arange(len(env)) + 0.5) * w / sr
        sel = t > max(0.08, t[-1] * 0.4)
        if sel.sum() >= 3:
            slope = float(np.polyfit(t[sel], env[sel], 1)[0])
        else:
            slope = float((env[-1] - env[0]) / max(t[-1] - t[0], 1e-3))
        decay = max(0.0, -slope)
    else:
        # strings and organ: loop the settled middle, before the bow stops or the key is let go
        med = float(np.median(env[len(env) // 4: 3 * len(env) // 4]))
        settled = np.nonzero(env >= med - 3)[0]
        start = (int(settled[0]) + 1) * w + int(0.3 * sr) if len(settled) else int(1.0 * sr)
        start = max(start, int(1.0 * sr))
        still = np.nonzero(env >= med - 6)[0]
        end = int(still[-1]) * w - int(0.3 * sr) if len(still) else n - int(0.5 * sr)
        end = min(end, n - int(0.2 * sr))
        le = end
        span = le - start
        target = min(4.0 * sr, 0.7 * span)
        lmax = int(target + 0.25 * sr)
        lmin = int(max(target - 0.25 * sr, 0.5 * sr))
        xf = int(min(0.4 * sr, lmin / 3))
        if le - lmax - xf < start:
            lmax = le - xf - start
        if lmax <= lmin:
            raise ValueError(f"settled part too short ({span / sr:.2f} s)")
        decay = 0.0
    ls, ncc = best_loop_start(mono, le, lmin, lmax, xf)
    L = le - ls
    y = x[:le].copy()
    # level ramp inside the loop so that the end comes back at the level of the start
    win = int(min(0.2 * sr, L / 4))
    if mode == "decay":
        ramp_db = decay * (L / sr)
    else:
        ramp_db = db(rms(mono[ls: ls + win])) - db(rms(mono[le - win: le]))
    g = 10 ** ((ramp_db * np.arange(L) / L) / 20)
    y[ls:le] *= g[:, None]
    # crossfade the frames before the end with the frames before the start
    i = np.arange(xf) / xf
    if ncc > 0.8:
        fo, fi = 1 - i, i
    else:
        fo, fi = np.cos(i * math.pi / 2), np.sin(i * math.pi / 2)
    y[le - xf: le] = y[le - xf: le] * fo[:, None] + x[ls - xf: ls] * fi[:, None]
    info = {"ncc": round(ncc, 3), "rampDb": round(ramp_db, 2), "crossfadeMs": round(1000 * xf / sr, 1),
            "loopS": round(L / sr, 3)}
    return y, ls, le, decay, info


def loop_check(y, ls, le, sr):
    """How the jump from the loop end to the loop start compares with the steps inside the loop.
    seam: the biggest step among the 4 frames on each side of the seam / 99th percentile step
    in the loop (about 1 or below = no click). levelDb: RMS 20 ms after the start minus 20 ms
    before the end."""
    f = y.astype(np.float64)
    joined = np.concatenate([f[le - 4: le], f[ls: ls + 4]])
    seam = np.max(np.abs(np.diff(joined, axis=0)))
    steps = np.abs(np.diff(f[ls:le], axis=0))
    p99 = float(np.percentile(steps, 99)) or 1.0
    w = int(0.02 * sr)
    lvl = db(rms(f[ls: ls + w])) - db(rms(f[le - w: le]))
    return {"seam": round(float(seam / p99), 2), "levelDb": round(lvl, 2)}


def process(job):
    """One output file. job: {"out", "inputs": [{"path", "gainL", "gainR"}], "stereo": bool,
    "loop": None | "decay" | "steady", "fadeEndMs": float}"""
    parts = []
    sr = None
    for inp in job["inputs"]:
        x, r = read_audio(inp["path"])
        if sr is None:
            sr = r
        elif r != sr:
            raise ValueError(f"sample rates differ in a mix: {inp['path']}")
        parts.append((x, inp.get("gainL", 1.0), inp.get("gainR", 1.0)))
    if job.get("stereo"):
        n = max(len(p[0]) for p in parts)
        x = np.zeros((n, 2))
        for p, gl, gr in parts:
            if p.shape[1] == 1:
                x[: len(p), 0] += p[:, 0] * gl
                x[: len(p), 1] += p[:, 0] * gr
            else:
                x[: len(p), 0] += p[:, 0] * gl
                x[: len(p), 1] += p[:, 1] * gr
    else:
        x = parts[0][0] * parts[0][1]
    x, cut = trim_head(x, sr)
    res = {"sr": sr, "channels": x.shape[1], "headCutMs": round(1000 * cut / sr, 2)}
    if job.get("loop"):
        x, ls, le, decay, info = make_loop(x, sr, job["loop"])
        res.update(loopStart=int(ls), loopEnd=int(le), loopDecayDbPerS=round(decay, 2), loop=info)
    fade = job.get("fadeEndMs")
    if fade:
        m = min(len(x), int(sr * fade / 1000))
        x[len(x) - m:] *= (0.5 + 0.5 * np.cos(np.linspace(0, math.pi, m)))[:, None]
    peak = float(np.max(np.abs(x)))
    if peak <= 0:
        raise ValueError(f"silent file {job['out']}")
    norm = 10 ** (NORM_DB / 20) / peak
    pcm = np.clip(np.round(x * norm * 32767), -32768, 32767).astype(np.int16)
    os.makedirs(os.path.dirname(job["out"]), exist_ok=True)
    write_flac(job["out"], pcm, sr)
    res.update(frames=len(pcm), peakDb=round(db(peak), 3), fileGainDb=round(db(peak) - NORM_DB, 3),
               bytes=os.path.getsize(job["out"]))
    if job.get("loop"):
        res["check"] = loop_check(pcm, res["loopStart"], res["loopEnd"], sr)
    return job["out"], res


# ---------------------------------------------------------------- shared helpers

def vel_gain_db(vel, veltrack):
    """Velocity gain assumed for the loudness estimate: SFZ amp_veltrack on the default curve,
    gain = (1 - t) + t * (vel / 127)^2 with t = veltrack / 100."""
    t = veltrack / 100.0
    return db((1 - t) + t * (vel / 127.0) ** 2)


def flat(sfz_path, defines=None):
    lines = []
    sfz_flat.expand(sfz_path, dict(defines or {}), lines)
    regions = sfz_flat.regions(lines, os.path.dirname(os.path.abspath(sfz_path)))
    cc = {}
    for line in lines:
        for m in re.finditer(r"set_(hd)?cc(\d+)=([-\d.]+)", line):
            v = float(m.group(3))
            cc[int(m.group(2))] = v if m.group(1) else v / 127.0
    return regions, cc


def cc_ok(r, cc):
    """Region conditions on CC values (locc/hicc), with the SFZ's set_cc values (0 when unset)."""
    for k, v in r.items():
        m = re.fullmatch(r"(lo|hi)cc(\d+)", k)
        if not m:
            continue
        val = cc.get(int(m.group(2)), 0.0) * 127
        if m.group(1) == "lo" and val < float(v):
            return False
        if m.group(1) == "hi" and val > float(v):
            return False
    return True


def safe_name(s):
    return re.sub(r"[^A-Za-z0-9_.-]", "_", s.replace("#", "s")).lstrip("-_")


def num(r, k, default=None):
    v = r.get(k)
    return default if v is None else float(v)


def fnum(x):
    """Write whole numbers as ints in JSON."""
    return int(x) if float(x).is_integer() else round(float(x), 3)


class Builder:
    def __init__(self, args):
        self.args = args
        self.out = os.path.abspath(args.out)
        self.src = os.path.abspath(os.path.expanduser(args.src))
        self.overrides = dict(s.split("=", 1) for s in args.src_dir)
        with open(os.path.join(HERE, "sources.json"), encoding="utf-8") as f:
            self.sources = {s["id"]: s for s in json.load(f)["sources"]}
        self.pool = concurrent.futures.ProcessPoolExecutor(max_workers=args.jobs)

    def root(self, sid):
        base = self.overrides.get(sid) or os.path.join(self.src, SRC_DIRS[sid])
        src = self.sources.get(sid)
        sub = (src or {}).get("use", {}).get("root", ".")
        return os.path.normpath(os.path.join(base, sub))

    def run(self, jobs):
        """Process the jobs (deduplicated by output path) in parallel; {out path: result}."""
        uniq = {}
        for j in jobs:
            uniq.setdefault(j["out"], j)
        return dict(self.pool.map(process, uniq.values()))


def region_json(d):
    keys = ["sample", "keyLo", "keyHi", "root", "tuneCents", "velLo", "velHi", "trigger", "gainDb",
            "ampVeltrack", "pitchKeytrack", "attackMs", "releaseMs", "rtDecayDbPerS", "loopStart",
            "loopEnd", "loopDecayDbPerS", "oneShot", "rrIndex", "rrCount", "chokeGroup", "chokedBy", "pan"]
    out = {}
    for k in keys:
        if k in d and d[k] is not None:
            v = d[k]
            out[k] = fnum(v) if isinstance(v, (int, float)) and not isinstance(v, bool) else v
    return out


def finish_instrument(b, kind, credits, regions, results, settings, loud_key, loud_vel=100):
    """Fill gainDb from the file results, set the instrument gainDb, write instrument.json."""
    folder = os.path.join(b.out, FOLDERS[kind])
    rows = []
    for r in regions:
        res = results[r.pop("_out")]
        r["sample"] = "samples/" + os.path.basename(r["_file"])
        r.pop("_file")
        r["gainDb"] = res["fileGainDb"] + r.pop("_volDb", 0.0)
        for k in ("loopStart", "loopEnd", "loopDecayDbPerS"):
            if k in res:
                r[k] = res[k]
        if not r.get("loopDecayDbPerS"):
            r.pop("loopDecayDbPerS", None)
        rows.append(r)
    # loudness: the loudest attack region that answers loud_key at loud_vel
    best = None
    for r in rows:
        if r.get("trigger", "attack") != "attack":
            continue
        if not (r["keyLo"] <= loud_key <= r["keyHi"] and r.get("velLo", 1) <= loud_vel <= r.get("velHi", 127)):
            continue
        peak = NORM_DB + r["gainDb"] + vel_gain_db(loud_vel, r.get("ampVeltrack", 100))
        best = peak if best is None else max(best, peak)
    if best is None:
        raise SystemExit(f"{kind}: no region plays key {loud_key} at velocity {loud_vel}")
    gain = round(TARGET_DB - best, 1)
    inst = {"format": "meloet-instrument", "version": 1, "kind": kind, "credits": credits, "gainDb": fnum(gain)}
    inst.update(settings)
    inst["regions"] = [region_json(r) for r in rows]
    with open(os.path.join(folder, "instrument.json"), "w", encoding="utf-8") as f:
        json.dump(inst, f, ensure_ascii=False, indent=1)
        f.write("\n")
    loops = [(os.path.basename(k), v) for k, v in results.items() if "check" in v]
    size = sum(v["bytes"] for v in results.values())
    summary = {"kind": kind, "files": len(results), "regions": len(rows), "bytes": size,
               "gainDb": gain, "loudKeyPeakBeforeGainDb": round(best, 2), "loops": loops}
    with open(os.path.join(folder, "build-log.json"), "w", encoding="utf-8") as f:
        json.dump({k: v for k, v in summary.items()}, f, indent=1)
        f.write("\n")
    return summary


def job_for(b, kind, name, inputs, **kw):
    out = os.path.join(b.out, FOLDERS[kind], "samples", name)
    j = {"out": out, "inputs": inputs}
    j.update(kw)
    return j


# ---------------------------------------------------------------- the instruments

def build_grand_piano(b):
    sid = "salamander-grand-piano"
    root = b.root(sid)
    regions, _ = flat(os.path.join(root, b.sources[sid]["use"]["sfz"]))
    layers = {2: (1, 34), 8: (35, 64), 12: (65, 96), 16: (97, 127)}
    all_layers = {}
    jobs, rows = [], []
    for r in regions:
        src = r["_sample"]
        base = os.path.basename(src)
        name = safe_name(os.path.splitext(base)[0]) + ".flac"
        row = {"_file": name, "keyLo": int(r["lokey"]), "keyHi": int(r["hikey"]),
               "ampVeltrack": num(r, "amp_veltrack", 100), "_volDb": num(r, "volume", 0.0)}
        if "on_locc64" in r:
            down = float(r["on_locc64"]) >= 64
            row.update(keyLo=0, keyHi=127, root=60, pitchKeytrack=0, ampVeltrack=0,
                       trigger="pedalDown" if down else "pedalUp",
                       rrIndex=1 if float(r.get("lorand", 0)) < 0.5 else 2, rrCount=2,
                       chokeGroup=int(r["group"]))
            if "off_by" in r:
                row["chokedBy"] = [int(r["off_by"])]
        else:
            m = re.match(r"[A-G]#?\d+v(\d+)$", os.path.splitext(base)[0])
            if r.get("trigger") != "release" and m:
                layer = int(m.group(1))
                all_layers.setdefault((int(r["lokey"]), int(r["hikey"])), []).append(
                    (int(num(r, "lovel", 1)), int(num(r, "hivel", 127)), src, num(r, "amp_veltrack", 100)))
                if layer not in layers:
                    continue
                row["velLo"], row["velHi"] = layers[layer]
                row["_src"] = src
            else:
                row["velLo"] = int(num(r, "lovel", 1))
                row["velHi"] = int(num(r, "hivel", 127))
                row["trigger"] = "release"
                row["rtDecayDbPerS"] = num(r, "rt_decay", 0)
            if "pitch_keycenter" in r:
                row["root"] = int(r["pitch_keycenter"])
            else:
                row["root"] = int(r["lokey"]) if num(r, "pitch_keytrack", 100) == 0 else 60
            if "pitch_keytrack" in r:
                row["pitchKeytrack"] = float(r["pitch_keytrack"])
            if "tune" in r:
                row["tuneCents"] = float(r["tune"])
            if "ampeg_release" in r:
                row["releaseMs"] = 1000 * float(r["ampeg_release"])
        if row.get("trigger", "attack") == "attack" and "releaseMs" not in row:
            row["releaseMs"] = 1000
        job = job_for(b, "grandPiano", name, [{"path": src}])
        row["_out"] = job["out"]
        jobs.append(job)
        rows.append(row)
    results = b.run(jobs)
    peaks = dict(b.pool.map(peak_db, sorted({p for v in all_layers.values() for _, _, p, _ in v})))
    fits = []
    for row in rows:
        src = row.pop("_src", None)
        if src is None:
            continue
        t, extra, err = fit_layer(all_layers[(row["keyLo"], row["keyHi"])], peaks, peaks[src],
                                  row["velLo"], row["velHi"])
        row["ampVeltrack"] = round(100 * t)
        row["_volDb"] = extra
        fits.append(err)
    s = finish_instrument(b, "grandPiano", [sid], rows, results,
                          {"polyphony": 64, "sustainPedal": True}, 60)
    s["velocityFitRmsDb"] = round(float(np.sqrt(np.mean(np.square(fits)))), 2)
    s["c4PeakByVelocity"] = velocity_sweep(os.path.join(b.out, FOLDERS["grandPiano"], "instrument.json"), 60)
    return s


def velocity_sweep(inst_path, key):
    """Estimated peak (dBFS) of one key at 16 velocities from 1 to 127."""
    with open(inst_path, encoding="utf-8") as f:
        inst = json.load(f)
    out = []
    for vel in np.linspace(1, 127, 16).round().astype(int):
        peaks = [NORM_DB + r["gainDb"] + inst["gainDb"] + vel_gain_db(vel, r.get("ampVeltrack", 100))
                 for r in inst["regions"] if r.get("trigger", "attack") == "attack"
                 and r["keyLo"] <= key <= r["keyHi"] and r.get("velLo", 1) <= vel <= r.get("velHi", 127)]
        out.append(round(max(peaks), 1))
    return out


def peak_db(path):
    x, _ = read_audio(path)
    return path, db(float(np.max(np.abs(x))))


def velocity_target(all16, peaks):
    """dB the source plays at each velocity 1..127 with all its layers (each at its middle
    velocity through its amp_veltrack), joined by straight lines so that no layer edge steps."""
    pts = sorted(((lo + hi) / 2, peaks[p] + vel_gain_db((lo + hi) / 2, vt)) for lo, hi, p, vt in all16)
    v = np.arange(1, 128)
    t = np.interp(v, [p[0] for p in pts], [p[1] for p in pts])
    return np.maximum.accumulate(t)


def fit_layer(all16, peaks, peak, lo, hi):
    """ampVeltrack (0..1) and extra dB for one kept layer so that it follows velocity_target()
    over its velocities. Returns (t, extra dB, weighted RMS error dB)."""
    target = velocity_target(all16, peaks)[lo - 1: hi]
    v = np.arange(lo, hi + 1)
    # the two edges count more: they are where this layer meets the next one
    w = np.ones(len(v))
    w[:2] = w[-2:] = 8
    best = None
    for t in np.linspace(0, 1, 101):
        g = np.array([vel_gain_db(x, 100 * t) for x in v])
        extra = float(np.average(target - peak - g, weights=w))
        err = float(np.sqrt(np.average((peak + extra + g - target) ** 2, weights=w)))
        if best is None or err < best[2]:
            best = (float(t), extra, err)
    return best


def build_electric_piano(b):
    sid = "gregsullivan-wurlitzer-ep200"
    root = b.root(sid)
    regions, cc = flat(os.path.join(root, b.sources[sid]["use"]["sfz"]))
    jobs, rows = [], []
    rel = 1000 * float(regions[0].get("ampeg_release_oncc72", 0)) * cc.get(72, 0.0)
    veltrack = num(regions[0], "amp_veltrack", 0) + num(regions[0], "amp_veltrack_oncc99", 0) * cc.get(99, 0.0)
    for r in regions:
        src = r["_sample"]
        name = safe_name(os.path.splitext(os.path.basename(src))[0]) + ".flac"
        job = job_for(b, "electricPiano", name, [{"path": src}], loop="decay")
        jobs.append(job)
        rows.append({"_file": name, "_out": job["out"], "_volDb": num(r, "volume", 0.0),
                     "keyLo": int(r["lokey"]), "keyHi": int(r["hikey"]), "root": int(r["pitch_keycenter"]),
                     "tuneCents": num(r, "tune"), "velLo": int(r["lovel"]), "velHi": int(r["hivel"]),
                     "ampVeltrack": veltrack, "attackMs": 1000 * num(r, "ampeg_attack", 0), "releaseMs": rel})
    results = b.run(jobs)
    return finish_instrument(b, "electricPiano", [sid], rows, results,
                             {"polyphony": 32, "sustainPedal": True}, 60)


def build_finger_bass(b):
    sid = "black-and-blue-basses"
    root = b.root(sid)
    regions, cc = flat(os.path.join(root, b.sources[sid]["use"]["sfz"]))
    jobs, rows = [], []
    for r in regions:
        if r.get("trigger") == "release" or not cc_ok(r, cc) or "locc100" in r:
            continue
        seq = int(num(r, "seq_position", 1))
        if seq > 2:
            continue
        src = r["_sample"]
        if "/darkblack/reg/" not in src.replace("\\", "/"):
            continue
        name = safe_name(os.path.splitext(os.path.basename(src))[0]) + ".flac"
        job = job_for(b, "fingerBass", name, [{"path": src}], fadeEndMs=100)
        jobs.append(job)
        rows.append({"_file": name, "_out": job["out"], "_volDb": num(r, "volume", 0.0),
                     "keyLo": int(r["lokey"]) - 12, "keyHi": int(r["hikey"]) - 12,
                     "root": int(num(r, "pitch_keycenter", 60)) - 12,
                     "velLo": int(num(r, "lovel", 1)), "velHi": int(num(r, "hivel", 127)),
                     "ampVeltrack": 50, "attackMs": 1000 * num(r, "ampeg_attack", 0),
                     "releaseMs": 1000 * num(r, "ampeg_release", 0.25), "rrIndex": seq, "rrCount": 2})
    results = b.run(jobs)
    return finish_instrument(b, "fingerBass", [sid], rows, results,
                             {"polyphony": 16, "sustainPedal": False}, 60)


def pan_gains(p):
    a = (max(-1.0, min(1.0, p)) + 1) * math.pi / 4
    return math.sqrt(2) * math.cos(a), math.sqrt(2) * math.sin(a)


DRUMS = [
    # (meloet key, master_label, group_label)
    (36, "Kick", "Center"), (37, "Snare1", "Sidestick"), (38, "Snare1", "Center"),
    (42, "Hats", "Tight"), (44, "Hats", "Pedal"), (46, "Hats", "Open"),
    (50, "Tom1", "Center"), (47, "Tom3", "Center"), (45, "Tom5", "Center"),
    (49, "Crash1", "Edge"), (51, "Ride", "Bow"),
]
DRUM_NAMES = {36: "kick", 37: "sidestick", 38: "snare", 39: "clap", 42: "hh-closed", 44: "hh-pedal",
              46: "hh-open", 45: "tom-low", 47: "tom-mid", 50: "tom-high", 49: "crash", 51: "ride"}
TAKES = 4


def build_studio_kit(b):
    sid = "naked-drums"
    root = b.root(sid)
    regions, cc = flat(os.path.join(root, b.sources[sid]["use"]["sfz"].replace("Stereo/Naked Drums.sfz", "User/Naked Drums GM.sfz")))
    groups = {}
    for r in regions:
        if r.get("trigger") == "release" or os.path.basename(r.get("sample", "")).startswith("*"):
            continue
        if "sw_last" in r and r["sw_last"] != r.get("sw_default"):
            continue
        if not cc_ok(r, cc):
            continue
        for key, ml, gl in DRUMS:
            if r.get("master_label") == ml and r.get("group_label") == gl:
                break
        else:
            continue
        seq = int(num(r, "seq_position", 1))
        if seq > TAKES:
            continue
        vol = num(r, "master_volume", 0) + num(r, "group_volume", 0) + num(r, "volume", 0)
        amp = 1.0
        pan = 0.0
        for k, v in r.items():
            m = re.fullmatch(r"amplitude_oncc(\d+)", k)
            if m and int(m.group(1)) != 7:
                amp *= float(v) / 100 * cc.get(int(m.group(1)), 0.0)
            m = re.fullmatch(r"pan_oncc(\d+)", k)
            if m and int(m.group(1)) != 10:
                pan += float(v) / 100 * (2 * cc.get(int(m.group(1)), 0.0) - 1)
        pan += num(r, "pan", 0) / 100
        g = amp * 10 ** (vol / 20)
        gl_, gr_ = pan_gains(pan)
        k = (key, int(r["lovel"]), int(r["hivel"]), seq)
        groups.setdefault(k, {"inputs": [], "r": r})["inputs"].append(
            {"path": r["_sample"], "gainL": g * gl_, "gainR": g * gr_, "_mono_pan": pan})
    jobs, rows = [], []
    layers = {}
    for (key, lo, hi, seq) in sorted(groups):
        layers.setdefault(key, set()).add((lo, hi))
    for (key, lo, hi, seq), grp in sorted(groups.items()):
        r = grp["r"]
        li = sorted(layers[key]).index((lo, hi)) + 1
        name = f"{DRUM_NAMES[key]}_v{li}_rr{seq}.flac"
        inputs = [{k: v for k, v in i.items() if not k.startswith("_")} for i in grp["inputs"]]
        job = job_for(b, "studioKit", name, inputs, stereo=True)
        jobs.append(job)
        row = {"_file": name, "_out": job["out"], "keyLo": key, "keyHi": key, "root": key,
               "tuneCents": num(r, "tune"), "velLo": lo, "velHi": hi,
               "ampVeltrack": num(r, "amp_veltrack", 100), "oneShot": True,
               "rrIndex": seq, "rrCount": TAKES}
        if key in (42, 44):
            row["chokeGroup"] = 1
        if key == 46:
            row["chokeGroup"] = 2
            row["chokedBy"] = [1]
            row["releaseMs"] = 1000 * num(r, "off_time", 0.2)
        rows.append(row)
    # claps (VCSL): no SFZ; six takes of one strength
    croot = b.root("vcsl-claps")
    clap_dir = os.path.join(croot, "Idiophones/Struck Idiophones/Claps")
    clap_rows = []
    for i in range(1, 7):
        name = f"clap_rr{i}.flac"
        job = job_for(b, "studioKit", name, [{"path": os.path.join(clap_dir, f"Clap_rr{i}.wav")}])
        jobs.append(job)
        clap_rows.append({"_file": name, "_out": job["out"], "keyLo": 39, "keyHi": 39, "root": 39,
                          "velLo": 1, "velHi": 127, "ampVeltrack": 99, "oneShot": True,
                          "rrIndex": i, "rrCount": 6})
    results = b.run(jobs)
    # the clap gets the peak (at velocity 100) of the snare's matching layer
    snare = [r for r in rows if r["keyLo"] == 38 and r["velLo"] <= 100 <= r["velHi"]]
    target = max(results[r["_out"]]["peakDb"] for r in snare) + vel_gain_db(100, 99)
    for r in clap_rows:
        r["_volDb"] = target - vel_gain_db(100, 99) - results[r["_out"]]["peakDb"]
    rows += clap_rows
    return finish_instrument(b, "studioKit", [sid, "vcsl-claps"], rows, results,
                             {"polyphony": 32, "sustainPedal": False}, 36)


STRING_PARTS = [
    # (prefix, sfz file, keys this part plays)
    ("bass", "ContrabassSusVB.sfz", (24, 35)),
    ("cello", "CelloEnsSusVib.sfz", (36, 47)),
    ("viola", "ViolaEnsSusVib.sfz", (48, 54)),
    ("violin", "ViolinEnsSusVib.sfz", (55, 86)),
]


def build_strings(b):
    sid = "vsco-2-ce-strings"
    root = b.root(sid)
    sfz_dir = b.overrides.get("vsco-2-ce-strings-sfz") or os.path.join(b.src, SRC_DIRS["vsco-2-ce-strings-sfz"])
    jobs, rows = [], []
    for prefix, sfz, (klo, khi) in STRING_PARTS:
        regions, _ = flat(os.path.join(sfz_dir, sfz))
        for r in regions:
            lo, hi = max(int(r["lokey"]), klo), min(int(r["hikey"]), khi)
            if lo > hi:
                continue
            rel = os.path.relpath(r["_sample"], sfz_dir)
            src = os.path.join(root, rel)
            name = prefix + "_" + safe_name(os.path.splitext(os.path.basename(src))[0]) + ".flac"
            job = job_for(b, "strings", name, [{"path": src}], loop="steady")
            jobs.append(job)
            rows.append({"_file": name, "_out": job["out"], "_volDb": num(r, "volume", 0.0),
                         "keyLo": lo, "keyHi": hi, "root": int(r["pitch_keycenter"]),
                         "velLo": max(1, int(num(r, "lovel", 1))), "velHi": int(num(r, "hivel", 127)),
                         "ampVeltrack": num(r, "amp_veltrack", 100),
                         "attackMs": 1000 * num(r, "ampeg_attack", 0), "releaseMs": 1000 * num(r, "ampeg_release", 0)})
    results = b.run(jobs)
    return finish_instrument(b, "strings", [sid], rows, results,
                             {"polyphony": 32, "sustainPedal": True}, 60)


NOTE = {"C": 0, "C#": 1, "D": 2, "D#": 3, "E": 4, "F": 5, "F#": 6, "G": 7, "G#": 8, "A": 9, "A#": 10, "B": 11}


def build_organ(b):
    sid = "vcsl-pipe-organ"
    root = b.root(sid)
    which = b.args.organ
    folder = os.path.join(root, "Aerophones/Edge-blown Aerophones/Pipe Organ", "Loud" if which == "loud" else "Quiet")
    files = []
    for f in sorted(os.listdir(folder)):
        m = re.search(r"_([A-G]#?)(\d)(?:_rr1)?\.wav$", f)
        if m:
            files.append((NOTE[m.group(1)] + 12 * (int(m.group(2)) + 2), f))  # name C3 = key 60
    files.sort()
    jobs, rows = [], []
    for i, (key, f) in enumerate(files):
        lo = key - 1 if i > 0 else key
        hi = key + 1 if i < len(files) - 1 else key
        name = safe_name(os.path.splitext(f)[0]) + ".flac"
        job = job_for(b, "organ", name, [{"path": os.path.join(folder, f)}], loop="steady")
        jobs.append(job)
        rows.append({"_file": name, "_out": job["out"], "keyLo": lo, "keyHi": hi, "root": key,
                     "ampVeltrack": 0, "attackMs": 1})
    results = b.run(jobs)
    rel = organ_release_ms(os.path.join(folder, files[len(files) // 2][1]))
    for r in rows:
        r["releaseMs"] = rel
    s = finish_instrument(b, "organ", [sid], rows, results, {"polyphony": 32, "sustainPedal": True}, 60)
    # An organ note holds its full level, so chords add up far more than on a piano: the level is
    # set on the chord the listening tool plays (C3 under C4 E4 G4) instead of on one C4.
    path = os.path.join(b.out, FOLDERS["organ"], "instrument.json")
    with open(path, encoding="utf-8") as f:
        inst = json.load(f)
    before = chord_peak_db(path, ORGAN_CHORD) - inst["gainDb"]
    inst["gainDb"] = fnum(round(ORGAN_CHORD_DB - before, 1))
    with open(path, "w", encoding="utf-8") as f:
        json.dump(inst, f, ensure_ascii=False, indent=1)
        f.write("\n")
    s.update(gainDb=inst["gainDb"], chordPeakBeforeGainDb=round(before, 2),
             c4PeakDb=round(s["loudKeyPeakBeforeGainDb"] + inst["gainDb"], 1), variant=which, releaseMs=rel)
    return s


ORGAN_CHORD = [48, 60, 64, 67]
ORGAN_CHORD_DB = -9.0


def chord_peak_db(inst_path, keys, vel=100, seconds=3.0):
    """Estimated peak (dBFS) of the keys pressed together at one velocity: each file is moved
    to its key's pitch by linear interpolation, scaled like the player does, and summed."""
    with open(inst_path, encoding="utf-8") as f:
        inst = json.load(f)
    total = None
    for key in keys:
        r = next(r for r in inst["regions"] if r.get("trigger", "attack") == "attack"
                 and r["keyLo"] <= key <= r["keyHi"] and r.get("velLo", 1) <= vel <= r.get("velHi", 127))
        x, sr = read_audio(os.path.join(os.path.dirname(inst_path), r["sample"]))
        ratio = 2 ** ((key - r["root"] + r.get("tuneCents", 0) / 100) / 12)
        n = int(seconds * sr)
        pos = np.arange(n) * ratio
        pos = pos[pos < len(x) - 1]
        y = np.stack([np.interp(pos, np.arange(len(x)), x[:, c]) for c in range(x.shape[1])], axis=1)
        if y.shape[1] == 1:
            y = np.repeat(y, 2, axis=1)
        y *= 10 ** ((r["gainDb"] + inst["gainDb"] + vel_gain_db(vel, r.get("ampVeltrack", 100))) / 20)
        if total is None:
            total = y
        else:
            m = min(len(total), len(y))
            total = total[:m] + y[:m]
    return db(float(np.max(np.abs(total))))


def organ_release_ms(path):
    """How long the recorded key-up takes to fall 60 dB below the held level (room included)."""
    x, sr = read_audio(path)
    env, w = env_db(x.mean(axis=1), sr, 0.02)
    med = float(np.median(env[len(env) // 4: len(env) // 2]))
    held = np.nonzero(env >= med - 6)[0]
    start = int(held[-1])
    below = np.nonzero(env[start:] <= med - 60)[0]
    end = start + (int(below[0]) if len(below) else len(env) - start)
    return int(round((end - start) * w / sr * 1000, -1))


# ---------------------------------------------------------------- FluidR3_GM organs (SoundFont 2)

FLUID_ORGANS = {"organDrawbar": 16, "organPercussive": 17, "organRock": 18, "organChurch": 19, "organReed": 20}
# SF2 2.01 8.1.2 (#38): a volume envelope release is the time to fall 100 dB, straight in dB (9.1.7
# and FluidSynth use 96 dB); the player's releaseMs is the time to fall 60 dB (player.rs RELEASE_FLOOR).
SF2_RELEASE_DB = 100.0
# A decay this short to a lower sustain level is taken as that lower level from the start.
SUSTAIN_FOLD_MS = 20.0
# Generators the 19-8 fields carry. The others are counted as dropped when not at their default.
SF2_USED = {"pan", "attackVolEnv", "releaseVolEnv", "initialAttenuation", "coarseTune", "fineTune",
            "sampleModes", "overridingRootKey", "exclusiveClass", "scaleTuning", "startAddrsOffset",
            "endAddrsOffset", "startloopAddrsOffset", "endloopAddrsOffset", "startAddrsCoarseOffset",
            "endAddrsCoarseOffset", "startloopAddrsCoarseOffset", "endloopAddrsCoarseOffset"}


def process_pcm(job):
    """One output file from SF2 sample data. job: {"out", "sr", "pcm": [int16 arrays, one per
    channel, the same length], "loop": None | (start, end) in frames of pcm}."""
    x = np.stack([np.asarray(c, dtype=np.float64) / 32768.0 for c in job["pcm"]], axis=1)
    sr = job["sr"]
    x, cut = trim_head(x, sr)
    res = {"sr": sr, "channels": x.shape[1], "headCutMs": round(1000 * cut / sr, 2)}
    if job["loop"]:
        ls, le = job["loop"][0] - cut, job["loop"][1] - cut
        if ls < max(1, int(sr * FADE_IN_MS / 1000)) or le - ls < 8 or le > len(x):
            raise ValueError(f"loop {job['loop']} does not fit after cutting {cut} frames: {job['out']}")
        x = x[:le]
        res.update(loopStart=int(ls), loopEnd=int(le))
    peak = float(np.max(np.abs(x)))
    if peak <= 0:
        raise ValueError(f"silent file {job['out']}")
    pcm = np.clip(np.round(x * (10 ** (NORM_DB / 20) / peak) * 32767), -32768, 32767).astype(np.int16)
    os.makedirs(os.path.dirname(job["out"]), exist_ok=True)
    write_flac(job["out"], pcm, sr)
    res.update(frames=len(pcm), peakDb=round(db(peak), 3), fileGainDb=round(db(peak) - NORM_DB, 3),
               bytes=os.path.getsize(job["out"]))
    if job["loop"]:
        res["check"] = loop_check(pcm, res["loopStart"], res["loopEnd"], sr)
    return job["out"], res


def soundfont(b, sid):
    if not hasattr(b, "_sf2"):
        b._sf2 = sf2.SoundFont(os.path.join(b.root(sid), b.sources[sid]["use"]["sf2"]))
    return b._sf2


def stereo_pairs(sf, zones):
    """Zones as [(left or mono zone, right zone or None)]. A left zone goes with the right zone of
    the same instrument, keys and velocities (FluidR3 leaves every sampleLink at 0, so the link
    is not used); every generator but pan must match."""
    out, used = [], set()
    for i, z in enumerate(zones):
        if i in used:
            continue
        t = sf.samples[z["sample"]]["type"] & ~sf2.ROM
        if t == sf2.MONO:
            out.append((z, None))
            continue
        want = sf2.RIGHT if t == sf2.LEFT else sf2.LEFT
        same = (z["inst"], z["keyLo"], z["keyHi"], z["velLo"], z["velHi"])
        for j in range(i + 1, len(zones)):
            w = zones[j]
            if j in used or sf.samples[w["sample"]]["type"] & ~sf2.ROM != want \
                    or (w["inst"], w["keyLo"], w["keyHi"], w["velLo"], w["velHi"]) != same:
                continue
            if {k: v for k, v in w["gen"].items() if k != "pan"} != {k: v for k, v in z["gen"].items() if k != "pan"}:
                raise SystemExit(f"stereo pair {sf.samples[z['sample']]['name']}: generators differ")
            used.add(j)
            out.append((z, w) if t == sf2.LEFT else (w, z))
            break
        else:
            raise SystemExit(f"no other side for {sf.samples[z['sample']]['name']}")
    return out


def count(d, k):
    d[k] = d.get(k, 0) + 1


def build_fluid_organ(b, kind):
    sid = "fluidr3-gm"
    sf = soundfont(b, sid)
    pname, zones, mods = sf.regions(0, FLUID_ORGANS[kind])
    if mods["preset"] or mods["instrument"]:
        raise SystemExit(f"{kind}: the preset has modulators, which this tool does not carry")
    jobs, rows = {}, []
    dropped, folded = {}, {}
    for left, right in stereo_pairs(sf, zones):
        g = left["gen"]
        s = sf.samples[left["sample"]]
        start, end, ls, le = sf.sample_points(left)
        # 8.1.2 sampleModes: 1 loops, 3 loops while the key is down (the file ends at the loop end
        # here, so both loop through the release as 19-8 says), 0 and 2 do not loop
        loop = (ls - start, le - start) if g["sampleModes"] & 3 in (1, 3) else None
        pcm = [sf.pcm(start, end)]
        if right is not None:
            r0, r1, rls, rle = sf.sample_points(right)
            if (r1 - r0, rls - r0, rle - r0) != (end - start, ls - start, le - start):
                raise SystemExit(f"{kind}: the two sides of {s['name']} have other lengths or loops")
            if (g["pan"], right["gen"]["pan"]) != (-500, 500):
                raise SystemExit(f"{kind}: stereo pair {s['name']} is not panned hard left and right")
            pcm.append(sf.pcm(r0, r1))
        name = safe_name(re.sub(r"\((L|R)\)$", "", s["name"]).strip().replace(" ", "_")) + ".flac"
        out = os.path.join(b.out, FOLDERS[kind], "samples", name)
        key = (left["sample"], right and right["sample"], start, end, loop)
        if out in jobs and jobs[out]["_key"] != key:
            raise SystemExit(f"{kind}: two different files would be named {name}")
        jobs[out] = {"out": out, "sr": s["rate"], "pcm": pcm, "loop": loop, "_key": key}
        row = {"_file": name, "_out": out, "keyLo": left["keyLo"], "keyHi": left["keyHi"],
               "velLo": max(1, left["velLo"]), "velHi": left["velHi"],
               # 8.4.1: the default velocity-to-attenuation modulator (960 cB, concave) is the
               # SFZ amp_veltrack 100 curve, (vel/127)^2
               "ampVeltrack": 100}
        # pitch: overridingRootKey (else the sample's originalPitch), coarseTune, fineTune and the
        # sample's pitchCorrection; with scaleTuning 100, a coarse tune is a move of the root key
        root = g["overridingRootKey"] if g["overridingRootKey"] >= 0 else s["originalPitch"]
        tune = g["fineTune"] + s["pitchCorrection"]
        if g["scaleTuning"] == 100:
            root -= g["coarseTune"]
        else:
            tune += 100 * g["coarseTune"]
            row["pitchKeytrack"] = g["scaleTuning"]
        inside = min(127, max(0, root))
        tune += 100 * (root - inside)
        row["root"] = inside
        if tune:
            row["tuneCents"] = tune
        # 8.1.3: initialAttenuation and sustainVolEnv are in centibels (FluidSynth takes 0.4 of
        # initialAttenuation, as the E-mu chips did; the spec value is used here)
        vol = -g["initialAttenuation"] / 10.0
        short_decay = sf2.timecents_ms(g["decayVolEnv"]) <= SUSTAIN_FOLD_MS and g["keynumToVolEnvDecay"] == 0
        no_effect = set()
        if g["sustainVolEnv"] > 0 and short_decay:
            vol -= g["sustainVolEnv"] / 10.0
            count(folded, "sustainVolEnv")
            no_effect |= {"sustainVolEnv", "decayVolEnv"}
        elif g["sustainVolEnv"] <= 0:
            no_effect |= {"decayVolEnv", "keynumToVolEnvDecay"}
        row["_volDb"] = vol
        row["attackMs"] = round(sf2.timecents_ms(g["attackVolEnv"]), 1)
        row["releaseMs"] = round(sf2.timecents_ms(g["releaseVolEnv"]) * 60.0 / SF2_RELEASE_DB, 1)
        if right is None and g["pan"]:
            row["pan"] = max(-1.0, min(1.0, g["pan"] / 500.0))
        if g["exclusiveClass"]:
            if g["exclusiveClass"] > 63:
                raise SystemExit(f"{kind}: exclusiveClass {g['exclusiveClass']} is above 63")
            row["chokeGroup"] = g["exclusiveClass"]
            row["chokedBy"] = [g["exclusiveClass"]]
        for k, v in g.items():
            if k not in SF2_USED and k not in no_effect and v != sf2.DEFAULT.get(k, 0):
                count(dropped, k)
        rows.append(row)
    for j in jobs.values():
        j.pop("_key")
    results = dict(b.pool.map(process_pcm, jobs.values()))
    s = finish_instrument(b, kind, [sid], rows, results, {"polyphony": 32, "sustainPedal": True}, 60)
    # the level is set on a chord, like the VCSL organ (build_organ)
    path = os.path.join(b.out, FOLDERS[kind], "instrument.json")
    with open(path, encoding="utf-8") as f:
        inst = json.load(f)
    before = looped_chord_peak_db(path, ORGAN_CHORD) - inst["gainDb"]
    inst["gainDb"] = fnum(round(ORGAN_CHORD_DB - before, 1))
    with open(path, "w", encoding="utf-8") as f:
        json.dump(inst, f, ensure_ascii=False, indent=1)
        f.write("\n")
    s.update(preset=pname, gainDb=inst["gainDb"], chordPeakBeforeGainDb=round(before, 2),
             c4PeakDb=round(s["loudKeyPeakBeforeGainDb"] + inst["gainDb"], 1),
             stereoFiles=sum(1 for j in jobs.values() if len(j["pcm"]) == 2),
             loopedFiles=sum(1 for j in jobs.values() if j["loop"]),
             keys=[min(r["keyLo"] for r in rows), max(r["keyHi"] for r in rows)],
             dropped=dropped, folded=folded)
    log = os.path.join(b.out, FOLDERS[kind], "build-log.json")
    with open(log, encoding="utf-8") as f:
        entry = json.load(f)
    entry.update({k: s[k] for k in ("preset", "gainDb", "chordPeakBeforeGainDb", "dropped", "folded")})
    with open(log, "w", encoding="utf-8") as f:
        json.dump(entry, f, indent=1)
        f.write("\n")
    return s


def looped_chord_peak_db(inst_path, keys, vel=100, seconds=3.0):
    """chord_peak_db() for files that loop: the read position goes back to loopStart at loopEnd."""
    with open(inst_path, encoding="utf-8") as f:
        inst = json.load(f)
    total = None
    for key in keys:
        r = next(r for r in inst["regions"] if r.get("trigger", "attack") == "attack"
                 and r["keyLo"] <= key <= r["keyHi"] and r.get("velLo", 1) <= vel <= r.get("velHi", 127))
        x, sr = read_audio(os.path.join(os.path.dirname(inst_path), r["sample"]))
        ratio = 2 ** (((key - r["root"]) * r.get("pitchKeytrack", 100) + r.get("tuneCents", 0)) / 1200)
        pos = np.arange(int(seconds * sr)) * ratio
        if "loopStart" in r:
            ls, le = r["loopStart"], r["loopEnd"]
            over = pos >= le - 1
            pos[over] = ls + np.mod(pos[over] - ls, le - ls)
        pos = pos[pos < len(x) - 1]
        y = np.stack([np.interp(pos, np.arange(len(x)), x[:, c]) for c in range(x.shape[1])], axis=1)
        if y.shape[1] == 1:
            y = np.repeat(y, 2, axis=1)
        y *= 10 ** ((r["gainDb"] + inst["gainDb"] + vel_gain_db(vel, r.get("ampVeltrack", 100))) / 20)
        if total is None:
            total = y
        else:
            m = min(len(total), len(y))
            total = total[:m] + y[:m]
    return db(float(np.max(np.abs(total))))


BUILDERS = {
    "grandPiano": build_grand_piano,
    "electricPiano": build_electric_piano,
    "fingerBass": build_finger_bass,
    "studioKit": build_studio_kit,
    "strings": build_strings,
    "organ": build_organ,
}
for _kind in FLUID_ORGANS:
    BUILDERS[_kind] = lambda b, k=_kind: build_fluid_organ(b, k)


# ---------------------------------------------------------------- pack files and the shape check

def write_pack(b, built):
    pj = os.path.join(b.out, "pack.json")
    pack = {"format": "meloet-sounds", "version": 1, "pack": b.args.pack, "instruments": {}, "credits": "credits.json"}
    if os.path.exists(pj):
        with open(pj, encoding="utf-8") as f:
            old = json.load(f)
        pack["instruments"].update(old.get("instruments", {}))
    for kind in built:
        pack["instruments"][kind] = FOLDERS[kind] + "/instrument.json"
    pack["instruments"] = {k: pack["instruments"][k] for k in ORDER if k in pack["instruments"]}
    used = []
    for path in pack["instruments"].values():
        with open(os.path.join(b.out, path), encoding="utf-8") as f:
            for c in json.load(f)["credits"]:
                if c not in used:
                    used.append(c)
    credits = []
    for sid in used:
        s = b.sources[sid]
        a = s["attribution"]
        entry = {"id": sid, "title": a["title"], "authors": [a["creator"]], "source": a["source_uri"],
                 "license": s["license"], "licenseUrl": a["license_uri"], "changes": CHANGES[sid]}
        if a.get("also"):
            entry["also"] = a["also"]
        credits.append(entry)
        dst = os.path.join(b.out, "licenses", sid)
        shutil.rmtree(dst, ignore_errors=True)
        shutil.copytree(os.path.join(REPO, s["license_copies"]), dst)
    with open(os.path.join(b.out, "credits.json"), "w", encoding="utf-8") as f:
        json.dump(credits, f, ensure_ascii=False, indent=1)
        f.write("\n")
    with open(pj, "w", encoding="utf-8") as f:
        json.dump(pack, f, ensure_ascii=False, indent=1)
        f.write("\n")


REGION_FIELDS = {
    "sample": str, "keyLo": int, "keyHi": int, "root": int, "tuneCents": (int, float), "velLo": int,
    "velHi": int, "trigger": str, "gainDb": (int, float), "ampVeltrack": (int, float),
    "pitchKeytrack": (int, float), "attackMs": (int, float), "releaseMs": (int, float),
    "rtDecayDbPerS": (int, float), "loopStart": int, "loopEnd": int, "loopDecayDbPerS": (int, float),
    "oneShot": bool, "rrIndex": int, "rrCount": int, "chokeGroup": int, "chokedBy": list, "pan": (int, float),
}
TRIGGERS = {"attack", "release", "pedalDown", "pedalUp"}


def check_instrument(path, deep=True):
    """Problems with one instrument.json (empty list when it has the 19-8 shape)."""
    bad = []
    with open(path, encoding="utf-8") as f:
        inst = json.load(f)
    want = {"format": "meloet-instrument", "version": 1}
    for k, v in want.items():
        if inst.get(k) != v:
            bad.append(f"{k} is {inst.get(k)!r}, not {v!r}")
    if inst.get("kind") not in FOLDERS:
        bad.append(f"unknown kind {inst.get('kind')!r}")
    for k, t in (("credits", list), ("gainDb", (int, float)), ("polyphony", int), ("sustainPedal", bool), ("regions", list)):
        if not isinstance(inst.get(k), t) or isinstance(inst.get(k), bool) != (t is bool):
            bad.append(f"{k} missing or wrong type")
    if isinstance(inst.get("polyphony"), int) and not 1 <= inst["polyphony"] <= 256:
        bad.append("polyphony must be 1 to 256")
    if isinstance(inst.get("gainDb"), (int, float)) and inst["gainDb"] > 48:
        bad.append("gainDb above 48")
    extra = set(inst) - {"format", "version", "kind", "credits", "gainDb", "polyphony", "sustainPedal", "regions"}
    if extra:
        bad.append(f"unknown fields {sorted(extra)}")
    frames = {}
    for i, r in enumerate(inst.get("regions") or []):
        where = f"region {i} ({r.get('sample')})"
        broken = False
        for k in ("sample", "keyLo", "keyHi", "root"):
            if k not in r:
                bad.append(f"{where}: no {k}")
                broken = True
        for k, v in r.items():
            t = REGION_FIELDS.get(k)
            if t is None:
                bad.append(f"{where}: unknown field {k}")
            elif isinstance(v, bool) != (t is bool) or not isinstance(v, t):
                bad.append(f"{where}: {k} has the wrong type")
                broken = True
        if broken:
            continue
        if not (0 <= r["keyLo"] <= r["keyHi"] <= 127 and 0 <= r["root"] <= 127):
            bad.append(f"{where}: keys out of range")
        if not (1 <= r.get("velLo", 1) <= r.get("velHi", 127) <= 127):
            bad.append(f"{where}: velocities out of range")
        if r.get("trigger", "attack") not in TRIGGERS:
            bad.append(f"{where}: trigger {r.get('trigger')!r}")
        if not -100 <= r.get("ampVeltrack", 100) <= 100:
            bad.append(f"{where}: ampVeltrack out of range")
        limits = {"gainDb": (-1e9, 48), "tuneCents": (-1200, 1200), "pitchKeytrack": (-1200, 1200),
                  "attackMs": (0, 60000), "releaseMs": (0, 60000), "rtDecayDbPerS": (0, 10000),
                  "loopDecayDbPerS": (0, 10000)}
        for k, (a, z) in limits.items():
            if k in r and not a <= r[k] <= z:
                bad.append(f"{where}: {k} {r[k]} outside {a}..{z}")
        if not -1 <= r.get("pan", 0) <= 1:
            bad.append(f"{where}: pan out of range")
        if ("rrIndex" in r) != ("rrCount" in r) or not 1 <= r.get("rrIndex", 1) <= r.get("rrCount", 1):
            bad.append(f"{where}: rrIndex/rrCount")
        if ("loopStart" in r) != ("loopEnd" in r):
            bad.append(f"{where}: loopStart without loopEnd")
        if not 0 <= r.get("chokeGroup", 0) <= 63:
            bad.append(f"{where}: chokeGroup must be 0 to 63")
        if "chokedBy" in r and not all(isinstance(c, int) and not isinstance(c, bool) and 0 <= c <= 63
                                       for c in r["chokedBy"]):
            bad.append(f"{where}: chokedBy must hold numbers 0 to 63")
        if "loopStart" in r and "loopEnd" in r and r["loopEnd"] - r["loopStart"] < 8:
            bad.append(f"{where}: a loop is at least 8 frames")
        sp = os.path.normpath(os.path.join(os.path.dirname(path), r["sample"]))
        if not sp.startswith(os.path.dirname(path) + os.sep) or not os.path.isfile(sp):
            bad.append(f"{where}: sample file missing")
            continue
        if deep:
            if sp not in frames:
                info = probe(sp)
                ok = info.get("codec_name") == "flac" and info.get("sample_fmt") in ("s16", "s16p") \
                    and int(info.get("channels", 0)) in (1, 2)
                frames[sp] = int(info.get("duration_ts") or 0) if ok else None
            if frames[sp] is None:
                bad.append(f"{where}: not FLAC 16 bit mono or stereo")
            # loopEnd is the frame after the last one played
            elif "loopStart" in r and not 0 <= r["loopStart"] < r["loopEnd"] <= frames[sp]:
                bad.append(f"{where}: loop {r['loopStart']}..{r['loopEnd']} outside {frames[sp]} frames")
    return bad


def check_pack(out, deep=True):
    bad = []
    with open(os.path.join(out, "pack.json"), encoding="utf-8") as f:
        pack = json.load(f)
    if pack.get("format") != "meloet-sounds" or pack.get("version") != 1:
        bad.append("pack.json format/version")
    with open(os.path.join(out, pack["credits"]), encoding="utf-8") as f:
        credits = {c["id"]: c for c in json.load(f)}
    for c in credits.values():
        for k in ("id", "title", "authors", "source", "license", "licenseUrl", "changes"):
            if not c.get(k):
                bad.append(f"credits {c.get('id')}: no {k}")
        if not os.path.isdir(os.path.join(out, "licenses", c["id"])):
            bad.append(f"credits {c['id']}: no licenses/{c['id']}/")
    for kind, rel in pack["instruments"].items():
        p = os.path.join(out, rel)
        for msg in check_instrument(p, deep):
            bad.append(f"{rel}: {msg}")
        with open(p, encoding="utf-8") as f:
            inst = json.load(f)
        if inst.get("kind") != kind:
            bad.append(f"{rel}: kind {inst.get('kind')} under {kind}")
        for c in inst.get("credits", []):
            if c not in credits:
                bad.append(f"{rel}: credit {c} not in credits.json")
    return bad


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out")
    ap.add_argument("--only", default=",".join(ORDER))
    ap.add_argument("--organ", choices=["loud", "quiet"], default="loud")
    ap.add_argument("--src", default="~/.cache/meloet/sounds-src")
    ap.add_argument("--src-dir", action="append", default=[])
    ap.add_argument("--pack", default="dev")
    ap.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 2) // 2))
    ap.add_argument("--check")
    a = ap.parse_args()
    if a.check:
        bad = check_pack(os.path.abspath(a.check))
        for m in bad:
            print("NG ", m)
        print("OK  pack shape" if not bad else f"{len(bad)} problems")
        sys.exit(1 if bad else 0)
    if not a.out:
        ap.error("--out is needed")
    kinds = [k.strip() for k in a.only.split(",") if k.strip()]
    for k in kinds:
        if k not in BUILDERS:
            ap.error(f"unknown instrument {k} (one of {', '.join(ORDER)})")
    b = Builder(a)
    os.makedirs(b.out, exist_ok=True)
    built = []
    try:
        for k in kinds:
            t0 = time.monotonic()
            shutil.rmtree(os.path.join(b.out, FOLDERS[k]), ignore_errors=True)
            s = BUILDERS[k](b)
            s["seconds"] = round(time.monotonic() - t0, 1)
            built.append(k)
            loops = s.pop("loops")
            print(json.dumps(s, ensure_ascii=False))
            if loops:
                seams = [v["check"]["seam"] for _, v in loops]
                lv = [abs(v["check"]["levelDb"]) for _, v in loops]
                worst = max(loops, key=lambda kv: kv[1]["check"]["seam"])
                print(f"  loops {len(loops)}: seam median {np.median(seams):.2f} max {max(seams):.2f} ({worst[0]}), "
                      f"|level step| median {np.median(lv):.2f} dB max {max(lv):.2f} dB")
    finally:
        b.pool.shutdown()
    write_pack(b, built)
    bad = check_pack(b.out)
    for m in bad:
        print("NG ", m)
    print("OK  pack shape" if not bad else f"{len(bad)} problems")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
