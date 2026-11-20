#!/usr/bin/env bash
# Measures where a recorded voice lands against the beat it was sung on: on a PipeWire of its
# own, the vocal track records the metronome from the output's monitor, and every click is
# looked for in the file where the take says its beat is. Before the recordings, 「遅れを測る」
# runs three times at every quantum.
#
# usage: tools/measure/vocal-timing.sh [--quanta 64,128] [--runs probe,tempo,meter,map,loop]
#                                      [--delay-ms N | --second-sink] [--out DIR]
#        tools/measure/vocal-timing.sh --monitor [--quanta 64,128] [--out DIR]
#
#   --quanta    the quantum of the private PipeWire, in turn (default 64,128,256,512,1024).
#   --runs      probe: 「遅れを測る」; tempo: 96 and 137.5 in 4/4; meter: 120 in 6/8; map: tempo
#               points, two ramps, 3/4 and 6/8; loop: bars 2 to 4, round and round.
#   --delay-ms  a pw-loopback with that delay and a second sink between the output and the
#               input, as a box outside the PC: every run is recorded as it comes, and again
#               with the number 「遅れを測る」 found in use.
#   --second-sink  the output's monitor is linked straight into a second sink, whose monitor
#               is the input (nothing between them, but another node than the one that
#               drives the graph).
#   --monitor   instead of the recordings: how late a sound comes out of a vocal track's
#               monitor (clicks played into a second sink that stands for the microphone;
#               DIR/monitor.md).
#
# Writes DIR/summary.md and DIR/runs.json (default target/measure/vocal-timing). About 13
# minutes for everything. Changes nothing on the PC: the quantum is set on the private
# PipeWire only.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
filter=a_recorded_voice
while [ $# -gt 0 ]; do
  case "$1" in
    --quanta) export MELOET_VTIMING_QUANTA=$2; shift 2 ;;
    --runs) export MELOET_VTIMING_RUNS=$2; shift 2 ;;
    --monitor) filter=the_monitor_is; shift ;;
    --second-sink) export MELOET_VTIMING_SECOND_SINK=1; shift ;;
    --delay-ms) export MELOET_VTIMING_DELAY_MS=$2; shift 2 ;;
    --out) case "$2" in /*) export MELOET_VTIMING_OUT=$2 ;; *) export MELOET_VTIMING_OUT=$PWD/$2 ;; esac; shift 2 ;;
    *) echo "usage: $0 [--quanta 64,128] [--runs probe,tempo,meter,map,loop] [--delay-ms N | --second-sink] [--monitor] [--out DIR]" >&2; exit 2 ;;
  esac
done

cd "$root"
cargo test --release -p meloet-core --test vocal_timing_private -- --ignored --nocapture "$filter"
