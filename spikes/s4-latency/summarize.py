#!/usr/bin/env python3
"""results/*.json -> results/summary.md for bet 4.

Times are on CLOCK_MONOTONIC. JS times are mapped onto it with the min-RTT ping offset (JS clock has 1 ms steps).
key -> screen: Rust receives the note (t_ns) -> JS handler (recvMs) -> first rAF after the class change (rafMs).
The compositor shows that frame at the next vsync, so "presented" adds one 60 Hz frame (16.7 ms) as an estimate.
"""
import json, pathlib, statistics as st

R = pathlib.Path(__file__).parent / "results"
FRAME = 1000 / 60


def q(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(p * len(xs)))] if xs else float("nan")


def row(xs):
    return f"{st.mean(xs):.2f} / {q(xs, .5):.2f} / {q(xs, .95):.2f} / {max(xs):.2f}" if xs else "—"


out = ["# s4-latency summary", "", "values: mean / p50 / p95 / max (ms)", ""]

out += ["## Key -> screen and key -> sound", "",
        "| run | n | Rust→JS handler | Rust→rAF | Rust→presented (est.) | ≤2 frames (est.) | Rust→audio callback | Rust→DAC (est.) | kernel→Rust | audio | late callbacks |",
        "|---|---|---|---|---|---|---|---|---|---|---|"]
for f in sorted(R.glob("latency__*.json")) + sorted(R.glob("real__*.json")):
    d = json.loads(f.read_text())
    ks = [k for k in d.get("keys", []) if k.get("on") and k.get("rafMs") is not None]
    a = [(k["recvMs"] - k["tNs"] / 1e6) for k in ks]
    b = [(k["rafMs"] - k["tNs"] / 1e6) for k in ks]
    pres = [x + FRAME for x in b]
    ok = sum(1 for x in pres if x <= 2 * FRAME) / len(pres) * 100 if pres else 0
    lat = d.get("audioLatency", [])
    cb = [x["recvToCallbackMs"] for x in lat]
    dac = [x["recvToPlaybackMs"] for x in lat]
    kern = [(m["t_ns"] - m["kernel_ns"]) / 1e6 for m in d.get("rustMidi", []) if m.get("kind") == "note" and m.get("kernel_ns")]
    au = d.get("audio", {})
    out.append(f"| {f.stem} | {len(ks)} | {row(a)} | {row(b)} | {row(pres)} | {ok:.0f} % | {row(cb)} | {row(dac)} | {row(kern)} | "
               f"{au.get('framesPerCallback')} frames | {au.get('lateCallbacks')} / {au.get('callbacks')} |")

out += ["", "## Per-frame load (input meter + 18 meters + position)", "",
        "| run | frames got | delivery lag | JS handler total (ms / 10 s) | Rust CPU % | WebKit CPU % | rAF p95 |", "|---|---|---|---|---|---|---|"]
for f in sorted(R.glob("load__*.json")):
    d = json.loads(f.read_text())
    c0, c1 = d.get("cpu", [None, None])
    if not c0:
        out.append(f"| {f.stem} | — | — | — | — | — | {d.get('error')} |"); continue
    secs = (c1["t_ns"] - c0["t_ns"]) / 1e9
    hz = c0["hz"]
    me = (c1["self"] - c0["self"]) / hz / secs * 100
    k0 = {k["pid"]: k["ticks"] for k in c0["children"]}
    wk = sum(k["ticks"] - k0.get(k["pid"], 0) for k in c1["children"] if "WebKit" in k["name"]) / hz / secs * 100
    out.append(f"| {f.stem} | {d['frames']} | {row(d['frameLagMs'])} | {d['handlerTotalMs']:.1f} | {me:.1f} | {wk:.1f} | {q(d['rafIntervals'], .95):.1f} |")

p = R / "plug.json"
if p.exists():
    d = json.loads(p.read_text())
    out += ["", "## Plug / unplug", "", "| event | detail | Rust t (ms) | UI t (ms) | Rust→UI (ms) |", "|---|---|---|---|---|"]
    for e in d.get("plug", []):
        if "udev" in e:
            u = e["udev"]
            out.append(f"| udev {u['action']} | {u['name']} | {u['tNs']/1e6:.1f} | {e['uiMs']:.1f} | {e['uiMs'] - u['tNs']/1e6:.2f} |")
        else:
            out.append(f"| seq {'connect' if e['connected'] else 'disconnect'} | {e.get('name') or e['client']} | {e['t_ns']/1e6:.1f} | {e['uiMs']:.1f} | {e['uiMs'] - e['t_ns']/1e6:.2f} |")

(R / "summary.md").write_text("\n".join(out) + "\n")
print("\n".join(out))
