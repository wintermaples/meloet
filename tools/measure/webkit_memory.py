#!/usr/bin/env python3
"""How the memory of the app's WebKitWebProcess grows over a long run, and where.

    python3 -I tools/measure/webkit_memory.py run OUT --scene stopped|playing|mixer|keys|recording
                [--path cpu|comp] [--seconds 1800] [--every 30] [--song FILE] [--env K=V ...]
    python3 -I tools/measure/webkit_memory.py summary OUT [OUT ...]

run: starts the app (remote.py: its own config, state and XDG data/config folders, so two runs
  can go side by side), opens --song, then: "stopped" does nothing, "playing" plays, "mixer"
  opens the mixer band and plays, "keys" presses virtual keys (4 a second, single keys and
  chords) while stopped, "recording" records those keys. Playing starts again from the top if
  the song ends.
  --path cpu is the app's own choice (WEBKIT_DISABLE_COMPOSITING_MODE=1 from main.rs);
  --path comp sets WEBKIT_DISABLE_COMPOSITING_MODE=0 from outside (the compositor). --env adds
  more of the environment the app and its web process start with (tries).
  Every --every seconds, for WebKitWebProcess and meloet: smaps_rollup, /proc/<pid>/status
  (RssAnon, RssShmem, threads), the resident memory by kind (memory.py), the open fds and the
  memfd ones (count, size), and the anonymous and shared mappings themselves (OUT/maps.jsonl:
  start, size, resident, name), so that what grew can be told apart afterwards. Notes when the
  web process is gone or another one has come (the app's stderr has the reason it gave).
summary: per run, the growth (MB/s and MB/h) from 2 minutes in to the end, by kind, and what the
  grown anonymous memory is made of (new mappings or grown ones, by size), into OUT/summary.md.
Reads /proc only; changes nothing on the PC.
"""
import argparse, json, os, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from memory import breakdown, mappings
from remote import App, comm, descendants

MiB = 1024 * 1024
WARMUP_S = 120


def status(pid):
    got = {}
    try:
        for line in open(f"/proc/{pid}/status"):
            key, _, rest = line.partition(":")
            if key in ("VmRSS", "RssAnon", "RssFile", "RssShmem", "VmSwap"):
                got[key] = round(int(rest.split()[0]) / 1024, 1)
            elif key == "Threads":
                got[key] = int(rest)
    except OSError:
        return None
    return got


def fds(pid):
    out = {"n": 0, "memfd": 0, "memfdMb": 0.0, "shm": 0, "names": {}}
    try:
        names = os.listdir(f"/proc/{pid}/fd")
    except OSError:
        return None
    for n in names:
        try:
            link = os.readlink(f"/proc/{pid}/fd/{n}")
        except OSError:
            continue
        out["n"] += 1
        if link.startswith("/memfd:"):
            out["memfd"] += 1
            try:
                out["memfdMb"] += os.stat(f"/proc/{pid}/fd/{n}").st_size / MiB
            except OSError:
                pass
            label = link.split(" (deleted)")[0]
        elif link.startswith("/dev/shm/"):
            out["shm"] += 1
            label = "/dev/shm"
        elif link.startswith("/"):
            label = "file"
        else:
            label = link.split(":")[0]
        out["names"][label] = out["names"].get(label, 0) + 1
    out["memfdMb"] = round(out["memfdMb"], 1)
    return out


def mapped(pid):
    """The anonymous, heap and shared mappings: [start, size, rss, perm, name]."""
    maps = mappings(pid)
    if maps is None:
        return None
    keep = []
    for m in maps:
        name = m["name"]
        if name and not (name == "[heap]" or name.startswith(("/memfd:", "/dev/shm/", "/SYSV", "[anon"))):
            continue
        keep.append([m["start"], m["end"] - m["start"], m["Rss"], m["perm"], name])
    return {"count": len(maps), "kept": keep}


def web_pid(pid):
    return next((c for c in descendants(pid) if comm(c) == "WebKitWebProces"), None)


def threads(pid):
    try:
        return sorted(open(f"/proc/{pid}/task/{t}/comm").read().strip() for t in os.listdir(f"/proc/{pid}/task"))
    except OSError:
        return []


def keep_playing(app, scene):
    try:
        state = app.snapshot()["transport"]
    except Exception:
        return None
    if state["state"] == "stopped":
        app.send({"type": "record" if scene == "recording" else "playStop"})
    return state["state"]


def run(a):
    os.makedirs(a.out, exist_ok=True)
    work = os.path.join(a.out, "xdg")
    env = {}
    for d in ("data", "config", "state"):
        os.makedirs(os.path.join(work, d), exist_ok=True)
        env[f"XDG_{d.upper()}_HOME"] = os.path.abspath(os.path.join(work, d))
    if a.path == "comp":
        env["WEBKIT_DISABLE_COMPOSITING_MODE"] = "0"
    for item in a.env:
        k, _, v = item.partition("=")
        env[k] = v
    meta = {"scene": a.scene, "path": a.path, "env": env, "song": a.song, "every": a.every,
            "seconds": a.seconds, "loadStart": os.getloadavg(), "startWall": time.time()}
    samples = open(os.path.join(a.out, "samples.jsonl"), "w", encoding="utf-8")
    maps_out = open(os.path.join(a.out, "maps.jsonl"), "w", encoding="utf-8")
    with App(a.out, song=a.song, env=env, settle_s=3) as app:
        web = web_pid(app.pid)
        meta["webPid"], meta["meloetPid"] = web, app.pid
        meta["webThreads"] = threads(web) if web else []
        meta["compositor"] = any("Compositor" in t for t in meta["webThreads"])
        try:
            meta["webEnvDisable"] = next((e.decode() for e in open(f"/proc/{web}/environ", "rb").read().split(b"\0")
                                          if e.startswith(b"WEBKIT_DISABLE_COMPOSITING_MODE=")), None)
        except OSError:
            meta["webEnvDisable"] = None
        if a.scene == "mixer":
            app.send({"type": "toggleBand", "band": "mixer"})
            time.sleep(1)
        if a.scene in ("playing", "mixer"):
            app.send({"type": "playStop"})
        if a.scene == "recording":
            app.send({"type": "record"})
        keys = None
        if a.scene in ("keys", "recording"):
            keys = app.play_keys("--pattern", "mixed", "--notes", "48,50,52,53,55,57,59,60,62,64,65,67",
                                 "--count", str(int(a.seconds * 5)), "--interval-ms", "250", "--jitter-ms", "50",
                                 wait=False)
        with open(os.path.join(a.out, "meta.json"), "w", encoding="utf-8") as f:
            json.dump(meta, f, indent=1)
        t0 = time.time()
        next_at = t0
        webs = [web]
        while time.time() - t0 < a.seconds:
            next_at += a.every
            now = web_pid(app.pid) if app.alive() else None
            if now != webs[-1]:
                webs.append(now)
                print(f"web process {webs[-2]} -> {now} at {time.time() - t0:.0f} s", flush=True)
            if not app.alive():
                print(f"the app ended at {time.time() - t0:.0f} s", flush=True)
                break
            transport = keep_playing(app, a.scene) if a.scene in ("playing", "mixer", "recording") else None
            row = {"t": round(time.time() - t0, 1), "mono": round(time.monotonic(), 3), "load": os.getloadavg()[0], "transport": transport,
                   "webPid": now}
            for who, pid in (("web", now), ("meloet", app.pid)):
                if pid is None:
                    row[who] = None
                    continue
                b = breakdown(pid)
                row[who] = {"rollup": b and b["rollup"], "kindsMb": b and b["kindsMb"], "status": status(pid),
                            "fds": fds(pid)}
                m = mapped(pid)
                if m:
                    row[who]["maps"] = m["count"]
                    maps_out.write(json.dumps({"t": row["t"], "who": who, "pid": pid, "kept": m["kept"]}) + "\n")
            samples.write(json.dumps(row) + "\n")
            samples.flush()
            maps_out.flush()
            time.sleep(max(0.0, next_at - time.time()))
        meta["webs"] = webs
        if keys:
            keys.terminate()
            keys.wait()
    meta["exitStatus"] = app.exit_status
    meta["loadEnd"] = os.getloadavg()
    with open(os.path.join(a.out, "meta.json"), "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=1)


def load_rows(out):
    with open(os.path.join(out, "samples.jsonl"), encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def maps_at(out, who, times):
    found = {}
    with open(os.path.join(out, "maps.jsonl"), encoding="utf-8") as f:
        for line in f:
            row = json.loads(line)
            if row["who"] == who and row["t"] in times:
                found[row["t"]] = row["kept"]
    return found


def bucket(size):
    for limit, label in ((256 << 10, "<256K"), (1 << 20, "<1M"), (4 << 20, "<4M"), (16 << 20, "<16M")):
        if size < limit:
            return label
    return ">=16M"


def grown(before, after):
    """What the resident anonymous/shared memory grew by, split into: mappings that are new, by
    size and name, and mappings that were there and hold more (or less)."""
    old = {m[0]: m for m in before}
    parts = {}
    for m in after:
        start, size, rss, _, name = m
        kind = name or "anon"
        if kind.startswith("/memfd:"):
            kind = "memfd"
        if start in old and old[start][1] == size:
            key = f"same {kind}"
            delta = rss - old[start][2]
        else:
            key = f"new {kind} {bucket(size)}"
            delta = rss
        p = parts.setdefault(key, [0, 0])
        p[0] += delta
        p[1] += 1
    gone = sum(m[2] for s, m in old.items() if s not in {x[0] for x in after})
    parts["gone"] = [-gone, 0]
    return {k: [round(v[0] / MiB, 1), v[1]] for k, v in sorted(parts.items(), key=lambda kv: -abs(kv[1][0])) if v[0]}


def summarize(out):
    meta = json.load(open(os.path.join(out, "meta.json"), encoding="utf-8"))
    rows = load_rows(out)
    first = next((r for r in rows if r["t"] >= WARMUP_S and r.get("web")), None)
    last = next((r for r in reversed(rows) if r.get("web") and r["webPid"] == (first or {}).get("webPid")), None)
    line = {"run": os.path.basename(out.rstrip("/")), "scene": meta["scene"], "path": meta["path"],
            "env": {k: v for k, v in meta["env"].items() if not k.startswith("XDG_")},
            "compositor": meta.get("compositor"), "webs": meta.get("webs"), "seconds": rows[-1]["t"] if rows else 0}
    if not first or not last or last["t"] <= first["t"]:
        return line
    span = last["t"] - first["t"]
    for who in ("web", "meloet"):
        a, b = first[who], last[who]
        rss = (a["status"]["VmRSS"], b["status"]["VmRSS"])
        line[who] = {"rssMb": rss, "mbPerS": round((rss[1] - rss[0]) / span, 3),
                     "mbPerH": round((rss[1] - rss[0]) / span * 3600),
                     "anon": (a["status"]["RssAnon"], b["status"]["RssAnon"]),
                     "shmem": (a["status"]["RssShmem"], b["status"]["RssShmem"]),
                     "kinds": {k: round(b["kindsMb"].get(k, 0) - a["kindsMb"].get(k, 0), 1) for k in b["kindsMb"]},
                     "fds": (a["fds"]["n"], b["fds"]["n"]), "memfd": (a["fds"]["memfd"], b["fds"]["memfd"]),
                     "memfdMb": (a["fds"]["memfdMb"], b["fds"]["memfdMb"]),
                     "maps": (a.get("maps"), b.get("maps")), "threads": (a["status"]["Threads"], b["status"]["Threads"])}
        found = maps_at(out, who, {first["t"], last["t"]})
        if first["t"] in found and last["t"] in found:
            line[who]["grown"] = grown(found[first["t"]], found[last["t"]])
    # the slope over the last third, to see whether it levels off
    tail = [r for r in rows if r.get("web") and r["webPid"] == first["webPid"] and r["t"] >= last["t"] - span / 3]
    if len(tail) > 1:
        d = tail[-1]["web"]["status"]["VmRSS"] - tail[0]["web"]["status"]["VmRSS"]
        line["web"]["lastThirdMbPerS"] = round(d / (tail[-1]["t"] - tail[0]["t"]), 3)
    line["load"] = round(sum(r["load"] for r in rows) / len(rows), 1)
    return line


def write_summary(outs):
    lines = [summarize(o) for o in outs]
    with open(os.path.join(outs[0], "summary.json"), "w", encoding="utf-8") as f:
        json.dump(lines, f, indent=1, ensure_ascii=False)
    for line in lines:
        print(json.dumps(line, ensure_ascii=False))


def main():
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest="what", required=True)
    r = sub.add_parser("run")
    r.add_argument("out")
    r.add_argument("--scene", choices=["stopped", "playing", "mixer", "keys", "recording"], required=True)
    r.add_argument("--path", choices=["cpu", "comp"], default="cpu")
    r.add_argument("--seconds", type=float, default=1800)
    r.add_argument("--every", type=float, default=30)
    r.add_argument("--song")
    r.add_argument("--env", action="append", default=[])
    s = sub.add_parser("summary")
    s.add_argument("outs", nargs="+")
    a = p.parse_args()
    if a.what == "run":
        run(a)
    else:
        write_summary(a.outs)


if __name__ == "__main__":
    main()
