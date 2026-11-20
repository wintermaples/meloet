#!/usr/bin/env bash
# Records for minutes in the real app (release, with its screen) while a virtual keyboard plays,
# then plays the take back and shoots the screen.
#
# usage: tools/measure/long-record.sh [--seconds 600] [--window 1920x1080] [--out DIR] [--no-build]
#                                     [--monitor SINK_NODE_NAME] [--piano-roll] [--mixer]
#                                     [--song FILE]
#
# --monitor: the sink whose monitor is recorded during the playback (default: the default sink).
# --piano-roll: open the piano roll band before recording and keep it open (it shows the take
#   being recorded, then the take).
# --mixer: open the mixer band before recording and keep it open.
# --song: open a copy of that song (MELOET_OPEN_SONG) before recording; the recording goes to
#   its selected track while the other tracks play. The copy is saved after the playback (before
#   the shot, which ends the app) and
#   kept as DIR/saved.meloet; long_record_ticks.py then compares where the keys were recorded
#   with when they were sent, through the song's tempo map (DIR/ticks.md).
#
# Watches (tools/measure/long_record_watch.py): PipeWire's error count (xruns) of the meloet
# stream and its driver, the RSS of meloet and its WebKitWebProcess every 10 s, and the GC helper
# threads' CPU time every second. Records what meloet plays back (the output's monitor) to tell
# whether the take sounds. Writes DIR/summary.md and the raw files. Changes nothing on the PC.
set -euo pipefail

seconds=600
window=1920x1080
out=
build=1
monitor=()
piano_roll=0
mixer=0
song=
while [ $# -gt 0 ]; do
  case "$1" in
    --seconds) seconds=$2; shift 2 ;;
    --window) window=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    --no-build) build=0; shift ;;
    --monitor) monitor=(--target "$2"); shift 2 ;;
    --piano-roll) piano_roll=1; shift ;;
    --mixer) mixer=1; shift ;;
    --song) song=$2; shift 2 ;;
    *) echo "usage: $0 [--seconds N] [--window WxH] [--out DIR] [--no-build] [--monitor NODE] [--piano-roll] [--mixer] [--song FILE]" >&2; exit 2 ;;
  esac
done

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${out:-$root/target/measure/long-record}
target=${CARGO_TARGET_DIR:-$root/target}
case "$target" in /*) ;; *) target=$root/$target ;; esac
vkeys_name=meloet-vkeys-long-$$

if [ "$build" = 1 ]; then
  (cd "$root/ui" && npm run build)
  (cd "$root" && cargo build --release -p meloet-app)
fi
(cd "$root" && cargo test --release -p meloet-core --test vkeys_long_play --no-run)
player=$(cd "$root" && cargo test --release -p meloet-core --test vkeys_long_play --no-run 2>&1 |
  sed -n 's/.*Executable .*(\(.*\))$/\1/p' | tail -1)
case "$player" in /*) ;; *) player=$root/$player ;; esac

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/config" "$work/state" "$out"
rm -f "$out"/*.json "$out"/*.png "$out"/*.wav "$out"/*.md "$out"/*.stderr "$out"/*.meloet

# This PC only: the agent user reaches the screen and the sound through this file.
if [ -f "$HOME/.config/meloet-agent/env.sh" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.config/meloet-agent/env.sh"
  export PIPEWIRE_RUNTIME_DIR=${PIPEWIRE_RUNTIME_DIR:-/run/user/1000}
fi
launcher=()
if command -v dbus-run-session >/dev/null; then launcher=(dbus-run-session --); fi

# Times after the page loaded, in ms. The count-in is 1 bar at 96 bpm (2.5 s).
rec_ms=3000
stop_ms=$((rec_ms + 2500 + seconds * 1000 + 6000))
play_ms=$((stop_ms + 2000))
play_for_ms=20000
snap_ms=$((play_ms + play_for_ms + 3000))
cmd() { printf '{"afterMs":%d,"command":%s}' "$1" "$2"; }
# Without --piano-roll the band is opened only for the last shot; with it, before recording.
band_at=$((play_ms + play_for_ms + 1000))
[ "$piano_roll" = 1 ] && band_at=2000
commands="[$(cmd 1500 '{"type":"chooserConfirm"}'),$(cmd 2500 '{"type":"toggleMetronome"}'),\
$(cmd $rec_ms '{"type":"record"}'),$(cmd $stop_ms '{"type":"record"}'),\
$(cmd $((stop_ms + 500)) '{"type":"toggleMetronome"}'),$(cmd $((stop_ms + 1000)) '{"type":"goToStart"}'),\
$(cmd $play_ms '{"type":"playStop"}'),$(cmd $((play_ms + play_for_ms)) '{"type":"playStop"}'),\
$(cmd $band_at '{"type":"toggleBand","band":"pianoRoll"}')"
[ "$mixer" = 1 ] && commands="$commands,$(cmd 2200 '{"type":"toggleBand","band":"mixer"}')"
open_song=()
if [ -n "$song" ]; then
  cp "$song" "$work/song.meloet"
  open_song=(MELOET_OPEN_SONG="$work/song.meloet")
  commands="$commands,$(cmd $((play_ms + play_for_ms + 500)) '{"type":"saveSong"}')"
fi
commands="$commands]"

# The keyboard is there before the app starts: the output chooser comes up only with one.
MELOET_LONGPLAY_NAME="$vkeys_name" MELOET_LONGPLAY_SECONDS="$seconds" MELOET_LONGPLAY_LEAD_MS=10000 \
MELOET_LONGPLAY_LOG="$out/vkeys.json" "$player" --ignored --exact play_for_minutes --nocapture \
  >/dev/null 2>&1 &
keys=$!
sleep 1

env "${open_song[@]}" \
MELOET_CONFIG_DIR="$work/config" \
MELOET_STATE_DIR="$work/state" \
MELOET_ONLY_VIRTUAL=1 \
MELOET_VKEYS_NAME="$vkeys_name" \
MELOET_WINDOW="$window" \
MELOET_COMMANDS="$commands" \
MELOET_SNAPSHOT="$out/after.png" \
MELOET_SNAPSHOT_AFTER_MS="$snap_ms" \
MELOET_EXIT_AFTER_MS=$((snap_ms + 15000)) \
  "${launcher[@]}" "$target/release/meloet" 2>"$out/meloet.stderr" &
launched=$!
started=$(date +%s.%N)

app=
for _ in $(seq 100); do
  app=$(pgrep -u "$(id -u)" -x meloet -n || true)
  [ -n "$app" ] && break
  sleep 0.1
done
[ -n "$app" ] || { echo "long-record.sh: meloet did not start" >&2; exit 1; }
python3 -I "$root/tools/measure/long_record_watch.py" "$out" $((seconds + 120)) "$app" &
watch=$!

wait "$keys"

# The take is played back about (page load) + play_ms; record from a little before to after.
until_play=$(python3 -c "import time; print(max(0.0, $started + $play_ms / 1000 - 3 - time.time()))")
sleep "$until_play"
pw-record -P stream.capture.sink=true "${monitor[@]}" \
  --format s16 --rate 48000 --channels 2 "$out/playback.wav" &
recorder=$!
sleep $((play_for_ms / 1000 + 8))
kill "$recorder" 2>/dev/null || true
wait "$launched" || true
wait "$watch" || true
python3 -I "$root/tools/measure/long_record_summary.py" "$out" | tee "$out/summary.md"
if [ -n "$song" ]; then
  cp "$work/song.meloet" "$out/saved.meloet"
  python3 -I "$root/tools/measure/long_record_ticks.py" "$out" --before "$song" | tee "$out/ticks.md"
fi
