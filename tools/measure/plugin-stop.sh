#!/usr/bin/env bash
# Stops a plugin host of the real app (release) from outside while it plays or records, and
# writes down what meloet did about it.
#
# usage: tools/measure/plugin-stop.sh --song FILE [--signal KILL|SEGV|STOP] [--state playing|recording]
#                                     [--reload] [--nth 0] [--window 1920x1080] [--out DIR]
#
# --song: a song whose first track (the play target) sounds with a plugin, made with
#   `make_song.py --hard-pan`: the plugin tracks are heard on the left, the others on the right.
#   A copy is opened, so the file stays as it is.
# --signal: what the plugin host gets, 12 s after the launch (default KILL). STOP makes it a
#   host that does not answer.
# --reload: 5 s later "もう一度読みこむ" for the first track's plugin.
# --nth: which host, in the order of their pids, when the song has more than one.
#
# Times after the page loaded: the shelf opens (2 s), play or record starts (3 s; recording
# begins after the count-in, about 5.6 s), a virtual keyboard plays a key every 300 ms from
# 6.5 s to 24.5 s, the signal (about 11.5 s), the reload (17 s), stop (25 s), save (26 s), a
# shot of the screen (28 s) and the end.
# Watches: stderr with the ms since the launch in front of each line (meloet says when the
# process of a plugin stopped), the plugin hosts (plug_watch.py), the RSS (rss_watch.py), and
# what comes out of the output (its monitor, pw-record). Writes DIR/summary.md, DIR/shot.png
# and the raw files in DIR/raw/ (saved.meloet is the song as saved at the end).
# Builds nothing. Changes nothing on the PC. The host that is stopped writes no crash report
# (MELOET_PLUGHOST_NO_DUMP).
set -euo pipefail

song=
sig=KILL
state=playing
reload=0
nth=0
window=1920x1080
out=
while [ $# -gt 0 ]; do
  case "$1" in
    --song) song=$2; shift 2 ;;
    --signal) sig=$2; shift 2 ;;
    --state) state=$2; shift 2 ;;
    --reload) reload=1; shift ;;
    --nth) nth=$2; shift 2 ;;
    --window) window=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    *) echo "usage: $0 --song FILE [--signal KILL|SEGV|STOP] [--state playing|recording] [--reload] [--nth N] [--window WxH] [--out DIR]" >&2; exit 2 ;;
  esac
done
[ -n "$song" ] || { echo "plugin-stop.sh: --song is needed" >&2; exit 2; }

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${out:-$root/target/measure/plugin-stop}
target=${CARGO_TARGET_DIR:-$root/target}
case "$target" in /*) ;; *) target=$root/$target ;; esac
for bin in meloet meloet-vkeys meloet-plughost; do
  [ -x "$target/release/$bin" ] || { echo "plugin-stop.sh: $target/release/$bin is not there" >&2; exit 1; }
done

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/config" "$work/state" "$work/song" "$out/raw"
rm -f "$out"/raw/* "$out"/summary.md "$out"/shot.png
cp "$song" "$work/song/stop.meloet"

# This PC only: the agent user reaches the screen and the sound through this file.
if [ -f "$HOME/.config/meloet-agent/env.sh" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.config/meloet-agent/env.sh"
  export PIPEWIRE_RUNTIME_DIR=${PIPEWIRE_RUNTIME_DIR:-/run/user/1000}
fi
launcher=()
if command -v dbus-run-session >/dev/null; then launcher=(dbus-run-session --); fi

signal_s=12
keys_from_s=6
keys=60
key_ms=300
stop_ms=25000
snap_ms=28000
cmd() { printf '{"afterMs":%d,"command":%s}' "$1" "$2"; }
commands="$(cmd 1500 '{"type":"chooserConfirm"}'),$(cmd 2000 '{"type":"toggleBand","band":"shelf"}')"
case "$state" in
  playing) commands="$commands,$(cmd 3000 '{"type":"playStop"}'),$(cmd $stop_ms '{"type":"playStop"}')" ;;
  recording) commands="$commands,$(cmd 2900 '{"type":"toggleMetronome"}'),$(cmd 3000 '{"type":"record"}'),$(cmd $stop_ms '{"type":"record"}')" ;;
  *) echo "plugin-stop.sh: --state must be playing or recording" >&2; exit 2 ;;
esac
if [ "$reload" = 1 ]; then
  commands="$commands,$(cmd 17000 '{"type":"reloadPlugin","slot":{"kind":"instrument","track":1}}')"
fi
commands="[$commands,$(cmd $((stop_ms + 1000)) '{"type":"saveSong"}')]"

vkeys_name=meloet-vkeys-stop-$$
"$target/release/meloet-vkeys" appear --name "$vkeys_name" --seconds 60 >/dev/null 2>&1 &
standby=$!
trap 'kill "$standby" 2>/dev/null || true; rm -rf "$work"' EXIT
sleep 1

stamp() {
  python3 -c 'import sys, time
t0 = int(sys.argv[1])
for line in sys.stdin:
    print(int(time.time() * 1000) - t0, line, end="", flush=True)' "$1"
}
python3 -I "$root/tools/measure/plug_watch.py" "$out/raw/plug-watch.json" "$target/release/meloet-plughost" \
  --seconds 34 --every-ms 250 --signal "$sig" --at "$signal_s" --nth "$nth" >/dev/null &
plug_watch=$!
recorded_ms=$(($(date +%s%N) / 1000000))
pw-record -P stream.capture.sink=true --format s16 --rate 48000 --channels 2 "$out/raw/output.wav" &
recorder=$!
launched_ms=$(($(date +%s%N) / 1000000))
env MELOET_CONFIG_DIR="$work/config" MELOET_STATE_DIR="$work/state" MELOET_ONLY_VIRTUAL=1 \
  MELOET_VKEYS_NAME="$vkeys_name" MELOET_WINDOW="$window" MELOET_OPEN_SONG="$work/song/stop.meloet" \
  MELOET_COMMANDS="$commands" MELOET_SNAPSHOT="$out/shot.png" MELOET_SNAPSHOT_AFTER_MS="$snap_ms" \
  MELOET_PLUGHOST_NO_DUMP=1 \
  "${launcher[@]}" "$target/release/meloet" 2> >(stamp "$launched_ms" >"$out/raw/meloet.stderr") &
launched=$!

app=
for _ in $(seq 100); do
  app=$(pgrep -u "$(id -u)" -x meloet -n || true)
  [ -n "$app" ] && break
  sleep 0.1
done
[ -n "$app" ] || { echo "plugin-stop.sh: meloet did not start" >&2; kill "$recorder" "$plug_watch" 2>/dev/null; exit 1; }
python3 -I "$root/tools/measure/rss_watch.py" "$out/raw/rss.json" "$app" &
rss_watch=$!

sleep "$keys_from_s"
"$target/release/meloet-vkeys" play --name "$vkeys_name" --notes 60,64,67,72 --count "$keys" \
  --interval-ms "$key_ms" --hold-ms 150 --velocity 60 --lead-ms 500 --log "$out/raw/vkeys.json" || true

ended=by-itself
for _ in $(seq 40); do
  kill -0 "$app" 2>/dev/null || break
  sleep 1
done
if kill -0 "$app" 2>/dev/null; then
  echo "plugin-stop.sh: meloet did not exit by itself; stopping it" >&2
  ended=stopped-by-the-script
  kill -TERM "$app"
fi
status=0
wait "$launched" || status=$?
kill "$recorder" 2>/dev/null || true
wait "$recorder" 2>/dev/null || true
wait "$plug_watch" || true
wait "$rss_watch" || true
cp "$work/song/stop.meloet" "$out/raw/saved.meloet"

python3 -I "$root/tools/measure/plugin_stop_summary.py" "$out/raw" "$launched_ms" \
  --recorded-ms "$recorded_ms" \
  --label "release, window $window, $state, SIG$sig$([ "$reload" = 1 ] && echo ', reload'), song $(basename "$song")" \
  --ended "$ended, exit status $status" | tee "$out/summary.md"
