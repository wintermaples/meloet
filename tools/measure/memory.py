#!/usr/bin/env python3
"""Where the memory of the real app (release build) is: meloet, its WebKitWebProcess and its
plugin processes, in scenes.

    python3 -I tools/measure/memory.py scenes OUT [--scenes start,piano,band8,surge16,vocal16,letgo]
                                                  [--songs DIR] [--settle 20] [--pack DIR]
    python3 -I tools/measure/memory.py watch OUT PID [--every 10] [--seconds 700]
    python3 -I tools/measure/memory.py one PID
    python3 -I tools/measure/memory.py summary OUT

scenes: starts the app once a scene (remote.py: its own config and state, a virtual keyboard),
  waits --settle seconds after the song is open and its sounds are read, and writes down the
  breakdown below (OUT/<scene>.json, OUT/summary.md). The songs are --songs/<name>.meloet:
  sine (1 track, the sine: make_song.py without --tracks), band8-all, surge16 and vocal16/vocal16.meloet (a folder with its
  audio). "piano" is a new song (its first track is the grand piano). "letgo" opens band8-all,
  then: every track deleted, all undone, the song closed (a new one), the shelf's
  strings auditioned and the shelf closed, with a breakdown after each step. "fetch" (only when
  named, with --pack DIR: a folder with catalog.json and its tars) starts with no sound pack
  (a new data folder; the development pack beside the program set aside meanwhile), fetches
  the whole pack from DIR served over http on this PC, and looks again.
watch: the breakdown of PID (meloet) and its WebKitWebProcess every --every seconds, into
  OUT/watch.jsonl, until the process ends (long-record.sh runs beside it).
one: the breakdown of one process, printed.

The breakdown of a process (/proc/<pid>/smaps): RSS and PSS (smaps_rollup), and the resident
memory sorted into: the main heap ([heap]), malloc's other arenas (anonymous read-write
mappings that lie in a 64 MiB-aligned reservation with its PROT_NONE rest, which is how
glibc's thread arenas look), other anonymous mappings (big blocks malloc maps on their own,
thread stacks, JavaScriptCore's heap in WebKit), shared memory (memfd, /dev/shm: the plugin
processes' blocks), the GPU driver (/dev/nvidia*, /dev/dri), and files (code and data of the
program and its libraries). With MELOET_MEASURE_DIR the app also writes memory.json (the
samples of each instrument in memory, and what else the core counts); it goes in as "core".
Reads /proc only; changes nothing on the PC.
"""
import argparse, json, os, re, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from remote import App, comm, descendants, plughosts

MiB = 1024 * 1024
ARENA = 64 * MiB
HEAD = re.compile(r"^([0-9a-f]+)-([0-9a-f]+) (\S+) \S+ \S+ \S+\s*(.*)$")


def mappings(pid):
    out, cur = [], None
    try:
        with open(f"/proc/{pid}/smaps") as f:
            for line in f:
                m = HEAD.match(line)
                if m:
                    cur = {"start": int(m[1], 16), "end": int(m[2], 16), "perm": m[3], "name": m[4].strip(),
                           "Rss": 0, "Pss": 0, "Anonymous": 0, "Swap": 0}
                    out.append(cur)
                    continue
                key, _, rest = line.partition(":")
                if cur is not None and key in ("Rss", "Pss", "Anonymous", "Swap"):
                    cur[key] = int(rest.split()[0]) * 1024
    except OSError:
        return None
    return out


def rollup(pid):
    got = {}
    try:
        for line in open(f"/proc/{pid}/smaps_rollup"):
            key, _, rest = line.partition(":")
            if key in ("Rss", "Pss", "Pss_Anon", "Pss_File", "Pss_Shmem", "Swap"):
                got[key] = round(int(rest.split()[0]) / 1024, 1)
    except OSError:
        return None
    return got


def kind(m, arena_bases):
    name = m["name"]
    if name == "[heap]":
        return "heap"
    if name.startswith("[stack"):
        return "stack"
    if name.startswith(("/memfd:", "/dev/shm/", "/SYSV")):
        return "shm"
    if name.startswith(("/dev/nvidia", "/dev/dri", "/dev/char")) or "nvidia" in name.lower():
        return "gpu"
    if name.startswith("/"):
        return "file"
    if name.startswith("["):
        return "other"
    if m["start"] - m["start"] % ARENA in arena_bases:
        return "arena"
    return "anon"


def breakdown(pid, top=8):
    maps = mappings(pid)
    if maps is None:
        return None
    # glibc's arenas: a 64 MiB-aligned reservation, read-write at its head, PROT_NONE after.
    arena_bases = set()
    for i, m in enumerate(maps):
        if (m["name"] == "" and m["start"] % ARENA == 0 and m["perm"].startswith("rw")
                and i + 1 < len(maps) and maps[i + 1]["perm"].startswith("---")
                and maps[i + 1]["start"] == m["end"] and maps[i + 1]["end"] - m["start"] <= ARENA):
            arena_bases.add(m["start"])
    kinds, files, big = {}, {}, []
    for m in maps:
        k = kind(m, arena_bases)
        kinds[k] = kinds.get(k, 0) + m["Rss"]
        if k == "file":
            base = os.path.basename(m["name"].split(" (deleted)")[0])
            files[base] = files.get(base, 0) + m["Rss"]
        if k in ("anon", "arena", "heap") and m["Rss"] >= 4 * MiB:
            big.append((m["Rss"], m["end"] - m["start"], k))
    big.sort(reverse=True)
    mb = lambda b: round(b / MiB, 1)
    return {
        "rollup": rollup(pid),
        "kindsMb": {k: mb(v) for k, v in sorted(kinds.items(), key=lambda kv: -kv[1])},
        "arenas": len(arena_bases),
        "bigAnon": [{"rssMb": mb(r), "sizeMb": mb(s), "kind": k} for r, s, k in big[:top]],
        "filesMb": {k: mb(v) for k, v in sorted(files.items(), key=lambda kv: -kv[1])[:top]},
    }


def web_of(pid):
    return next((c for c in descendants(pid) if comm(c) == "WebKitWebProces"), None)


def everything(pid, measure_dir=None):
    out = {"wallMs": round(time.time() * 1000), "meloet": breakdown(pid)}
    web = web_of(pid)
    out["web"] = breakdown(web) if web else None
    out["hosts"] = {str(h): breakdown(h) for h in plughosts(pid)}
    if measure_dir:
        try:
            with open(os.path.join(measure_dir, "memory.json"), encoding="utf-8") as f:
                out["core"] = json.load(f)
        except (OSError, ValueError):
            out["core"] = None
    return out


def js_heap(app):
    """What the page can tell of its own memory (WebKit has no performance.memory)."""
    return app.js("""
const gl = [...document.querySelectorAll('canvas')];
return {canvases: gl.length,
        canvasPixels: gl.reduce((s, c) => s + c.width * c.height, 0),
        nodes: document.getElementsByTagName('*').length,
        mem: (performance.memory ? {used: performance.memory.usedJSHeapSize} : null)};""")


def core_counts(app):
    try:
        with open(os.path.join(app.measure, "memory.json"), encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def settle(app, seconds):
    """Waits until no instrument is being read (the core's memory.json says so), then `seconds`."""
    until = time.time() + 120
    time.sleep(3)
    while time.time() < until:
        counts = core_counts(app)
        if counts and counts["model"]["loading"] == 0:
            break
        time.sleep(0.5)
    time.sleep(seconds)


def scene(out, name, song, steps, settle_s, song_in_place=False, sf2=False, env=None):
    print(f"-- {name}", flush=True)
    result = {"scene": name, "load": os.getloadavg(), "steps": []}
    with App(os.path.join(out, name), song=song, song_in_place=song_in_place, sf2=sf2, env=env) as app:
        settle(app, settle_s)
        snap = everything(app.pid, app.measure)
        snap["step"] = "open"
        snap["page"] = js_heap(app)
        result["steps"].append(snap)
        for label, act in steps:
            act(app)
            settle(app, settle_s)
            snap = everything(app.pid, app.measure)
            snap["step"] = label
            snap["page"] = js_heap(app)
            result["steps"].append(snap)
    result["loadAfter"] = os.getloadavg()
    with open(os.path.join(out, f"{name}.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)
    return result


def tracks(app):
    return app.snapshot()["song"]["tracks"]


def delete_tracks(app):
    for t in tracks(app):
        app.send({"type": "deleteTrack", "track": t["id"]})
        time.sleep(0.3)


def undo_all(app):
    for _ in range(len(app.snapshot()["song"].get("tracks", [])) + 20):
        app.send({"type": "undo"})
        time.sleep(0.1)


def new_song(app):
    app.send({"type": "newSong"})
    dialog = app.wait(lambda s: s["workspace"]["songDialog"] or s["song"]["file"] == "neverSaved", 10)
    if dialog["workspace"]["songDialog"]:
        app.send({"type": "answerSongDialog", "dialog": dialog["workspace"]["songDialog"]["id"],
                  "answer": "dontSave"})


def audition(instrument):
    def act(app):
        app.send({"type": "openShelf", "mode": {"kind": "newTrack"}})
        time.sleep(0.5)
        app.send({"type": "shelfSelectSound", "sound": {"kind": "builtin", "instrument": instrument}})
    return act


def shelf_close(app):
    app.send({"type": "shelfCancel"})
    time.sleep(0.3)
    app.send({"type": "toggleBand", "band": "shelf"})


def fetch_all(app):
    app.send({"type": "fetchPack"})
    app.wait(lambda s: s["sounds"]["have"] == s["sounds"]["total"] and s["sounds"]["fetch"]["state"] == "idle",
             600, "the whole pack", every=1.0)


class LocalPack:
    """The pack of --pack (catalog.json and its tars) served over http on this PC, with the
    catalog's URLs pointing here; the app's development sound pack beside it is set aside
    meanwhile, and the app's data folder is a new one, so that everything is fetched."""

    def __init__(self, pack, work):
        import functools, http.server, threading
        self.serve = os.path.join(work, "served")
        os.makedirs(self.serve, exist_ok=True)
        self.server = http.server.ThreadingHTTPServer(
            ("127.0.0.1", 0), functools.partial(Quiet, directory=self.serve))
        origin = f"http://127.0.0.1:{self.server.server_address[1]}/"
        with open(os.path.join(pack, "catalog.json"), encoding="utf-8") as f:
            catalog = json.load(f)
        for item in [catalog["common"], *catalog["instruments"]]:
            os.symlink(os.path.abspath(os.path.join(pack, item["file"])), os.path.join(self.serve, item["file"]))
            item["url"] = origin + item["file"]
        self.catalog = os.path.join(self.serve, "catalog.json")
        with open(self.catalog, "w", encoding="utf-8") as f:
            json.dump(catalog, f)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.env = {"MELOET_PACK_ORIGIN": origin, "MELOET_PACK_CATALOG": self.catalog,
                    "XDG_DATA_HOME": os.path.join(work, "data")}
        from remote import app_bin
        self.pinned = os.path.join(os.path.dirname(app_bin), "sounds")
        self.aside = self.pinned + ".memory-aside"
        if os.path.lexists(self.pinned):
            os.rename(self.pinned, self.aside)

    def close(self):
        self.server.shutdown()
        if os.path.lexists(self.aside):
            os.rename(self.aside, self.pinned)


def quiet_handler():
    import http.server

    class Handler(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *args):
            pass
    return Handler


Quiet = quiet_handler()


def run_scenes(a):
    os.makedirs(a.out, exist_ok=True)
    songs = os.path.abspath(a.songs)
    plan = {
        "start": (os.path.join(songs, "sine.meloet"), [], False, False),
        "piano": (None, [], False, False),
        "band8": (os.path.join(songs, "band8-all.meloet"), [], False, False),
        "surge16": (os.path.join(songs, "surge16.meloet"), [], False, False),
        "vocal16": (os.path.join(songs, "vocal16", "vocal16.meloet"), [], True, False),
        "letgo": (os.path.join(songs, "band8-all.meloet"),
                  [("tracks deleted", delete_tracks), ("undone", undo_all), ("new song", new_song),
                   ("strings auditioned", audition("strings")), ("shelf closed", shelf_close)],
                  False, False),
    }
    for name in a.scenes.split(","):
        if name == "fetch":
            if not a.pack:
                raise SystemExit("the fetch scene needs --pack")
            local = LocalPack(a.pack, os.path.join(os.path.abspath(a.out), "fetch-work"))
            try:
                scene(a.out, name, None, [("all fetched", fetch_all)], a.settle, env=local.env)
            finally:
                local.close()
        else:
            song, steps, in_place, sf2 = plan[name]
            scene(a.out, name, song, steps, a.settle, in_place, sf2)
        time.sleep(3)
    write_summary(a.out)


def write_summary(out):
    """OUT/summary.md from every OUT/<scene>.json, in the order of the scenes above."""
    order = ["start", "piano", "band8", "surge16", "vocal16", "letgo", "fetch"]
    names = sorted((n[:-5] for n in os.listdir(out) if n.endswith(".json")),
                   key=lambda n: (order.index(n) if n in order else len(order), n))
    results = []
    for n in names:
        with open(os.path.join(out, f"{n}.json"), encoding="utf-8") as f:
            results.append(json.load(f))
    text = summary(results)
    with open(os.path.join(out, "summary.md"), "w", encoding="utf-8") as f:
        f.write(text)
    print(text)


def row(label, b):
    if not b:
        return f"| {label} | — |"
    r = b["rollup"] or {}
    k = b["kindsMb"]
    return (f"| {label} | {r.get('Rss')} | {r.get('Pss')} | {k.get('heap', 0)} | {k.get('arena', 0)} ({b['arenas']}) | "
            f"{k.get('anon', 0)} | {k.get('shm', 0)} | {k.get('gpu', 0)} | {k.get('file', 0)} |")


def summary(results):
    lines = ["| Scene / step / process | RSS | PSS | heap | arena (count) | anon | shm | gpu | file |",
             "|---|---|---|---|---|---|---|---|---|"]
    for r in results:
        for s in r["steps"]:
            where = f"{r['scene']} / {s['step']}"
            lines.append(row(f"{where} / meloet", s["meloet"]))
            lines.append(row(f"{where} / web", s["web"]))
            for pid, h in s["hosts"].items():
                lines.append(row(f"{where} / plughost {pid}", h))
            core = s.get("core") or {}
            held = [f"{b['name']} {b['mb']:.0f}" for b in core.get("sounds", []) + core.get("sf2", [])]
            if held:
                lines.append(f"| {where} / samples (MiB) | {', '.join(held)} |||||||||")
            m = core.get("malloc")
            if m:
                lines.append(f"| {where} / malloc (MiB) | in use {m['inUseMb']:.0f}, free {m['freeMb']:.0f}"
                             f" (of which {m['mappedMb']:.0f} in separate mappings, in use) |||||||||")
    return "\n".join(lines) + "\n"


def run_watch(a):
    os.makedirs(a.out, exist_ok=True)
    path = os.path.join(a.out, "watch.jsonl")
    end = time.time() + a.seconds
    with open(path, "a", encoding="utf-8") as f:
        while time.time() < end and os.path.exists(f"/proc/{a.pid}"):
            snap = everything(a.pid, a.measure_dir)
            if snap["meloet"] is None:
                break
            f.write(json.dumps(snap) + "\n")
            f.flush()
            time.sleep(a.every)


def main():
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest="what", required=True)
    s = sub.add_parser("scenes")
    s.add_argument("out")
    s.add_argument("--scenes", default="start,piano,band8,surge16,vocal16,letgo")
    s.add_argument("--songs", default="target/measure/songs")
    s.add_argument("--settle", type=float, default=20)
    s.add_argument("--pack", help="fetch: a folder with catalog.json and the tars it names")
    w = sub.add_parser("watch")
    w.add_argument("out")
    w.add_argument("pid", type=int)
    w.add_argument("--every", type=float, default=10)
    w.add_argument("--seconds", type=float, default=700)
    w.add_argument("--measure-dir")
    o = sub.add_parser("one")
    o.add_argument("pid", type=int)
    m = sub.add_parser("summary")
    m.add_argument("out")
    a = p.parse_args()
    if a.what == "scenes":
        run_scenes(a)
    elif a.what == "summary":
        write_summary(a.out)
    elif a.what == "watch":
        run_watch(a)
    else:
        print(json.dumps(everything(a.pid), indent=1))


if __name__ == "__main__":
    main()
