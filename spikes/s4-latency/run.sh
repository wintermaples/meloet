#!/usr/bin/env bash
# Bet 4 runs. Usage:
#   run.sh devices                 list output devices per host (cpal)
#   run.sh latency [buffer]        virtual keys -> Rust -> screen and -> sound, every transport
#   run.sh latency-load            same, while meters and position stream at 60 Hz
#   run.sh load                    per-frame meters + position at 60 and 144 Hz, every transport
#   run.sh plug <secs>             waits while a person plugs and unplugs the keyboard
#   run.sh real <secs> [transport] waits while a person presses real keys
set -u
cd "$(dirname "$0")/../.."
BIN=./target/release/s4-latency
VK=./target/release/vkeys
OUT=spikes/s4-latency/results
mkdir -p "$OUT"
export WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-/run/user/1000/wayland-0} DISPLAY=${DISPLAY:-:0} NO_AT_BRIDGE=1
# cpal looks for pipewire-0 under PIPEWIRE_RUNTIME_DIR or XDG_RUNTIME_DIR, not PIPEWIRE_REMOTE.
export PIPEWIRE_REMOTE=${PIPEWIRE_REMOTE:-/run/user/1000/pipewire-0} PIPEWIRE_RUNTIME_DIR=${PIPEWIRE_RUNTIME_DIR:-/run/user/1000}

# See spikes/s1-render/run_matrix.sh for why each launch gets a private session bus.
app() { local out=$1 q=$2; shift 2; timeout 300 dbus-run-session -- $BIN --query "$q" --out "$OUT/$out.json" "$@" > "$OUT/$out.log" 2>&1; echo "$out exit=$?"; }

case "${1:-}" in
  devices) app devices "mode=devices" ;;
  latency)
    buf=${2:-}
    for t in event chan-json chan-bin pull; do
      ( sleep 4; $VK 120 120 300 > /dev/null ) &
      app "latency__${t}__buf${buf:-default}" "mode=latency&transport=$t&dur=50000" ${buf:+--buffer $buf}
      wait
    done ;;
  latency-load)  # the realistic case: meters and position keep streaming at 60 Hz while keys arrive
    for t in event chan-json chan-bin pull; do
      ( sleep 4; $VK 120 120 300 > /dev/null ) &
      app "latency-load__${t}" "mode=latency&transport=$t&dur=50000&hz=60"
      wait
    done ;;
  load)
    for hz in 60 144; do for t in event chan-json chan-bin pull; do
      app "load__${t}__${hz}hz" "mode=load&transport=$t&hz=$hz&dur=10000"
    done; done ;;
  plug) app "plug" "mode=plug&transport=chan-bin&dur=$(( ${2:-90} * 1000 ))" ;;
  real) app "real__${3:-chan-bin}" "mode=latency&transport=${3:-chan-bin}&dur=$(( ${2:-60} * 1000 ))" ;;
  *) sed -n 2,8p "$0" ;;
esac
