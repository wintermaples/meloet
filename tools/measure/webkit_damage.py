# Run by gdb (webkit-damage.sh): starts the app as gdb's child, attaches to its WebKitWebProcess and
# writes down, for each display of the page without accelerated compositing, the rectangles the page
# reported as changed (need) and the rectangles WebKit then painted (rect).
#
# gdb has to be an ancestor of the process (kernel.yama.ptrace_scope = 1), so the app is started here.
# Each stop takes gdb milliseconds: several frames of the page end up in one display, so read the
# rectangles and what turns them into one big rectangle, not how often it happens.
import os
import re
import subprocess
import time

import gdb

LIB = "libwebkit2gtk-4.1.so"
DISPLAY = "WebKit::DrawingAreaCoordinatedGraphics::display()"
NEED = "WebKit::DrawingAreaCoordinatedGraphics::setNeedsDisplayInRect(WebCore::IntRect const&)"
DRAW = "WebKit::WebPage::drawRect(WebCore::GraphicsContext&, WebCore::IntRect const&)"


def offsets(debug_file):
    """Function name -> offset in the library, from the symbol table of its debug file."""
    found = {}
    out = subprocess.run(["nm", "-C", debug_file], capture_output=True, text=True, check=True).stdout
    for line in out.splitlines():
        m = re.match(r"([0-9a-f]+) \w (.*)", line)
        if m and m.group(2) in (DISPLAY, NEED, DRAW):
            found[m.group(2)] = int(m.group(1), 16)
    missing = [n for n in (DISPLAY, NEED, DRAW) if n not in found]
    if missing:
        raise SystemExit(f"webkit_damage: {debug_file} has no symbol for {missing}")
    return found


def main():
    out_path = os.environ["DAMAGE_OUT"]
    command = os.environ["DAMAGE_COMMAND"]
    wait_s = float(os.environ.get("DAMAGE_WAIT_S", "12"))
    stops = int(os.environ.get("DAMAGE_STOPS", "2500"))
    offs = offsets(os.environ["DAMAGE_DEBUG_FILE"])
    for c in ("set pagination off", "set debuginfod enabled off", "set confirm off", "set print thread-events off"):
        gdb.execute(c)

    app = subprocess.Popen(["bash", "-c", command], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(wait_s)
    pids = subprocess.run(
        ["pgrep", "-u", str(os.getuid()), "-n", "-f", "WebKitWebProcess"], capture_output=True, text=True
    ).stdout.split()
    if not pids:
        raise SystemExit("webkit_damage: no WebKitWebProcess")
    pid = pids[0]
    base = None
    for line in open(f"/proc/{pid}/maps"):
        f = line.split()
        if len(f) >= 6 and LIB in f[5] and f[2] == "00000000":
            base = int(f[0].split("-")[0], 16)
            break
    if base is None:
        raise SystemExit(f"webkit_damage: {LIB} is not mapped in {pid}")

    display, need, draw = (base + offs[n] for n in (DISPLAY, NEED, DRAW))
    with open(out_path, "w") as out:
        gdb.execute(f"attach {pid}", to_string=True)
        for address in (display, need, draw):
            gdb.execute(f"break *{address}", to_string=True)
        for _ in range(stops):
            try:
                gdb.execute("continue", to_string=True)
                pc = int(gdb.parse_and_eval("$pc"))
                if pc == display:
                    out.write("display\n")
                    continue
                # The IntRect is the last argument: x, y, width, height.
                register = "$rsi" if pc == need else "$rdx"
                values = gdb.execute(f"x/4dw {register}", to_string=True).split(":")[1].split()
                out.write(("need " if pc == need else "rect ") + " ".join(values) + "\n")
            except gdb.error as e:
                out.write(f"stopped: {e}\n")
                break
        gdb.execute("delete", to_string=True)
        gdb.execute("detach", to_string=True)
    app.wait()


main()
