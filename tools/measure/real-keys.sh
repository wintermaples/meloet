#!/usr/bin/env bash
# Key -> screen and key -> sound with a real keyboard.
#
# usage: tools/measure/real-keys.sh [LABEL]
#
# Starts the release meloet with MELOET_MEASURE_DIR set and the usual settings (the output you
# chose before). Play about 100 keys, then close the window. The table is printed and written
# to target/measure/real-keys/<time>-<LABEL>/summary.md. To measure while playing or recording,
# press Space or R in meloet before you play; give a LABEL such as "playing" to tell the runs apart.
#
# MELOET_BIN chooses the binary (default: target/lead/release/meloet, then target/release/meloet).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
label=${1:-stopped}
bin=${MELOET_BIN:-}
if [ -z "$bin" ]; then
  for candidate in "$root/target/lead/release/meloet" "$root/target/release/meloet"; do
    if [ -x "$candidate" ]; then bin=$candidate; break; fi
  done
fi
if [ -z "$bin" ] || [ ! -x "$bin" ]; then
  echo "real-keys.sh: no release meloet found; build it with: cargo build --release -p meloet-app" >&2
  exit 1
fi

out="$root/target/measure/real-keys/$(date +%H%M%S)-$label"
mkdir -p "$out/raw"
echo "meloet: $bin"
echo "Play about 100 keys, then close the window."
MELOET_MEASURE_DIR="$out/raw" "$bin" 2>"$out/raw/meloet.stderr" || true
python3 -I "$root/tools/measure/summarize.py" "$out/raw" --out "$out" --label "real keyboard, $label"
echo "written: $out/summary.md"
