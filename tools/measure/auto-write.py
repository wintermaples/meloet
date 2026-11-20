#!/usr/bin/env python3
"""Where points written with 「● 書く」 land, against when the fader was moved.

usage: python3 -I tools/measure/auto-write.py OUT [--state recording|playing] [--drags 10]
                                              [--moves 6] [--song X.meloet]
       python3 -I tools/measure/auto-write.py OUT --long SECONDS [--hold-ms 2000] [--song X.meloet]

Starts the real app (release) with a song of one track at 125 bpm (a tick is 1 ms) whose volume
lane is one flat line, opens the mixer, turns 「● 書く」 on for the volume and starts. Then the
page gets made-up pointer events on the track's fader: down, --moves moves about 230 ms apart
(zigzag, so no point is thinned away), up. The time of every event is the page's `Date.now()`
right before it is dispatched (whole ms), brought to CLOCK_MONOTONIC here, with no trip
through the app in between. The song is saved and the lane's points are read from the file.
The pointer events are made up inside the page (the window is a Wayland one, so made-up X
events do not reach it): the way from the mouse to the page is not in the numbers.

Two rulers say when a tick was heard:
- recording: a virtual keyboard plays all along; the recorded notes (placed where they were
  heard when pressed, docs/architecture.md 12) against the times the keys were sent give
  `time = a + b * tick`.
- both states: the place where the song stopped (the place heard when the core took the stop)
  against the time the page sent the stop. This one is late by the page -> core delay of that
  one command, so it reads the lag too small by that much.
A point's lag is `time heard(tick) - time dispatched`: what the way from the page to the core
adds (a move is sent from the page's next frame callback, the let-go at once).

--long: instead, writes for SECONDS without a stop: a move every frame, let go and pressed
again every --hold-ms (each let-go is one step), and writes the RSS of meloet and the number of
points after every step (what the history holds: a copy of the lane's points per step,
docs/architecture.md 21-5).

Writes OUT/summary.md and OUT/points.json (or OUT/long.json). Changes nothing on the PC; the
song is heard.
"""
import argparse, json, os, sys, time

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import remote  # noqa: E402

BPM = 125
FLAT = 0.5
VOLUME = {"kind": "volume"}
FADER = '[data-test="mixer-fader"]'
# Parts of the fader's height, from where it was pressed: no three in a row on one line.
ZIGZAG = [0.2, -0.1, 0.25, -0.15, 0.1, -0.2, 0.15, -0.25, 0.05, -0.05]

PAGE = """
const el = k.q('%s', 0);
// A made-up pointer is not one the page may capture.
el.setPointerCapture = () => {};
el.hasPointerCapture = () => false;
const r = el.getBoundingClientRect();
const x = r.left + r.width / 2, y = r.top + r.height / 2;
if (!window.__k6frame) {
  window.__k6frame = { at: 0 };
  const tick = () => { window.__k6frame.at = Date.now(); requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
}
const out = [];
const put = (kind, dy, buttons) => {
  const at = Date.now();
  k.ptr(el, kind, x, y + dy, buttons);
  out.push({ kind, at, sinceFrame: at - window.__k6frame.at, part: -dy / r.height });
};
""" % FADER

DRAG = PAGE + """
const moves = %s, gap = %d;
put("pointerdown", 0, 1);
for (const part of moves) {
  await k.sleep(gap + Math.random() * 17);
  put("pointermove", -part * r.height, 1);
}
await k.sleep(gap + Math.random() * 17);
put("pointerup", -moves[moves.length - 1] * r.height, 0);
return out;
"""

LONG_DRAG = PAGE + """
const until = performance.now() + %d;
let n = 0, i = %d;
put("pointerdown", 0, 1);
while (performance.now() < until) {
  await k.frame();
  i++;
  const part = 0.3 * Math.sin(i / 37) + 0.1 * Math.sin(i / 5.3) + 0.02 * ((i * 7919) %% 13 - 6) / 6;
  k.ptr(el, "pointermove", x, y - part * r.height, 1);
  n++;
}
k.ptr(el, "pointerup", x, y, 0);
return { moves: n, i };
"""

# The page's times are `Date.now()`: whole ms, cut off below, so a time read as `at` is
# at + 0.5 ms on average. Python brings them to CLOCK_MONOTONIC (`wall_to_mono_us`), with no
# trip through the app in between. This measures the two legs of an `invoke` by themselves:
# `lo` is the smallest of clock - before (page -> Rust), `hi` of clock - after (Rust -> page).
CLOCK = """
let lo = Infinity, hi = Infinity;
for (let i = 0; i < 400; i++) {
  const a = Date.now();
  const mono = await invoke('clock');
  const b = Date.now();
  lo = Math.min(lo, mono - a * 1000);
  hi = Math.min(hi, mono - b * 1000);
  if (i % 7 === 0) await k.sleep(1);
}
return { lo, hi };
"""

STOP = """
const a = Date.now();
await invoke('send', { command: { type: 'playStop' } });
const b = Date.now();
return { a, b };
"""


def wall_to_mono_us():
    """CLOCK_MONOTONIC - CLOCK_REALTIME in µs (the pair read closest together of 200)."""
    best = None
    for _ in range(200):
        a = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
        w = time.time_ns()
        b = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
        if best is None or b - a < best[0]:
            best = (b - a, ((a + b) / 2 - w) / 1000)
    return best[1]


def song_for(out, bars):
    path = os.path.join(out, "song.meloet")
    remote.make_song(path, "--tracks", "1", "--notes", "0", "--bars", str(bars), "--tempo", str(BPM),
                     "--lanes", "volume", "--lane-points", "1")
    with open(path, encoding="utf-8") as f:
        made = json.load(f)
    made["song"]["tracks"][0]["automation"][0]["points"] = [[0, FLAT]]
    with open(path, "w", encoding="utf-8") as f:
        json.dump(made, f, ensure_ascii=False)
    return path


def start(app, state):
    app.send({"type": "toggleBand", "band": "mixer"})
    app.wait(lambda s: app.js(f"return k.all('{FADER}').length") > 0, 10, "the mixer")
    track = app.snapshot()["song"]["tracks"][0]["id"]
    app.send({"type": "setLaneWrite", "track": track, "target": VOLUME, "on": True})
    app.wait(lambda s: s["automation"]["armed"], 5, "「● 書く」")
    app.send({"type": "record" if state == "recording" else "playStop"})
    app.wait(lambda s: s["transport"]["state"] == state, 20, state)
    return track


def lane_points(saved):
    for lane in saved["song"]["tracks"][0].get("automation", []):
        if lane["target"] == VOLUME:
            return lane["points"]
    return []


def recorded(saved):
    """[(tick, note)] of the take the recording made (the newest one)."""
    take = max(saved["takes"], key=lambda t: t["id"], default=None)
    if not take or not take["notes"]:
        return []
    for track in saved["song"]["tracks"]:
        for clip in track["clips"]:
            for held in clip["takes"]:
                if held["take"] == take["id"]:
                    return [(clip["start"] + held["offset"] + n[0], n[2]) for n in take["notes"]]
    return []


def line(xs, ys):
    """Least squares y = a + b x; (a, b, largest |residual|)."""
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    b = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs)
    a = my - b * mx
    return a, b, max(abs(y - a - b * x) for x, y in zip(xs, ys))


def stats(v):
    if not v:
        return "—"
    s = sorted(v)
    return (f"{sum(s) / len(s):+.2f} / {s[0]:+.2f} / {s[len(s) // 2]:+.2f} / "
            f"{s[min(len(s) - 1, int(0.95 * len(s)))]:+.2f} / {s[-1]:+.2f} ({len(s)})")


def run(a):
    out = a.out
    os.makedirs(out, exist_ok=True)
    song = a.song or song_for(out, 64)
    keys_log = os.path.join(out, "keys.json")
    with remote.App(out, song=song) as app:
        start(app, a.state)
        clock0 = app.js(CLOCK)
        keys = None
        if a.state == "recording":
            seconds = a.drags * ((a.moves + 1) * 0.24 + 0.5) + 3
            keys = app.play_keys("--notes", "60,62,64,65,67", "--count", int(seconds / 0.4), "--interval-ms", 393, "--jitter-ms", 13,
                                 "--hold-ms", 100, "--velocity", 30, wait=False, log=keys_log)
        time.sleep(1.0)
        drags = []
        for d in range(a.drags):
            moves = [ZIGZAG[(d + i) % len(ZIGZAG)] for i in range(a.moves)]
            drags.append(app.js(DRAG % (json.dumps(moves), 230)))
            time.sleep(0.5)
        if keys:
            keys.wait()
        clock1 = app.js(CLOCK)
        stop = app.js(STOP)
        state = app.wait(lambda s: s["transport"]["state"] == "stopped", 10, "the stop")
        stopped_at = state["transport"]["position"]
        status = app.status()
        app.send({"type": "saveSong"})
        app.wait(lambda s: s["song"]["file"] == "saved", 10, "the save")
    with open(os.path.join(out, "raw", "saved.meloet"), encoding="utf-8") as f:
        saved = json.load(f)

    offset = wall_to_mono_us()
    mono = lambda wall_ms: (wall_ms + 0.5) * 1000 + offset
    legs = [((c["lo"] - offset) / 1000, (offset - c["hi"]) / 1000) for c in (clock0, clock1)]
    points = [p for p in lane_points(saved) if abs(p[1] - FLAT) > 1e-6]
    events = [e for d in drags for e in d if e["kind"] != "pointerdown"]
    us_per_tick = 60e6 / BPM / 480

    rulers = {}
    notes = recorded(saved)
    if notes and os.path.exists(keys_log):
        with open(keys_log) as f:
            sent = [n["on_t_us"] for n in json.load(f)["notes"]]
        if len(sent) == len(notes):
            ka, kb, worst = line([t for t, _ in notes], sent)
            rulers["keys"] = {"a": ka, "b": kb, "worstMs": worst / 1000, "notes": len(notes)}
        else:
            rulers["keysError"] = f"{len(sent)} keys sent, {len(notes)} notes recorded"
    # The stop was heard at `stopped_at` some time between sending it and its reply.
    rulers["stop"] = {"a": mono(stop["a"]) - stopped_at * us_per_tick, "b": us_per_tick,
                      "replyMs": stop["b"] - stop["a"], "tick": stopped_at}

    rows = []
    if len(points) == len(events):
        for (tick, value), e in zip(points, events):
            row = {"tick": tick, "value": value, "kind": e["kind"], "monoUs": mono(e["at"]),
                   "sinceFrameMs": e["sinceFrame"]}
            for name in ("keys", "stop"):
                if name in rulers:
                    r = rulers[name]
                    row[name + "LagMs"] = (r["a"] + r["b"] * tick - row["monoUs"]) / 1000
            rows.append(row)
    with open(os.path.join(out, "points.json"), "w") as f:
        json.dump({"rulers": rulers, "clock": [clock0, clock1], "legsMs": legs, "stop": stop, "rows": rows,
                   "points": lane_points(saved), "drags": drags, "status": status}, f, indent=1)

    tick_ms = us_per_tick / 1000
    md = [f"# 書いた点の位置（{a.state}、ドラッグ {a.drags} 回 × 動き {a.moves} 回）", "",
          f"- テンポ {BPM}（1 ティック = {tick_ms:.3f} ms）。点 {len(points)}、出来事 {len(events)}。",
          f"- ページの時刻は `Date.now()`（1 ms きざみ、読みに 0.5 ms 足した）を Python で CLOCK_MONOTONIC にした。"
          f"`invoke('clock')` のいちばん速い片道: 画面 → Rust {legs[0][0]:.2f} ms ・ {legs[1][0]:.2f} ms、"
          f"Rust → 画面 {legs[0][1]:.2f} ms ・ {legs[1][1]:.2f} ms（始め ・ 終わり。1 ms きざみの端をふくむ）。",
          f"- 止めた位置 {stopped_at} ティック、止める意図の往復 {stop['b'] - stop['a']:.2f} ms。状態行: {status}"]
    if "keys" in rulers:
        k = rulers["keys"]
        md.append(f"- 鍵のものさし: {k['notes']} 鍵、1 ティック = {k['b'] / 1000:.5f} ms、線からのはずれ最大 {k['worstMs']:.3f} ms。"
                  f"止めた位置のものさしとの差 {(rulers['stop']['a'] + rulers['stop']['b'] * stopped_at - k['a'] - k['b'] * stopped_at) / 1000:+.3f} ms。")
    if "keysError" in rulers:
        md.append(f"- 鍵のものさしは作れなかった: {rulers['keysError']}")
    if not rows:
        md.append(f"- **点と出来事の数が合わない**（点 {len(points)}、出来事 {len(events)}）。`points.json` を見る。")
    md += ["", "点が置かれた位置が耳に届いた時刻 − 出来事を送った時刻（ms。平均 / 最小 / まん中 / p95 / 最大（数））:", "",
           "| ものさし | 動かした（次のコマの呼び出しで送る） | 離した（すぐ送る） |", "|---|---|---|"]
    for name, label in (("keys", "鍵"), ("stop", "止めた位置")):
        if name in rulers and rows:
            md.append(f"| {label} | {stats([r[name + 'LagMs'] for r in rows if r['kind'] == 'pointermove'])} | "
                      f"{stats([r[name + 'LagMs'] for r in rows if r['kind'] == 'pointerup'])} |")
    if rows:
        md += ["", f"動かした出来事から、ページの 60 Hz のコマの次の番まで（ms。意図はこれを待たずに出ていた）: {stats([16.67 - r['sinceFrameMs'] for r in rows if r['kind'] == 'pointermove'])}"]
    text = "\n".join(md) + "\n"
    with open(os.path.join(out, "summary.md"), "w") as f:
        f.write(text)
    print(text)


def run_long(a):
    out = a.out
    os.makedirs(out, exist_ok=True)
    bars = int(a.long / (240 / BPM)) + 8
    song = a.song or song_for(out, bars)
    rows = []
    with remote.App(out, song=song) as app:
        track = start(app, "playing")
        lane = lambda s: next(l for l in s["song"]["tracks"][0]["lanes"] if l["target"] == VOLUME)
        began = time.monotonic()
        rows.append({"t": 0, "steps": 0, "points": 1, **app.memory()})
        i = 0
        while time.monotonic() - began < a.long:
            done = app.js(LONG_DRAG % (a.hold_ms, i), timeout=a.hold_ms / 1000 + 20)
            i = done["i"]
            s = app.snapshot()
            rows.append({"t": round(time.monotonic() - began, 2), "steps": s["history"]["total"], "points": lane(s)["points"], "moves": done["moves"],
                         "writing": s["automation"]["writing"], **app.memory()})
            if s["transport"]["state"] != "playing":
                break
        app.send({"type": "playStop"})
        time.sleep(1)
        rows.append({"t": round(time.monotonic() - began, 2), "points": lane(app.snapshot())["points"], **app.memory()})
        app.send({"type": "saveSong"})
        app.wait(lambda s: s["song"]["file"] == "saved", 10, "the save")
        del track
    with open(os.path.join(out, "long.json"), "w") as f:
        json.dump(rows, f, indent=1)
    steps = rows[1:-1]
    held = sum(r["points"] for r in steps)
    first, last = rows[0], rows[-1]
    md = [f"# {a.long:.0f} 秒ずっと書く（{a.hold_ms} ms ごとに離す）", "",
          f"- 手 {len(steps)}、送った動き {sum(r['moves'] for r in steps)}、帯の点 {first['points']} → {last['points']}。",
          f"- 履歴が持つ点の写し（手ごとの点の数の和）: {held} 点 = {held * 16 / 1048576:.1f} MiB（1 点 16 バイト）。",
          f"- meloet の RSS: {first['meloet']['rssMb']} → {last['meloet']['rssMb']} MB"
          f"（最大 {max(r['meloet']['rssMb'] for r in rows)}）。"
          f"WebKitWebProcess: {first['web']['rssMb']} → {last['web']['rssMb']} MB"
          f"（最大 {max(r['web']['rssMb'] for r in rows)}）。"]
    text = "\n".join(md) + "\n"
    with open(os.path.join(out, "summary.md"), "w") as f:
        f.write(text)
    print(text)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out")
    p.add_argument("--state", default="recording", choices=["recording", "playing"])
    p.add_argument("--drags", type=int, default=10)
    p.add_argument("--moves", type=int, default=6)
    p.add_argument("--song")
    p.add_argument("--long", type=float)
    p.add_argument("--hold-ms", type=int, default=2000)
    a = p.parse_args()
    if a.long:
        run_long(a)
    else:
        run(a)


if __name__ == "__main__":
    main()
