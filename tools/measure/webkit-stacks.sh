#!/usr/bin/env bash
# Where the main thread of the WebKitWebProcess is, sampled with gdb (perf is closed on this PC).
# Tells which part of WebKit a heavy frame is in: script, style, layout, painting, WebGL.
#
# usage: tools/measure/webkit-stacks.sh [--out DIR] [--wait S] [--seconds S] [--every MS] -- COMMAND...
#   e.g. tools/measure/webkit-stacks.sh --wait 20 --seconds 20 -- \
#          python3 -I tools/measure/arrange_parts.py OUT --song FILE --splits 11 --hold 1500
#
# --wait: seconds from starting COMMAND to attaching. --seconds: how long to sample. --every: ms between samples.
# COMMAND starts the app (gdb may only attach to its own descendants here) and must keep the page doing
# the thing to look at from --wait to --wait + --seconds.
# Needs the debug file of libwebkit2gtk-4.1 in the debuginfod cache (see the top of webkit-damage.sh).
# The stops slow the page down: read the shares, not the times. Writes DIR/stacks.txt.
set -euo pipefail

out=
wait_s=12
seconds=15
every=25
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out=$2; shift 2 ;;
    --wait) wait_s=$2; shift 2 ;;
    --seconds) seconds=$2; shift 2 ;;
    --every) every=$2; shift 2 ;;
    --) shift; break ;;
    *) echo "webkit-stacks.sh: unknown argument $1" >&2; exit 1 ;;
  esac
done
[ $# -gt 0 ] || { echo "webkit-stacks.sh: no command" >&2; exit 1; }

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${out:-$root/target/measure/webkit-stacks}
mkdir -p "$out"

lib=/usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0
id=$(file -L "$lib" | grep -o 'BuildID\[sha1\]=[0-9a-f]*' | cut -d= -f2)
debug=${DEBUGINFOD_CACHE_PATH:-$HOME/.cache/debuginfod_client}/$id/debuginfo
[ -f "$debug" ] || { echo "webkit-stacks.sh: $debug is not there (see the top of webkit-damage.sh)" >&2; exit 1; }

command="$(printf '%q ' "$@")"
STACKS_OUT="$out/stacks.txt" STACKS_COMMAND="$command" STACKS_WAIT_S="$wait_s" STACKS_SECONDS="$seconds" \
  STACKS_EVERY_MS="$every" STACKS_DEBUG_FILE="$debug" \
  gdb -q -batch -x "$root/tools/measure/webkit_stacks.py" >/dev/null 2>"$out/gdb.stderr" || true
head -60 "$out/stacks.txt"
