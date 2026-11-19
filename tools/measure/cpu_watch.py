#!/usr/bin/env python3
"""The CPU time meloet and its WebKitWebProcess use between two moments of a run.

usage: python3 -I tools/measure/cpu_watch.py OUT.json FROM_S TO_S [--exe PATH]

Waits (30 s at most) for a process called meloet (with --exe: one started from that file),
then reads /proc FROM_S and TO_S seconds later and writes OUT.json: for each of the two
processes the CPU seconds in between and that as a share of one core. Reads /proc only.
"""
import json, os, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from remote import comm, descendants, mem


def find(exe):
    for p in os.listdir("/proc"):
        if not p.isdigit() or comm(int(p)) != "meloet":
            continue
        try:
            if os.stat(f"/proc/{p}").st_uid != os.getuid():
                continue
            if exe and os.readlink(f"/proc/{p}/exe") != exe:
                continue
        except OSError:
            continue
        return int(p)
    return None


def main(out, from_s, to_s, exe=None):
    until = time.time() + 30
    pid = None
    while pid is None and time.time() < until:
        pid = find(exe)
        time.sleep(0.1)
    if pid is None:
        sys.exit("cpu_watch.py: no meloet")
    start = time.time()

    def look():
        web = next((c for c in descendants(pid) if comm(c) == "WebKitWebProces"), None)
        return {"wall": time.time(), "meloet": mem(pid), "web": mem(web) if web else None}

    time.sleep(max(0, start + from_s - time.time()))
    a = look()
    time.sleep(max(0, start + to_s - time.time()))
    b = look()
    wall = b["wall"] - a["wall"]
    report = {"fromS": from_s, "toS": to_s, "seconds": round(wall, 2)}
    for name in ("meloet", "web"):
        if a[name] and b[name]:
            cpu = b[name]["cpuS"] - a[name]["cpuS"]
            report[name] = {"cpuS": round(cpu, 2), "percentOfOneCore": round(100 * cpu / wall, 1),
                            "rssMb": b[name]["rssMb"]}
    with open(out, "w") as f:
        json.dump(report, f, indent=1)
    print(json.dumps(report))


if __name__ == "__main__":
    args = sys.argv[1:]
    exe = None
    if "--exe" in args:
        i = args.index("--exe")
        exe = args[i + 1]
        del args[i:i + 2]
    main(args[0], float(args[1]), float(args[2]), exe)
