#!/usr/bin/env bash
# Starts the real app (release) for some seconds and writes down what it does by itself: the
# engine's load as the screen receives it, the WebGL contexts the page made and holds, the RSS
# of meloet and its WebKitWebProcess, and when things happened after the launch (the page, the
# first frame, each sound read).
#
# usage: tools/measure/app-watch.sh [--seconds 30] [--song FILE] [--window 1920x1080]
#                                   [--commands 'JSON entries'] [--no-keyboard] [--shot]
#                                   [--out DIR]
#
# --song: open that song (MELOET_OPEN_SONG).
# --commands: entries for MELOET_COMMANDS after the chooser's confirm (1500 ms), without the
#   brackets, e.g. '{"afterMs":2500,"command":{"type":"toggleBand","band":"mixer"}}'.
# --no-keyboard: start no virtual keyboard. Without a keyboard the output chooser does not come
#   up and no stream is opened: for the launch and the memory only.
# --shot: write DIR/shot.png 2 s before the end (the app then exits with the shot).
#
# The sounds are whatever MELOET_SOUNDS_DIR names (unset: where the app looks by itself).
# Builds nothing: run tools/measure/build-dev.sh first (the hooks this reads are only there).
# Writes DIR/summary.md and the raw files in DIR/raw/. Changes nothing on the PC.
set -euo pipefail

seconds=30
song=
window=1920x1080
extra=
keyboard=1
shot=0
out=
while [ $# -gt 0 ]; do
  case "$1" in
    --seconds) seconds=$2; shift 2 ;;
    --song) song=$2; shift 2 ;;
    --window) window=$2; shift 2 ;;
    --commands) extra=$2; shift 2 ;;
    --no-keyboard) keyboard=0; shift ;;
    --shot) shot=1; shift ;;
    --out) out=$2; shift 2 ;;
    *) echo "usage: $0 [--seconds N] [--song FILE] [--window WxH] [--commands JSON] [--no-keyboard] [--shot] [--out DIR]" >&2; exit 2 ;;
  esac
done

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${out:-$root/target/measure/app-watch}
target=${CARGO_TARGET_DIR:-$root/target}
case "$target" in /*) ;; *) target=$root/$target ;; esac
for bin in meloet meloet-vkeys; do
  [ -x "$target/release/$bin" ] || { echo "app-watch.sh: $target/release/$bin is not there" >&2; exit 1; }
done

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/config" "$work/state" "$work/measure" "$out/raw"
rm -f "$out"/raw/* "$out"/summary.md "$out"/shot.png

# This PC only: the agent user reaches the screen and the sound through this file.
if [ -f "$HOME/.config/meloet-agent/env.sh" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.config/meloet-agent/env.sh"
  export PIPEWIRE_RUNTIME_DIR=${PIPEWIRE_RUNTIME_DIR:-/run/user/1000}
fi
launcher=()
if command -v dbus-run-session >/dev/null; then launcher=(dbus-run-session --); fi

vkeys_name=meloet-vkeys-watch-$$
keys=
if [ "$keyboard" = 1 ]; then
  "$target/release/meloet-vkeys" appear --name "$vkeys_name" --seconds $((seconds + 60)) >/dev/null 2>&1 &
  keys=$!
  sleep 1
fi

commands="[{\"afterMs\":1500,\"command\":{\"type\":\"chooserConfirm\"}}${extra:+,$extra}]"
vars=(MELOET_CONFIG_DIR="$work/config" MELOET_STATE_DIR="$work/state" MELOET_MEASURE_DIR="$work/measure"
  MELOET_ACCEPT_VIRTUAL=1 MELOET_VKEYS_NAME="$vkeys_name" MELOET_WINDOW="$window" MELOET_UI_SCRIPT=watch
  MELOET_COMMANDS="$commands")
[ -n "$song" ] && vars+=(MELOET_OPEN_SONG="$song")
if [ "$shot" = 1 ]; then
  vars+=(MELOET_SNAPSHOT="$out/shot.png" MELOET_SNAPSHOT_AFTER_MS=$((seconds * 1000 - 2000)))
else
  vars+=(MELOET_EXIT_AFTER_MS=$((seconds * 1000)))
fi

# Each line of stderr gets the ms since the launch in front.
stamp() {
  python3 -c 'import sys, time
t0 = int(sys.argv[1])
for line in sys.stdin:
    print(int(time.time() * 1000) - t0, line, end="", flush=True)' "$1"
}
# Not `date +%s%3N`: uutils date 0.10 drops the leading zeros of the milliseconds.
launched_ms=$(($(date +%s%N) / 1000000))
env "${vars[@]}" "${launcher[@]}" "$target/release/meloet" 2> >(stamp "$launched_ms" >"$out/raw/meloet.stderr") &
launched=$!

app=
for _ in $(seq 100); do
  app=$(pgrep -u "$(id -u)" -x meloet -n || true)
  [ -n "$app" ] && break
  sleep 0.1
done
[ -n "$app" ] || { echo "app-watch.sh: meloet did not start" >&2; exit 1; }
python3 -I "$root/tools/measure/rss_watch.py" "$out/raw/rss.json" "$app" &
watch=$!

for _ in $(seq $((seconds + 40))); do
  kill -0 "$app" 2>/dev/null || break
  sleep 1
done
if kill -0 "$app" 2>/dev/null; then
  echo "app-watch.sh: meloet did not exit by itself; stopping it" >&2
  kill -TERM "$app"
fi
wait "$launched" || true
wait "$watch" || true
if [ -n "$keys" ]; then kill "$keys" 2>/dev/null || true; fi

cp "$work/measure/"*.json "$out/raw/" 2>/dev/null || true
python3 -I "$root/tools/measure/app_watch_summary.py" "$out/raw" "$launched_ms" \
  --label "release, window $window, ${seconds} s${song:+, song $(basename "$song")}, sounds ${MELOET_SOUNDS_DIR:-not named}" |
  tee "$out/summary.md"
