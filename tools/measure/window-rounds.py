#!/usr/bin/env python3
"""Plugin windows in the real app, many times over: open and close, and visits (spec 15-C).

usage: python3 -I tools/measure/window-rounds.py open-close [--rounds 50] [--out DIR]
       python3 -I tools/measure/window-rounds.py visits [--rounds 220] [--seed 1] [--out DIR]

Both open a song with Surge XT (VST3) as the instrument of track 1 and Dragonfly Hall Reverb
(LV2) in an insert (made here with make_song.py), and drive the release build through
remote.py. The windows come up on the X display for real. The sound goes to the chosen output.

open-close: the song plays in a loop and a virtual keyboard plays into Surge XT. First a
stretch with no window (the same length as the rounds take), then --rounds times: open Surge
XT's window, 0.3 s, close it; the same for Dragonfly. Writes the time from the intent to the
window being in the state (measured in the page, about 2 ms fine), the RSS and PSS of meloet,
its WebKitWebProcess and the plugin hosts every 10 rounds, and the blocks whose answer came
too late (`plugins.json`) for the stretch without and with the windows.

visits: --rounds steps drawn from: a visit that changes something (the keyboard goes to the
plugin's window, a made-up pointer drag inside it, the keyboard goes away), a visit that
changes nothing, undo, redo, a visit with a value set in the parameters panel in the middle.
After each step the count of undo steps and the plugin's own state (the song is saved, which
is no step, and Surge XT's bytes are read from the file) are compared with what a model of the
history says.
Writes every step and every mismatch. Made-up events only: the real pointer is not touched.
"""
import argparse, base64, hashlib, json, os, random, re, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from remote import App, X, RemoteError, make_song, target

INST = {"kind": "instrument", "track": 1}
REVERB = {"kind": "insert", "insert": {"strip": {"kind": "track", "id": 1}, "slot": 2}}


def song_file(out):
    os.makedirs(out, exist_ok=True)
    return make_song(os.path.join(out, "windows.meloet"), "--tracks", "2", "--notes", "120", "--bars", "8",
                     "--instruments", "plugin,builtin", "--plugin", "Surge XT",
                     "--effect-plugin", "Dragonfly Hall Reverb", "--effect-format", "lv2")


OPEN_JS = """
const slot = %s, want = %s;
const has = (s) => s.windows.list.some((w) => JSON.stringify(w.slot) === JSON.stringify(slot));
const t0 = performance.now();
await invoke('send', {command: {type: want ? 'openPluginWindow' : 'closePluginWindow', slot}});
for (;;) {
  const s = (await invoke('snapshot')).state;
  if (has(s) === want) return performance.now() - t0;
  if (performance.now() - t0 > 8000) throw new Error('window: waited 8 s');
}
"""


def late_blocks(app):
    try:
        with open(os.path.join(app.measure, "plugins.json")) as f:
            return {p["pid"]: p for p in json.load(f)["processes"]}
    except (OSError, ValueError):
        return {}


def quantiles(values):
    v = sorted(values)
    pick = lambda q: round(v[min(len(v) - 1, int(q * len(v)))], 1)
    return {"n": len(v), "min": round(v[0], 1), "p50": pick(0.5), "p95": pick(0.95), "max": round(v[-1], 1)}


def open_close(a):
    out = a.out or os.path.join(target, "measure", "window-open-close")
    report = {"rounds": a.rounds, "load": os.getloadavg()[0], "memory": [], "open": {}, "close": {}}
    with App(out, song=song_file(out)) as app:
        app.send({"type": "setLoop", "fromBar": 1, "toBar": 9})
        app.send({"type": "playStop"})
        keys = app.play_keys("--notes", "60,64,67,72", "--count", "100000", "--interval-ms", "240", "--hold-ms", "150",
                             "--velocity", "50", wait=False)
        time.sleep(2)
        report["memory"].append({"round": "before", **app.memory()})
        # One round of each first, to know how long the stretch without windows has to be.
        opens = {"Surge XT": [], "Dragonfly Hall Reverb": []}
        closes = {"Surge XT": [], "Dragonfly Hall Reverb": []}

        def one_round():
            for name, slot in (("Surge XT", INST), ("Dragonfly Hall Reverb", REVERB)):
                opens[name].append(app.js(OPEN_JS % (json.dumps(slot), "true")))
                time.sleep(0.3)
                closes[name].append(app.js(OPEN_JS % (json.dumps(slot), "false")))
                time.sleep(0.05)

        t0 = time.time()
        one_round()
        per_round = time.time() - t0
        report["memory"].append({"round": 1, **app.memory()})
        time.sleep(1.2)
        quiet_from = late_blocks(app)
        time.sleep(per_round * (a.rounds - 1))
        time.sleep(1.2)
        quiet_to = late_blocks(app)
        for i in range(2, a.rounds + 1):
            one_round()
            if i % 10 == 0:
                report["memory"].append({"round": i, **app.memory()})
                print(f"round {i}: {report['memory'][-1]['meloet']}", flush=True)
            if not app.alive():
                raise RemoteError(f"the app ended in round {i}")
        time.sleep(1.2)
        busy_to = late_blocks(app)
        report["hostsAlive"] = len(app.hosts())
        report["transport"] = app.snapshot()["transport"]["state"]
        keys.terminate()
        keys.wait()

        def late(a_, b_):
            return {str(pid): {"unansweredBlocks": b_[pid]["unansweredBlocks"] - a_.get(pid, {}).get("unansweredBlocks", 0),
                               "waitedMs": round(b_[pid]["waitedMs"] - a_.get(pid, {}).get("waitedMs", 0), 1),
                               "key": b_[pid]["key"]} for pid in b_}

        report["seconds"] = round(per_round * (a.rounds - 1), 1)
        report["lateWithoutWindows"] = late(quiet_from, quiet_to)
        report["lateWithWindows"] = late(quiet_to, busy_to)
        report["open"] = {k: quantiles(v) for k, v in opens.items()}
        report["close"] = {k: quantiles(v) for k, v in closes.items()}
        report["openFirst"] = {k: round(v[0], 1) for k, v in opens.items()}
    report["exitStatus"] = app.exit_status
    with open(os.path.join(out, "report.json"), "w") as f:
        json.dump(report, f, indent=1)
    print(json.dumps({k: report[k] for k in ("open", "close", "openFirst", "lateWithoutWindows", "lateWithWindows",
                                             "seconds", "hostsAlive", "exitStatus")}, indent=1))
    for m in report["memory"]:
        hosts = " ".join(f"{h['rssMb']}/{h['pssMb']}" for h in m["hosts"].values() if h)
        print(m["round"], "meloet", m["meloet"]["rssMb"], m["meloet"]["pssMb"], "web", m["web"]["rssMb"],
              m["web"]["pssMb"], "hosts", hosts)


# Places in Surge XT's window (1597x995) where a made-up drag changes something, found by trying.
SPOTS = [("polyphony", 245, 80), ("sceneMode", 95, 80), ("patchNext", 470, 80), ("patchPrev", 320, 80)]
POLYPHONY = 240235375  # Surge XT's VST3 ParamID of "Polyphony Limit"


class Visits:
    def __init__(self, app, x):
        self.app, self.x = app, x
        self.win = None
        self.raw = {}

    def differing(self, a, b):
        """Where two states (by hash) differ: up to 6 places, each with the text around it."""
        import difflib
        ta, tb = self.raw[a].decode("latin-1"), self.raw[b].decode("latin-1")
        out = []
        for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, ta, tb, autojunk=False).get_opcodes():
            if tag != "equal":
                out.append({"at": i1, "before": ta[max(0, i1 - 60):i1], "want": ta[i1:i2][:80], "got": tb[j1:j2][:80]})
        return {"bytes": [len(ta), len(tb)], "places": len(out), "first": out[:6]}

    def state(self):
        """What the plugin itself holds now: the song is saved (no step; meloet asks the plugin
        for its state then) and Surge XT's bytes are read from the file. (hash, a few words)."""
        before = os.stat(self.app.song).st_mtime_ns
        self.app.send({"type": "saveSong"})
        until = time.time() + 5
        while os.stat(self.app.song).st_mtime_ns == before:
            if time.time() > until:
                raise RemoteError("the song was not saved in 5 s")
            time.sleep(0.004)
        with open(self.app.song, encoding="utf-8") as f:
            data = json.load(f)
        raw = base64.b64decode(data["song"]["tracks"][0]["instrument"].get("state") or "")
        text = raw.decode("latin-1")
        words = [m.group(1) for m in (re.search(r'<meta name="([^"]*)"', text),
                                       re.search(r'<polylimit [^>]*value="([^"]*)"', text),
                                       re.search(r'<scenemode [^>]*value="([^"]*)"', text)) if m]
        digest = hashlib.sha256(raw).hexdigest()[:12]
        self.raw[digest] = raw
        return digest, " / ".join(words)

    def count(self):
        s = self.app.snapshot()["song"]
        return s["undoCount"], s["redoCount"]

    def wait_count(self, want, timeout=2.5):
        until = time.time() + timeout
        while time.time() < until:
            if self.count() == want:
                break
            time.sleep(0.03)
        # It must also stay: a second step that comes late would be a mistake.
        time.sleep(0.2)
        return self.count()

    def enter(self):
        self.x.focus(self.win)
        time.sleep(0.12)

    def leave(self):
        self.x.unfocus()
        time.sleep(0.3)

    def poke(self, rng):
        name, px, py = rng.choice(SPOTS)
        dy = rng.choice([-40, -25, 25, 40])
        self.x.drag(self.win, px, py, dy, steps=6, pause=0.008)
        time.sleep(0.1)
        return f"{name} {dy:+d}"


def visits(a):
    out = a.out or os.path.join(target, "measure", "window-visits")
    rng = random.Random(a.seed)
    steps = []
    with App(out, song=song_file(out)) as app:
        x = X()
        v = Visits(app, x)
        app.send({"type": "selectInstrument", "track": 1})
        app.wait(lambda s: s["windows"]["list"], 8, "Surge XT's window")
        time.sleep(0.6)
        host = next(pid for pid in app.hosts() if x.find(pid, "Surge XT"))
        v.win = x.find(host, "Surge XT")
        x.unfocus()
        time.sleep(0.4)
        # The model: the plugin's state at each place of the history, and where we are in it.
        # A place the model does not know (after a mismatch) is None until it is seen.
        cur = v.state()
        history = [cur]
        at = 0

        def moved(note):
            """After something that may have changed the plugin: a new place when it did."""
            nonlocal cur, at
            now = v.state()
            if now[0] != cur[0]:
                history[at + 1:] = [now]
                at += 1
                cur = now
                return note
            return note + " (nothing moved)"

        t0 = time.time()
        for i in range(a.rounds):
            kind = rng.choices(["change", "nothing", "undo", "redo", "panel"], [45, 10, 22, 13, 10])[0]
            before = v.count()
            note = ""
            if kind == "change":
                v.enter()
                note = v.poke(rng)
                if rng.random() < 0.3:
                    note += ", " + v.poke(rng)
                v.leave()
                note = moved(note)
            elif kind == "nothing":
                v.enter()
                time.sleep(rng.choice([0.05, 0.2]))
                v.leave()
            elif kind == "undo":
                app.send({"type": "undo"})
                at = max(0, at - 1)
            elif kind == "redo":
                app.send({"type": "redo"})
                at = min(len(history) - 1, at + 1)
            elif kind == "panel":
                # In the window, a change there, then a value from the panel, then out: the
                # window's part is one step and the panel's value another.
                v.enter()
                note = moved(v.poke(rng))
                sel = app.snapshot()["params"]["selected"]
                value = rng.choice([0.1, 0.3, 0.5, 0.7])
                app.send({"type": "setPluginParam", "slot": INST, "table": sel["table"], "param": POLYPHONY,
                          "value": value, "commit": True})
                time.sleep(0.3)
                v.leave()
                note = moved(note + f", panel polyphony {value}")
            want = (at, len(history) - 1 - at)
            got = v.wait_count(want)
            now = v.state()
            if history[at] is None:
                history[at] = now
            step = {"i": i, "kind": kind, "note": note, "before": before, "want": want, "got": got,
                    "state": now, "wantState": history[at],
                    "ok": got == want and now[0] == history[at][0]}
            cur = now
            steps.append(step)
            if now[0] != history[at][0]:
                step["differs"] = v.differing(history[at][0], now[0])
            if not step["ok"]:
                print("MISMATCH", json.dumps(step, ensure_ascii=False), flush=True)
                # Go on from what the app has, so that one mismatch is counted once.
                history = [None] * got[0] + [now] + [None] * got[1]
                at = got[0]
            if i % 20 == 19:
                print(f"step {i + 1}: undo/redo {got}, {round(time.time() - t0)} s", flush=True)
            if not app.alive() or not app.snapshot()["windows"]["list"]:
                raise RemoteError(f"the app or the window went away at step {i}")
        memory = app.memory()
        final = v.count()
    kinds = {}
    for s in steps:
        k = kinds.setdefault(s["kind"], {"n": 0, "bad": 0, "moved": 0})
        k["n"] += 1
        k["bad"] += not s["ok"]
        k["moved"] += s["kind"] in ("change", "panel") and "(nothing moved)" not in s["note"]
    report = {"rounds": a.rounds, "seed": a.seed, "kinds": kinds, "mismatches": [s for s in steps if not s["ok"]],
              "final": final, "memory": memory, "exitStatus": app.exit_status, "steps": steps}
    with open(os.path.join(out, "report.json"), "w") as f:
        json.dump(report, f, indent=1, ensure_ascii=False)
    print(json.dumps({k: report[k] for k in ("rounds", "kinds", "final", "exitStatus")}, ensure_ascii=False))
    print("mismatches:", len(report["mismatches"]))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("what", choices=["open-close", "visits"])
    p.add_argument("--rounds", type=int)
    p.add_argument("--seed", type=int, default=1)
    p.add_argument("--out")
    a = p.parse_args()
    a.rounds = a.rounds or (50 if a.what == "open-close" else 220)
    (open_close if a.what == "open-close" else visits)(a)


main()
