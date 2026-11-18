#!/usr/bin/env python3
"""Watches a running meloet for tools/measure/long-record.sh.

usage: python3 -I tools/measure/long_record_watch.py OUT_DIR SECONDS MELOET_PID

Every second: the error count (ERR, xruns) PipeWire keeps for the `meloet` stream and for each
ALSA output and the BUSY time of the meloet stream (how long its callback took; read with
`pw-top -b`, nothing is changed), and the CPU time of the
JavaScriptCore GC helper threads (`HeapHelper`) of meloet and its WebKitWebProcess. Every 10
seconds: the RSS of both. Writes OUT_DIR/watch.json.
"""
import json, os, subprocess, sys, threading, time


def children(pid):
    out = []
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            stat = open(f"/proc/{p}/stat").read()
        except OSError:
            continue
        fields = stat[stat.rindex(")") + 2:].split()
        if int(fields[1]) == pid:
            out.append(int(p))
    return out


def comm(pid):
    try:
        return open(f"/proc/{pid}/comm").read().strip()
    except OSError:
        return None


def web_process(pid):
    """The WebKitWebProcess under meloet (it may sit under a helper)."""
    todo = [pid]
    while todo:
        p = todo.pop()
        for c in children(p):
            if comm(c) == "WebKitWebProces":
                return c
            todo.append(c)
    return None


def rss_mb(pid):
    try:
        for line in open(f"/proc/{pid}/status"):
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) / 1024
    except OSError:
        pass
    return None


def micros(text):
    """pw-top's time column (`183.2us`, `1.2ms`) in microseconds; None when it is not a time."""
    for unit, scale in (("ns", 0.001), ("us", 1.0), ("µs", 1.0), ("ms", 1000.0), ("s", 1e6)):
        if text.endswith(unit):
            try:
                return float(text[:-len(unit)]) * scale
            except ValueError:
                return None
    return None


def gc_ticks(pid):
    """CPU ticks of each HeapHelper thread, by thread id (the threads come and go)."""
    total = {}
    try:
        tids = os.listdir(f"/proc/{pid}/task")
    except OSError:
        return None
    for t in tids:
        try:
            st = open(f"/proc/{pid}/task/{t}/stat").read()
        except OSError:
            continue
        if st[st.index("(") + 1:st.rindex(")")] != "HeapHelper":
            continue
        f = st[st.rindex(")") + 2:].split()
        total[t] = int(f[11]) + int(f[12])
    return total


def main():
    out, seconds, pid = sys.argv[1], float(sys.argv[2]), int(sys.argv[3])
    os.makedirs(out, exist_ok=True)
    errs = {}  # name -> last ERR seen
    busy = {}  # the meloet stream's last BUSY (the time its callback took)
    err_lock = threading.Lock()
    raw = {}

    def read_top():
        # pw-top can end (or fail to connect); start it again so that the stream is not missed.
        while not stop.is_set():
            top = subprocess.Popen(["stdbuf", "-oL", "pw-top", "-b"], stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, text=True, bufsize=1)
            for line in top.stdout:
                parts = line.split()
                if len(parts) < 10 or parts[0] not in ("R", "S", "I", "C") or not parts[1].isdigit():
                    continue
                name = parts[-1]
                try:
                    err = int(parts[8])
                except ValueError:
                    continue
                with err_lock:
                    errs[name] = err
                    if name == "meloet":
                        busy["us"] = micros(parts[5])
                    raw.setdefault("names", set()).add(name)
                    if name == "meloet" and "meloetLine" not in raw:
                        raw["meloetLine"] = line.rstrip()
                if stop.is_set():
                    break
            top.kill()
            top.wait()
            if not stop.is_set():
                with err_lock:
                    raw["restarts"] = raw.get("restarts", 0) + 1
                    raw["lastStderr"] = (top.stderr.read() or "").strip()[-500:]
                time.sleep(1)

    stop = threading.Event()
    threading.Thread(target=read_top, daemon=True).start()
    start = time.monotonic()
    hz = os.sysconf("SC_CLK_TCK")
    web = None
    rows = []
    last_gc = {}
    while time.monotonic() - start < seconds and comm(pid) == "meloet":
        t = time.monotonic() - start
        web = web or web_process(pid)
        row = {"t": round(t, 1)}
        for label, p in (("meloet", pid), ("web", web)):
            g = gc_ticks(p) if p else None
            if g is not None:
                before = last_gc.get(label)
                if before is not None:
                    # A thread that is new since the last second counts whole; one that ended is lost.
                    row[f"gcMs_{label}"] = sum(v - before.get(t, 0) for t, v in g.items()) * 1000 / hz
                last_gc[label] = g
        with err_lock:
            row["errMeloet"] = errs.get("meloet")
            row["busyUs"] = busy.get("us")
            row["errOutputs"] = {n: e for n, e in errs.items() if n.startswith("alsa_output")}
        if len(rows) % 10 == 0:
            row["rssMeloetMb"] = rss_mb(pid)
            row["rssWebMb"] = rss_mb(web) if web else None
        rows.append(row)
        time.sleep(max(0.0, start + len(rows) - time.monotonic()))
    stop.set()
    with open(os.path.join(out, "watch.json"), "w") as f:
        json.dump({"pwTopMeloetLine": raw.get("meloetLine"), "pwTopRestarts": raw.get("restarts", 0),
                   "pwTopStderr": raw.get("lastStderr"), "pwTopNames": sorted(raw.get("names", ())),
                   "rows": rows}, f)
        f.write("\n")


main()
