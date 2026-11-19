#!/usr/bin/env python3
"""Samples the plugin hosts (`meloet-plughost host ...`) of a running meloet until told to end.

usage: python3 -I tools/measure/plug_watch.py OUT.json PLUGHOST_EXE [--seconds N] [--every-ms 500]
                                              [--signal KILL|SEGV|STOP --at SECONDS [--nth 0]]

Every --every-ms it reads /proc for processes whose command line starts with PLUGHOST_EXE and
`host`, and notes for each its state (R, S, T ...), RSS, PSS (smaps_rollup), threads and CPU
time. --signal: SECONDS after the start it sends that signal to the --nth host (in the order
of their pids) and writes down when, on the wall clock and on CLOCK_MONOTONIC (the clock of
meloet's own records). SEGV is sent twice, 2 ms apart: the handler Rust's runtime keeps for
SIGSEGV (to tell a stack overflow) takes the first one sent from outside, puts the default
back and returns, so only the second one ends the process as a real fault would. The PSS of a
host that runs with MELOET_PLUGHOST_NO_DUMP cannot be read (0). Ends at SIGTERM or after --seconds, and writes OUT.json:
  hosts: per pid {args, firstMs, lastMs, peakRssMb, peakPssMb, cpuS, lastState},
  peakHosts, peakSumRssMb, peakSumPssMb, lastSumPssMb,
  signal: {name, pid, args, wallMs, monoUs} when one was sent,
  samples: [{wallMs, hosts: [{pid, state, rssMb, pssMb, threads}]}].
Reads /proc only; sends nothing but the one signal asked for, and only to a plugin host.
"""
import json, os, signal, sys, time

TICK = os.sysconf("SC_CLK_TCK")
PAGE = os.sysconf("SC_PAGE_SIZE")


def pss_mb(pid):
    try:
        for line in open(f"/proc/{pid}/smaps_rollup"):
            if line.startswith("Pss:"):
                return int(line.split()[1]) / 1024
    except OSError:
        pass
    return None


def hosting(exe):
    found = {}
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            args = open(f"/proc/{p}/cmdline", "rb").read().split(b"\0")
            if len(args) < 2 or args[0].decode(errors="replace") != exe or args[1] != b"host":
                continue
            stat = open(f"/proc/{p}/stat").read()
            rest = stat[stat.rindex(")") + 2:].split()
            rss = int(open(f"/proc/{p}/statm").read().split()[1]) * PAGE / 1e6
        except (OSError, ValueError, IndexError):
            continue
        found[int(p)] = dict(args=" ".join(a.decode(errors="replace") for a in args[1:] if a),
                             state=rest[0], rssMb=round(rss, 1), pssMb=pss_mb(p),
                             threads=int(rest[17]), cpuS=(int(rest[11]) + int(rest[12])) / TICK)
    return found


def option(args, name, default=None):
    return args[args.index(name) + 1] if name in args else default


def main():
    args = sys.argv[1:]
    out, exe = args[0], args[1]
    seconds = float(option(args, "--seconds", 600))
    every = float(option(args, "--every-ms", 500)) / 1000
    sig_name = option(args, "--signal")
    sig_at = float(option(args, "--at", 0))
    nth = int(option(args, "--nth", 0))
    stop = []
    signal.signal(signal.SIGTERM, lambda *_: stop.append(1))
    signal.signal(signal.SIGINT, lambda *_: stop.append(1))
    seen, samples = {}, []
    sent = None
    peak_n, peak_rss, peak_pss, last_pss = 0, 0.0, 0.0, None
    start = time.time()
    while not stop and time.time() - start < seconds:
        now = int(time.time() * 1000)
        found = hosting(exe)
        if sig_name and sent is None and time.time() - start >= sig_at and len(found) > nth:
            pid = sorted(found)[nth]
            os.kill(pid, getattr(signal, "SIG" + sig_name))
            if sig_name == "SEGV":
                time.sleep(0.002)
                try:
                    os.kill(pid, signal.SIGSEGV)
                except ProcessLookupError:
                    pass
            sent = dict(name=sig_name, pid=pid, args=found[pid]["args"], wallMs=int(time.time() * 1000),
                        monoUs=int(time.clock_gettime(time.CLOCK_MONOTONIC) * 1e6))
        for pid, f in found.items():
            s = seen.setdefault(pid, dict(args=f["args"], firstMs=now, peakRssMb=0.0, peakPssMb=0.0))
            s.update(lastMs=now, cpuS=round(f["cpuS"], 2), lastState=f["state"],
                     peakRssMb=max(s["peakRssMb"], f["rssMb"]),
                     peakPssMb=round(max(s["peakPssMb"], f["pssMb"] or 0.0), 1))
        if found:
            rss = sum(f["rssMb"] for f in found.values())
            pss = sum(f["pssMb"] or 0.0 for f in found.values())
            peak_n, peak_rss, peak_pss, last_pss = max(peak_n, len(found)), max(peak_rss, rss), max(peak_pss, pss), pss
        samples.append(dict(wallMs=now, hosts=[dict(pid=p, state=f["state"], rssMb=f["rssMb"],
                                                    pssMb=round(f["pssMb"] or 0.0, 1), threads=f["threads"])
                                               for p, f in sorted(found.items())]))
        time.sleep(every)
    result = dict(hosts={str(p): s for p, s in seen.items()}, peakHosts=peak_n,
                  peakSumRssMb=round(peak_rss, 1), peakSumPssMb=round(peak_pss, 1),
                  lastSumPssMb=round(last_pss, 1) if last_pss is not None else None,
                  signal=sent, samples=samples)
    with open(out, "w") as f:
        json.dump(result, f)
    print(json.dumps({k: v for k, v in result.items() if k != "samples"}, ensure_ascii=False, indent=1))


main()
