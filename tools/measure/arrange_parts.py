#!/usr/bin/env python3
"""Where a frame of the arrange view goes while it is scrolled sideways or zoomed, part by part.

usage:
    python3 -I tools/measure/arrange_parts.py OUT --song FILE [--splits N] [--how frame|task]
                                              [--hide SELECTOR] [--tall]

Opens the song in the real app (remote.py), cuts every clip --splits times (S, not saved), and
sends wheel events from inside the page, one a frame (sideways; zooming out and in, twice; zooming
all the way out and then sideways there, where the most clips show). Around each event the page's
clock is read:

  jsMs      the wheel handler and Svelte's flush (the microtasks after it): the $derived values
            and the writes to the DOM
  layoutMs  a layout forced right after (offsetWidth)
  busyMs    of the engine's frame that drew the change (gl/profile.ts), with its parts
            (measureMs, paintMs, glMs). What is left of busyMs is WebKit's style and painting.
  cpuMsPerFrame  CPU time of the WebKitWebProcess over the part (its main thread, all threads),
            for each engine frame. Other work on the PC moves this less than it moves busyMs.

--how frame: the event is sent in an animation frame callback (what audio_app.py arrange does).
             The engine then asks for its frame after the callbacks of this frame ran, so it draws
             every second frame.
--how task:  the event is sent in a task after the frame (as a real wheel event comes).
--hide:      `display: none` on these elements first (to see what a part of the page costs).
--css:       a style sheet added to the page first (to try a change without building).

Writes OUT/summary.json. Changes nothing on the PC and does not save the song.
"""
import argparse, json, os, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from remote import App
from audio_app import BAR, audio_clips, vocal_tracks, wait_ready, finish

WHEEL = """
const el = document.querySelector('.lanes-view'), r = el.getBoundingClientRect();
const w = (dy, more) => el.dispatchEvent(new WheelEvent('wheel', {deltaY: dy, deltaMode: 0,
  clientX: r.left + r.width / 2, clientY: r.top + 40, bubbles: true, cancelable: true, ...more}));
const task = %s;
const out = [];
for (const [dy, more, n] of %s) for (let i = 0; i < n; i++) {
  await k.frame();
  if (task) await new Promise((done) => setTimeout(done, 0));
  const a = performance.now();
  w(dy, more);
  for (let j = 0; j < 8; j++) await Promise.resolve();
  const b = performance.now();
  void document.body.offsetWidth;
  out.push([a, b - a, performance.now() - b]);
}
await k.frame(); await k.frame();
return out;
"""

COUNT = """
const n = (s) => document.querySelectorAll(s).length;
return {labels: n('.lanes .clip-label'), handles: n('.lanes .handle'), all: n('.lanes *')};
"""


def q(values, x):
    v = sorted(values)
    return round(v[min(len(v) - 1, int(x * len(v)))], 2) if v else None


def cpu_ms(pid):
    """CPU time of the process so far: [its main thread, all its threads] (schedstat: ns on a CPU)."""
    def of(task):
        try:
            with open(f"/proc/{pid}/task/{task}/schedstat") as f:
                return int(f.read().split()[0]) / 1e6
        except (OSError, ValueError, IndexError):
            return 0.0
    return [of(pid), sum(of(t) for t in os.listdir(f"/proc/{pid}/task"))]


def stats(values):
    return {"mean": round(sum(values) / len(values), 2) if values else None, "p95": q(values, .95), "max": q(values, 1)}


def read(raw, events):
    """Each event with the first engine frame at or after it (and before the next event)."""
    with open(os.path.join(raw, "frame-profile.json")) as f:
        kept = json.load(f)["kept"]
    js, layout, busy, rest, parts = [], [], [], [], {k: [] for k in ("callbacksMs", "measureMs", "paintMs", "glMs")}
    each = []
    gaps = []
    last = None
    i = 0
    for n, (at, js_ms, layout_ms) in enumerate(events):
        until = events[n + 1][0] if n + 1 < len(events) else at + 100
        js.append(js_ms)
        layout.append(layout_ms)
        while i < len(kept) and kept[i]["atMs"] < at - 1:
            i += 1
        if i < len(kept) and kept[i]["atMs"] < until and kept[i]["busyMs"] is not None:
            f = kept[i]
            busy.append(f["busyMs"])
            each.append([n, js_ms, layout_ms, f["busyMs"], f["glMs"]])
            for k in parts:
                parts[k].append(f[k])
            rest.append(f["busyMs"] - f["callbacksMs"] - f["measureMs"] - f["paintMs"] - f["glMs"])
            if last is not None:
                gaps.append(f["atMs"] - last)
            last = f["atMs"]
    return {"events": len(events), "engineFrames": len(busy), "jsMs": stats(js), "layoutMs": stats(layout),
            "busyMs": stats(busy), "restOfBusyMs": stats(rest), "frameGapMs": {"p50": q(gaps, .5), "max": q(gaps, 1)},
            **{k: stats(v) for k, v in parts.items()},
            # For each engine frame: the event's number, jsMs, layoutMs, busyMs, glMs.
            "each": each}


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("out")
    p.add_argument("--song", required=True)
    p.add_argument("--splits", type=int, default=0)
    p.add_argument("--how", choices=["frame", "task"], default="frame")
    p.add_argument("--hide", action="append", default=[])
    p.add_argument("--tall", action="store_true")
    p.add_argument("--css", default="")
    p.add_argument("--hold", type=int, default=60, help="how many times to go right and left when zoomed all the way out")
    a = p.parse_args()
    result = {"song": a.song, "splits": a.splits, "how": a.how, "hide": a.hide, "tall": a.tall}
    events = {}
    cpu = {}
    with App(a.out, song=a.song, song_in_place=True) as app:
        state = wait_ready(app) if audio_clips(app.snapshot()) else app.snapshot()
        for i in range(a.splits):
            app.send({"type": "locate", "tick": (i + 1) * (110 // max(1, a.splits)) * BAR})
            app.send({"type": "selectAllClips"})
            app.send({"type": "splitClips"})
            time.sleep(0.15)
        app.send({"type": "selectClip", "clip": None})
        app.send({"type": "goToStart"})
        if a.tall:
            vocals = vocal_tracks(app.snapshot())
            if vocals:
                app.send({"type": "setTallTrack", "track": vocals[0]["id"]})
        time.sleep(1.5)
        result["clips"] = len(audio_clips(app.snapshot()))
        for selector in a.hide:
            app.js("for (const e of document.querySelectorAll(%s)) e.style.display = 'none';" % json.dumps(selector))
        if a.css:
            app.js("const s = document.createElement('style'); s.textContent = %s; document.head.appendChild(s);" % json.dumps(a.css))
        result["nodes"] = app.js(COUNT)
        task = "true" if a.how == "task" else "false"
        for name, steps in (("sideways", [[120, {"shiftKey": True}, 120], [-120, {"shiftKey": True}, 120]]),
                            ("zoom", [[-100, {"ctrlKey": True}, 40], [100, {"ctrlKey": True}, 40]]),
                            ("zoom2", [[-100, {"ctrlKey": True}, 40], [100, {"ctrlKey": True}, 40]]),
                            ("out", [[100, {"ctrlKey": True}, 40]]),
                            ("sidewaysOut", [[40, {"shiftKey": True}, 1], [-40, {"shiftKey": True}, 1]] * a.hold)):
            before = cpu_ms(app.web_pid())
            events[name] = app.js(WHEEL % (task, json.dumps(steps)), timeout=120 + a.hold)
            cpu[name] = [round(b - a, 1) for a, b in zip(before, cpu_ms(app.web_pid()))]
            if name == "out":
                result["nodesOut"] = app.js(COUNT)
            time.sleep(0.5)
        time.sleep(2.5)
    for name, ev in events.items():
        result[name] = read(os.path.join(a.out, "raw"), ev)
        frames = max(1, result[name]["engineFrames"])
        result[name]["cpuMsPerFrame"] = {"mainThread": round(cpu[name][0] / frames, 2), "process": round(cpu[name][1] / frames, 2)}
    finish(a.out, result)


if __name__ == "__main__":
    main()
