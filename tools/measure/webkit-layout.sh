#!/usr/bin/env bash
# Shows which rectangles one layout (or one style update) of the page asks WebKitGTK to repaint, and
# when WebKit gives up and hands on one rectangle around all of them. This happens before the rule
# webkit-damage.sh shows: the rectangles of one layout are gathered in a Region, and a Region that
# gets too complex (gridSize over 256) becomes its bounding rectangle (RenderView.cpp,
# accumulateRepaintRect; docs/architecture.md, section 7). A cover slot does not help against it.
# What to look for: a layout that asks for hundreds of rectangles (something that changes every
# frame runs a layout: a `top`, a `width`, a text), and "one rectangle around all".
#
# usage: tools/measure/webkit-layout.sh [--out DIR] [--wait S] [--stops N] [--shown N]
#                                       [--run 'COMMAND'] [app-watch.sh arguments]
#   e.g. tools/measure/webkit-layout.sh --song band8-vol.meloet \
#          --commands '{"afterMs":1300,"command":{"type":"toggleBand","band":"mixer"}},{"afterMs":2600,"command":{"type":"playStop"}}'
#
# --wait: seconds from the launch to attaching (default 12: the song is open and playing).
# --stops: stops of gdb to write down (default 4000; one for each rectangle asked for).
# --shown: examples printed per kind of layout (default 5).
# --run: start this command instead of app-watch.sh (60 s), for a run that drives the app itself
#   (tools/measure/remote.py). Its WebKitWebProcess has to be there after --wait seconds.
#
# The app runs under gdb, because gdb may only attach to its own descendants here.
# Needs gdb, and the debug file of libwebkit2gtk-4.1 in the debuginfod cache: run once
#   DEBUGINFOD_URLS=https://debuginfod.ubuntu.com gdb -batch -ex 'set debuginfod enabled on' \
#     -ex 'info line WebKit::WebPage::drawRect' /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0
# The stops slow the page down a lot: read which rectangles there are, not how often.
# In DIR/layout.txt: `ask` is a rectangle asked for (CSS px), `flush` the end of a layout or a style
# update, `need` what reaches the drawing area (an `ask` outside a layout goes straight to `need`).
set -euo pipefail

out=
wait_s=12
stops=4000
shown=5
run=
rest=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out=$2; shift 2 ;;
    --wait) wait_s=$2; shift 2 ;;
    --stops) stops=$2; shift 2 ;;
    --shown) shown=$2; shift 2 ;;
    --run) run=$2; shift 2 ;;
    *) rest+=("$1"); shift ;;
  esac
done

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${out:-$root/target/measure/webkit-layout}
mkdir -p "$out"

lib=/usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0
id=$(file -L "$lib" | grep -o 'BuildID\[sha1\]=[0-9a-f]*' | cut -d= -f2)
debug=${DEBUGINFOD_CACHE_PATH:-$HOME/.cache/debuginfod_client}/$id/debuginfo
[ -f "$debug" ] || { echo "webkit-layout.sh: $debug is not there (see the top of this file)" >&2; exit 1; }

command=${run:-$(printf '%q ' "$root/tools/measure/app-watch.sh" --seconds 60 --out "$out/app-watch" "${rest[@]}")}
LAYOUT_OUT="$out/layout.txt" LAYOUT_COMMAND="$command" LAYOUT_WAIT_S="$wait_s" LAYOUT_STOPS="$stops" LAYOUT_DEBUG_FILE="$debug" \
  gdb -q -batch -x "$root/tools/measure/webkit_layout.py" >/dev/null 2>"$out/gdb.stderr" || true
python3 -I "$root/tools/measure/webkit_layout_summary.py" "$out/layout.txt" "$shown" | tee "$out/summary.txt"
