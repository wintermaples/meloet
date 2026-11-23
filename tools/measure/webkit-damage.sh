#!/usr/bin/env bash
# Shows which rectangles of the page WebKitGTK repaints each frame, and when it gives up and
# repaints one rectangle around all of them (docs/history/measurements/2026-11-19-round4-mixer.md, section 6).
# Only tells something where the page is painted without accelerated compositing (meloet sets
# WEBKIT_DISABLE_COMPOSITING_MODE=1; docs/architecture.md 7).
# Gives the paints (and how many a second), the whole-window paints, the rectangles per paint and
# the largest count of a paint that was not whole-window.
#
# usage: tools/measure/webkit-damage.sh [--out DIR] [--wait S] [--shown N] [--whole-px N] [--per-rect]
#                                       [app-watch.sh arguments]
#   e.g. tools/measure/webkit-damage.sh --song band8.meloet \
#          --commands '{"afterMs":1300,"command":{"type":"toggleBand","band":"mixer"}},{"afterMs":2600,"command":{"type":"playStop"}}'
#
# --wait: seconds from the launch to attaching (default 12: the song is open and playing).
# --shown: examples printed per kind of paint (default 5).
# --whole-px: a paint of one rectangle this large or larger is a whole-window paint (default 500000).
# --per-rect: also stop at every rectangle the page reports as changed, as the tool did before
#   2026-11-24 (to compare with older numbers, or to see what was changed). Where many rectangles
#   change, the page then runs slower (15 to 25 paints a second with E1's 10 000-note song), and a
#   line that moves every frame moves further between paints and makes more rectangles: things
#   that change every frame look worse than they are.
#
# The app is the release build in ${CARGO_TARGET_DIR:-target} (app-watch.sh; build it with
# tools/measure/build-dev.sh).
# Starts app-watch.sh (60 s) under gdb, because gdb may only attach to its own descendants here.
# Needs gdb, and the debug file of libwebkit2gtk-4.1 in the debuginfod cache: run once
#   DEBUGINFOD_URLS=https://debuginfod.ubuntu.com gdb -batch -ex 'set debuginfod enabled on' \
#     -ex 'info line WebKit::WebPage::drawRect' /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0
# gdb stops at every paint, so the page runs slower than in the app (the summary says how many
# paints a second): read the rectangles per paint, not times.
set -euo pipefail

out=
wait_s=12
shown=5
whole_px=500000
per_rect=0
rest=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out=$2; shift 2 ;;
    --wait) wait_s=$2; shift 2 ;;
    --shown) shown=$2; shift 2 ;;
    --whole-px) whole_px=$2; shift 2 ;;
    --per-rect) per_rect=1; shift ;;
    *) rest+=("$1"); shift ;;
  esac
done

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${out:-$root/target/measure/webkit-damage}
mkdir -p "$out"

lib=/usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0
id=$(file -L "$lib" | grep -o 'BuildID\[sha1\]=[0-9a-f]*' | cut -d= -f2)
debug=${DEBUGINFOD_CACHE_PATH:-$HOME/.cache/debuginfod_client}/$id/debuginfo
[ -f "$debug" ] || { echo "webkit-damage.sh: $debug is not there (see the top of this file)" >&2; exit 1; }

command="$(printf '%q ' "$root/tools/measure/app-watch.sh" --seconds 60 --out "$out/app-watch" "${rest[@]}")"
DAMAGE_OUT="$out/damage.txt" DAMAGE_COMMAND="$command" DAMAGE_WAIT_S="$wait_s" DAMAGE_DEBUG_FILE="$debug" \
  DAMAGE_PER_RECT="$per_rect" gdb -q -batch -x "$root/tools/measure/webkit_damage.py" >/dev/null 2>"$out/gdb.stderr" || true
python3 -I "$root/tools/measure/webkit_damage_summary.py" "$out/damage.txt" "$shown" "$whole_px" | tee "$out/summary.txt"
