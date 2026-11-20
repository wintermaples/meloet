# Run by gdb (webkit-layout.sh): starts the app as gdb's child, attaches to its WebKitWebProcess and
# writes down every rectangle the page asks to repaint (ask), where a layout or a style update hands
# over the rectangles it gathered (flush), and what then reaches the drawing area (need).
#
# WebKit gathers the rectangles of one layout (or one style update) in a Region
# (RenderView::accumulateRepaintRect). Once that Region is too complex it becomes the one rectangle
# around everything gathered so far, long before the drawing area's own rule (webkit-damage.sh).
#
# gdb has to be an ancestor of the process (kernel.yama.ptrace_scope = 1), so the app is started here.
# Each stop takes gdb milliseconds: read which rectangles there are, not how often.
import os
import re
import subprocess
import time

import gdb

LIB = "libwebkit2gtk-4.1.so"
ASK = "WebCore::RenderView::repaintViewRectangle(WebCore::LayoutRect const&)"
FLUSH = "WebCore::RenderView::flushAccumulatedRepaintRegion() const"
NEED = "WebKit::DrawingAreaCoordinatedGraphics::setNeedsDisplayInRect(WebCore::IntRect const&)"
NAMES = (ASK, FLUSH, NEED)
# A LayoutUnit is 1/64 of a CSS pixel.
LAYOUT_UNIT = 64


def offsets(debug_file):
    """Function name -> offset in the library, from the symbol table of its debug file."""
    found = {}
    out = subprocess.run(["nm", "-C", debug_file], capture_output=True, text=True, check=True).stdout
    for line in out.splitlines():
        m = re.match(r"([0-9a-f]+) \w (.*)", line)
        if m and m.group(2) in NAMES:
            found[m.group(2)] = int(m.group(1), 16)
    missing = [n for n in NAMES if n not in found]
    if missing:
        raise SystemExit(f"webkit_layout: {debug_file} has no symbol for {missing}")
    return found


def descendants(pid):
    kids = {}
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            stat = open(f"/proc/{p}/stat").read()
        except OSError:
            continue
        kids.setdefault(int(stat[stat.rindex(")") + 2:].split()[1]), []).append(int(p))
    out, todo = [], [pid]
    while todo:
        for c in kids.get(todo.pop(), []):
            out.append(c)
            todo.append(c)
    return out


def web_process(app_pid):
    """The WebKitWebProcess under the command started here (not another app's of the same user)."""
    for p in sorted(descendants(app_pid), reverse=True):
        try:
            if "WebKitWebProcess" in open(f"/proc/{p}/cmdline").read():
                return p
        except OSError:
            pass
    return None


def main():
    out_path = os.environ["LAYOUT_OUT"]
    command = os.environ["LAYOUT_COMMAND"]
    wait_s = float(os.environ.get("LAYOUT_WAIT_S", "12"))
    stops = int(os.environ.get("LAYOUT_STOPS", "4000"))
    offs = offsets(os.environ["LAYOUT_DEBUG_FILE"])
    for c in ("set pagination off", "set debuginfod enabled off", "set confirm off", "set print thread-events off"):
        gdb.execute(c)

    app = subprocess.Popen(["bash", "-c", command], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(wait_s)
    pid = web_process(app.pid)
    if pid is None:
        raise SystemExit("webkit_layout: no WebKitWebProcess under the command")
    base = None
    for line in open(f"/proc/{pid}/maps"):
        f = line.split()
        if len(f) >= 6 and LIB in f[5] and f[2] == "00000000":
            base = int(f[0].split("-")[0], 16)
            break
    if base is None:
        raise SystemExit(f"webkit_layout: {LIB} is not mapped in {pid}")

    ask, flush, need = (base + offs[n] for n in NAMES)
    with open(out_path, "w") as out:
        gdb.execute(f"attach {pid}", to_string=True)
        for address in (ask, flush, need):
            gdb.execute(f"break *{address}", to_string=True)
        for _ in range(stops):
            try:
                gdb.execute("continue", to_string=True)
                pc = int(gdb.parse_and_eval("$pc"))
                if pc == flush:
                    out.write("flush\n")
                    continue
                # The rectangle is the last argument: x, y, width, height.
                values = [int(v) for v in gdb.execute("x/4dw $rsi", to_string=True).split(":")[1].split()]
                if pc == ask:
                    out.write("ask " + " ".join(str(round(v / LAYOUT_UNIT, 1)) for v in values) + "\n")
                else:
                    out.write("need " + " ".join(map(str, values)) + "\n")
            except gdb.error as e:
                out.write(f"stopped: {e}\n")
                break
        gdb.execute("delete", to_string=True)
        gdb.execute("detach", to_string=True)
    app.wait()


main()
