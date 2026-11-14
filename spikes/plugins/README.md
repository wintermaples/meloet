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
