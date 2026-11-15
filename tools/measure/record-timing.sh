#!/usr/bin/env bash
# Measures where recorded keys land against the beat they were played on, through the real ALSA
# sequencer (a virtual keyboard of its own name) and PipeWire.
#
# usage: tools/measure/record-timing.sh [private] [live OUTPUT_ID] [--beats N] [--quanta 64,128]
#
#   private          a PipeWire of its own, quantum fixed to 64, 128, 256, 512, 1024 in turn,
#                    tempos 96.00 and 137.50, each on time and 5 ms late (about 12 minutes)
#   live OUTPUT_ID   the PipeWire of the person using the PC at its running quantum, 137.50 bpm;
#                    the metronome sounds. `live list` prints the output ids.
#
# Writes target/measure/record-timing/{private,live}/summary.md and runs.json.
# Changes nothing on the PC: the PipeWire quantum is only read.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
private=0
live=
beats=
quanta=
while [ $# -gt 0 ]; do
  case "$1" in
    private) private=1; shift ;;
    live) live=$2; shift 2 ;;
    --beats) beats=$2; shift 2 ;;
    --quanta) quanta=$2; shift 2 ;;
    *) echo "usage: $0 [private] [live OUTPUT_ID|list] [--beats N] [--quanta 64,128]" >&2; exit 2 ;;
  esac
done
if [ "$private" = 0 ] && [ -z "$live" ]; then private=1; fi

cd "$root"
if [ -n "$beats" ]; then export MELOET_TIMING_BEATS=$beats; fi
if [ -n "$quanta" ]; then export MELOET_TIMING_QUANTA=$quanta; fi
run() { cargo test --release -p meloet-core --test "$1" -- --ignored --nocapture; }

if [ "$private" = 1 ]; then
  run record_timing_private
fi
if [ -n "$live" ]; then
  # This PC only: the agent user reaches the sound through this file.
  if [ -f "$HOME/.config/meloet-agent/env.sh" ]; then
    # shellcheck disable=SC1091
    source "$HOME/.config/meloet-agent/env.sh"
    export PIPEWIRE_RUNTIME_DIR=${PIPEWIRE_RUNTIME_DIR:-/run/user/1000}
  fi
  if [ "$live" = list ]; then
    run record_timing_live || true
  else
    MELOET_TIMING_OUTPUT="$live" run record_timing_live
  fi
fi
