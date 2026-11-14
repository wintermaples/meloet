#!/usr/bin/env bash
# Runs every combination of env vars x window size x renderer x meters, plus a resize stress per env set.
# Results: results/raw/<env>__<size>__<renderer>__m<meters>.json, results/raw/stress__<env>.json, results/runs.tsv
set -u
cd "$(dirname "$0")/../.."
BIN=./target/release/s1-render
OUT=spikes/s1-render/results
RAW=$OUT/raw
mkdir -p "$RAW"
DUR=${DUR:-10000}
WARM=${WARM:-1500}
DISPLAY_MS=${DISPLAY_MS:-6.951} # 143.87 Hz, from xrandr
export WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-/run/user/1000/wayland-0} DISPLAY=${DISPLAY:-:0} NO_AT_BRIDGE=1

# name|env assignments. The first 8 are the three variables from the Tauri "Linux graphics" page, on and off.
ENVS=(
  "base|"
  "dmabufoff|WEBKIT_DISABLE_DMABUF_RENDERER=1"
  "nvsync|__NV_DISABLE_EXPLICIT_SYNC=1"
  "nocomp|WEBKIT_DISABLE_COMPOSITING_MODE=1"
  "dmabufoff+nvsync|WEBKIT_DISABLE_DMABUF_RENDERER=1 __NV_DISABLE_EXPLICIT_SYNC=1"
  "dmabufoff+nocomp|WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1"
  "nvsync+nocomp|__NV_DISABLE_EXPLICIT_SYNC=1 WEBKIT_DISABLE_COMPOSITING_MODE=1"
  "all3|WEBKIT_DISABLE_DMABUF_RENDERER=1 __NV_DISABLE_EXPLICIT_SYNC=1 WEBKIT_DISABLE_COMPOSITING_MODE=1"
  "forcedmabuf|WEBKIT_FORCE_DMABUF_RENDERER=1"
  "forcedmabuf+nvsync|WEBKIT_FORCE_DMABUF_RENDERER=1 __NV_DISABLE_EXPLICIT_SYNC=1"
  "x11|GDK_BACKEND=x11"
)
SIZES=("1280x800" "1920x1080")
[ -n "${ONLY:-}" ] && ENVS=("${ENVS[@]:0:$ONLY}")

echo -e "file\texit\tseconds" > "$OUT/runs.tsv"
one() { # file envstr args...
  local f=$1 e=$2; shift 2
  local t0=$(date +%s.%N)
  # A private session bus: this agent runs as another user whose xdg-desktop-portal cannot start,
  # and GTK would otherwise wait 25 s for it on every launch.
  env $e timeout 60 dbus-run-session -- $BIN "$@" --out "$RAW/$f.json" > "$RAW/$f.log" 2>&1
  local ec=$?
  echo -e "$f\t$ec\t$(echo "$(date +%s.%N) - $t0" | bc)" >> "$OUT/runs.tsv"
  [ -f "$RAW/$f.json" ] || echo "{\"noResult\":true,\"exit\":$ec}" > "$RAW/$f.json"
  echo "$f exit=$ec"
}
for pair in "${ENVS[@]}"; do
  name=${pair%%|*}; e=${pair#*|}
  for s in "${SIZES[@]}"; do
    w=${s%x*}; h=${s#*x}
    for r in 2d gl; do for m in 0 1; do
      one "${name}__${s}__${r}__m${m}" "$e" --renderer $r --meters $m --w $w --h $h --dur $DUR --warm $WARM --display $DISPLAY_MS
    done; done
  done
  one "stress__${name}" "$e" --renderer gl --meters 1 --w 1920 --h 1080 --dur 9000 --warm 500 --display $DISPLAY_MS --resize-stress 8
done
