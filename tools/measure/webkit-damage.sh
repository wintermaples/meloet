#!/usr/bin/env bash
# Shows which rectangles of the page WebKitGTK repaints each frame, and when it gives up and
# repaints one rectangle around all of them (docs/history/measurements/2026-11-19-round4-mixer.md, section 6).
# Only tells something where the page is painted without accelerated compositing (NVIDIA with
# Ubuntu's WebKitGTK).
#
# usage: tools/measure/webkit-damage.sh [--out DIR] [--wait S] [--shown N] [app-watch.sh arguments]
#   e.g. tools/measure/webkit-damage.sh --song band8.meloet \
#          --commands '{"afterMs":1300,"command":{"type":"toggleBand","band":"mixer"}},{"afterMs":2600,"command":{"type":"playStop"}}'
#
# --wait: seconds from the launch to attaching (default 12: the song is open and playing).
# --shown: examples printed per kind of paint (default 5).
#
# Starts app-watch.sh (60 s) under gdb, because gdb may only attach to its own descendants here.
# Needs gdb, and the debug file of libwebkit2gtk-4.1 in the debuginfod cache: run once
#   DEBUGINFOD_URLS=https://debuginfod.ubuntu.com gdb -batch -ex 'set debuginfod enabled on' \
#     -ex 'info line WebKit::WebPage::drawRect' /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0
# The stops slow the page down a lot: read which rectangles there are, not how often.
set -euo pipefail

out=
wait_s=12
shown=5
rest=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out=$2; shift 2 ;;
    --wait) wait_s=$2; shift 2 ;;
    --shown) shown=$2; shift 2 ;;
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
  gdb -q -batch -x "$root/tools/measure/webkit_damage.py" >/dev/null 2>"$out/gdb.stderr" || true
python3 -I "$root/tools/measure/webkit_damage_summary.py" "$out/damage.txt" "$shown" | tee "$out/summary.txt"
