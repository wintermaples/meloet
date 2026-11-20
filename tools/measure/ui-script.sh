#!/usr/bin/env bash
# Plays mouse input on the piano roll of the real app (release) and writes where each frame's time
# went, and the RSS of meloet and its WebKitWebProcess.
#
# usage: tools/measure/ui-script.sh --script SCRIPT [--song FILE | --notes N --bars B]
#                                   [--window 1920x1080] [--out DIR] [--no-build]
#
# --script: MELOET_UI_SCRIPT, e.g. pr-scroll:100, pr-zoom:100, pr-drag:1000, or several joined
#   with "," (ui/src/dev/script.ts). Shift + wheel, Mod + wheel, and drag-and-let-go of a note,
#   sent as DOM events to the note area.
#   Or the history band's parts (ui/src/dev/historyScript.ts), not mixed with the pr-* ones:
#   hist-make:5000 (makes 5000 steps by changing the tempo), hist-open:20, hist-scroll:200,
#   hist-jump:20 (500 steps back, then forward). The piano roll is not opened for them.
# --song: the song to open (MELOET_OPEN_SONG). Without it, one is made with make_song.py
#   (--notes, default 10000, over --bars, default 100).
#
# Starts meloet with a fresh settings and state directory, opens the song, selects its clip
# (id 2, as make_song.py writes it), opens the piano roll band (not for hist-*), waits until the script reports
# that it is done, and stops meloet with SIGTERM. Writes DIR/summary.md and the raw files in
# DIR/raw/. Changes nothing on the PC.
set -euo pipefail

script=
song=
notes=10000
bars=100
window=1920x1080
out=
build=1
while [ $# -gt 0 ]; do
  case "$1" in
    --script) script=$2; shift 2 ;;
    --song) song=$2; shift 2 ;;
    --notes) notes=$2; shift 2 ;;
    --bars) bars=$2; shift 2 ;;
    --window) window=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    --no-build) build=0; shift ;;
    *) echo "usage: $0 --script SCRIPT [--song FILE | --notes N --bars B] [--window WxH] [--out DIR] [--no-build]" >&2; exit 2 ;;
  esac
done
[ -n "$script" ] || { echo "ui-script.sh: --script is needed" >&2; exit 2; }

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${out:-$root/target/measure/ui-script}
target=${CARGO_TARGET_DIR:-$root/target}
case "$target" in /*) ;; *) target=$root/$target ;; esac

if [ "$build" = 1 ]; then
  (cd "$root/ui" && npm run build)
  (cd "$root" && cargo build --release -p meloet-app)
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/config" "$work/state" "$work/measure" "$out/raw"
rm -f "$out"/raw/*.json "$out"/summary.*
if [ -z "$song" ]; then
  song=$work/song.meloet
  python3 -I "$root/tools/measure/make_song.py" "$song" --notes "$notes" --bars "$bars" >/dev/null
fi

# This PC only: the agent user reaches the screen and the sound through this file.
if [ -f "$HOME/.config/meloet-agent/env.sh" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.config/meloet-agent/env.sh"
  export PIPEWIRE_RUNTIME_DIR=${PIPEWIRE_RUNTIME_DIR:-/run/user/1000}
fi
launcher=()
if command -v dbus-run-session >/dev/null; then launcher=(dbus-run-session --); fi

cmd() { printf '{"afterMs":%d,"command":%s}' "$1" "$2"; }
case "$script" in
  hist-*) commands="[$(cmd 1000 '{"type":"chooserConfirm"}')]" ;;
  *) commands="[$(cmd 1000 '{"type":"chooserConfirm"}'),$(cmd 1500 '{"type":"selectClip","clip":2}'),\
$(cmd 1700 '{"type":"toggleBand","band":"pianoRoll"}')]" ;;
esac

MELOET_CONFIG_DIR="$work/config" \
MELOET_STATE_DIR="$work/state" \
MELOET_MEASURE_DIR="$work/measure" \
MELOET_WINDOW="$window" \
MELOET_OPEN_SONG="$song" \
MELOET_UI_SCRIPT="$script" \
MELOET_COMMANDS="$commands" \
MELOET_EXIT_AFTER_MS=1800000 \
  "${launcher[@]}" "$target/release/meloet" 2>"$out/raw/meloet.stderr" &
launched=$!

app=
for _ in $(seq 100); do
  app=$(pgrep -u "$(id -u)" -x meloet -n || true)
  [ -n "$app" ] && break
  sleep 0.1
done
[ -n "$app" ] || { echo "ui-script.sh: meloet did not start" >&2; exit 1; }
python3 -I "$root/tools/measure/rss_watch.py" "$out/raw/rss.json" "$app" &
watch=$!

finished() {
  python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); sys.exit(0 if r["done"] or r["error"] else 1)' \
    "$work/measure/ui-script.json" 2>/dev/null
}
for _ in $(seq 1800); do
  finished && break
  kill -0 "$app" 2>/dev/null || break
  sleep 1
done
# The frame profile is sent every 2 s.
sleep 4
kill -TERM "$app" 2>/dev/null || true
wait "$launched" || true
wait "$watch" || true

cp "$work/measure/"*.json "$out/raw/" 2>/dev/null || true
python3 -I "$root/tools/measure/ui_script_summary.py" "$out/raw" \
  --label "release, window $window, $script, $(basename "$song")" | tee "$out/summary.md"
