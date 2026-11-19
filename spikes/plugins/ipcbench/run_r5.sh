#!/usr/bin/env bash
# Round-5 numbers for the plugin host design. One JSON line per run, with a label, in spikes/plugins/results/r5-ipc/.
#   run_r5.sh mem|mem-lv2-shared|mem80|state|rt|rt16|rt80|scan   (rt alone is ~30 minutes; keep the PC quiet)
# Needs: CARGO_TARGET_DIR=target/r5-measure cargo build --release -p pk-ipcbench -p pk-probe
set -u
cd "$(dirname "$0")/../../.."
B=${B:-target/r5-measure/release}
OUT=spikes/plugins/results/r5-ipc
mkdir -p "$OUT"
export DISPLAY=${DISPLAY:-:0}
ITERS=${ITERS:-10000}

SURGE_V="vst3=/usr/lib/vst3/Surge XT.vst3#ABCDEF019182FAEB566D624153675854"
SURGE_L="lv2=https://surge-synthesizer.github.io/lv2/surge-xt"
SURGE_LB="lv2b=https://surge-synthesizer.github.io/lv2/surge-xt@file:///usr/lib/lv2/Surge%20XT.lv2/"
LSP_V="vst3=/usr/lib/vst3/lsp-plugins.vst3#647370206B31732020202020756E7363"
LSP_L="lv2=http://lsp-plug.in/plugins/lv2/compressor_stereo"
LSP_LB="lv2b=http://lsp-plug.in/plugins/lv2/compressor_stereo@file:///usr/lib/lv2/lsp-plugins.lv2/"
ZAM_V="vst3=/usr/lib/vst3/ZamComp.vst3#2046504473616C63504D435A00000000"
ZAM_L="lv2=urn:zamaudio:ZamComp"
ZAM_LB="lv2b=urn:zamaudio:ZamComp@file:///usr/lib/lv2/ZamComp.lv2/"

# run <file> <label> <pk-ipcbench args...>
run() {
  local file=$1 label=$2; shift 2
  local r; r=$("$B/pk-ipcbench" "$@")
  echo "{\"label\":\"$label\",\"result\":${r:-null}}" >> "$OUT/$file"
  echo "$label done"
}
rep() { local n=$1 x=$2; local i; for ((i = 0; i < n; i++)); do printf '%s\n' "$x"; done; }

case "${1:-}" in
mem)
  : > "$OUT/mem.jsonl"
  run mem.jsonl idle mem --kind idle --repeat 30
  run mem.jsonl x11-lib mem --kind x11 --repeat 30
  run mem.jsonl x11-window mem --kind x11 --repeat 5 --window
  run mem.jsonl host-empty mem --kind host --repeat 30
  for pair in "surge-vst3|$SURGE_V" "surge-lv2|$SURGE_L" "surge-lv2-onebundle|$SURGE_LB" "lsp-vst3|$LSP_V" "lsp-lv2|$LSP_L" \
              "lsp-lv2-onebundle|$LSP_LB" "zam-vst3|$ZAM_V" "zam-lv2|$ZAM_L" "zam-lv2-onebundle|$ZAM_LB"; do
    name=${pair%%|*}; spec=${pair#*|}
    run mem.jsonl "$name" mem --kind host --repeat 5 "$spec"
    run mem.jsonl "$name-window" mem --kind host --window "$spec"
  done
  for pair in "surge-vst3|$SURGE_V" "surge-lv2-onebundle|$SURGE_LB" "lsp-vst3|$LSP_V" "lsp-lv2-onebundle|$LSP_LB" "zam-vst3|$ZAM_V" "zam-lv2-onebundle|$ZAM_LB"; do
    name=${pair%%|*}; spec=${pair#*|}
    for n in 2 4 8; do
      mapfile -t many < <(rep "$n" "$spec")
      run mem.jsonl "$name-same-process-x$n" mem --kind host "${many[@]}"
      run mem.jsonl "$name-own-process-x$n" mem --kind host --procs "$n" "$spec"
    done
  done
  ;;
mem-lv2-shared)
  : > "$OUT/mem-lv2-shared.jsonl"
  for pair in "surge-lv2|$SURGE_LB" "lsp-lv2|$LSP_LB" "zam-lv2|$ZAM_LB"; do
    name=${pair%%|*}; spec=lv2s${pair#*|lv2b}
    for n in 1 2 4 8; do
      mapfile -t many < <(rep "$n" "$spec")
      run mem-lv2-shared.jsonl "$name-shared-world-same-process-x$n" mem --kind host "${many[@]}"
    done
  done
  ;;
mem80)
  # 16 tracks x (Surge XT + 2 LSP compressors + 2 ZamComp), all VST3, split four ways.
  : > "$OUT/mem80.jsonl"
  mapfile -t s16 < <(rep 16 "$SURGE_V"); mapfile -t l32 < <(rep 32 "$LSP_V"); mapfile -t z32 < <(rep 32 "$ZAM_V")
  run mem80.jsonl each-own-surge-x16 mem --kind host --procs 16 "$SURGE_V"
  run mem80.jsonl each-own-lsp-x32 mem --kind host --procs 32 "$LSP_V"
  run mem80.jsonl each-own-zam-x32 mem --kind host --procs 32 "$ZAM_V"
  run mem80.jsonl by-plugin-surge-x16 mem --kind host "${s16[@]}"
  run mem80.jsonl by-plugin-lsp-x32 mem --kind host "${l32[@]}"
  run mem80.jsonl by-plugin-zam-x32 mem --kind host "${z32[@]}"
  run mem80.jsonl by-track-x16 mem --kind host --procs 16 "$SURGE_V" "$LSP_V" "$LSP_V" "$ZAM_V" "$ZAM_V"
  run mem80.jsonl all-in-one mem --kind host "${s16[@]}" "${l32[@]}" "${z32[@]}"
  ;;
state)
  : > "$OUT/state.jsonl"
  for pair in "surge-vst3|$SURGE_V" "surge-lv2|$SURGE_L" "lsp-vst3|$LSP_V" "lsp-lv2|$LSP_L" "zam-vst3|$ZAM_V" "zam-lv2|$ZAM_L"; do
    run state.jsonl "${pair%%|*}" mem --kind host --state-iters 100 "${pair#*|}"
  done
  ;;
rt)
  : > "$OUT/rt.jsonl"
  for load in "" "--load"; do
    for sched in other fifo mixed; do
      run rt.jsonl "futex-$sched${load:+-load}" rt --wake futex --sched $sched --iters "$ITERS" $load
      run rt.jsonl "eventfd-$sched${load:+-load}" rt --wake eventfd --sched $sched --iters "$ITERS" $load
      for s in 20 50 200; do
        run rt.jsonl "spin$s-$sched${load:+-load}" rt --wake spin --spin-us $s --sched $sched --iters "$ITERS" $load
      done
    done
  done
  ;;
rt16)
  : > "$OUT/rt16.jsonl"
  for load in "" "--load"; do
    for sched in other fifo mixed; do
      for mode in seq fanout; do
        run rt16.jsonl "futex-$mode-$sched${load:+-load}" rt --wake futex --sched $sched --children 16 --mode $mode --iters "$ITERS" $load
        run rt16.jsonl "eventfd-$mode-$sched${load:+-load}" rt --wake eventfd --sched $sched --children 16 --mode $mode --iters "$ITERS" $load
        run rt16.jsonl "spin50-$mode-$sched${load:+-load}" rt --wake spin --spin-us 50 --sched $sched --children 16 --mode $mode --iters "$ITERS" $load
      done
    done
  done
  ;;
rt80)
  : > "$OUT/rt80.jsonl"
  for load in "" "--load"; do
    for sched in other fifo mixed; do
      for mode in seq fanout; do
        run rt80.jsonl "futex-$mode-$sched${load:+-load}" rt --wake futex --sched $sched --children 80 --mode $mode --iters 3000 $load
      done
    done
  done
  ;;
scan)
  /usr/bin/time -f '{"wallS":%e,"userS":%U,"sysS":%S}' -o "$OUT/scan.time.json" "$B/pk-probe" scan > "$OUT/scan.json" 2> /dev/null
  cat "$OUT/scan.time.json"
  ;;
*) echo "usage: run_r5.sh mem|mem-lv2-shared|mem80|state|rt|rt16|rt80|scan"; exit 1 ;;
esac
