#!/usr/bin/env python3
"""Samples the RSS of meloet and its WebKitWebProcess every second until meloet ends.

usage: python3 -I tools/measure/rss_watch.py OUT.json MELOET_PID

Writes OUT.json as {"samples": [{"wallMs", "meloetMb", "webMb"}, ...]} every 10 samples and at
the end. Reads /proc only.
"""
import json, os, sys, time


def children(pid):
    out = []
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            stat = open(f"/proc/{p}/stat").read()
        except OSError:
            continue
        if int(stat[stat.rindex(")") + 2:].split()[1]) == pid:
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
    if pid is None:
        return None
    try:
        for line in open(f"/proc/{pid}/status"):
            if line.startswith("VmRSS:"):
                return round(int(line.split()[1]) / 1024, 1)
    except OSError:
        pass
    return None


def main(out, pid):
    samples = []
    web = None

    def write():
        tmp = out + ".tmp"
        with open(tmp, "w") as f:
            json.dump({"samples": samples}, f)
        os.replace(tmp, out)

    while os.path.exists(f"/proc/{pid}"):
        if web is None or not os.path.exists(f"/proc/{web}"):
            web = web_process(pid)
        samples.append({"wallMs": int(time.time() * 1000), "meloetMb": rss_mb(pid), "webMb": rss_mb(web)})
        if len(samples) % 10 == 0:
            write()
        time.sleep(1)
    write()


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], int(sys.argv[2]))
