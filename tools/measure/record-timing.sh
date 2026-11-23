#!/usr/bin/env bash
# Measures where recorded keys land against the beat they were played on, through the real ALSA
# sequencer (a virtual keyboard of its own name) and PipeWire.
#
# usage: tools/measure/record-timing.sh [private] [keep] [live OUTPUT_ID] [--beats N] [--quanta 64,128]
#                                       [--tempos 120,30] [--meter 6/8] [--no-maps]
#
#   private          a PipeWire of its own, quantum fixed to 64, 128, 256, 512, 1024 in turn,
#                    tempos 96.00 and 137.50, each on time and 5 ms late (about 12 minutes).
#                    --tempos and --meter change the tempos and the meter of these runs;
#                    --no-maps leaves out the runs on the changing timeline.
#   keep             the same runs as private, but the song only plays and what was played is
#                    kept afterwards (「さっきのを残す」); written under keep/.
#   live OUTPUT_ID   the PipeWire of the person using the PC at its running quantum, 137.50 bpm;
#                    the metronome sounds. `live list` prints the output ids.
#
# Writes target/measure/record-timing/{private,keep,live}/summary.md and runs.json.
# Changes nothing on the PC: the PipeWire quantum is only read.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
private=0
keep=0
live=
beats=
quanta=
while [ $# -gt 0 ]; do
  case "$1" in
    private) private=1; shift ;;
    keep) keep=1; shift ;;
    live) live=$2; shift 2 ;;
    --beats) beats=$2; shift 2 ;;
    --quanta) quanta=$2; shift 2 ;;
    --tempos) export MELOET_TIMING_TEMPOS=$2; shift 2 ;;
    --meter) export MELOET_TIMING_METER=$2; shift 2 ;;
    --no-maps) export MELOET_TIMING_MAPS=0; shift ;;
    *) echo "usage: $0 [private] [keep] [live OUTPUT_ID|list] [--beats N] [--quanta 64,128] [--tempos 120,30] [--meter 6/8] [--no-maps]" >&2; exit 2 ;;
  esac
done
if [ "$private" = 0 ] && [ "$keep" = 0 ] && [ -z "$live" ]; then private=1; fi

cd "$root"
if [ -n "$beats" ]; then export MELOET_TIMING_BEATS=$beats; fi
if [ -n "$quanta" ]; then export MELOET_TIMING_QUANTA=$quanta; fi
# $1: the test file; $2: the start of the name of the tests in it to run (all without it).
run() { cargo test --release -p meloet-core --test "$1" ${2:+"$2"} -- --ignored --nocapture; }

if [ "$private" = 1 ]; then
  run record_timing_private recorded_keys
fi
if [ "$keep" = 1 ]; then
  run record_timing_private kept_keys
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
