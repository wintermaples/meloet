#!/usr/bin/env python3
"""The start of the real app (release build) as the fonts see it: when the page first painted,
when each woff2 was there, and the memory some seconds after the launch.

usage:
    python3 -I tools/measure/font_start.py OUT [--song FILE] [--seconds 30] [--band mixer|pianoRoll]
                                               [--shot PNG]

Starts the app (remote.py: its own config, a virtual keyboard, the output chooser confirmed),
opens --song where it is (not saved), opens --band, waits until --seconds after the launch and
reads:

  launchToPageMs    the launch -> the page's time 0 (performance.timeOrigin)
  firstPaintMs      the page's time 0 -> first-contentful-paint (WebKit's paint timing)
  fonts             the faces of document.fonts (all, loaded), and `log.done`: [ms from the
                    page's time 0, faces loaded by then] each time the count changed in the
                    first 15 s (looked at every 8 ms; `atInstall`: those already loaded when the
                    watch began, right after the page finished loading)
  memory            RSS and PSS of meloet and its WebKitWebProcess (/proc)
  cpuMs             CPU time of the WebKitWebProcess so far (its main thread, all threads)

--shot: what the WebView shows at the end, as a PNG (to compare two builds pixel by pixel).

CARGO_TARGET_DIR names the build (remote.py). Writes OUT/summary.json. Changes nothing on the PC.
"""
import argparse, json, os, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from remote import App

PAGE = """
await document.fonts.ready;
const faces = [...document.fonts];
return {
  timeOriginMs: performance.timeOrigin,
  paint: Object.fromEntries(performance.getEntriesByType('paint').map((e) => [e.name, e.startTime])),
  log: window.__fontLog || null,
  faces: faces.length,
  loaded: faces.filter((f) => f.status === 'loaded').length,
};
"""


# Runs right after the page finished loading (MELOET_EVAL).
LISTEN = """
const count = () => { let n = 0; for (const f of document.fonts) if (f.status === 'loaded') n++; return n; };
const log = window.__fontLog = {atMs: performance.now(), atInstall: count(), done: []};
let last = log.atInstall;
const timer = setInterval(() => {
  const n = count(), now = performance.now();
  if (n !== last) { log.done.push([Math.round(now), n]); last = n; }
  if (now > 15000) clearInterval(timer);
}, 8);
"""


def cpu_ms(pid):
    def of(task):
        try:
            with open(f"/proc/{pid}/task/{task}/schedstat") as f:
                return int(f.read().split()[0]) / 1e6
        except (OSError, ValueError, IndexError):
            return 0.0
    return [round(of(pid)), round(sum(of(t) for t in os.listdir(f"/proc/{pid}/task")))]


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("out")
    p.add_argument("--song")
    p.add_argument("--seconds", type=float, default=30)
    p.add_argument("--band", choices=["mixer", "pianoRoll"])
    p.add_argument("--shot")
    a = p.parse_args()
    listen = json.dumps([{"afterMs": 0, "js": LISTEN}])
    with App(a.out, song=a.song, song_in_place=bool(a.song), env={"MELOET_EVAL": listen}) as app:
        if a.band:
            app.send({"type": "toggleBand", "band": a.band})
        time.sleep(max(0.0, a.seconds - (time.time() * 1000 - app.launched_ms) / 1000))
        page = app.js(PAGE)
        result = {
            "launchToPageMs": round(page["timeOriginMs"] - app.launched_ms),
            "firstPaintMs": page["paint"],
            "fonts": {"faces": page["faces"], "loaded": page["loaded"], "log": page["log"]},
            "atS": round((time.time() * 1000 - app.launched_ms) / 1000, 1),
            "memory": app.memory(),
            "cpuMs": cpu_ms(app.web_pid()),
        }
        if a.shot:
            app.shot(os.path.abspath(a.shot))
    with open(os.path.join(a.out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()
