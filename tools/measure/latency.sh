#!/usr/bin/env bash
# Measures key -> screen and key -> sound of the release build with the virtual keyboard.
#
# usage: tools/measure/latency.sh [--window 1920x1080] [--count 120] [--out DIR] [--keep-ui]
#
# --keep-ui: do not build ui/; the app takes whatever ui/dist holds (to measure an older screen).
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
while [ $# -gt 0 ]; do
  case "$1" in
    --window) window=$2; shift 2 ;;
    --count) count=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    --keep-ui) keep_ui=1; shift ;;
    *) echo "usage: $0 [--window WxH] [--count N] [--out DIR] [--keep-ui]" >&2; exit 2 ;;
  esac
done

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${out:-$root/target/measure/latency}
target=${CARGO_TARGET_DIR:-$root/target}

interval_ms=257
hold_ms=120
velocity=25
confirm_ms=1500   # after the page loaded: the chooser is up, confirm its first choosable output
settle_s=8        # from start to the first key: page load, chooser, stream open
lead_ms=500
# Only this client counts as a keyboard, so that virtual keyboards of other runs stay out.
vkeys_name=meloet-vkeys-latency-$$
tail_ms=6000      # after the last key: the screen reports every 2 s
exit_ms=$((settle_s * 1000 + lead_ms + count * interval_ms + tail_ms))

if [ "$keep_ui" = 0 ]; then (cd "$root/ui" && npm run build); fi
(cd "$root" && cargo build --release -p meloet-app -p meloet-midi)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/config" "$work/measure" "$out/raw"

# This PC only: the agent user reaches the screen and the sound through this file.
if [ -f "$HOME/.config/meloet-agent/env.sh" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.config/meloet-agent/env.sh"
  export PIPEWIRE_RUNTIME_DIR=${PIPEWIRE_RUNTIME_DIR:-/run/user/1000}
fi

# GTK can wait 30 s for a session bus that is not there.
launcher=()
if command -v dbus-run-session >/dev/null; then launcher=(dbus-run-session --); fi

MELOET_CONFIG_DIR="$work/config" \
MELOET_MEASURE_DIR="$work/measure" \
MELOET_ACCEPT_VIRTUAL=1 \
MELOET_VKEYS_NAME="$vkeys_name" \
MELOET_WINDOW="$window" \
MELOET_COMMANDS="[{\"afterMs\":$confirm_ms,\"command\":{\"type\":\"chooserConfirm\"}}]" \
MELOET_EXIT_AFTER_MS="$exit_ms" \
  "${launcher[@]}" "$target/release/meloet" 2>"$out/raw/meloet.stderr" &
app=$!

sleep "$settle_s"
if ! kill -0 "$app" 2>/dev/null; then
  echo "latency.sh: meloet ended before the first key:" >&2
  cat "$out/raw/meloet.stderr" >&2
  exit 1
fi
"$target/release/meloet-vkeys" play --name "$vkeys_name" --count "$count" --interval-ms "$interval_ms" \
  --hold-ms "$hold_ms" --velocity "$velocity" --lead-ms "$lead_ms" --log "$work/vkeys.json"

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

cp "$work/vkeys.json" "$out/raw/" 2>/dev/null || true
cp "$work/measure/"*.json "$out/raw/" 2>/dev/null || true
python3 -I "$root/tools/measure/summarize.py" "$out/raw" --vkeys "$out/raw/vkeys.json" \
  --out "$out" --label "release, window $window, $count keys"
