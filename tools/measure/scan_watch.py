#!/usr/bin/env python3
"""Samples the children that scan plugin bundles (`meloet-plughost scan ...`) until told to end.

usage: python3 -I tools/measure/scan_watch.py OUT.json PLUGHOST_EXE [--seconds N] [--every-ms 200]

Every --every-ms it reads /proc for processes whose command line starts with PLUGHOST_EXE and
`scan`, and notes their number, their RSS and their CPU time. Ends at SIGTERM or after
--seconds, and writes OUT.json:
  firstSeenWallMs / lastSeenWallMs: when a scanning child was first and last seen,
  firstSeenMonoUs / lastSeenMonoUs: the same on CLOCK_MONOTONIC (the clock of the key records),
  seenSeconds: the time between them (the scan's length, to one sampling step),
  peakChildren, peakSumRssMb, peakOneRssMb (+ its bundle), cpuSeconds (user + system of all
  children, as last seen), children (how many were seen), longest and biggest (5 each),
  samples: [{wallMs, children, sumRssMb}].
Reads /proc only.
"""
import json, os, signal, sys, time

TICK = os.sysconf("SC_CLK_TCK")
PAGE = os.sysconf("SC_PAGE_SIZE")


def scanning(exe):
    found = {}
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            args = open(f"/proc/{p}/cmdline", "rb").read().split(b"\0")
            if len(args) < 4 or args[0].decode(errors="replace") != exe or args[1] != b"scan":
                continue
            stat = open(f"/proc/{p}/stat").read()
            rest = stat[stat.rindex(")") + 2:].split()
            rss = int(open(f"/proc/{p}/statm").read().split()[1]) * PAGE / 1e6
        except (OSError, ValueError, IndexError):
            continue
        found[int(p)] = dict(bundle=args[3].decode(errors="replace"), rssMb=rss,
                             cpuS=(int(rest[11]) + int(rest[12])) / TICK)
    return found


def main():
    args = sys.argv[1:]
    out, exe = args[0], args[1]
    seconds = float(args[args.index("--seconds") + 1]) if "--seconds" in args else 600.0
    every = float(args[args.index("--every-ms") + 1]) / 1000 if "--every-ms" in args else 0.2
    stop = []
    signal.signal(signal.SIGTERM, lambda *_: stop.append(1))
    signal.signal(signal.SIGINT, lambda *_: stop.append(1))
    seen, samples = {}, []
    first = last = first_mono = last_mono = None
    peak_n = 0
    peak_sum = 0.0
    until = time.time() + seconds
    while not stop and time.time() < until:
        now = int(time.time() * 1000)
        found = scanning(exe)
        if found:
            mono = int(time.clock_gettime(time.CLOCK_MONOTONIC) * 1e6)
            first, first_mono = first or now, first_mono or mono
            last, last_mono = now, mono
        for pid, f in found.items():
            s = seen.setdefault(pid, dict(bundle=f["bundle"], firstMs=now, rssMb=0.0))
            s.update(lastMs=now, cpuS=f["cpuS"], rssMb=max(s["rssMb"], f["rssMb"]))
        total = sum(f["rssMb"] for f in found.values())
        peak_n = max(peak_n, len(found))
        peak_sum = max(peak_sum, total)
        samples.append(dict(wallMs=now, children=len(found), sumRssMb=round(total, 1)))
        time.sleep(every)
    kids = [dict(bundle=s["bundle"], seconds=round((s["lastMs"] - s["firstMs"]) / 1000, 1),
                 rssMb=round(s["rssMb"], 1), cpuS=round(s["cpuS"], 2)) for s in seen.values()]
    biggest = sorted(kids, key=lambda k: -k["rssMb"])[:5]
    result = dict(
        firstSeenWallMs=first, lastSeenWallMs=last, firstSeenMonoUs=first_mono, lastSeenMonoUs=last_mono,
        seenSeconds=round((last - first) / 1000, 1) if first else None,
        peakChildren=peak_n, peakSumRssMb=round(peak_sum, 1),
        peakOneRssMb=biggest[0]["rssMb"] if biggest else None,
        peakOneBundle=biggest[0]["bundle"] if biggest else None,
        cpuSeconds=round(sum(k["cpuS"] for k in kids), 1), children=len(kids),
        longest=sorted(kids, key=lambda k: -k["seconds"])[:5], biggest=biggest, samples=samples)
    with open(out, "w") as f:
        json.dump(result, f)
    print(json.dumps({k: v for k, v in result.items() if k != "samples"}, ensure_ascii=False, indent=1))


main()
