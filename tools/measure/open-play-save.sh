#!/usr/bin/env bash
# Opens a song in the real app (release), plays keys on its first track, plays the song, saves
# it and ends: to tell whether a song with plugins and SF2 sounds comes back the same.
#
# usage: tools/measure/open-play-save.sh --song FILE [--window 1920x1080] [--out DIR]
#
# --song: best one of `make_song.py --hard-pan` (the plugin tracks are heard on the left, the
#   others on the right). A copy is opened, so the file stays as it is.
#
# Times after the launch: a virtual keyboard plays 20 keys (60, 64, 67, 72 in turn, one every
# 300 ms) from 6.5 s while the song stands still; the song plays from about 13.5 s to 19.5 s;
# it is saved at about 20.5 s; meloet exits at about 22.5 s.
# Records what comes out of the output (its monitor, pw-record) and writes DIR/summary.md:
# how loud and how high the keys were on the left, how loud each side was while the song
# played, and the plugins and SF2 presets of the saved song with the size and sha256 of each
# plugin state. DIR/raw/saved.meloet is the song as saved.
# Builds nothing (tools/measure/build-dev.sh first). Changes nothing on the PC.
set -euo pipefail

song=
window=1920x1080
out=
while [ $# -gt 0 ]; do
  case "$1" in
    --song) song=$2; shift 2 ;;
    --window) window=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    *) echo "usage: $0 --song FILE [--window WxH] [--out DIR]" >&2; exit 2 ;;
  esac
done
[ -n "$song" ] || { echo "open-play-save.sh: --song is needed" >&2; exit 2; }

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${out:-$root/target/measure/open-play-save}
target=${CARGO_TARGET_DIR:-$root/target}
case "$target" in /*) ;; *) target=$root/$target ;; esac
for bin in meloet meloet-vkeys meloet-plughost; do
  [ -x "$target/release/$bin" ] || { echo "open-play-save.sh: $target/release/$bin is not there" >&2; exit 1; }
done

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/config" "$work/state" "$work/song" "$out/raw"
rm -f "$out"/raw/* "$out"/summary.md
cp "$song" "$work/song/song.meloet"

# This PC only: the agent user reaches the screen and the sound through this file.
if [ -f "$HOME/.config/meloet-agent/env.sh" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.config/meloet-agent/env.sh"
  export PIPEWIRE_RUNTIME_DIR=${PIPEWIRE_RUNTIME_DIR:-/run/user/1000}
fi
launcher=()
if command -v dbus-run-session >/dev/null; then launcher=(dbus-run-session --); fi

cmd() { printf '{"afterMs":%d,"command":%s}' "$1" "$2"; }
commands="[$(cmd 1500 '{"type":"chooserConfirm"}'),$(cmd 13000 '{"type":"playStop"}'),\
$(cmd 19000 '{"type":"playStop"}'),$(cmd 20000 '{"type":"saveSong"}')]"

vkeys_name=meloet-vkeys-ops-$$
"$target/release/meloet-vkeys" appear --name "$vkeys_name" --seconds 40 >/dev/null 2>&1 &
standby=$!
trap 'kill "$standby" 2>/dev/null || true; rm -rf "$work"' EXIT
sleep 1

recorded_ms=$(($(date +%s%N) / 1000000))
pw-record -P stream.capture.sink=true --format s16 --rate 48000 --channels 2 "$out/raw/output.wav" &
recorder=$!
launched_ms=$(($(date +%s%N) / 1000000))
env MELOET_CONFIG_DIR="$work/config" MELOET_STATE_DIR="$work/state" MELOET_ONLY_VIRTUAL=1 \
  MELOET_VKEYS_NAME="$vkeys_name" MELOET_WINDOW="$window" MELOET_OPEN_SONG="$work/song/song.meloet" \
  MELOET_COMMANDS="$commands" MELOET_EXIT_AFTER_MS=22000 \
  "${launcher[@]}" "$target/release/meloet" 2>"$out/raw/meloet.stderr" &
launched=$!

sleep 6
"$target/release/meloet-vkeys" play --name "$vkeys_name" --notes 60,64,67,72 --count 20 \
  --interval-ms 300 --hold-ms 200 --velocity 80 --lead-ms 500 --log "$out/raw/vkeys.json" || true

status=0
wait "$launched" || status=$?
kill "$recorder" 2>/dev/null || true
wait "$recorder" 2>/dev/null || true
cp "$work/song/song.meloet" "$out/raw/saved.meloet"

python3 -I "$root/tools/measure/open_play_save_summary.py" "$out/raw" \
  --keys-ms $(((launched_ms - recorded_ms + 7000))) --song-ms $(((launched_ms - recorded_ms + 14500))) \
  --label "release, window $window, song $(basename "$song"), exit status $status" | tee "$out/summary.md"
