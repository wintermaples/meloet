#!/usr/bin/env bash
# Measures key -> screen and key -> sound of the release build with the virtual keyboard.
#
# usage: tools/measure/latency.sh [--window 1920x1080] [--count 120] [--out DIR] [--keep-ui]
#                                 [--no-build] [--state stopped|playing|recording] [--jitter-ms N]
#                                 [--pattern spaced|mixed] [--piano-roll [--song FILE]]
#                                 [--velocity-fixed V] [--mixer] [--cache-home DIR]
#                                 [--plugin-window]
#
# --keep-ui: do not build ui/; the app takes whatever ui/dist holds (to measure an older screen).
# --no-build: build nothing; take the release binaries and ui/dist as they are (build first, so
#   that the PC is not busy while measuring).
# --state: what the transport does while the keys are played. playing and recording turn the
#   metronome on and start before the first key (recording: after its count-in).
# --jitter-ms: each key comes up to N ms later than its even step (random, below 137).
# --pattern: spaced (the default) plays one key at a time; mixed plays single keys, chords of
#   2..4 (keys 0..6 ms apart) and runs of 2..4 keys 20..150 ms apart, like real playing. The
#   summary splits key -> screen into spaced keys and keys soon after (within one frame).
# --piano-roll: open a song (MELOET_OPEN_SONG), select its clip (id 2) and open the piano roll
#   band before the keys, and keep it open. Without --song, make_song.py writes one with 200
#   notes over 16 bars.
# --song: open that song (MELOET_OPEN_SONG), also without --piano-roll.
# --mixer: open the mixer band before the keys, and keep it open.
# --cache-home: XDG_CACHE_HOME for the app. An empty folder makes this start the first one for
#   the plugin scan (the scan then runs while the keys are played); scan_watch.py writes down
#   the scanning children to DIR/raw/scan-watch.json.
# --plugin-window: open the window of the plugin that is the instrument of track 1 (a --song
#   with a plugin track, and DISPLAY for the plugin host) before the keys, and keep it open.
# --velocity-fixed: turn "ベロシティ固定" on at V (1..127) right after the chooser.
#
# Builds ui/ and the release binaries, starts meloet with a fresh settings directory, confirms
# the output chooser, plays --count keys (quiet), lets meloet exit, and writes
# DIR/summary.md and DIR/summary.json (plus the raw files in DIR/raw/).
# Changes nothing on the PC: the PipeWire quantum is whatever it already is.
set -euo pipefail

window=1920x1080
count=120
out=
keep_ui=0
build=1
state=stopped
jitter_ms=0
pattern=spaced
piano_roll=0
mixer=0
song=
velocity_fixed=
cache_home=
plugin_window=0
while [ $# -gt 0 ]; do
  case "$1" in
    --window) window=$2; shift 2 ;;
    --count) count=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    --keep-ui) keep_ui=1; shift ;;
    --no-build) build=0; shift ;;
    --state) state=$2; shift 2 ;;
    --jitter-ms) jitter_ms=$2; shift 2 ;;
    --pattern) pattern=$2; shift 2 ;;
    --piano-roll) piano_roll=1; shift ;;
    --mixer) mixer=1; shift ;;
    --song) song=$2; shift 2 ;;
    --velocity-fixed) velocity_fixed=$2; shift 2 ;;
    --cache-home) cache_home=$2; shift 2 ;;
    --plugin-window) plugin_window=1; shift ;;
    *) echo "usage: $0 [--window WxH] [--count N] [--out DIR] [--keep-ui] [--no-build] [--state stopped|playing|recording] [--jitter-ms N] [--pattern spaced|mixed] [--piano-roll] [--song FILE] [--mixer] [--velocity-fixed V] [--cache-home DIR]" >&2; exit 2 ;;
  esac
done

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${out:-$root/target/measure/latency}
target=${CARGO_TARGET_DIR:-$root/target}
case "$target" in /*) ;; *) target=$root/$target ;; esac

interval_ms=257
hold_ms=120
velocity=25
confirm_ms=1500   # after the page loaded: the chooser is up, confirm its first choosable output
settle_s=8        # from start to the first key: page load, chooser, stream open
lead_ms=500
# Only this client counts as a keyboard, so that virtual keyboards of other runs stay out.
vkeys_name=meloet-vkeys-latency-$$
tail_ms=6000      # after the last key: the screen reports every 2 s
case "$pattern" in
  spaced)
    play_args=(--pattern spaced)
    play_ms=$((count * interval_ms + jitter_ms)) ;;
  mixed)
    # Enough notes that a chord or a run never needs one that is still held.
    play_args=(--pattern mixed --notes 60,62,64,65,67,69,71,72 --spread-ms 6 --fast-ms 20-150)
    # Every key is at most interval + jitter after the one before (a run's 150 ms is less).
    play_ms=$((count * (interval_ms + jitter_ms))) ;;
  *) echo "latency.sh: --pattern must be spaced or mixed" >&2; exit 2 ;;
esac
exit_ms=$((settle_s * 1000 + lead_ms + play_ms + tail_ms))

start_ms=$((confirm_ms + 1000))   # the stream is open by then
case "$state" in
  stopped) transport= ;;
  playing) transport=",{\"afterMs\":$start_ms,\"command\":{\"type\":\"toggleMetronome\"}},{\"afterMs\":$((start_ms + 100)),\"command\":{\"type\":\"playStop\"}}" ;;
  recording) transport=",{\"afterMs\":$start_ms,\"command\":{\"type\":\"toggleMetronome\"}},{\"afterMs\":$((start_ms + 100)),\"command\":{\"type\":\"record\"}}" ;;
  *) echo "latency.sh: --state must be stopped, playing or recording" >&2; exit 2 ;;
esac

if [ "$build" = 1 ]; then
  if [ "$keep_ui" = 0 ]; then (cd "$root/ui" && npm run build); fi
  (cd "$root" && cargo build --release -p meloet-app -p meloet-midi -p meloet-plughost)
fi
for bin in meloet meloet-vkeys meloet-plughost; do
  if [ ! -x "$target/release/$bin" ]; then
    echo "latency.sh: $target/release/$bin is not there (build it, or run without --no-build)" >&2
    exit 1
  fi
done

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/config" "$work/state" "$work/measure" "$out/raw"

# The song is opened when the page has loaded; the clip is selected and the band opened before
# the transport starts.
view=
label_view=
open_song=()
if [ "$piano_roll" = 1 ] && [ -z "$song" ]; then
  song=$work/song.meloet
  python3 -I "$root/tools/measure/make_song.py" "$song" --notes 200 --bars 16 >/dev/null
fi
if [ -n "$song" ]; then
  open_song=(MELOET_OPEN_SONG="$song")
  label_view=", song $(basename "$song")"
fi
if [ "$piano_roll" = 1 ]; then
  view=",{\"afterMs\":$((confirm_ms - 500)),\"command\":{\"type\":\"selectClip\",\"clip\":2}},{\"afterMs\":$((confirm_ms - 300)),\"command\":{\"type\":\"toggleBand\",\"band\":\"pianoRoll\"}}"
  label_view="$label_view, piano roll open"
fi
if [ "$mixer" = 1 ]; then
  view="$view,{\"afterMs\":$((confirm_ms - 200)),\"command\":{\"type\":\"toggleBand\",\"band\":\"mixer\"}}"
  label_view="$label_view, mixer open"
fi
if [ "$plugin_window" = 1 ]; then
  view="$view,{\"afterMs\":$((confirm_ms + 400)),\"command\":{\"type\":\"openPluginWindow\",\"slot\":{\"kind\":\"instrument\",\"track\":1}}}"
  label_view="$label_view, plugin window open"
fi
if [ -n "$velocity_fixed" ]; then
  view="$view,{\"afterMs\":$((confirm_ms + 200)),\"command\":{\"type\":\"setVelocityFixed\",\"on\":true,\"value\":$velocity_fixed}}"
  label_view="$label_view, velocity fixed at $velocity_fixed"
fi

# This PC only: the agent user reaches the screen and the sound through this file.
if [ -f "$HOME/.config/meloet-agent/env.sh" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.config/meloet-agent/env.sh"
  export PIPEWIRE_RUNTIME_DIR=${PIPEWIRE_RUNTIME_DIR:-/run/user/1000}
fi

# GTK can wait 30 s for a session bus that is not there.
launcher=()
if command -v dbus-run-session >/dev/null; then launcher=(dbus-run-session --); fi

# The output chooser comes up only when there is a keyboard, and the stream opens when it is
# confirmed: a keyboard has to be there from the start, not only when the keys are played.
"$target/release/meloet-vkeys" appear --name "$vkeys_name" --seconds $((exit_ms / 1000 + 60)) >/dev/null 2>&1 &
standby=$!
trap 'kill "$standby" 2>/dev/null || true; rm -rf "$work"' EXIT
sleep 1

scan_watch=
if [ -n "$cache_home" ]; then
  mkdir -p "$cache_home"
  open_song+=(XDG_CACHE_HOME="$cache_home")
  label_view="$label_view, cache home $cache_home"
  python3 -I "$root/tools/measure/scan_watch.py" "$out/raw/scan-watch.json" "$target/release/meloet-plughost" \
    --seconds $((exit_ms / 1000 + 20)) &
  scan_watch=$!
fi

env "${open_song[@]}" \
MELOET_CONFIG_DIR="$work/config" \
MELOET_STATE_DIR="$work/state" \
MELOET_MEASURE_DIR="$work/measure" \
MELOET_ACCEPT_VIRTUAL=1 \
MELOET_VKEYS_NAME="$vkeys_name" \
MELOET_WINDOW="$window" \
MELOET_COMMANDS="[{\"afterMs\":$confirm_ms,\"command\":{\"type\":\"chooserConfirm\"}}$view$transport]" \
MELOET_EXIT_AFTER_MS="$exit_ms" \
  "${launcher[@]}" "$target/release/meloet" 2>"$out/raw/meloet.stderr" &
app=$!

sleep "$settle_s"
if ! kill -0 "$app" 2>/dev/null; then
  echo "latency.sh: meloet ended before the first key:" >&2
  cat "$out/raw/meloet.stderr" >&2
  exit 1
fi
"$target/release/meloet-vkeys" play --name "$vkeys_name" --count "$count" --interval-ms "$interval_ms" --jitter-ms "$jitter_ms" \
  --hold-ms "$hold_ms" --velocity "$velocity" --lead-ms "$lead_ms" --log "$work/vkeys.json" "${play_args[@]}"

# The page may have loaded late; give the exit hook that long, then stop waiting.
limit=$((exit_ms / 1000 + 30))
for _ in $(seq "$limit"); do
  kill -0 "$app" 2>/dev/null || break
  sleep 1
done
if kill -0 "$app" 2>/dev/null; then
  echo "latency.sh: meloet did not exit by itself; stopping it" >&2
  kill "$app"
fi
wait "$app" || true
if [ -n "$scan_watch" ]; then
  kill -TERM "$scan_watch" 2>/dev/null || true
  wait "$scan_watch" || true
  cp "$cache_home/meloet/plugins/scan.json" "$out/raw/" 2>/dev/null || true
fi

cp "$work/vkeys.json" "$out/raw/" 2>/dev/null || true
cp "$work/measure/"*.json "$out/raw/" 2>/dev/null || true
python3 -I "$root/tools/measure/summarize.py" "$out/raw" --vkeys "$out/raw/vkeys.json" \
  --out "$out" --label "release, window $window, $state, $count keys, $pattern, jitter $jitter_ms ms$label_view"
