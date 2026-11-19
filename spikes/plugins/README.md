# plugins — bets 2 and 3, VST3 and LV2

- `core`  the format-free boundary (`Plugin`, `PluginInfo`, `Kind`, `UiKind`) and an X11 host window
- `vst3`  VST3 adapter on the `vst3` crate; all unsafe COM code stays here
- `lv2`   LV2 adapter on `livi`/`lilv`, plus our own X11UI loader
- `probe` `pk-probe`, which runs every check and prints JSON

```bash
cargo build --release -p pk-probe
./run_all.sh                                     # scan + full test on 6 targets + crash tests -> results/
../../target/release/pk-probe scan               # one child process per bundle/plugin, 10 s timeout each
../../target/release/pk-probe test vst3 "/usr/lib/vst3/Surge XT.vst3#ABCDEF019182FAEB566D624153675854" [--show 8] [--play]
../../target/release/pk-probe test lv2 http://synthv1.sourceforge.net/lv2
../../target/release/pk-probe crash lv2 http://synthv1.sourceforge.net/lv2 kill|segv|hang
```

`tests/crashy.c` is a fake VST3 whose factory segfaults; `run_all.sh` builds it and mixes it into the scan.

## ipcbench — numbers before the plugin host design (round 5)

- `ipcbench`  `pk-ipcbench` (round trips over shared memory, PSS of child processes), `pk-ipchost` (a child that loads plugins), `pk-ipcx11` (a child with only the X11 pieces)

```bash
CARGO_TARGET_DIR=target/r5-measure cargo build --release -p pk-ipcbench -p pk-probe
B=../../target/r5-measure/release
$B/pk-ipcbench rt --wake futex|eventfd|spin [--spin-us 50] --sched other|fifo|mixed [--children 16 --mode seq|fanout] [--load]
$B/pk-ipcbench mem --kind idle|x11|host [--procs 8] [--repeat 30] [--window] [--state-iters 100] "vst3=<bundle>#<classid>" "lv2s=<uri>@<bundle uri>"
ipcbench/run_r5.sh mem|mem-lv2-shared|mem80|state|rt|rt16|rt80|scan   # every run of docs/measurements/2026-11-19-round5-ipc.md -> results/r5-ipc/
```

`mixed` is an engine on SCHED_FIFO with children left on SCHED_OTHER. `--load` runs one busy process per CPU beside the test.
The children get the memfd by number and report through it; their stdout and stderr go to /dev/null. `--window` shows a real window for a few seconds.
