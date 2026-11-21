# Run by gdb (webkit-stacks.sh): starts a command as gdb's child, attaches to its WebKitWebProcess and
# takes the stack of the main thread many times (a sampling profile for a PC where perf is closed:
# kernel.perf_event_paranoid = 4, and gdb may only attach to its own descendants).
#
# Every stop takes gdb some milliseconds, so the page runs slower than usual: read the shares, not the times.
# Functions inside libwebkit2gtk are named from the symbol table of its debug file; other frames by their library.
import bisect
import collections
import os
import re
import signal
import subprocess
import threading
import time

import gdb

LIB = "libwebkit2gtk-4.1.so"


def symbols(debug_file):
    """Sorted (offset, name) of the functions of the library, from its debug file."""
    out = subprocess.run(["nm", "-C", "--defined-only", debug_file], capture_output=True, text=True, check=True).stdout
    table = []
    for line in out.splitlines():
        m = re.match(r"([0-9a-f]+) [tTwW] (.*)", line)
        if m:
            table.append((int(m.group(1), 16), m.group(2)))
    table.sort()
    return [a for a, _ in table], [n for _, n in table]


def maps(pid):
    """(start, end, file) of what the process has mapped as code, and where the library starts."""
    areas = []
    base = None
    for line in open(f"/proc/{pid}/maps"):
        f = line.split()
        if len(f) < 6:
            continue
        start, end = (int(x, 16) for x in f[0].split("-"))
        if LIB in f[5] and f[2] == "00000000":
            base = start
        if "x" in f[1]:
            areas.append((start, end, os.path.basename(f[5])))
    return areas, base


def short(name):
    """A C++ name without its arguments and template arguments."""
    name = re.sub(r"\(.*", "", name)
    while True:
        cut = re.sub(r"<[^<>]*>", "", name)
        if cut == name:
            return name
        name = cut


def main():
    out_path = os.environ["STACKS_OUT"]
    command = os.environ["STACKS_COMMAND"]
    wait_s = float(os.environ.get("STACKS_WAIT_S", "12"))
    seconds = float(os.environ.get("STACKS_SECONDS", "15"))
    every_s = float(os.environ.get("STACKS_EVERY_MS", "25")) / 1000
    addresses, names = symbols(os.environ["STACKS_DEBUG_FILE"])
    for c in ("set pagination off", "set debuginfod enabled off", "set confirm off", "set print thread-events off"):
        gdb.execute(c)

    app = subprocess.Popen(["bash", "-c", command], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(wait_s)
    pids = subprocess.run(
        ["pgrep", "-u", str(os.getuid()), "-n", "-f", "WebKitWebProcess"], capture_output=True, text=True
    ).stdout.split()
    if not pids:
        raise SystemExit("webkit_stacks: no WebKitWebProcess")
    pid = int(pids[0])
    areas, base = maps(pid)
    if base is None:
        raise SystemExit(f"webkit_stacks: {LIB} is not mapped in {pid}")

    def name_of(pc):
        for start, end, file in areas:
            if start <= pc < end:
                if LIB in file:
                    i = bisect.bisect_right(addresses, pc - base) - 1
                    return short(names[i]) if i >= 0 else file
                return f"[{file}]"
        return "[?]"

    gdb.execute(f"attach {pid}", to_string=True)
    gdb.execute("handle SIGINT stop print nopass", to_string=True)
    done = threading.Event()

    def knock():
        until = time.time() + seconds
        while time.time() < until and not done.is_set():
            time.sleep(every_s)
            try:
                os.kill(pid, signal.SIGINT)
            except OSError:
                break
        done.set()

    threading.Thread(target=knock, daemon=True).start()
    inclusive = collections.Counter()
    leaves = collections.Counter()
    stacks = collections.Counter()
    samples = 0
    while not done.is_set():
        try:
            gdb.execute("continue", to_string=True)
            main_thread = next((t for t in gdb.selected_inferior().threads() if t.ptid[1] == pid), None)
            if main_thread is None:
                break
            main_thread.switch()
            frame = gdb.newest_frame()
            stack = []
            while frame is not None and len(stack) < 80:
                stack.append(name_of(frame.pc()))
                frame = frame.older()
        except gdb.error:
            break
        samples += 1
        leaves[stack[0]] += 1
        for n in set(stack):
            inclusive[n] += 1
        stacks[" < ".join(stack[:12])] += 1
    done.set()
    try:
        gdb.execute("detach", to_string=True)
    except gdb.error:
        pass
    with open(out_path, "w") as out:
        out.write(f"samples {samples}\n\n# in the stack (share of the samples)\n")
        for n, c in inclusive.most_common(150):
            out.write(f"{100 * c / max(1, samples):5.1f} %  {n}\n")
        out.write("\n# on top of the stack\n")
        for n, c in leaves.most_common(60):
            out.write(f"{100 * c / max(1, samples):5.1f} %  {n}\n")
        out.write("\n# the stacks seen most (the innermost 12 frames)\n")
        for n, c in stacks.most_common(25):
            out.write(f"{100 * c / max(1, samples):5.1f} %  {n}\n")
    app.wait()


main()
